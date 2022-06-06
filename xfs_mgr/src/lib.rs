use std::sync::Mutex;

use lazy_static::lazy_static;
use libloading::Symbol;
use log::{trace, LevelFilter};
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    Config,
};
use winapi::{
    shared::{
        basetsd::UINT_PTR,
        minwindef::{DWORD, HINSTANCE, LPARAM, LPVOID, LPWORD, UINT, ULONG, WORD},
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

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn DllMain(_hinst_dll: HINSTANCE, fdw_reason: DWORD, _: LPVOID) -> bool {
    if fdw_reason == DLL_PROCESS_ATTACH {
        let logfile = FileAppender::builder().build("output.log").unwrap();
        let config = Config::builder()
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
            .build(Root::builder().appender("logfile").build(LevelFilter::Trace))
            .unwrap();

        log4rs::init_config(config).unwrap();
    }
    true
}

#[derive(Default, Clone, Copy)]
pub struct SPData {
    _request_id: u32,
    _library: u32,
}

#[allow(non_snake_case)]
#[derive(Clone, Copy)]
struct Timer {
    lpContext: LPVOID,
    hWnd: HWND,
}

unsafe impl Send for Timer {}

pub struct AppData {
    _services: [SPData; 8192],
    handles: [bool; 8192],
    started: bool,
    timers: [Option<Timer>; 65535],
}

lazy_static! {
    static ref APP_DATA: Mutex<AppData> = Mutex::new(AppData {
        _services: [SPData::default(); 8192],
        handles: [false; 8192],
        started: false,
        timers: [None; 65535],
    });
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSCancelAsyncRequest(hService: HSERVICE, RequestID: REQUESTID) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSCancelBlockingCall(dwThreadID: DWORD) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSCleanUp() -> HRESULT {
    let mut app_data = APP_DATA.lock().unwrap();

    *app_data = AppData {
        _services: [SPData::default(); 8192],
        handles: [false; 8192],
        started: false,
        timers: [None; 65535],
    };

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSClose(hService: HSERVICE) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncClose(hService: HSERVICE, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSCreateAppHandle(lphApp: LPHAPP) -> HRESULT {
    let mut app_data = APP_DATA.lock().unwrap();

    if !app_data.started {
        return WFS_ERR_NOT_STARTED;
    }

    let index = app_data.handles.iter().position(|h| !h);

    if index.is_none() {
        return WFS_ERR_INTERNAL_ERROR;
    }

    let index = index.unwrap() + 1;
    app_data.handles[index - 1] = true;

    unsafe {
        *lphApp = index as HAPP;
    }

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSDeregister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncDeregister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSDestroyAppHandle(hApp: HAPP) -> HRESULT {
    if hApp.is_null() {
        return WFS_ERR_INVALID_APP_HANDLE;
    }

    let mut app_data = APP_DATA.lock().unwrap();

    if !app_data.started {
        return WFS_ERR_NOT_STARTED;
    }

    let index = hApp as usize - 1;
    app_data.handles[index] = false;

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSExecute(hService: HSERVICE, dwCommand: DWORD, lpCmdData: LPVOID, dwTimeOut: DWORD, lppResult: LPWFSRESULT) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncExecute(hService: HSERVICE, dwCommand: DWORD, lpCmdData: LPVOID, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSFreeResult(lpResult: LPWFSRESULT) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSGetInfo(hService: HSERVICE, dwCategory: DWORD, lpQueryDetails: LPVOID, dwTimeOut: DWORD, lppResult: LPWFSRESULT) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncGetInfo(hService: HSERVICE, dwCategory: DWORD, lpQueryDetails: LPVOID, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSIsBlocking() -> bool {
    false
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSLock(hService: HSERVICE, dwTimeOut: DWORD, lppResult: LPWFSRESULT) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncLock(hService: HSERVICE, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    WFS_SUCCESS
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
    if lpszLogicalName.is_null() || lpszAppID.is_null() || lphService.is_null() || lpSrvcVersion.is_null() || lpSPIVersion.is_null() {
        return WFS_ERR_INVALID_POINTER;
    }

    let app_data = APP_DATA.lock().unwrap();
    if !app_data.started {
        return WFS_ERR_NOT_STARTED;
    }

    WFS_SUCCESS
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
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSRegister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncRegister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSSetBlockingHook(lpBlockFunc: XFSBLOCKINGHOOK, lppPrevFunc: LPXFSBLOCKINGHOOK) -> HRESULT {
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

    let mut app_data = APP_DATA.lock().unwrap();

    if app_data.started {
        return WFS_ERR_ALREADY_STARTED;
    }

    app_data.started = true;

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSUnhookBlockingHook() -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSUnlock(hService: HSERVICE) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFSAsyncUnlock(hService: HSERVICE, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    WFS_SUCCESS
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
pub extern "stdcall" fn WFMGetTraceLevel() -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMKillTimer(wTimerID: WORD) -> HRESULT {
    let mut app_data = APP_DATA.lock().unwrap();

    if app_data.timers[wTimerID as usize].is_none() {
        return WFS_ERR_INVALID_TIMER;
    }

    unsafe {
        if KillTimer(std::ptr::null::<HWND>() as HWND, wTimerID as UINT_PTR) == 0 {
            return WFS_ERR_INTERNAL_ERROR;
        }
    }

    // TODO: Free pointer?
    app_data.timers[wTimerID as usize] = None;

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

    let mut app_data = APP_DATA.lock().unwrap();
    let timer_id = app_data.timers.iter().position(|t| t.is_none());

    if timer_id.is_none() {
        return WFS_ERR_INTERNAL_ERROR;
    }

    app_data.timers[timer_id.unwrap()] = Some(Timer { hWnd, lpContext });

    unsafe {
        if SetTimer(std::ptr::null::<HWND>() as HWND, timer_id.unwrap() as UINT_PTR, dwTimeVal, Some(timer_proc)) == 0 {
            return WFS_ERR_INTERNAL_ERROR;
        }
        *lpwTimerID = timer_id.unwrap() as u16;
    }

    WFS_SUCCESS
}

unsafe extern "system" fn timer_proc(_: HWND, _: UINT, id_event: UINT_PTR, _: DWORD) {
    let app_data = APP_DATA.lock().unwrap();
    let timer = app_data.timers[id_event].unwrap();
    PostMessageA(timer.hWnd, WFS_TIMER_EVENT, id_event, timer.lpContext as LPARAM);
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn WFMSetTraceLevel(_hService: HSERVICE, _dwTraceLevel: DWORD) -> HRESULT {
    WFS_SUCCESS
}
