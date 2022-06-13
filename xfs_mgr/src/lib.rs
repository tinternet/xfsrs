use std::{
    collections::HashMap,
    ffi::{c_void, CStr, CString},
    mem, ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use lazy_static::lazy_static;
use log::{error, trace, LevelFilter};
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    Config,
};
use winapi::{
    shared::{
        basetsd::{UINT_PTR, ULONG_PTR},
        minwindef::{DWORD, HINSTANCE, HKEY, LPARAM, LPDWORD, LPVOID, LPWORD, MAX_PATH, UINT, ULONG, WORD},
        windef::HWND,
        winerror::HRESULT,
    },
    um::{
        heapapi::{GetProcessHeap, HeapAlloc, HeapFree},
        processthreadsapi::GetCurrentThreadId,
        winnt::{DLL_PROCESS_ATTACH, HEAP_ZERO_MEMORY, LPSTR},
        winuser::{DispatchMessageW, GetMessageW, KillTimer, PostMessageA, SetTimer, TranslateMessage},
    },
};

use conf::*;
use xfslib::*;

mod conf;
mod spi;
mod window;

lazy_static! {
    // holds service handles
    static ref SERVICES: Mutex<Vec<Option<Service>>> = Mutex::new((0..8192).map(|_| None).collect());

    // holds app handles
    static ref APP_HANDLES: Mutex<[bool; 8192]> = Mutex::new([false; 8192]);

    // indicates whether WFSStartup has been called
    static ref STARTED: AtomicBool = AtomicBool::new(false);

    // holds application timers
    static ref TIMERS: Mutex<Vec<Option<Timer>>> = Mutex::new((0..65535).map(|_| None).collect());

    // holds application & service providers buffers
    static ref BUFFERS: Mutex<HashMap<ULONG_PTR, Vec<ULONG_PTR>>> = Mutex::new(HashMap::new());

    // holds blocked threads and unblock flag
    static ref BLOCKED_THREADS: Mutex<HashMap<DWORD, bool>> = Mutex::new(HashMap::new());

    // holds application defined blocking hook
    static ref BLOCKING_HOOK: Mutex<Option<XFSBLOCKINGHOOK>> = Mutex::new(None);
}

/// Unwraps result, logging error if any and returning xfs internal error value.
macro_rules! xfs_unwrap {
    ($l:expr) => {
        match $l {
            Ok(result) => result,
            Err(error) => {
                error!("{:?}", error);
                return WFS_ERR_INTERNAL_ERROR;
            }
        }
    };
}

/// Asserts that the WFSStartup function has been called.
macro_rules! assert_started {
    () => {
        if !STARTED.load(Ordering::SeqCst) {
            return WFS_ERR_NOT_STARTED;
        }
    };
}

/// Asserts that the current thread id does not have a blocking call in progress.
macro_rules! assert_unblocked {
    () => {{
        let thread_id = unsafe { GetCurrentThreadId() };

        if xfs_unwrap!(BLOCKED_THREADS.lock()).contains_key(&thread_id) {
            return WFS_ERR_OP_IN_PROGRESS;
        }
    }};
}

/// Asserts that the current thread id does not have a blocking call in progress and sets the blocking status to true.
macro_rules! block_thread {
    () => {{
        let thread_id = unsafe { GetCurrentThreadId() };
        let mut blocked_threads = xfs_unwrap!(BLOCKED_THREADS.lock());

        if blocked_threads.contains_key(&thread_id) {
            return WFS_ERR_OP_IN_PROGRESS;
        } else {
            blocked_threads.insert(thread_id, false);
        }
    }};
}

/// Gets a service by handle and increments the request id
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

