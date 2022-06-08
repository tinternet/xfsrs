use std::{
    ffi::{c_void, CStr, CString},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use conf::XFS_CONFIG;
use lazy_static::lazy_static;
// use libloading::Symbol;
use log::{trace, LevelFilter};
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    Config,
};
use winapi::{
    shared::{
        basetsd::UINT_PTR,
        minwindef::{DWORD, HINSTANCE, HKEY, LPARAM, LPDWORD, LPVOID, LPWORD, MAX_PATH, UINT, ULONG, WORD},
        windef::HWND,
        winerror::HRESULT,
    },
    um::{
        heapapi::{GetProcessHeap, HeapAlloc, HeapFree},
        winnt::{DLL_PROCESS_ATTACH, HEAP_ZERO_MEMORY, LPSTR},
        winuser::{KillTimer, PostMessageA, SetTimer},
    },
};
use xfslib::*;

mod buffer;
mod conf;
mod spi;
mod window;

lazy_static! {
    static ref SERVICES: Mutex<Vec<Option<Service>>> = Mutex::new((0..8192).map(|_| None).collect());
    static ref HANDLES: Mutex<[bool; 8192]> = Mutex::new([false; 8192]);
    static ref STARTED: AtomicBool = AtomicBool::new(false);
    static ref TIMERS: Mutex<Vec<Option<TimerCtx>>> = Mutex::new((0..65535).map(|_| None).collect());
}

macro_rules! xfs_unwrap {
    ($l:expr) => {
        match $l {
            Ok(result) => result,
            Err(error) => {
                trace!("{:?}", error);
                return WFS_ERR_INTERNAL_ERROR;
            }
        }
    };
}

macro_rules! assert_started {
    () => {
        if !STARTED.load(Ordering::SeqCst) {
            return WFS_ERR_NOT_STARTED;
        }
    };
}

macro_rules! get_service_req {
    ($hService:expr, $services:expr) => {{
        match $hService {
            0 => return WFS_ERR_INVALID_HSERVICE,
            _ => match $services.get_mut($hService as usize - 1).and_then(|service| service.as_mut()) {
                Some(service) => {
                    service.request_id += 1;
                    service
                }
                None => return WFS_ERR_INVALID_HSERVICE,
            },
        }
    }};
}

struct Service {
    request_id: u32,
    library: libloading::Library,
    trace_level: DWORD,
}

#[allow(non_snake_case)]
struct TimerCtx {
    hWnd: HWND,
    lpContext: LPVOID,
}

unsafe impl Send for TimerCtx {}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSCancelAsyncRequest(hService: HSERVICE, RequestID: REQUESTID) -> HRESULT {
    trace!("WFSCancelAsyncRequest");
    assert_started!();

    if hService == 0 {
        return WFS_ERR_INVALID_HSERVICE;
    }

    let services = xfs_unwrap!(SERVICES.lock());

    if let Some(service) = services.get(hService as usize - 1).and_then(|service| service.as_ref()) {
        let wfp_cancel_async_request = unsafe { xfs_unwrap!(service.library.get::<spi::WfpCancelAsyncRequest>(b"WFPCancelAsyncRequest")) };
        return wfp_cancel_async_request(hService, RequestID);
    }

