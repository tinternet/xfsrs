use std::ffi::CStr;
use std::{collections::HashMap, sync::Mutex};

use lazy_static::lazy_static;
use log::{error, trace, LevelFilter};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;
use log_derive::{logfn, logfn_inputs};
use winapi::shared::minwindef::BYTE;
use winapi::{
    shared::{
        basetsd::{UINT_PTR, ULONG_PTR},
        minwindef::{DWORD, HINSTANCE, LPARAM, LPVOID, LPWORD, UINT, ULONG, WORD},
        windef::HWND,
        winerror::HRESULT,
    },
    um::{
        winnt::{DLL_PROCESS_ATTACH, LPSTR},
        winuser::{KillTimer, PostMessageA, SetTimer},
    },
};

use xfslib::*;

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

lazy_static! {
    // holds application & service providers buffers
    static ref BUFFERS: Mutex<HashMap<ULONG_PTR, Buffer>> = Mutex::new(HashMap::new());

    // holds application timers
    static ref TIMERS: Mutex<Vec<Option<Timer>>> = Mutex::new((0..65535).map(|_| None).collect());
}

#[allow(dead_code)]
struct Buffer {
    buffer: Vec<BYTE>,
    children: Vec<Vec<BYTE>>,
}

struct Timer {
    hwnd: ULONG_PTR,
    lpcontext: ULONG_PTR,
    timer_id: usize,
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMAllocateBuffer(ulSize: ULONG, ulFlags: ULONG, lppvData: *mut LPVOID) -> HRESULT {
    if lppvData.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    let mut buffers = xfs_unwrap!(BUFFERS.lock());
    let buffer: Vec<BYTE> = vec![0; ulSize as usize];

    unsafe {
        lppvData.write(buffer.as_ptr() as LPVOID);
    }

    buffers.insert(buffer.as_ptr() as ULONG_PTR, Buffer { buffer, children: vec![] });
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMAllocateMore(ulSize: ULONG, lpvOriginal: LPVOID, lppvData: *mut LPVOID) -> HRESULT {
    if lppvData.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    let mut buffers = xfs_unwrap!(BUFFERS.lock());
    let original_buffer = match buffers.get_mut(&(lpvOriginal as ULONG_PTR)) {
        Some(list) => list,
        None => xfs_reject!(WFS_ERR_INVALID_BUFFER),
    };

    let buffer: Vec<BYTE> = vec![0; ulSize as usize];

    unsafe {
        lppvData.write(buffer.as_ptr() as LPVOID);
    }

    original_buffer.children.push(buffer);
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMFreeBuffer(lpvData: LPVOID) -> HRESULT {
    if lpvData.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    let mut buffers = xfs_unwrap!(BUFFERS.lock());

    if buffers.remove(&(lpvData as ULONG_PTR)).is_none() {
        xfs_reject!(WFS_ERR_INVALID_BUFFER);
    }
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMKillTimer(wTimerID: WORD) -> HRESULT {
    let mut timers = xfs_unwrap!(TIMERS.lock());

    let timer = match timers.get(wTimerID as usize - 1) {
        Some(Some(timer)) => timer,
        _ => xfs_reject!(WFS_ERR_INVALID_TIMER),
    };

    unsafe {
        if KillTimer(timer.hwnd as HWND, timer.timer_id) == 0 {
            xfs_reject!(WFS_ERR_INTERNAL_ERROR);
        }
    }

    timers.insert(wTimerID as usize - 1, None);
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMOutputTraceData(lpszData: LPSTR) -> HRESULT {
    trace!("XFS TRACE --- {}", xfs_unwrap!(unsafe { CStr::from_ptr(lpszData).to_str() }));
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMSetTimer(hWnd: HWND, lpContext: LPVOID, dwTimeVal: DWORD, lpwTimerID: LPWORD) -> HRESULT {
    if hWnd.is_null() {
        xfs_reject!(WFS_ERR_INVALID_HWND);
    }
    if lpwTimerID.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }
    if dwTimeVal == 0 {
        xfs_reject!(WFS_ERR_INVALID_DATA);
    }

    let mut timers = xfs_unwrap!(TIMERS.lock());

    let free = match timers.iter().position(|t| t.is_none()) {
        Some(index) => index,
        None => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    };

    let id_event = (&*timers) as *const _ as usize + free;

    if unsafe { SetTimer(hWnd, id_event, dwTimeVal, Some(timer_proc)) } == 0 {
        xfs_reject!(WFS_ERR_INTERNAL_ERROR);
    }

    let timer = Timer {
        hwnd: hWnd as ULONG_PTR,
        lpcontext: lpContext as ULONG_PTR,
        timer_id: id_event,
    };
    timers[free] = Some(timer);

    unsafe {
        lpwTimerID.write((free + 1) as u16);
    }

    unsafe extern "system" fn timer_proc(_: HWND, _: UINT, id_event: UINT_PTR, _: DWORD) {
        let timers = TIMERS.lock().unwrap(); // TODO: don't unwrap this shit, think of a better way to handle it?
        let timer = &timers[id_event as usize - (&*timers as *const _ as usize)].as_ref().unwrap();
        trace!("timer_proc: {}", timer.timer_id);
        PostMessageA(timer.hwnd as HWND, WFS_TIMER_EVENT, id_event, timer.lpcontext as LPARAM);
    }

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMSetTraceLevel(_hService: HSERVICE, _dwTraceLevel: DWORD) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn CleanUp() -> HRESULT {
    {
        let mut timers = xfs_unwrap!(TIMERS.lock());
        for timer in timers.iter_mut() {
            if let Some(timer) = timer {
                unsafe {
                    KillTimer(std::ptr::null_mut(), timer.timer_id);
                }
            }
        }
    }

    let mut buffers = xfs_unwrap!(BUFFERS.lock());
    buffers.clear();

    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn DllMain(_hinst_dll: HINSTANCE, fdw_reason: DWORD, _: LPVOID) -> bool {
    if fdw_reason == DLL_PROCESS_ATTACH {
        let logfile = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} {l} {L} - {m}\n")))
            .build("C:\\XFS_SUPP.log")
            .unwrap();
        let config = Config::builder()
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
            .build(Root::builder().appender("logfile").build(LevelFilter::Trace))
            .unwrap();

        log4rs::init_config(config).unwrap();
        trace!("XFS SUPP DLL INIT");
    }
    true
}
