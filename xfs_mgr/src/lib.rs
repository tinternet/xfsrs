use std::{
    collections::HashSet,
    mem, ptr,
    sync::{
        atomic::{AtomicBool, AtomicPtr, Ordering},
        Mutex,
    },
};

use lazy_static::lazy_static;
use log::error;
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

mod mgr;
mod spi;

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
        if xfs_unwrap!(BLOCKED_THREADS.lock()).contains(unsafe { &GetCurrentThreadId() }) {
            xfs_reject!(WFS_ERR_OP_IN_PROGRESS);
        }
    };
}

/// Asserts that the current thread id does not have a blocking call in progress and sets the blocking status to true.
macro_rules! block_thread {
    () => {
        if !xfs_unwrap!(BLOCKED_THREADS.lock()).insert(unsafe { GetCurrentThreadId() }) {
            xfs_reject!(WFS_ERR_OP_IN_PROGRESS);
        }
    };
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSCancelAsyncRequest(h_service: HSERVICE, request_id: REQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    mgr::cancel_request(h_service, request_id)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSCancelBlockingCall(dwThreadID: DWORD) -> HRESULT {
    assert_started!();
    let mut blocks = BLOCKED_THREADS.lock().unwrap_or_else(|e| e.into_inner());
    if blocks.remove(&dwThreadID) {
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
    mgr::clean_up()
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSClose(hService: HSERVICE) -> HRESULT {
    assert_started!();
    block_thread!();
    call_async(WFS_CLOSE_COMPLETE, |hwnd, reqid| mgr::async_close(hService, hwnd, reqid), &mut ptr::null_mut())
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncClose(hService: HSERVICE, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    mgr::async_close(hService, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSCreateAppHandle(lphApp: LPHAPP) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    mgr::create_app_handle(lphApp)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSDeregister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND) -> HRESULT {
    assert_started!();
    block_thread!();
    call_async(
        WFS_DEREGISTER_COMPLETE,
        |hwnd, request_id| mgr::async_deregister(hService, dwEventClass, hWndReg, hwnd, request_id),
        &mut ptr::null_mut(),
    )
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncDeregister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    mgr::async_deregister(hService, dwEventClass, hWndReg, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSDestroyAppHandle(hApp: HAPP) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    mgr::destroy_app_handle(hApp)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSExecute(hService: HSERVICE, dwCommand: DWORD, lpCmdData: LPVOID, dwTimeOut: DWORD, lppResult: *mut LPWFSRESULT) -> HRESULT {
    assert_started!();
    block_thread!();
    call_async(
        WFS_EXECUTE_COMPLETE,
        |hwnd, request_id| mgr::async_execute(hService, dwCommand, lpCmdData, dwTimeOut, hwnd, request_id),
        lppResult,
    )
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncExecute(hService: HSERVICE, dwCommand: DWORD, lpCmdData: LPVOID, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    mgr::async_execute(hService, dwCommand, lpCmdData, dwTimeOut, hWnd, lpRequestID)
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
    call_async(
        WFS_GETINFO_COMPLETE,
        |hwnd, request_id| mgr::async_get_info(hService, dwCategory, lpQueryDetails, dwTimeOut, hwnd, request_id),
        lppResult,
    )
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncGetInfo(hService: HSERVICE, dwCategory: DWORD, lpQueryDetails: LPVOID, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    mgr::async_get_info(hService, dwCategory, lpQueryDetails, dwTimeOut, hWnd, lpRequestID)
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
    call_async(WFS_LOCK_COMPLETE, |hwnd, request_id| mgr::async_lock(hService, dwTimeOut, hwnd, request_id), lppResult)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncLock(hService: HSERVICE, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    mgr::async_lock(hService, dwTimeOut, hWnd, lpRequestID)
}

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
    block_thread!();
    call_async(
        WFS_OPEN_COMPLETE,
        |hwnd, request_id| {
            mgr::async_open(
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
    )
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
    assert_unblocked!();
    mgr::async_open(
        lpszLogicalName,
        hApp,
        lpszAppID,
        dwTraceLevel,
        dwTimeOut,
        lphService,
        hWnd,
        dwSrvcVersionsRequired,
        lpSrvcVersion,
        lpSPIVersion,
        lpRequestID,
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
    call_async(
        WFS_REGISTER_COMPLETE,
        |hwnd, request_id| mgr::async_register(hService, dwEventClass, hWndReg, hwnd, request_id),
        &mut ptr::null_mut(),
    )
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFSAsyncRegister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    assert_started!();
    assert_unblocked!();
    mgr::async_register(hService, dwEventClass, hWndReg, hWnd, lpRequestID)
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
    mgr::start_up(dwVersionsRequired, lpWFSVersion)
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
    mgr::async_unlock(hService, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMGetTraceLevel(h_service: HSERVICE, lpdw_trace_level: LPDWORD) -> HRESULT {
    assert_started!();
    mgr::get_trace_level(h_service, lpdw_trace_level)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMReleaseDLL(hProvider: HPROVIDER) -> HRESULT {
    assert_started!();
    mgr::release_dll(hProvider)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMAllocateBuffer(ulSize: ULONG, ulFlags: ULONG, lppvData: *mut LPVOID) -> HRESULT {
    xfslib::supp::allocate_buffer(ulSize, ulFlags, lppvData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMAllocateMore(ulSize: ULONG, lpvOriginal: LPVOID, lppvData: *mut LPVOID) -> HRESULT {
    xfslib::supp::allocate_more(ulSize, lpvOriginal, lppvData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMFreeBuffer(lpvData: LPVOID) -> HRESULT {
    xfslib::supp::free_buffer(lpvData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMKillTimer(wTimerID: WORD) -> HRESULT {
    xfslib::supp::kill_timer(wTimerID)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMOutputTraceData(lpsz_data: LPSTR) -> HRESULT {
    xfslib::supp::output_trace_data(lpsz_data)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMSetTimer(hWnd: HWND, lpContext: LPVOID, dwTimeVal: DWORD, lpwTimerID: LPWORD) -> HRESULT {
    xfslib::supp::set_timer(hWnd, lpContext, dwTimeVal, lpwTimerID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMSetTraceLevel(hService: HSERVICE, dwTraceLevel: DWORD) -> HRESULT {
    assert_started!();
    mgr::set_trace_level(hService, dwTraceLevel)
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
    call_async(WFS_UNLOCK_COMPLETE, |hwnd, request_id| mgr::async_unlock(hService, hwnd, request_id), &mut ptr::null_mut())
}

/// Calls asynchronous function on the current thread.
fn call_async(message: u32, async_fn: impl Fn(HWND, LPREQUESTID) -> HRESULT, lpp_result: *mut LPWFSRESULT) -> HRESULT {
    let window = SyncWindow::new(message);
    let mut request_id = 0;
    let result = async_fn(window.handle(), &mut request_id);

    if result != WFS_SUCCESS {
        xfs_reject!(result);
    }

    loop {
        // Execute application hook or default hook dispatching window messages
        let hook = BLOCKING_HOOK.load(Ordering::SeqCst);
        let hook = if hook.is_null() { default_block_hook } else { unsafe { *hook } };
        unsafe {
            hook();
        }

        // The request was cancelled
        if !WFSIsBlocking() {
            // Drain the remaining messages
            while unsafe { hook() } {}
            // mgr::cancel_request(h_service, request_id);
            xfs_reject!(WFS_ERR_CANCELED);
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
unsafe extern "stdcall" fn default_block_hook() -> bool {
    let mut msg = mem::zeroed();
    if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) != 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
        return true;
    }
    false
}