struct Timer {
    hwnd: ULONG_PTR,
    lpcontext: ULONG_PTR,
    timer_id: usize,
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSCancelAsyncRequest(hService: HSERVICE, RequestID: REQUESTID) -> HRESULT {
    trace!("WFSCancelAsyncRequest");
    assert_started!();
    assert_unblocked!();

    let services = xfs_unwrap!(SERVICES.lock());
    if let Some(service) = services.get(hService as usize - 1).and_then(|service| service.as_ref()) {
        let wfp_cancel_async_request = unsafe { xfs_unwrap!(service.library.get::<spi::WfpCancelAsyncRequest>(b"WFPCancelAsyncRequest")) };
        return wfp_cancel_async_request(hService, RequestID);
    }

    if hService == 0 {
        return WFS_ERR_INVALID_HSERVICE;
    }

    if let Some(service) = services.get(hService as usize - 1).and_then(|service| service.as_ref()) {
        let wfp_cancel_async_request = unsafe { xfs_unwrap!(service.library.get::<spi::WfpCancelAsyncRequest>(b"WFPCancelAsyncRequest")) };
        return wfp_cancel_async_request(hService, RequestID);
    }

    WFS_ERR_INVALID_HSERVICE
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSCancelBlockingCall(dwThreadID: DWORD) -> HRESULT {
    trace!("WFSCancelBlockingCall");
    assert_started!();

    let thread_id = match dwThreadID {
        0 => unsafe { GetCurrentThreadId() },
        _ => dwThreadID,
    };

    let mut blocks = xfs_unwrap!(BLOCKED_THREADS.lock());

    if blocks.contains_key(&thread_id) {
        blocks.insert(thread_id, true);
    }

    WFS_SUCCESS
}

// TODO: Close service providers
#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSCleanUp() -> HRESULT {
    trace!("WFSCleanUp");
    assert_unblocked!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    *services = (0..8192).map(|_| None).collect();

    let mut timers = xfs_unwrap!(TIMERS.lock());
    *timers = (0..65535).map(|_| None).collect();

    let mut handles = xfs_unwrap!(APP_HANDLES.lock());
    *handles = [false; 8192];

    STARTED.store(false, Ordering::SeqCst);

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSClose(hService: HSERVICE) -> HRESULT {
    trace!("WFSClose");
    assert_started!();
    block_thread!();
    call_async(WFS_CLOSE_COMPLETE, |hwnd, reqid| WFSAsyncClose(hService, hwnd, reqid), &mut ptr::null_mut())
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncClose(hService: HSERVICE, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncClose");
    assert_started!();
    assert_unblocked!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_close = unsafe {
        *lpRequestID = service.request_id as u32;
        trace!("CALLING WFP Close");
        xfs_unwrap!(service.library.get::<spi::WfpClose>(b"WFPClose"))
    };

    wfp_close(hService, hWnd, unsafe { *lpRequestID })
}

/// Requests a new, unique application handle value.
///
/// This function is used by an application to request a unique (within a single system) application
/// handle from the XFS Manager (to be used in subsequent WFSOpen/WFSAsyncOpen calls).
/// Note that an application may call this function multiple times in order to create multiple
/// “application identities” for itself with respect to the XFS subsystem. See Sections 4.5 and 4.8.2
/// for additional discussion.
///
/// # Note:
/// As per section Section 4.5, neither service nor application handles may be shared among two or more applications.
/// This algorithm makes use of the uniqueness of system pointers.
/// It takes the base pointer of array in the process memory and adds offset to it,
/// thus making it unique for each application process and/or thread.
#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSCreateAppHandle(lphApp: LPHAPP) -> HRESULT {
    trace!("WFSCreateAppHandle");
    assert_started!();
    assert_unblocked!();

    let mut handles = xfs_unwrap!(APP_HANDLES.lock());

    let free = match handles.iter().position(|h| !h) {
        Some(index) => {
            handles[index] = true;
            index
        }
        None => return WFS_ERR_INTERNAL_ERROR,
    };

    let ptr = (&*handles) as *const _ as usize + free;
    unsafe {
        *lphApp = ptr as HAPP;
    }
    trace!("Created app handle {:?}", ptr);

    trace!("XFS APP HANDLE DONE");
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSDeregister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND) -> HRESULT {
    trace!("WFSDeregister");
    assert_started!();
    block_thread!();
    call_async(
        WFS_DEREGISTER_COMPLETE,
        |hwnd, request_id| WFSAsyncDeregister(hService, dwEventClass, hWndReg, hwnd, request_id),
        &mut ptr::null_mut(),
    )
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncDeregister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncDeregister");
    assert_started!();
    assert_unblocked!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_deregister = unsafe {
        *lpRequestID = service.request_id as u32;
        xfs_unwrap!(service.library.get::<spi::WFPDeregister>(b"WFPDeregister"))
    };

    wfp_deregister(hService, dwEventClass, hWndReg, hWnd, unsafe { *lpRequestID })
}

/// Makes the specified application handle invalid.
///
/// This function is used by an application to indicate to the XFS Manager that it will no longer use
/// the specified application handle (from a previous WFSCreateAppHandle call). See
/// WFSCreateAppHandle and Sections 4.5 and 4.8.2 for additional discussion.
#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSDestroyAppHandle(hApp: HAPP) -> HRESULT {
    trace!("WFSDestroyAppHandle");
    assert_started!();
    assert_unblocked!();

    if hApp.is_null() {
        return WFS_ERR_INVALID_APP_HANDLE;
    }

    let mut handles = xfs_unwrap!(APP_HANDLES.lock());
    let ptr = (&*handles) as *const _ as usize;
    let index = hApp as usize - ptr;

    match handles.get_mut(index) {
        Some(h) => *h = false,
        None => return WFS_ERR_INVALID_APP_HANDLE,
    }

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSExecute(hService: HSERVICE, dwCommandd: DWORD, lpCmdData: LPVOID, dwTimeOut: DWORD, lppResult: &mut LPWFSRESULT) -> HRESULT {
    trace!("WFSExecute");
    assert_started!();
    block_thread!();
    call_async(
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
    assert_unblocked!();

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
    assert_unblocked!();
    WFMFreeBuffer(lpResult as *mut c_void)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSGetInfo(hService: HSERVICE, dwCategory: DWORD, lpQueryDetails: LPVOID, dwTimeOut: DWORD, lppResult: &mut LPWFSRESULT) -> HRESULT {
    trace!("WFSGetInfo");
    assert_started!();
    block_thread!();
    call_async(
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
    assert_unblocked!();

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
    trace!("WFSIsBlocking");
    let thread_id = unsafe { GetCurrentThreadId() };
    BLOCKED_THREADS.lock().unwrap().contains_key(&thread_id)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSLock(hService: HSERVICE, dwTimeOut: DWORD, lppResult: &mut LPWFSRESULT) -> HRESULT {
    trace!("WFSLock");
    assert_started!();
    block_thread!();
    call_async(WFS_LOCK_COMPLETE, |hwnd, request_id| WFSAsyncLock(hService, dwTimeOut, hwnd, request_id), lppResult)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncLock(hService: HSERVICE, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncLock");
    assert_started!();
    assert_unblocked!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_lock = unsafe {
        *lpRequestID = service.request_id as u32;
        xfs_unwrap!(service.library.get::<spi::WFPLock>(b"WFPLock"))
    };

    wfp_lock(hService, dwTimeOut, hWnd, unsafe { *lpRequestID })
}

/// Initiates a session (a series of service requests terminated with the WFSClose function) between the application and
/// the specified service. This does not necessarily mean that the hardware is opened. This command will return with
/// WFS_SUCCESS even if the hardware is inoperable, offline or powered off. The status of the device can be
/// requested through a WFSGetInfo command.
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
    block_thread!();

    let result = call_async(
        WFS_OPEN_COMPLETE,
        |hwnd, request_id| {
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
        },
        &mut ptr::null_mut(),
    );

    trace!("WFSOpen result: {:?}", result);
    result
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
    assert_started!();
    assert_unblocked!();

    unsafe {
        trace!("OPENING DEVICE: {}", CStr::from_ptr(lpszLogicalName).to_str().unwrap());
    }
    // if lpszLogicalName.is_null() || lpszAppID.is_null() || lphService.is_null() || lpSrvcVersion.is_null() || lpSPIVersion.is_null() {
    //     return WFS_ERR_INVALID_POINTER;
    // }

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
        trace!("WFSOpen: path: {:?}", path);

        if WFM_OPEN_KEY(WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT, path.as_ptr() as *mut i8, &mut lgl_key) != WFS_SUCCESS {
            return WFS_ERR_INVALID_SERVPROV;
        }

        let name = xfs_unwrap!(CString::new("provider"));

        if WFM_QUERY_VALUE(lgl_key, name.as_ptr() as *mut i8, lgl_prov_path.as_mut_ptr() as *mut i8, lgl_prov_len) != WFS_SUCCESS {
            WFM_CLOSE_KEY(lgl_key);
            return WFS_ERR_INVALID_SERVPROV;
        }

        WFM_CLOSE_KEY(lgl_key);
    }

    let mut phy_prov_path: [u8; MAX_PATH] = [0; MAX_PATH]; // Change size as needed.
    let phy_prov_len = &mut (MAX_PATH as u32);

    unsafe {
        let mut phy_key: HKEY = ptr::null_mut();

        let lgl_prov_path = &lgl_prov_path[..*lgl_prov_len as usize];
        let lgl_prov_path = xfs_unwrap!(std::str::from_utf8(lgl_prov_path));
        let path = xfs_unwrap!(CString::new(format!("SERVICE_PROVIDERS\\{}", lgl_prov_path)));

        trace!("OPENING {}", path.to_str().unwrap());

        if WFM_OPEN_KEY(WFS_CFG_HKEY_MACHINE_XFS_ROOT, path.as_ptr() as *mut i8, &mut phy_key) != WFS_SUCCESS {
            return WFS_ERR_INVALID_SERVPROV;
        }

        let name = xfs_unwrap!(CString::new("dllname"));

        if WFM_QUERY_VALUE(phy_key, name.as_ptr() as *mut i8, phy_prov_path.as_mut_ptr() as *mut i8, phy_prov_len) != WFS_SUCCESS {
            WFM_CLOSE_KEY(phy_key);
            return WFS_ERR_INVALID_SERVPROV;
        }

        WFM_CLOSE_KEY(phy_key);
    }

    let phy_prov_path = &phy_prov_path[..*phy_prov_len as usize];
    let phy_prov_path = xfs_unwrap!(std::str::from_utf8(phy_prov_path));

    trace!("WFSOpen: phy_prov_path: {:?}", phy_prov_path);
    let library = unsafe { xfs_unwrap!(libloading::Library::new(phy_prov_path)) };

    let mut services = xfs_unwrap!(SERVICES.lock());

    let service_index = match services.iter().position(|s| s.is_none()) {
        Some(index) => index,
        None => return WFS_ERR_INTERNAL_ERROR,
    };

    services[service_index] = Some(Service {
        library,
        request_id: 1,
        trace_level: dwTraceLevel,
    });

    unsafe {
        *lphService = service_index as u16 + 1;
        *lpRequestID = 1;
    }

    let service_handle = (&*services) as *const _ as HPROVIDER;
    let service_handle = unsafe { service_handle.add(service_index) };

    let service = services[service_index].as_ref().unwrap();

    unsafe {
        let wfp_open = xfs_unwrap!(service.library.get::<spi::WfpOpen>(b"WFPOpen"));

        trace!("WFSOpen: calling spi::WfpOpen");
        wfp_open(
            *lphService,
            lpszLogicalName,
            hApp,
            lpszAppID,
            dwTraceLevel,
            dwTimeOut,
            hWnd,
            *lpRequestID,
            service_handle,
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
    block_thread!();
    call_async(
        WFS_GETINFO_COMPLETE,
        |hwnd, request_id| WFSAsyncRegister(hService, dwEventClass, hWndReg, hwnd, request_id),
        &mut ptr::null_mut(),
    )
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncRegister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncRegister");
    assert_started!();
    assert_unblocked!();

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
pub extern "stdcall" fn WFSSetBlockingHook(lpBlockFunc: XFSBLOCKINGHOOK, lppPrevFunc: LPXFSBLOCKINGHOOK) -> HRESULT {
    trace!("WFSSetBlockingHook");
    assert_started!();
    assert_unblocked!();

    let mut blocking_hook = xfs_unwrap!(BLOCKING_HOOK.lock());

    match *blocking_hook {
        Some(prev) => unsafe {
            *lppPrevFunc = prev;
            *blocking_hook = Some(lpBlockFunc);
        },
        None => {
            *blocking_hook = Some(lpBlockFunc);
        }
    }

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
    assert_unblocked!();
    let mut blocking_hook = xfs_unwrap!(BLOCKING_HOOK.lock());
    *blocking_hook = None;
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSUnlock(hService: HSERVICE) -> HRESULT {
    trace!("WFSUnlock");
    assert_started!();
    block_thread!();
    call_async(WFS_GETINFO_COMPLETE, |hwnd, request_id| WFSAsyncUnlock(hService, hwnd, request_id), &mut ptr::null_mut())
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncUnlock(hService: HSERVICE, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncUnlock");
    assert_started!();
    assert_unblocked!();

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

    let mut buffers = xfs_unwrap!(BUFFERS.lock());

    unsafe {
        *lppvData = HeapAlloc(GetProcessHeap(), ulFlags, ulSize as usize);
    }

    if lppvData.is_null() {
        return WFS_ERR_OUT_OF_MEMORY;
    }

    unsafe {
        buffers.insert(*lppvData as ULONG_PTR, Vec::new());
    }

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMAllocateMore(ulSize: ULONG, lpvOriginal: LPVOID, lppvData: *mut LPVOID) -> HRESULT {
    if lppvData.is_null() {
        return WFS_ERR_INVALID_POINTER;
    }

    let mut buffers = xfs_unwrap!(BUFFERS.lock());
    let list = match buffers.get_mut(&(lpvOriginal as ULONG_PTR)) {
        Some(list) => list,
        None => return WFS_ERR_INVALID_BUFFER,
    };

    unsafe {
        *lppvData = HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, ulSize as usize);
    }

    if lppvData.is_null() {
        return WFS_ERR_OUT_OF_MEMORY;
    }

    unsafe {
        list.push(*lppvData as ULONG_PTR);
    }

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMFreeBuffer(lpvData: LPVOID) -> HRESULT {
    if lpvData.is_null() {
        return WFS_ERR_INVALID_POINTER;
    }

    let mut buffers = xfs_unwrap!(BUFFERS.lock());
    let list = match buffers.get_mut(&(lpvData as ULONG_PTR)) {
        Some(list) => list,
        None => return WFS_ERR_INVALID_BUFFER,
    };

    for &ptr in list.iter() {
        unsafe {
            HeapFree(GetProcessHeap(), 0, ptr as *mut c_void);
        }
    }

    buffers.remove(&(lpvData as ULONG_PTR));

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

    let timer = match timers.get_mut(wTimerID as usize - 1) {
        Some(Some(timer)) => timer,
        _ => return WFS_ERR_INVALID_TIMER,
    };

    unsafe {
        if KillTimer(std::ptr::null_mut(), timer.timer_id) == 0 {
            return WFS_ERR_INTERNAL_ERROR;
        }
    }

    timers.insert(wTimerID as usize - 1, None);
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMOutputTraceData(lpszData: LPSTR) -> HRESULT {
    trace!("WFMOutputTraceData: {}", xfs_unwrap!(unsafe { CStr::from_ptr(lpszData).to_str() }));
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

    let free = match timers.iter().position(|t| t.is_none()) {
        Some(index) => index,
        None => return WFS_ERR_INTERNAL_ERROR,
    };

    let id_event = (&*timers) as *const _ as usize + free;
    let timer_id = unsafe { SetTimer(hWnd, id_event, dwTimeVal, Some(timer_proc)) };

    let timer = Timer {
        hwnd: hWnd as ULONG_PTR,
        lpcontext: lpContext as ULONG_PTR,
        timer_id,
    };
    timers[free] = Some(timer);

    unsafe {
        *lpwTimerID = (free + 1) as u16;
    }

    unsafe extern "system" fn timer_proc(_: HWND, _: UINT, id_event: UINT_PTR, _: DWORD) {
        let timers = TIMERS.lock().unwrap(); // TODO: don't unwrap this shit, think of a better way to handle it?
        let timer = &timers[id_event as usize - (&*timers as *const _ as usize)].as_ref().unwrap();
        PostMessageA(timer.hwnd as HWND, WFS_TIMER_EVENT, id_event, timer.lpcontext as LPARAM);
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
    let service = match services.get_mut(hService as usize - 1).and_then(|service| service.as_mut()) {
        Some(service) => service,
        None => return WFS_ERR_INVALID_HSERVICE,
    };

    service.trace_level = dwTraceLevel;
    let wfp_set_trace_level = unsafe { xfs_unwrap!(service.library.get::<spi::WFPSetTraceLevel>(b"WFPSetTraceLevel")) };
    return wfp_set_trace_level(hService, dwTraceLevel);
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn DllMain(_hinst_dll: HINSTANCE, fdw_reason: DWORD, _: LPVOID) -> bool {
    if fdw_reason == DLL_PROCESS_ATTACH {
        let logfile = FileAppender::builder().build("C:\\XFS_MGR.txt").unwrap();
        let config = Config::builder()
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
            .build(Root::builder().appender("logfile").build(LevelFilter::Trace))
            .unwrap();

        log4rs::init_config(config).unwrap();
        trace!("XFS DLL INIT");
    }
    true
}

/// Calls asynchronous function on the current thread.
/// Within the XFS Manager, a blocking (synchronous) function is handled as follows:
/// 1. The XFS Manager creates a transitory HWND on the calling thread to receive the completion message for
/// the operation e.g. WFS_EXECUTE_COMPLETE.
/// 2. The XFS Manager calls the Service Provider WFP API, passing the transitory HWND.
/// 3. The XFS Manager waits for the completion message to be received. It does this by entering a loop
/// equivalent to the following pseudo code, calling the current blocking hook (a Windows message dispatch
/// routine) waiting for the completion message to be received from the Service Provider.
/// ``` for(;;) {
/// /* flush messages for good user response */
/// for(;;) {
/// BlockingHook();
/// /* check for WFSCancelBlockingCall() */
/// if ( operation_cancelled() )
///  break;
/// /* check to see if operation completed */
/// if( operation_completed() )
///  break; /* normal completion */
/// }
/// where the Default Blocking Hook is equivalent to:
/// BOOL DefaultBlockingHook(void) {
///  MSG msg = {0};
///  BOOL ret = GetMessage(&msg, NULL, 0, 0);
///  if( (int) ret != -1 ) {
///  TranslateMessage(&msg);
///  DispatchMessage(&msg);
///  }
///  /* FALSE if we got a WM_QUIT message */
///  return( ret );
/// }
/// ```
/// 4. On reception of the completion message, the XFS Manager exits the loop.
/// 5. The XFS Manager destroys the transitory HWND.
/// 6. The blocking operation completes. The blocking function return code is copied from the completion
/// message lpWFSResult hResult field. If applicable, the lpWFSResult is also returned.
///
/// The thread, on which the blocking function has been called, is not permitted to issue any XFS calls other than the
/// following two specific functions provided to assist the developer in this situation.
///
/// • WFSIsBlocking determines whether or not a blocking function is in progress.
/// • WFSCancelBlockingCall cancels a blocking function in progress.
///
/// Any other XFS function, called from a thread with a blocking function in progress, will fail with the error
/// WFS_ERR_OP_IN_PROGRESS.
///
/// Developers must be aware that WFSIsBlocking cannot simply be called in a loop waiting for the blocking function
/// to complete. The application must allow the message handler to return to allow control to return to the blocking
/// hook. Otherwise, the blocking function will not complete.
/// Although this mechanism is sufficient for simple applications, it cannot support those applications which require
/// more complex message processing while a blocking function is executing, such as processing messages relating to
/// MDI (Multiple Document Interface) events, accelerator key translations, and modeless dialogs. For such
/// applications, the XFS API includes the function WFSSetBlockingHook, which allows the developer to define a
/// custom blocking hook which will be called instead of the default blocking hook described above. It is not intended
/// as a mechanism for performing general application functions while blocked; it is still true that the only XFS
/// functions that may be called from a blocking routine are WFSIsBlocking and WFSCancelBlockingCall. The
/// asynchronous versions of the XFS functions must be used to allow an application to continue processing while an
/// operation is in progress. Developers must be aware of their responsibility when replacing the default blocking hook.
/// CWA 16926-1:2020 (E)
/// 39
/// The developer must ensure:
///
/// • All messages are processed in the order received. If not, the potential exists for the Service Provider to be
/// blamed for sending messages in the wrong order e.g. a WFS_EXECUTE_EVENT message after a
/// WFS_EXECUTE_COMPLETE.
/// • All messages are processed. If not, the potential exists that the thread message queue will fill preventing
/// other messages being added to the queue, including the Service Provider attempt to post the completion
/// message being waited on.
///
/// The developer must be aware that replacing the default blocking hook impacts the process. The custom blocking
/// hook will be called from every thread which makes use of XFS blocking functions.
///
/// In a multi-threaded environment, the developer of a multi-threaded application must be aware that it is the
/// responsibility of the application, not the XFS Manager, to synchronize access to a service by multiple threads.
/// Failure to synchronize calls to a service leads to unpredictable results; for example, if two threads "simultaneously"
/// issue WFSExecute requests to send data to the same service, there is no guarantee as to the order in which the data
/// is sent. This is true in general; the application is responsible for coordinating access by multiple threads to any
/// object (e.g. other forms of I/O, such as file I/O), using appropriate synchronization mechanisms. The XFS Manager
/// can not, and will not, address these issues. The possible consequences of failing to observe these rules are beyond
/// the scope of this specification.
///
/// In order to allow maximum flexibility in the design and implementation of applications, especially in multithreaded environments, the concept of "application identity" can optionally be managed explicitly by the
/// application developer using the concept of application handles. See Sections 4.5 and 4.8.2 for additional discussion
/// of this concept.
fn call_async(message: u32, async_fn: impl Fn(HWND, LPREQUESTID) -> HRESULT, lpp_result: &mut LPWFSRESULT) -> HRESULT {
    let window = window::SyncWindow::new(message);
    let mut request_id = 0;

    let result = async_fn(window.handle(), &mut request_id);

    if result != WFS_SUCCESS {
        return result;
    }

    loop {
        // Execute application hook or default hook dispatching window messages
        match *xfs_unwrap!(BLOCKING_HOOK.lock()) {
            Some(hook) => {
                hook();
            }
            None => unsafe {
                default_block_hook();
            },
        }

        let thread_id = unsafe { GetCurrentThreadId() };
        let mut blocked_threads = xfs_unwrap!(BLOCKED_THREADS.lock());

        // Check if the call was cancelled
        if blocked_threads.get(&thread_id).unwrap_or(&false) == &true {
            blocked_threads.remove(&thread_id); // cleanup
            return WFS_ERR_CANCELED;
        }

        // Check if we received result from the async call
        if let Some(resultptr) = xfs_unwrap!(window.try_receive()) {
            let createstruct: *mut WFSRESULT = resultptr as *mut _;
            *lpp_result = createstruct;
            return unsafe { (*createstruct).hResult };
        }
    }
}

/// Default blocking hook for synchronous calls
unsafe fn default_block_hook() {
    let mut msg = mem::zeroed();
    if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) != 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}
