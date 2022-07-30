use std::ffi::CStr;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::{collections::HashMap, sync::Mutex};

use lazy_static::lazy_static;
use log::{error, trace, LevelFilter};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;
use log_derive::{logfn, logfn_inputs};
use winapi::shared::basetsd::UINT_PTR;
use winapi::shared::minwindef::UINT;
use winapi::um::winuser::{KillTimer, PostMessageA, SetTimer};
use winapi::{
    shared::{
        basetsd::ULONG_PTR,
        minwindef::{DWORD, HINSTANCE, LPVOID, LPWORD, ULONG, WORD},
        windef::HWND,
        winerror::HRESULT,
    },
    um::winnt::{DLL_PROCESS_ATTACH, LPSTR},
};
use xfslib::*;

lazy_static! {
    // holds application & service providers buffers
    static ref BUFFERS: Mutex<HashMap<ULONG_PTR, Allocation>> = Mutex::new(HashMap::new());

    // holds application timers
    static ref TIMERS: Vec<AtomicPtr<Timer>> = (0..65535).map(|_| AtomicPtr::new(ptr::null_mut())).collect();
}

struct Allocation {
    _buffer: Vec<u8>,
    extended: Vec<Vec<u8>>,
    flags: ULONG,
}

impl Allocation {
    fn new(buffer: Vec<u8>, flags: ULONG) -> Self {
        let extended = vec![];
        Self { _buffer: buffer, extended, flags }
    }
}

