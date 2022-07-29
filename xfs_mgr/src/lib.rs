use std::{
    collections::HashMap,
    ffi::{CStr, CString},
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
    encode::pattern::PatternEncoder,
    Config,
};
use log_derive::{logfn, logfn_inputs};
use winapi::{
    shared::{
        minwindef::{DWORD, HINSTANCE, HKEY, LPDWORD, LPVOID, LPWORD, MAX_PATH, ULONG, WORD},
        windef::HWND,
        winerror::HRESULT,
    },
    um::{
        processthreadsapi::GetCurrentThreadId,
        winnt::{DLL_PROCESS_ATTACH, LPSTR},
        winuser::{DispatchMessageW, GetMessageW, TranslateMessage},
    },
};

use conf::*;
use supp::*;
use xfslib::*;

mod conf;
mod spi;
mod supp;
mod window;

lazy_static! {
    // holds service handles
    static ref SERVICES: Mutex<Vec<Option<Service>>> = Mutex::new((0..8192).map(|_| None).collect());

    // holds app handles
    static ref APP_HANDLES: Mutex<[bool; 8192]> = Mutex::new([false; 8192]);

    // indicates whether WFSStartup has been called
    static ref STARTED: AtomicBool = AtomicBool::new(false);

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

macro_rules! xfs_reject {
    ($l:expr) => {{
        error!("XFS_SUPP {}", stringify!($l));
        return $l;
    }};
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
            xfs_reject!(WFS_ERR_OP_IN_PROGRESS);
        }
    }};
}

/// Asserts that the current thread id does not have a blocking call in progress and sets the blocking status to true.
macro_rules! block_thread {
    () => {{
        let thread_id = unsafe { GetCurrentThreadId() };
        let mut blocked_threads = xfs_unwrap!(BLOCKED_THREADS.lock());

        if blocked_threads.contains_key(&thread_id) {
            xfs_reject!(WFS_ERR_OP_IN_PROGRESS);
        } else {
            blocked_threads.insert(thread_id, false);
        }
    }};
}

/// Gets a service by handle and increments the request id
macro_rules! get_service_req {
    ($hService:expr, $services:expr) => {{
        match $hService {
            0 => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
            _ => match $services.get_mut($hService as usize - 1).and_then(|service| service.as_mut()) {
                Some(service) => {
                    service.request_id += 1;
                    service
                }
                None => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
            },
        }
    }};
}

