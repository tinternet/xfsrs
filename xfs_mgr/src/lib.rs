use std::{
    collections::HashSet,
    mem, ptr,
    sync::{
        atomic::{AtomicBool, AtomicPtr, Ordering},
        Mutex,
    },
};

use lazy_static::lazy_static;
use log_derive::{logfn, logfn_inputs};
use winapi::{
    shared::{
        minwindef::{DWORD, HINSTANCE, LPDWORD, LPVOID, LPWORD, ULONG, WORD},
        windef::HWND,
        winerror::HRESULT,
    },
    um::{
        processthreadsapi::GetCurrentThreadId,
        winnt::LPSTR,
        winuser::{DispatchMessageW, GetMessageW, TranslateMessage},
    },
};

use xfslib::*;

mod manager;
mod service;
// mod spi;

lazy_static! {
    // indicates whether WFSStartup has been called
    static ref STARTED: AtomicBool = AtomicBool::new(false);
    // holds blocked threads and unblock flag
    static ref BLOCKED_THREADS: Mutex<HashSet<DWORD>> = Mutex::new(HashSet::new());
    // holds application defined blocking hook
    static ref BLOCKING_HOOK: AtomicPtr<XFSBLOCKINGHOOK> = AtomicPtr::new(ptr::null_mut());
}

/// Asserts that the WFSStartup function has been called.
macro_rules! assert_started {
    () => {
        if !STARTED.load(Ordering::SeqCst) {
            xfs_reject!(WFS_ERR_NOT_STARTED);
        }
    };
}

/// Asserts that the current thread id does not have a blocking call in progress.
macro_rules! assert_unblocked {
    () => {
        if blocked_threads!().contains(unsafe { &GetCurrentThreadId() }) {
            xfs_reject!(WFS_ERR_OP_IN_PROGRESS);
        }
    };
}

/// Asserts that the current thread id does not have a blocking call in progress and sets the blocking status to true.
macro_rules! block_thread {
    () => {
        if !blocked_threads!().insert(unsafe { GetCurrentThreadId() }) {
            xfs_reject!(WFS_ERR_OP_IN_PROGRESS);
        }
    };
}