struct Timer {
    hwnd: HWND,
    context: LPVOID,
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMAllocateBuffer(ulSize: ULONG, ulFlags: ULONG, lppvData: *mut LPVOID) -> HRESULT {
    if lppvData.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    let mut buffer: Vec<u8> = match ulFlags & WFS_MEM_ZEROINIT {
        0 => vec![0; ulSize as usize],
        _ => vec![0; ulSize as usize],
    };

    let ptr: *mut u8 = buffer.as_mut_ptr();
    // SAFETY: we know that lppvData is not null
    unsafe { lppvData.write(ptr as *mut _) };

    let allocation = Allocation::new(buffer, ulFlags);
    xfs_unwrap!(BUFFERS.lock()).insert(ptr as ULONG_PTR, allocation);
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMAllocateMore(ulSize: ULONG, lpvOriginal: LPVOID, lppvData: *mut LPVOID) -> HRESULT {
    if lppvData.is_null() || lpvOriginal.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    let mut buffers = xfs_unwrap!(BUFFERS.lock());
    let allocation = match buffers.get_mut(&(lpvOriginal as ULONG_PTR)) {
        Some(allocation) => allocation,
        None => xfs_reject!(WFS_ERR_INVALID_BUFFER),
    };
    let mut buffer: Vec<u8> = match allocation.flags & WFS_MEM_ZEROINIT {
        0 => vec![0; ulSize as usize],
        _ => vec![0; ulSize as usize],
    };

    // SAFETY: we know that lppvData is not null
    unsafe { lppvData.write(buffer.as_mut_ptr() as *mut _) };

    allocation.extended.push(buffer);
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
    if xfs_unwrap!(BUFFERS.lock()).remove(&(lpvData as ULONG_PTR)).is_none() {
        xfs_reject!(WFS_ERR_INVALID_BUFFER);
    }
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMKillTimer(wTimerID: WORD) -> HRESULT {
    if wTimerID == 0 {
        xfs_reject!(WFS_ERR_INVALID_TIMER);
    }

    let timer = TIMERS[wTimerID as usize - 1].swap(ptr::null_mut(), Ordering::SeqCst);

    // Verify that the timer was not destroyed
    if timer.is_null() {
        xfs_reject!(WFS_ERR_INVALID_TIMER);
    }

    // SAFETY: we checked that timer is not null and we know it's not dropped yet since we are using atomic swap
    let timer = unsafe { Box::from_raw(timer) };

    // SAFETY: all parameters are valid
    unsafe {
        KillTimer(timer.hwnd, wTimerID as usize);
    }

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

    let timer = Timer { hwnd: hWnd, context: lpContext };
    let timer_ptr = Box::into_raw(Box::new(timer));

    let timer_id = match TIMERS.iter().position(|p| p.compare_exchange(ptr::null_mut(), timer_ptr, Ordering::SeqCst, Ordering::SeqCst).is_ok()) {
        Some(index) => index + 1,
        None => {
            // SAFETY: the timer was allocated and not dropped yet
            let _ = unsafe { Box::from_raw(timer_ptr) };
            xfs_reject!(WFS_ERR_INTERNAL_ERROR)
        }
    };

    unsafe {
        if SetTimer(hWnd, timer_id, dwTimeVal, Some(timer_proc)) == 0 {
            TIMERS[timer_id as usize - 1].store(ptr::null_mut(), Ordering::SeqCst);
            let _ = Box::from_raw(timer_ptr);
            xfs_reject!(WFS_ERR_INTERNAL_ERROR);
        }
        *lpwTimerID = timer_id as u16;
    }

    unsafe extern "system" fn timer_proc(hwnd: HWND, _msg: UINT, id_event: UINT_PTR, _elapsed: DWORD) {
        let ptr = TIMERS[id_event as usize - 1].swap(ptr::null_mut(), Ordering::SeqCst);

        if !ptr.is_null() {
            let timer = Box::from_raw(ptr);
            PostMessageA(hwnd, WFS_TIMER_EVENT, id_event, timer.context as _);
        }
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
pub extern "stdcall" fn DllMain(_hinst_dll: HINSTANCE, fdw_reason: DWORD, _: LPVOID) -> bool {
    if fdw_reason == DLL_PROCESS_ATTACH {
        let logfile = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} {l} {L} - {m}\n")))
            .build("$ENV{Public}\\XFS_SUPP.log")
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

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn test_allocate() {
        let mut buffer = ptr::null_mut();
        let result = WFMAllocateBuffer(10, WFS_MEM_ZEROINIT, &mut buffer);
        assert_eq!(result, WFS_SUCCESS);
        assert_ne!(buffer, ptr::null_mut());
    }

    #[test]
    fn test_allocate_fail() {
        let result = WFMAllocateBuffer(0, WFS_MEM_ZEROINIT, ptr::null_mut());
        assert_eq!(result, WFS_ERR_INVALID_POINTER);
    }

    #[test]
    fn test_allocate_more() {
        let mut buffer = ptr::null_mut();
        let result = WFMAllocateBuffer(10, WFS_MEM_ZEROINIT, &mut buffer);
        assert_eq!(result, WFS_SUCCESS);
        assert_ne!(buffer, ptr::null_mut());

        let mut buffer2 = ptr::null_mut();
        let result2 = WFMAllocateMore(10, buffer, &mut buffer2);
        assert_eq!(result2, WFS_SUCCESS);
        assert_ne!(buffer2, ptr::null_mut());
    }

    #[test]
    fn test_allocate_more_fail() {
        let result = WFMAllocateMore(10, ptr::null_mut(), ptr::null_mut());
        assert_eq!(result, WFS_ERR_INVALID_POINTER);

        let mut buffer = ptr::null_mut();
        let result = WFMAllocateBuffer(10, WFS_MEM_SHARE, &mut buffer);
        assert_eq!(result, WFS_SUCCESS);

        let result = WFMAllocateMore(10, buffer, ptr::null_mut());
        assert_eq!(result, WFS_ERR_INVALID_POINTER);
    }

    #[test]
    fn test_free() {
        let mut buffer = ptr::null_mut();
        let result = WFMAllocateBuffer(10, WFS_MEM_ZEROINIT, &mut buffer);
        assert_eq!(result, WFS_SUCCESS);
        assert_ne!(buffer, ptr::null_mut());

        let result = WFMFreeBuffer(buffer);
        assert_eq!(result, WFS_SUCCESS);

        let result = WFMFreeBuffer(buffer);
        assert_eq!(result, WFS_ERR_INVALID_BUFFER);
    }

    #[test]
    fn test_free_parent_first() {
        let mut buffer = ptr::null_mut();
        let result = WFMAllocateBuffer(10, WFS_MEM_ZEROINIT, &mut buffer);
        assert_eq!(result, WFS_SUCCESS);
        assert_ne!(buffer, ptr::null_mut());

        let mut buffer2 = ptr::null_mut();
        let result2 = WFMAllocateMore(10, buffer, &mut buffer2);
        assert_eq!(result2, WFS_SUCCESS);
        assert_ne!(buffer2, ptr::null_mut());

        let result = WFMFreeBuffer(buffer);
        assert_eq!(result, WFS_SUCCESS);

        let result = WFMFreeBuffer(buffer);
        assert_eq!(result, WFS_ERR_INVALID_BUFFER);

        let result = WFMFreeBuffer(buffer2);
        assert_eq!(result, WFS_ERR_INVALID_BUFFER);
    }

    #[test]
    fn test_free_child_first() {
        let mut parent = ptr::null_mut();
        let result = WFMAllocateBuffer(10, WFS_MEM_ZEROINIT, &mut parent);
        assert_eq!(result, WFS_SUCCESS);
        assert_ne!(parent, ptr::null_mut());

        let mut child = ptr::null_mut();
        let result = WFMAllocateMore(10, parent, &mut child);
        assert_eq!(result, WFS_SUCCESS);
        assert_ne!(child, ptr::null_mut());

        let result = WFMFreeBuffer(child);
        assert_eq!(result, WFS_ERR_INVALID_BUFFER);

        let result = WFMFreeBuffer(parent);
        assert_eq!(result, WFS_SUCCESS);

        let result = WFMFreeBuffer(parent);
        assert_eq!(result, WFS_ERR_INVALID_BUFFER);
    }

    #[test]
    fn test_timer() {
        let window = SyncWindow::new(WFS_TIMER_EVENT);
        let mut value = 100;
        let mut timer_id = 0;
        let result = WFMSetTimer(window.handle(), &mut value as *mut _ as *mut _, 100, &mut timer_id);
        assert_eq!(result, WFS_SUCCESS);
        assert_ne!(timer_id, 0);

        let result = WFMKillTimer(timer_id);
        assert_eq!(result, WFS_SUCCESS);

        let result = WFMKillTimer(timer_id);
        assert_eq!(result, WFS_ERR_INVALID_TIMER);
    }

    #[test]
    fn test_timer_tick() {
        let window = SyncWindow::new(WFS_TIMER_EVENT);
        let mut value = 100;
        let mut timer_id = 0;
        let result = WFMSetTimer(window.handle(), &mut value as *mut _ as *mut _, 1, &mut timer_id);
        assert_eq!(result, WFS_SUCCESS);
        assert_ne!(timer_id, 0);

        let start = Instant::now();
        loop {
            if let Some(response) = window.try_receive().unwrap() {
                let response = unsafe { &*(response as *mut i32) };
                assert_eq!(response, &value);
                break;
            }
            if start.elapsed().as_secs() > 1 {
                panic!("1 ms timer did not finish for more than 1 second");
            }
        }

        let result = WFMKillTimer(timer_id);
        assert_eq!(result, WFS_ERR_INVALID_TIMER, "Timer must be automatically deallocated");
    }
}