struct Service {
    service_id: HSERVICE,
    request_id: u32,
    library: libloading::Library,
    trace_level: DWORD,
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSCancelAsyncRequest(hService: HSERVICE, RequestID: REQUESTID) -> HRESULT {
    assert_started!();
    // assert_unblocked!();

    if hService == 0 {
        xfs_reject!(WFS_ERR_INVALID_HSERVICE);
    }

    let services = xfs_unwrap!(SERVICES.lock());
    let service = match services.get(hService as usize - 1).and_then(|service| service.as_ref()) {
        Some(service) => service,
        None => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
    };
    let cancel = unsafe { xfs_unwrap!(service.library.get::<spi::WfpCancelAsyncRequest>(b"WFPCancelAsyncRequest")) };

    cancel(hService, RequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSCancelBlockingCall(dwThreadID: DWORD) -> HRESULT {
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
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSCleanUp() -> HRESULT {
    // assert_unblocked!();
    STARTED.store(false, Ordering::SeqCst);

    {
        let mut services = xfs_unwrap!(SERVICES.lock());
        services.iter_mut().filter_map(|s| s.take()).for_each(drop);
    }

    {
        let mut handles = xfs_unwrap!(APP_HANDLES.lock());
        *handles = [false; 8192];
    }

    {
        let mut blocked_threads = xfs_unwrap!(BLOCKED_THREADS.lock());
        *blocked_threads = HashMap::new();
    }

    {
        let mut blocking_hook = xfs_unwrap!(BLOCKING_HOOK.lock());
        *blocking_hook = None;
    }

    // unsafe {
    //     (XFS_SUPP_CLEANUP)();
    // }

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSClose(hService: HSERVICE) -> HRESULT {
    assert_started!();
    // block_thread!();
    call_async(WFS_CLOSE_COMPLETE, |hwnd, reqid| WFSAsyncClose(hService, hwnd, reqid), &mut ptr::null_mut())
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncClose(hService: HSERVICE, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    // assert_unblocked!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_close = unsafe {
        lpRequestID.write(service.request_id as u32);
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
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSCreateAppHandle(lphApp: LPHAPP) -> HRESULT {
    assert_started!();
    // assert_unblocked!();

    let mut handles = xfs_unwrap!(APP_HANDLES.lock());

    let free = match handles.iter().position(|h| !h) {
        Some(index) => index,
        None => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    };

    handles[free] = true;
    let ptr = (&*handles) as *const _ as usize + free;

    unsafe {
        lphApp.write(ptr as HAPP);
    }

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSDeregister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND) -> HRESULT {
    assert_started!();
    // block_thread!();
    call_async(
        WFS_DEREGISTER_COMPLETE,
        |hwnd, request_id| WFSAsyncDeregister(hService, dwEventClass, hWndReg, hwnd, request_id),
        &mut ptr::null_mut(),
    )
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncDeregister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    // assert_unblocked!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_deregister = unsafe {
        lpRequestID.write(service.request_id as u32);
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
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSDestroyAppHandle(hApp: HAPP) -> HRESULT {
    assert_started!();
    // assert_unblocked!();

    if hApp.is_null() {
        return WFS_ERR_INVALID_APP_HANDLE;
    }

    let mut handles = xfs_unwrap!(APP_HANDLES.lock());
    let ptr = (&*handles) as *const _ as usize;
    let index = hApp as usize - ptr;

    match handles.get_mut(index) {
        Some(h) => *h = false,
        None => xfs_reject!(WFS_ERR_INVALID_APP_HANDLE),
    }

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSExecute(hService: HSERVICE, dwCommandd: DWORD, lpCmdData: LPVOID, dwTimeOut: DWORD, lppResult: *mut LPWFSRESULT) -> HRESULT {
    assert_started!();
    // block_thread!();
    call_async(
        WFS_EXECUTE_COMPLETE,
        |hwnd, request_id| WFSAsyncExecute(hService, dwCommandd, lpCmdData, dwTimeOut, hwnd, request_id),
        lppResult,
    )
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncExecute(hService: HSERVICE, dwCommand: DWORD, lpCmdData: LPVOID, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    // assert_unblocked!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_execute = unsafe {
        lpRequestID.write(service.request_id as u32);
        xfs_unwrap!(service.library.get::<spi::WFPExecute>(b"WFPExecute"))
    };

    wfp_execute(hService, dwCommand, lpCmdData, dwTimeOut, hWnd, unsafe { *lpRequestID })
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSFreeResult(lpResult: LPWFSRESULT) -> HRESULT {
    assert_started!();
    // assert_unblocked!();
    unsafe { WFMFreeBuffer(lpResult as *mut _) }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSGetInfo(hService: HSERVICE, dwCategory: DWORD, lpQueryDetails: LPVOID, dwTimeOut: DWORD, lppResult: *mut LPWFSRESULT) -> HRESULT {
    assert_started!();
    // block_thread!();
    call_async(
        WFS_GETINFO_COMPLETE,
        |hwnd, request_id| WFSAsyncGetInfo(hService, dwCategory, lpQueryDetails, dwTimeOut, hwnd, request_id),
        lppResult,
    )
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncGetInfo(hService: HSERVICE, dwCategory: DWORD, lpQueryDetails: LPVOID, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    // assert_unblocked!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_get_info = unsafe {
        lpRequestID.write(service.request_id as u32);
        xfs_unwrap!(service.library.get::<spi::WFPGetInfo>(b"WFPGetInfo"))
    };

    wfp_get_info(hService, dwCategory, lpQueryDetails, dwTimeOut, hWnd, unsafe { *lpRequestID })
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSIsBlocking() -> bool {
    let thread_id = unsafe { GetCurrentThreadId() };
    BLOCKED_THREADS.lock().unwrap().contains_key(&thread_id)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSLock(hService: HSERVICE, dwTimeOut: DWORD, lppResult: *mut LPWFSRESULT) -> HRESULT {
    assert_started!();
    // block_thread!();
    call_async(WFS_LOCK_COMPLETE, |hwnd, request_id| WFSAsyncLock(hService, dwTimeOut, hwnd, request_id), lppResult)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncLock(hService: HSERVICE, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    // assert_unblocked!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_lock = unsafe {
        lpRequestID.write(service.request_id as u32);
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
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
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
    assert_started!();
    // block_thread!();

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
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
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
    // assert_unblocked!();

    if lpszLogicalName.is_null() || lpSrvcVersion.is_null() || lpSPIVersion.is_null() || lphService.is_null() || lpRequestID.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    fn get_value(root: HKEY, path: CString, name: CString) -> Result<String, HRESULT> {
        let mut key = ptr::null_mut();

        // SAFETY: the path pointer is function argument, and it is not null
        unsafe {
            if WFM_OPEN_KEY(root, path.as_ptr() as *mut _, &mut key) != WFS_SUCCESS {
                error!("WFM_OPEN_KEY failed");
                return Err(WFS_ERR_INVALID_SERVPROV);
            }
        }

        let mut value_buffer: Vec<u8> = Vec::with_capacity(MAX_PATH);
        let mut value_len = MAX_PATH as u32;

        // SAFETY:
        // - the key pointer is not null as the WFM_OPEN_KEY call succeeded
        // - the value pointer is function argument, and it is not null
        // - the value buffer pointer is not null as the vector allocated this memory
        unsafe {
            if WFM_QUERY_VALUE(key, name.as_ptr() as *mut i8, value_buffer.as_mut_ptr() as *mut _, &mut value_len) != WFS_SUCCESS {
                WFM_CLOSE_KEY(key);
                error!("WFM_QUERY_VALUE failed");
                return Err(WFS_ERR_INVALID_SERVPROV);
            }
        }

        // SAFETY: We know that the buffer is at least as large as the value_len.
        unsafe {
            value_buffer.set_len(value_len as usize);
        }

        // SAFETY: the key was opened by WFM_OPEN_KEY, so it is a valid pointer.
        unsafe {
            WFM_CLOSE_KEY(key);
        }

        Ok(String::from_utf8(value_buffer).map_err(|error| {
            error!("{}", error);
            WFS_ERR_INTERNAL_ERROR
        })?)
    }

    let logical_name = xfs_unwrap!(unsafe { CStr::from_ptr(lpszLogicalName) }.to_str());
    let path = xfs_unwrap!(CString::new(format!("LOGICAL_SERVICES\\{}", logical_name)));
    let lgl_prov_path = match get_value(WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT, path, CString::new("provider").unwrap()) {
        Ok(lgl_prov_path) => lgl_prov_path,
        Err(error) => return error,
    };

    let path = xfs_unwrap!(CString::new(format!("SERVICE_PROVIDERS\\{}", lgl_prov_path)));
    let phy_prov_path = match get_value(WFS_CFG_HKEY_MACHINE_XFS_ROOT, path, CString::new("dllname").unwrap()) {
        Ok(phy_prov_path) => phy_prov_path,
        Err(error) => return error,
    };

    // SAFETY: The service providers are safe to use.
    let library = unsafe { xfs_unwrap!(libloading::Library::new(phy_prov_path)) };

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service_index = match services.iter().position(|s| s.is_none()) {
        Some(index) => index,
        None => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    };

    services[service_index] = Some(Service {
        service_id: service_index as u16 + 1,
        library,
        request_id: 1,
        trace_level: dwTraceLevel,
    });
    let service = services[service_index].as_ref().unwrap();

    // SAFETY: The service providers are safe to use. All pointers are checked and not null.
    unsafe {
        *lphService = service_index as u16 + 1;
        *lpRequestID = 1;

        let wfp_open = xfs_unwrap!(service.library.get::<spi::WfpOpen>(b"WFPOpen"));
        let service_handle = ((&*services) as *const _ as HPROVIDER).add(service_index);

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
        )
    }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSRegister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND) -> HRESULT {
    assert_started!();
    // block_thread!();
    if hService == 0 {
        return WFS_SUCCESS;
    }
    call_async(
        WFS_REGISTER_COMPLETE,
        |hwnd, request_id| WFSAsyncRegister(hService, dwEventClass, hWndReg, hwnd, request_id),
        &mut ptr::null_mut(),
    )
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncRegister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    // assert_unblocked!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_register = unsafe {
        lpRequestID.write(service.request_id as u32);
        xfs_unwrap!(service.library.get::<spi::WFPRegister>(b"WFPRegister"))
    };

    wfp_register(hService, dwEventClass, hWndReg, hWnd, unsafe { *lpRequestID })
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
// #[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSSetBlockingHook(lpBlockFunc: XFSBLOCKINGHOOK, lppPrevFunc: *mut XFSBLOCKINGHOOK) -> HRESULT {
    assert_started!();
    // assert_unblocked!();

    let mut blocking_hook = xfs_unwrap!(BLOCKING_HOOK.lock());

    match *blocking_hook {
        Some(prev) => unsafe {
            lppPrevFunc.write(prev);
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
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSStartUp(dwVersionsRequired: DWORD, lpWFSVersion: LPWFSVERSION) -> HRESULT {
    let range = VersionRange::new(dwVersionsRequired);

    if range.start > range.end {
        xfs_reject!(WFS_ERR_INTERNAL_ERROR);
    }
    if range.start > Version::new_explicit(3, 30) {
        xfs_reject!(WFS_ERR_API_VER_TOO_HIGH);
    }
    if range.end < Version::new_explicit(2, 00) {
        xfs_reject!(WFS_ERR_API_VER_TOO_LOW);
    }
    if lpWFSVersion.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    let mut version = WFSVERSION {
        w_version: Version::new_explicit(3, 0).value(),
        w_low_version: Version::new_explicit(2, 0).value(),
        w_high_version: Version::new_explicit(3, 30).value(),
        sz_description: [0; WFSDDESCRIPTION_LEN + 1],
        sz_system_status: [0; WFSDSYSSTATUS_LEN + 1],
    };

    let description = "Rust XFS Manager v2.00 to v3.30".as_bytes();
    let mut description_array = [0; WFSDDESCRIPTION_LEN + 1];
    for i in 0..std::cmp::min(description.len(), WFSDDESCRIPTION_LEN) {
        description_array[i] = description[i] as i8;
    }

    let desc_mut = ptr::addr_of_mut!(version.sz_description);
    unsafe { desc_mut.write_unaligned(description_array) };

    // SAFETY: The pointer is not null. Using ptr::write to avoid dropping memory allocated by the caller.
    unsafe {
        lpWFSVersion.write(version);
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
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSUnhookBlockingHook() -> HRESULT {
    assert_started!();
    // assert_unblocked!();
    let mut blocking_hook = xfs_unwrap!(BLOCKING_HOOK.lock());
    *blocking_hook = None;
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSUnlock(hService: HSERVICE) -> HRESULT {
    assert_started!();
    // block_thread!();
    call_async(WFS_UNLOCK_COMPLETE, |hwnd, request_id| WFSAsyncUnlock(hService, hwnd, request_id), &mut ptr::null_mut())
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncUnlock(hService: HSERVICE, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    // assert_unblocked!();

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = get_service_req!(hService, services);

    let wfp_unlock = unsafe {
        lpRequestID.write(service.request_id as u32);
        xfs_unwrap!(service.library.get::<spi::WFPUnlock>(b"WFPUnlock"))
    };

    wfp_unlock(hService, hWnd, unsafe { *lpRequestID })
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMGetTraceLevel(hService: HSERVICE, lpdwTraceLevel: LPDWORD) -> HRESULT {
    assert_started!();

    let services = xfs_unwrap!(SERVICES.lock());

    if let Some(service) = services.get(hService as usize - 1).and_then(|service| service.as_ref()) {
        unsafe {
            lpdwTraceLevel.write(service.trace_level);
        }
        return WFS_SUCCESS;
    }

    xfs_reject!(WFS_ERR_INVALID_HSERVICE);
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMReleaseDLL(hProvider: HPROVIDER) -> HRESULT {
    let mut services = xfs_unwrap!(SERVICES.lock());
    let service_handle = (&*services) as *const _ as usize;
    let index = hProvider as usize - service_handle;
    services[index] = None;
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMAllocateBuffer(ulSize: ULONG, ulFlags: ULONG, lppvData: *mut LPVOID) -> HRESULT {
    (WFM_ALLOCATE_BUFFER)(ulSize, ulFlags, lppvData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMAllocateMore(ulSize: ULONG, lpvOriginal: LPVOID, lppvData: *mut LPVOID) -> HRESULT {
    (WFM_ALLOCATE_MORE)(ulSize, lpvOriginal, lppvData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMFreeBuffer(lpvData: LPVOID) -> HRESULT {
    (WFM_FREE_BUFFER)(lpvData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMKillTimer(wTimerID: WORD) -> HRESULT {
    (WFM_KILL_TIMER)(wTimerID)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMOutputTraceData(lpszData: LPSTR) -> HRESULT {
    trace!("WFMOutputTraceData: {}", xfs_unwrap!(unsafe { CStr::from_ptr(lpszData).to_str() }));
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMSetTimer(hWnd: HWND, lpContext: LPVOID, dwTimeVal: DWORD, lpwTimerID: LPWORD) -> HRESULT {
    (WFM_SET_TIMER)(hWnd, lpContext, dwTimeVal, lpwTimerID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMSetTraceLevel(hService: HSERVICE, dwTraceLevel: DWORD) -> HRESULT {
    assert_started!();

    if hService == 0 {
        xfs_reject!(WFS_ERR_INVALID_HSERVICE);
    }

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = match services.get_mut(hService as usize - 1).and_then(|service| service.as_mut()) {
        Some(service) => service,
        None => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
    };

    service.trace_level = dwTraceLevel;
    let wfp_set_trace_level = unsafe { xfs_unwrap!(service.library.get::<spi::WFPSetTraceLevel>(b"WFPSetTraceLevel")) };
    return wfp_set_trace_level(hService, dwTraceLevel);
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn DllMain(_hinst_dll: HINSTANCE, fdw_reason: DWORD, _: LPVOID) -> bool {
    if fdw_reason == DLL_PROCESS_ATTACH {
        let logfile = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} {l} {L} - {m}\n")))
            .build("C:\\Diebold\\XFS_MGR.log")
            .unwrap();
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
fn call_async(message: u32, async_fn: impl Fn(HWND, LPREQUESTID) -> HRESULT, lpp_result: *mut LPWFSRESULT) -> HRESULT {
    let window = window::SyncWindow::new(message);
    let mut request_id = 0;

    let result = async_fn(window.handle(), &mut request_id);

    if result != WFS_SUCCESS {
        return result;
    }

    loop {
        // Execute application hook or default hook dispatching window messages
        match *xfs_unwrap!(BLOCKING_HOOK.lock()) {
            Some(hook) => unsafe {
                hook();
            },
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
            unsafe { lpp_result.write(resultptr as LPWFSRESULT) };
            let wfs_result = resultptr as *const WFSRESULT;
            let wfs_result = unsafe { std::mem::transmute::<*const WFSRESULT, &WFSRESULT>(wfs_result) };
            return unsafe { ptr::addr_of!(wfs_result.hResult).read_unaligned() };
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