    WFS_ERR_INVALID_HSERVICE
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSCancelBlockingCall(_dwThreadID: DWORD) -> HRESULT {
    trace!("WFSCancelBlockingCall");
    assert_started!();
    WFS_SUCCESS // TODO: finish
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSCleanUp() -> HRESULT {
    trace!("WFSCleanUp");

    let mut services = xfs_unwrap!(SERVICES.lock());
    *services = (0..8192).map(|_| None).collect();

    let mut timers = xfs_unwrap!(TIMERS.lock());
    *timers = (0..65535).map(|_| None).collect();

    let mut handles = xfs_unwrap!(HANDLES.lock());
    *handles = [false; 8192];

    STARTED.store(false, Ordering::SeqCst);

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSClose(hService: HSERVICE) -> HRESULT {
    trace!("WFSClose");
    assert_started!();
    call_async(WFS_CLOSE_COMPLETE, |hwnd, reqid| WFSAsyncClose(hService, hwnd, reqid))
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncClose(hService: HSERVICE, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncClose");
    assert_started!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_close = unsafe {
        *lpRequestID = service.request_id as u32;
        xfs_unwrap!(service.library.get::<spi::WfpClose>(b"WfpClose"))
    };

    wfp_close(hService, hWnd, unsafe { *lpRequestID })
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSCreateAppHandle(lphApp: &mut HAPP) -> HRESULT {
    trace!("WFSCreateAppHandle");
    assert_started!();

    let mut handles = xfs_unwrap!(HANDLES.lock());
    let index = match handles.iter().position(|h| !h) {
        Some(index) => index,
        None => return WFS_ERR_INTERNAL_ERROR,
    };

    handles[index] = true;

    let index: usize = index + 1;
    let index_ptr: *const usize = &index;
    let index_happ: HAPP = index_ptr as HAPP;

    unsafe {
        *lphApp = index_happ;
        trace!("Created app handle {:?}", **lphApp);
    }

    trace!("XFS APP HANDLE DONE");
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSDeregister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND) -> HRESULT {
    trace!("WFSDeregister");
    assert_started!();
    call_async(WFS_DEREGISTER_COMPLETE, |hwnd, request_id| WFSAsyncDeregister(hService, dwEventClass, hWndReg, hwnd, request_id))
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncDeregister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncDeregister");
    assert_started!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_deregister = unsafe {
        *lpRequestID = service.request_id as u32;
        xfs_unwrap!(service.library.get::<spi::WFPDeregister>(b"WFPDeregister"))
    };

    wfp_deregister(hService, dwEventClass, hWndReg, hWnd, unsafe { *lpRequestID })
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSDestroyAppHandle(hApp: HAPP) -> HRESULT {
    if hApp.is_null() {
        return WFS_ERR_INVALID_APP_HANDLE;
    }

    assert_started!();
    let mut handles = xfs_unwrap!(HANDLES.lock());
    let index = hApp as usize - 1;
    handles[index] = false;

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSExecute(hService: HSERVICE, dwCommandd: DWORD, lpCmdData: LPVOID, dwTimeOut: DWORD, lppResult: &mut LPWFSRESULT) -> HRESULT {
    trace!("WFSExecute");
    assert_started!();
    call_async_result(
        WFS_EXECUTE_COMPLETE,
        |hwnd, request_id| WFSAsyncExecute(hService, dwCommandd, lpCmdData, dwTimeOut, hwnd, request_id),
        lppResult,
    )
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncExecute(hService: HSERVICE, dwCommand: DWORD, lpCmdData: LPVOID, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncExecute");
    assert_started!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_execute = unsafe {
        *lpRequestID = service.request_id as u32;
        xfs_unwrap!(service.library.get::<spi::WFPExecute>(b"WFPExecute"))
    };

    wfp_execute(hService, dwCommand, lpCmdData, dwTimeOut, hWnd, unsafe { *lpRequestID })
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSFreeResult(lpResult: LPWFSRESULT) -> HRESULT {
    assert_started!();
    WFMFreeBuffer(lpResult as *mut c_void)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSGetInfo(hService: HSERVICE, dwCategory: DWORD, lpQueryDetails: LPVOID, dwTimeOut: DWORD, lppResult: &mut LPWFSRESULT) -> HRESULT {
    trace!("WFSGetInfo");
    assert_started!();
    call_async_result(
        WFS_GETINFO_COMPLETE,
        |hwnd, request_id| WFSAsyncGetInfo(hService, dwCategory, lpQueryDetails, dwTimeOut, hwnd, request_id),
        lppResult,
    )
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncGetInfo(hService: HSERVICE, dwCategory: DWORD, lpQueryDetails: LPVOID, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncGetInfo");
    assert_started!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_get_info = unsafe {
        *lpRequestID = service.request_id as u32;
        xfs_unwrap!(service.library.get::<spi::WFPGetInfo>(b"WFPGetInfo"))
    };

    wfp_get_info(hService, dwCategory, lpQueryDetails, dwTimeOut, hWnd, unsafe { *lpRequestID })
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSIsBlocking() -> bool {
    false
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSLock(hService: HSERVICE, dwTimeOut: DWORD, lppResult: &mut LPWFSRESULT) -> HRESULT {
    trace!("WFSLock");
    assert_started!();
    call_async_result(WFS_LOCK_COMPLETE, |hwnd, request_id| WFSAsyncLock(hService, dwTimeOut, hwnd, request_id), lppResult)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncLock(hService: HSERVICE, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncLock");
    assert_started!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_lock = unsafe {
        *lpRequestID = service.request_id as u32;
        xfs_unwrap!(service.library.get::<spi::WFPLock>(b"WFPLock"))
    };

    wfp_lock(hService, dwTimeOut, hWnd, unsafe { *lpRequestID })
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSOpen(
    lpszLogicalName: LPSTR,
    hApp: HAPP,
    lpszAppID: LPSTR,
    dwTraceLevel: DWORD,
    dwTimeOut: DWORD,
    dwSrvcVersionsRequired: DWORD,
    lpSrvcVersion: LPWFSVERSION,
    lpSPIVersion: LPWFSVERSION,
    lphService: LPHSERVICE,
) -> HRESULT {
    trace!("WFSOpen");
    assert_started!();
    call_async(WFS_OPEN_COMPLETE, |hwnd, request_id| {
        WFSAsyncOpen(
            lpszLogicalName,
            hApp,
            lpszAppID,
            dwTraceLevel,
            dwTimeOut,
            lphService,
            hwnd,
            dwSrvcVersionsRequired,
            lpSrvcVersion,
            lpSPIVersion,
            request_id,
        )
    })
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncOpen(
    lpszLogicalName: LPSTR,
    hApp: HAPP,
    lpszAppID: LPSTR,
    dwTraceLevel: DWORD,
    dwTimeOut: DWORD,
    lphService: LPHSERVICE,
    hWnd: HWND,
    dwSrvcVersionsRequired: DWORD,
    lpSrvcVersion: LPWFSVERSION,
    lpSPIVersion: LPWFSVERSION,
    lpRequestID: LPREQUESTID,
) -> HRESULT {
    if lpszLogicalName.is_null() || lpszAppID.is_null() || lphService.is_null() || lpSrvcVersion.is_null() || lpSPIVersion.is_null() {
        return WFS_ERR_INVALID_POINTER;
    }

    assert_started!();

    let service_index = {
        let services = xfs_unwrap!(SERVICES.lock());
        match services.iter().position(|s| s.is_none()) {
            Some(index) => index,
            None => return WFS_ERR_INTERNAL_ERROR,
        }
    };

    unsafe {
        *lphService = 0;
        *lpRequestID = 0;

        (*lpSrvcVersion).w_version = 0;
        (*lpSrvcVersion).w_low_version = 0;
        (*lpSrvcVersion).w_high_version = 0;
        (*lpSrvcVersion).sz_description = [0; WFSDDESCRIPTION_LEN + 1];
        (*lpSrvcVersion).sz_system_status = [0; WFSDDESCRIPTION_LEN + 1];

        (*lpSPIVersion).w_version = 0;
        (*lpSPIVersion).w_low_version = 0;
        (*lpSPIVersion).w_high_version = 0;
        (*lpSPIVersion).sz_description = [0; WFSDDESCRIPTION_LEN + 1];
        (*lpSPIVersion).sz_system_status = [0; WFSDDESCRIPTION_LEN + 1];
    }

    let mut lgl_prov_path: [u8; MAX_PATH] = [0; MAX_PATH]; // Change size as needed.
    let lgl_prov_len = &mut (MAX_PATH as u32);

    unsafe {
        let mut lgl_key: HKEY = ptr::null_mut();

        let lpszLogicalName = xfs_unwrap!(CStr::from_ptr(lpszLogicalName).to_str());
        let path = xfs_unwrap!(CString::new(format!("LOGICAL_SERVICES\\{}", lpszLogicalName)));

        if (XFS_CONFIG.WFMOpenKey)(WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT, path.as_ptr() as *mut i8, &mut lgl_key) != WFS_SUCCESS {
            return WFS_ERR_INVALID_SERVPROV;
        }

        let name = xfs_unwrap!(CString::new("provider"));

        if (XFS_CONFIG.WFMQueryValue)(lgl_key, name.as_ptr() as *mut i8, lgl_prov_path.as_mut_ptr() as *mut i8, lgl_prov_len) != WFS_SUCCESS {
            (XFS_CONFIG.WFMCloseKey)(lgl_key);
            return WFS_ERR_INVALID_SERVPROV;
        }

        (XFS_CONFIG.WFMCloseKey)(lgl_key);
    }

    let mut phy_prov_path: [u8; MAX_PATH] = [0; MAX_PATH]; // Change size as needed.
    let phy_prov_len = &mut (MAX_PATH as u32);

    unsafe {
        let mut phy_key: HKEY = ptr::null_mut();

        let lgl_prov_path = &lgl_prov_path[..*lgl_prov_len as usize];
        let lgl_prov_path = xfs_unwrap!(xfs_unwrap!(CStr::from_bytes_with_nul(lgl_prov_path)).to_str());
        let path = xfs_unwrap!(CString::new(format!("SERVICE_PROVIDERS\\{}", lgl_prov_path)));

        trace!("OPENING {}", path.to_str().unwrap());

        if (XFS_CONFIG.WFMOpenKey)(WFS_CFG_HKEY_MACHINE_XFS_ROOT, path.as_ptr() as *mut i8, &mut phy_key) != WFS_SUCCESS {
            return WFS_ERR_INVALID_SERVPROV;
        }

        let name = xfs_unwrap!(CString::new("dllname"));

        if (XFS_CONFIG.WFMQueryValue)(phy_key, name.as_ptr() as *mut i8, phy_prov_path.as_mut_ptr() as *mut i8, phy_prov_len) != WFS_SUCCESS {
            (XFS_CONFIG.WFMCloseKey)(phy_key);
            return WFS_ERR_INVALID_SERVPROV;
        }

        (XFS_CONFIG.WFMCloseKey)(phy_key);
    }

    let phy_prov_path = &phy_prov_path[..*phy_prov_len as usize];
    let phy_prov_path = xfs_unwrap!(xfs_unwrap!(CStr::from_bytes_with_nul(phy_prov_path)).to_str());

    let library = unsafe { xfs_unwrap!(libloading::Library::new(phy_prov_path)) };

    let mut services = xfs_unwrap!(SERVICES.lock());
    let request_id = 1;
    services[service_index] = Some(Service {
        library,
        request_id,
        trace_level: dwTraceLevel,
    });

    unsafe {
        *lphService = service_index as u16 + 1;
        *lpRequestID = 1;
    }

    let mut service_index = service_index;
    let serviceHandle = &mut service_index as *mut usize as *mut c_void;

    let service = services[service_index].as_ref().unwrap();

    unsafe {
        let wfp_open = xfs_unwrap!(service.library.get::<spi::WfpOpen>(b"WFPOpen"));

        wfp_open(
            *lphService,
            lpszLogicalName,
            hApp,
            lpszAppID,
            dwTraceLevel,
            dwTimeOut,
            hWnd,
            *lpRequestID,
            serviceHandle,
            VersionRange::new_explicit(Version::new_explicit(3, 0), Version::new_explicit(3, 30)).value(),
            lpSPIVersion,
            dwSrvcVersionsRequired,
            lpSrvcVersion,
        );
    }

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSRegister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND) -> HRESULT {
    trace!("WFSRegister");
    assert_started!();
    call_async(WFS_GETINFO_COMPLETE, |hwnd, request_id| WFSAsyncRegister(hService, dwEventClass, hWndReg, hwnd, request_id))
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncRegister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncRegister");
    assert_started!();
    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_register = unsafe {
        *lpRequestID = service.request_id as u32;
        xfs_unwrap!(service.library.get::<spi::WFPRegister>(b"WFPRegister"))
    };

    wfp_register(hService, dwEventClass, hWndReg, hWnd, unsafe { *lpRequestID })
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSSetBlockingHook(_lpBlockFunc: XFSBLOCKINGHOOK, _lppPrevFunc: LPXFSBLOCKINGHOOK) -> HRESULT {
    trace!("WFSSetBlockingHook");
    assert_started!();
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSStartUp(dwVersionsRequired: DWORD, lpWFSVersion: LPWFSVERSION) -> HRESULT {
    let range = VersionRange::new(dwVersionsRequired);

    if range.start > range.end {
        return WFS_ERR_INTERNAL_ERROR;
    }

    if range.start > Version::new_explicit(3, 30) {
        return WFS_ERR_API_VER_TOO_HIGH;
    }

    if range.end < Version::new_explicit(2, 00) {
        return WFS_ERR_API_VER_TOO_LOW;
    }

    if lpWFSVersion.is_null() {
        return WFS_ERR_INVALID_POINTER;
    }

    unsafe {
        (*lpWFSVersion).w_version = 3;
        (*lpWFSVersion).w_low_version = 2;
        (*lpWFSVersion).w_high_version = 7683;
        (*lpWFSVersion).sz_system_status[0] = '\0' as i8;
    }

    let description = "Rust XFS Manager v2.00 to v3.20".as_bytes();
    for i in 0..description.len() {
        unsafe {
            (*lpWFSVersion).sz_description[i] = description[i] as i8;
        }
    }

    if STARTED.load(Ordering::SeqCst) {
        return WFS_ERR_ALREADY_STARTED;
    }

    STARTED.store(true, Ordering::SeqCst);
    trace!("XFS STARTUP DONE");

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSUnhookBlockingHook() -> HRESULT {
    trace!("WFSUnhookBlockingHook");
    assert_started!();
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSUnlock(hService: HSERVICE) -> HRESULT {
    trace!("WFSUnlock");
    assert_started!();
    call_async(WFS_GETINFO_COMPLETE, |hwnd, request_id| WFSAsyncUnlock(hService, hwnd, request_id))
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncUnlock(hService: HSERVICE, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncUnlock");
    assert_started!();
    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_unlock = unsafe {
        *lpRequestID = service.request_id as u32;
        xfs_unwrap!(service.library.get::<spi::WFPUnlock>(b"WFPUnlock"))
    };

    wfp_unlock(hService, hWnd, unsafe { *lpRequestID })
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMAllocateBuffer(ulSize: ULONG, ulFlags: ULONG, lppvData: *mut LPVOID) -> HRESULT {
    if lppvData.is_null() {
        return WFS_ERR_INVALID_POINTER;
    }

    unsafe {
        *lppvData = HeapAlloc(GetProcessHeap(), ulFlags, ulSize as usize);
    }

    if lppvData.is_null() {
        return WFS_ERR_OUT_OF_MEMORY;
    }

    unsafe {
        buffer::ALLOC_MAP.lock().unwrap().insert(buffer::Data::new(*lppvData), buffer::List::new());
    }

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMAllocateMore(ulSize: ULONG, lpvOriginal: LPVOID, lppvData: *mut LPVOID) -> HRESULT {
    if lppvData.is_null() {
        return WFS_ERR_INVALID_POINTER;
    }

    let mut locked = buffer::ALLOC_MAP.lock().unwrap();

    if !locked.contains_key(&buffer::Data::new(lpvOriginal)) {
        return WFS_ERR_INVALID_POINTER;
    }

    unsafe {
        *lppvData = HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, ulSize as usize);
    }

    if lppvData.is_null() {
        return WFS_ERR_OUT_OF_MEMORY;
    }

    let list = locked.get_mut(&buffer::Data::new(lpvOriginal)).unwrap();

    unsafe {
        list.push_back(*lppvData);
    }

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMFreeBuffer(lpvData: LPVOID) -> HRESULT {
    if lpvData.is_null() {
        return WFS_ERR_INVALID_POINTER;
    }

    let mut locked = buffer::ALLOC_MAP.lock().unwrap();

    if !locked.contains_key(&buffer::Data::new(lpvData)) {
        return WFS_ERR_INVALID_BUFFER;
    }

    let list = locked.get(&buffer::Data::new(lpvData)).unwrap();

    for ptr in list.iter() {
        unsafe {
            HeapFree(GetProcessHeap(), 0, *ptr);
        }
    }

    locked.remove(&buffer::Data::new(lpvData));

    unsafe {
        HeapFree(GetProcessHeap(), 0, lpvData);
    }

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMGetTraceLevel(hService: HSERVICE, lpdwTraceLevel: LPDWORD) -> HRESULT {
    trace!("WFMGetTraceLevel");
    assert_started!();

    let services = xfs_unwrap!(SERVICES.lock());

    if let Some(service) = services.get(hService as usize - 1).and_then(|service| service.as_ref()) {
        unsafe {
            *lpdwTraceLevel = service.trace_level;
        }
        return WFS_SUCCESS;
    }

    WFS_ERR_INVALID_HSERVICE
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMKillTimer(wTimerID: WORD) -> HRESULT {
    let mut timers = xfs_unwrap!(TIMERS.lock());

    if timers[wTimerID as usize].is_none() {
        return WFS_ERR_INVALID_TIMER;
    }

    unsafe {
        if KillTimer(std::ptr::null::<HWND>() as HWND, wTimerID as UINT_PTR) == 0 {
            return WFS_ERR_INTERNAL_ERROR;
        }
    }

    // TODO: Free pointer?
    timers[wTimerID as usize] = None;

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMOutputTraceData(_lpszData: LPSTR) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMReleaseDLL(_hProvider: HPROVIDER) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMSetTimer(hWnd: HWND, lpContext: LPVOID, dwTimeVal: DWORD, lpwTimerID: LPWORD) -> HRESULT {
    if hWnd.is_null() {
        return WFS_ERR_INVALID_HWND;
    }

    if lpwTimerID.is_null() {
        return WFS_ERR_INVALID_POINTER;
    }

    if dwTimeVal == 0 {
        return WFS_ERR_INVALID_DATA;
    }

    let mut timers = xfs_unwrap!(TIMERS.lock());
    let timer_id = timers.iter().position(|t| t.is_none());

    if timer_id.is_none() {
        return WFS_ERR_INTERNAL_ERROR;
    }

    timers.insert(timer_id.unwrap(), Some(TimerCtx { hWnd, lpContext }));

    unsafe {
        unsafe extern "system" fn timer_proc(_: HWND, _: UINT, id_event: UINT_PTR, _: DWORD) {
            let timers = TIMERS.lock().unwrap();
            let timer = timers.get(id_event).unwrap().as_ref().unwrap();
            PostMessageA(timer.hWnd, WFS_TIMER_EVENT, id_event, timer.lpContext as LPARAM);
        }

        if SetTimer(std::ptr::null::<HWND>() as HWND, timer_id.unwrap() as UINT_PTR, dwTimeVal, Some(timer_proc)) == 0 {
            return WFS_ERR_INTERNAL_ERROR;
        }
        *lpwTimerID = timer_id.unwrap() as u16;
    }

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMSetTraceLevel(hService: HSERVICE, dwTraceLevel: DWORD) -> HRESULT {
    trace!("WFMSetTraceLevel");
    assert_started!();
    if hService == 0 {
        return WFS_ERR_INVALID_HSERVICE;
    }

    let mut services = xfs_unwrap!(SERVICES.lock());

    if let Some(service) = services.get_mut(hService as usize - 1).and_then(|service| service.as_mut()) {
        service.trace_level = dwTraceLevel;
        let wfp_set_trace_level = unsafe { xfs_unwrap!(service.library.get::<spi::WFPSetTraceLevel>(b"WFPSetTraceLevel")) };
        return wfp_set_trace_level(hService, dwTraceLevel);
    }

    WFS_ERR_INVALID_HSERVICE
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn DllMain(_hinst_dll: HINSTANCE, fdw_reason: DWORD, _: LPVOID) -> bool {
    if fdw_reason == DLL_PROCESS_ATTACH {
        let logfile = FileAppender::builder().build("xfs-mgr.log").unwrap();
        let config = Config::builder()
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
            .build(Root::builder().appender("logfile").build(LevelFilter::Trace))
            .unwrap();

        log4rs::init_config(config).unwrap();
        trace!("XFS DLL INIT");
    }
    true
}

fn call_async(message: u32, async_fn: impl Fn(HWND, LPREQUESTID) -> HRESULT) -> HRESULT {
    let window = window::SyncWindow::new(message);
    let request_id = ptr::null_mut();

    let result = async_fn(window.handle(), request_id);

    if result != WFS_SUCCESS {
        return result;
    }

    let resultptr = xfs_unwrap!(window.wait());
    let createstruct: *mut WFSRESULT = resultptr as *mut _;

    unsafe { (*createstruct).hResult }
}

fn call_async_result(message: u32, async_fn: impl Fn(HWND, LPREQUESTID) -> HRESULT, lpp_result: &mut LPWFSRESULT) -> HRESULT {
    let window = window::SyncWindow::new(message);
    let request_id = ptr::null_mut();

    let result = async_fn(window.handle(), request_id);

    if result != WFS_SUCCESS {
        return result;
    }

    let resultptr = xfs_unwrap!(window.wait());
    let createstruct: *mut WFSRESULT = resultptr as *mut _;
    *lpp_result = createstruct;

    unsafe { (*createstruct).hResult }
}