macro_rules! blocked_threads {
    () => {
        BLOCKED_THREADS.lock().unwrap_or_else(|e| e.into_inner())
    };
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSCancelAsyncRequest(h_service: HSERVICE, request_id: REQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    manager::cancel_request(h_service, request_id)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSCancelBlockingCall(dwThreadID: DWORD) -> HRESULT {
    assert_started!();
    if blocked_threads!().remove(&dwThreadID) {
        return WFS_SUCCESS;
    }
    xfs_reject!(WFS_ERR_NO_BLOCKING_CALL);
}

// TODO: Close service providers
#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSCleanUp() -> HRESULT {
    assert_started!();
    assert_unblocked!();
    manager::clean_up()
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSClose(h_service: HSERVICE) -> HRESULT {
    assert_started!();
    block_thread!();
    let window = SyncWindow::new(WFS_CLOSE_COMPLETE);
    let result = manager::close_service(h_service, window.handle(), ptr::null_mut());

    if result != WFS_SUCCESS {
        xfs_reject!(result);
    }

    let result = match wait_result(window) {
        Ok(result) => result,
        Err(error) => return error,
    };

    unsafe { ptr::addr_of!(result.hResult).read_unaligned() }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncClose(service: HSERVICE, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    manager::close_service(service, wnd, request_id)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSCreateAppHandle(lphApp: LPHAPP) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    manager::create_app_handle(lphApp)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSDeregister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND) -> HRESULT {
    assert_started!();
    block_thread!();
    let window = SyncWindow::new(WFS_DEREGISTER_COMPLETE);
    let result = manager::deregister(hService, dwEventClass, hWndReg, window.handle(), ptr::null_mut());

    if result != WFS_SUCCESS {
        xfs_reject!(result);
    }

    let result = match wait_result(window) {
        Ok(result) => result,
        Err(error) => return error,
    };

    unsafe { ptr::addr_of!(result.hResult).read_unaligned() }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncDeregister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    manager::deregister(hService, dwEventClass, hWndReg, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSDestroyAppHandle(hApp: HAPP) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    manager::destroy_app_handle(hApp)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSExecute(hService: HSERVICE, dwCommand: DWORD, lpCmdData: LPVOID, dwTimeOut: DWORD, lppResult: *mut LPWFSRESULT) -> HRESULT {
    assert_started!();
    block_thread!();
    let window = SyncWindow::new(WFS_EXECUTE_COMPLETE);
    let result = manager::execute(hService, dwCommand, lpCmdData, dwTimeOut, window.handle(), ptr::null_mut());

    if result != WFS_SUCCESS {
        xfs_reject!(result);
    }

    let result = match wait_result(window) {
        Ok(result) => result,
        Err(error) => return error,
    };

    unsafe { ptr::addr_of!(result.hResult).read_unaligned() }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncExecute(hService: HSERVICE, dwCommand: DWORD, lpCmdData: LPVOID, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    manager::execute(hService, dwCommand, lpCmdData, dwTimeOut, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSFreeResult(lpResult: LPWFSRESULT) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    unsafe { WFMFreeBuffer(lpResult as *mut _) }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSGetInfo(hService: HSERVICE, dwCategory: DWORD, lpQueryDetails: LPVOID, dwTimeOut: DWORD, lppResult: *mut LPWFSRESULT) -> HRESULT {
    assert_started!();
    block_thread!();
    let window = SyncWindow::new(WFS_GETINFO_COMPLETE);
    let result = manager::get_info(hService, dwCategory, lpQueryDetails, dwTimeOut, window.handle(), ptr::null_mut());

    if result != WFS_SUCCESS {
        xfs_reject!(result);
    }

    let result = match wait_result(window) {
        Ok(result) => result,
        Err(error) => return error,
    };

    unsafe { ptr::addr_of!(result.hResult).read_unaligned() }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncGetInfo(hService: HSERVICE, dwCategory: DWORD, lpQueryDetails: LPVOID, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    manager::get_info(hService, dwCategory, lpQueryDetails, dwTimeOut, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSIsBlocking() -> bool {
    let thread_id = unsafe { GetCurrentThreadId() };
    let blocks = BLOCKED_THREADS.lock().unwrap_or_else(|e| e.into_inner());
    blocks.contains(&thread_id)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSLock(hService: HSERVICE, dwTimeOut: DWORD, lppResult: *mut LPWFSRESULT) -> HRESULT {
    assert_started!();
    block_thread!();
    let window = SyncWindow::new(WFS_LOCK_COMPLETE);
    let result = manager::lock(hService, dwTimeOut, window.handle(), ptr::null_mut());

    if result != WFS_SUCCESS {
        xfs_reject!(result);
    }

    let result = match wait_result(window) {
        Ok(result) => result,
        Err(error) => return error,
    };

    unsafe { ptr::addr_of!(result.hResult).read_unaligned() }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncLock(hService: HSERVICE, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    manager::lock(hService, dwTimeOut, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSOpen(
    logical_name: LPSTR,
    app: HAPP,
    app_id: LPSTR,
    trace_level: DWORD,
    time_out: DWORD,
    srvc_versions_required: DWORD,
    srvc_version: LPWFSVERSION,
    spiversion: LPWFSVERSION,
    service: LPHSERVICE,
) -> HRESULT {
    assert_started!();
    block_thread!();

    let window = SyncWindow::new(WFS_OPEN_COMPLETE);
    let result = manager::open_service(
        logical_name,
        app,
        app_id,
        trace_level,
        time_out,
        service,
        window.handle(),
        srvc_versions_required,
        srvc_version,
        spiversion,
        ptr::null_mut(),
    );

    if result != WFS_SUCCESS {
        xfs_reject!(result);
    }

    let result = match wait_result(window) {
        Ok(result) => result,
        Err(error) => return error,
    };

    unsafe { ptr::addr_of!(result.hResult).read_unaligned() }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncOpen(
    logical_name: LPSTR,
    app: HAPP,
    app_id: LPSTR,
    trace_level: DWORD,
    time_out: DWORD,
    service: LPHSERVICE,
    wnd: HWND,
    srvc_versions_required: DWORD,
    srvc_version: LPWFSVERSION,
    spiversion: LPWFSVERSION,
    request_id: LPREQUESTID,
) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    manager::open_service(
        logical_name,
        app,
        app_id,
        trace_level,
        time_out,
        service,
        wnd,
        srvc_versions_required,
        srvc_version,
        spiversion,
        request_id,
    )
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSRegister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND) -> HRESULT {
    assert_started!();
    block_thread!();

    if hService == 0 {
        return WFS_SUCCESS;
    }

    let window = SyncWindow::new(WFS_REGISTER_COMPLETE);
    let result = manager::register(hService, dwEventClass, hWndReg, window.handle(), ptr::null_mut());

    if result != WFS_SUCCESS {
        xfs_reject!(result);
    }

    let result = match wait_result(window) {
        Ok(result) => result,
        Err(error) => return error,
    };

    unsafe { ptr::addr_of!(result.hResult).read_unaligned() }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncRegister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    manager::register(hService, dwEventClass, hWndReg, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
pub extern "stdcall" fn WFSSetBlockingHook(lpBlockFunc: *mut XFSBLOCKINGHOOK, lppPrevFunc: *mut *mut XFSBLOCKINGHOOK) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    let previous = BLOCKING_HOOK.swap(lpBlockFunc, Ordering::SeqCst);
    unsafe { lppPrevFunc.write(previous) };
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSStartUp(dwVersionsRequired: DWORD, lpWFSVersion: LPWFSVERSION) -> HRESULT {
    if STARTED.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return WFS_ERR_ALREADY_STARTED;
    }
    manager::start_up(dwVersionsRequired, lpWFSVersion)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSUnhookBlockingHook() -> HRESULT {
    assert_started!();
    assert_unblocked!();
    BLOCKING_HOOK.store(ptr::null_mut(), Ordering::SeqCst);
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncUnlock(hService: HSERVICE, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    manager::unlock(hService, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMGetTraceLevel(h_service: HSERVICE, lpdw_trace_level: LPDWORD) -> HRESULT {
    assert_started!();
    manager::get_trace_level(h_service, lpdw_trace_level)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMReleaseDLL(hProvider: HPROVIDER) -> HRESULT {
    assert_started!();
    manager::release_dll(hProvider)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMAllocateBuffer(ulSize: ULONG, ulFlags: ULONG, lppvData: *mut LPVOID) -> HRESULT {
    xfslib::heap::allocate_buffer(ulSize, ulFlags, lppvData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMAllocateMore(ulSize: ULONG, lpvOriginal: LPVOID, lppvData: *mut LPVOID) -> HRESULT {
    xfslib::heap::allocate_more(ulSize, lpvOriginal, lppvData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMFreeBuffer(lpvData: LPVOID) -> HRESULT {
    xfslib::heap::free_buffer(lpvData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMKillTimer(wTimerID: WORD) -> HRESULT {
    xfslib::timer::kill_timer(wTimerID)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMOutputTraceData(lpsz_data: LPSTR) -> HRESULT {
    xfslib::output_trace_data(lpsz_data)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMSetTimer(hWnd: HWND, lpContext: LPVOID, dwTimeVal: DWORD, lpwTimerID: LPWORD) -> HRESULT {
    xfslib::timer::set_timer(hWnd, lpContext, dwTimeVal, lpwTimerID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMSetTraceLevel(hService: HSERVICE, dwTraceLevel: DWORD) -> HRESULT {
    assert_started!();
    manager::set_trace_level(hService, dwTraceLevel)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn DllMain(hinst_dll: HINSTANCE, fdw_reason: DWORD, _: LPVOID) -> bool {
    module_init(hinst_dll, fdw_reason);
    true
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSUnlock(hService: HSERVICE) -> HRESULT {
    assert_started!();
    block_thread!();
    let window = SyncWindow::new(WFS_UNLOCK_COMPLETE);
    let result = manager::unlock(hService, window.handle(), ptr::null_mut());

    if result != WFS_SUCCESS {
        xfs_reject!(result);
    }

    let result = match wait_result(window) {
        Ok(result) => result,
        Err(error) => return error,
    };

    unsafe { ptr::addr_of!(result.hResult).read_unaligned() }
}

fn wait_result(window: SyncWindow) -> Result<WFSRESULT, HRESULT> {
    loop {
        // Execute application hook or default hook dispatching window messages
        let hook = BLOCKING_HOOK.load(Ordering::SeqCst);
        let hook = if hook.is_null() { default_block_hook } else { unsafe { *hook } };
        unsafe { hook() };

        // Quit if the process cancelled the call
        if !WFSIsBlocking() {
            // Drain the remaining messages
            while unsafe { hook() } {}
            // mgr::cancel_request(h_service, request_id);
            xfs_reject_err!(WFS_ERR_CANCELED);
        }

        // Check if we received result from the async call
        if let Some(resultptr) = xfs_unwrap_err!(window.try_receive()) {
            let wfs_result = resultptr as *const WFSRESULT;
            let result = unsafe { std::mem::transmute_copy::<*const WFSRESULT, WFSRESULT>(&wfs_result) };
            return Ok(result);
        }
    }
}

/// Default blocking hook for synchronous calls
unsafe extern "stdcall" fn default_block_hook() -> bool {
    let mut msg = mem::zeroed();
    if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) != 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
        return true;
    }
    false
}
