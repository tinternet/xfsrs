use std::ffi::CStr;
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::Arc;
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
        minwindef::{DWORD, HINSTANCE, LPVOID, LPWORD, ULONG, WORD},
        windef::HWND,
        winerror::HRESULT,
    },
    um::winnt::{DLL_PROCESS_ATTACH, LPSTR},
};
use xfslib::*;

lazy_static! {
    // holds application & service providers buffers
    static ref HEAP: Mutex<Heap> = Mutex::new(Heap::new());
    // holds application timers
    static ref TIMERS: Vec<AtomicPtr<Timer>> = (0..65535).map(|_| AtomicPtr::new(ptr::null_mut())).collect();
}

const MAX_HEAP_SIZE: usize = 1 * 1000 * 1000 * 1000; // 1 GB

struct Timer {
    hwnd: HWND,
    context: LPVOID,
}

struct Heap {
    allocations: HashMap<usize, Allocation>,
    total_bytes: Arc<AtomicUsize>,
}

struct Allocation {
    buffer: Vec<u8>,
    flags: ULONG,
    child: Vec<Allocation>,
    heap: Arc<AtomicUsize>,
}

impl Allocation {
    fn new(buffer: Vec<u8>, flags: ULONG, heap: Arc<AtomicUsize>) -> Self {
        let child = Vec::with_capacity(0);
        Self { buffer, flags, child, heap }
    }
}

impl Drop for Allocation {
    fn drop(&mut self) {
        self.heap.fetch_sub(self.buffer.len(), Ordering::SeqCst);
    }
}

unsafe impl Send for Heap {}

impl Heap {
    fn new() -> Self {
        let allocations = HashMap::new();
        let total_bytes = Arc::new(AtomicUsize::new(0));
        Heap { allocations, total_bytes }
    }

    fn try_allocate(&mut self, size: usize, flags: ULONG) -> Result<Allocation, HRESULT> {
        let new_size = self.total_bytes.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            value.checked_add(size).and_then(|new| if new > MAX_HEAP_SIZE { None } else { Some(new) })
        });
        if new_size.is_err() {
            return Err(WFS_ERR_OUT_OF_MEMORY);
        }
        let buffer = vec![0; size];
        let allocation = Allocation::new(buffer, flags, self.total_bytes.clone());
        Ok(allocation)
    }

    fn allocate_buffer(&mut self, size: usize, flags: ULONG) -> Result<LPVOID, HRESULT> {
        let mut allocation = self.try_allocate(size, flags)?;
        let pointer = allocation.buffer.as_mut_ptr() as LPVOID;
        self.allocations.insert(pointer as usize, allocation);
        Ok(pointer)
    }

    fn allocate_more(&mut self, size: usize, parent_buffer: LPVOID) -> Result<LPVOID, HRESULT> {
        let flags = match self.allocations.get(&(parent_buffer as usize)) {
            Some(allocation) => allocation.flags,
            None => return Err(WFS_ERR_INVALID_BUFFER),
        };
        let mut allocation = self.try_allocate(size, flags)?;
        let pointer = allocation.buffer.as_mut_ptr() as LPVOID;
        self.allocations.get_mut(&(parent_buffer as usize)).unwrap().child.push(allocation);
        Ok(pointer)
    }

    fn deallocate(&mut self, buffer: LPVOID) -> Result<(), HRESULT> {
        if self.allocations.remove(&(buffer as usize)).is_none() {
            return Err(WFS_ERR_INVALID_BUFFER);
        }
        Ok(())
    }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMAllocateBuffer(ulSize: ULONG, ulFlags: ULONG, lppvData: *mut LPVOID) -> HRESULT {
    if lppvData.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }
    let mut heap = xfs_unwrap!(HEAP.lock());
    let buffer = match heap.allocate_buffer(ulSize as usize, ulFlags) {
        Ok(buffer) => buffer,
        Err(error) => return error,
    };
    unsafe { lppvData.write(buffer) };
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
    let mut heap = xfs_unwrap!(HEAP.lock());
    let buffer = match heap.allocate_more(ulSize as usize, lpvOriginal) {
        Ok(buffer) => buffer,
        Err(error) => return error,
    };
    unsafe { lppvData.write(buffer) };
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
    let mut heap = xfs_unwrap!(HEAP.lock());

    match heap.deallocate(lpvData) {
        Ok(_) => WFS_SUCCESS,
        Err(error) => error,
    }
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
        for _ in 0..100000 {
            let mut parent = ptr::null_mut();
            let mut child = ptr::null_mut();
            assert_eq!(WFMAllocateBuffer(10, WFS_MEM_ZEROINIT, &mut parent), WFS_SUCCESS);
            assert_eq!(WFMAllocateMore(MAX_HEAP_SIZE as u32 + 1, parent, &mut child), WFS_ERR_OUT_OF_MEMORY);
            assert_eq!(WFMAllocateMore(10, parent, &mut child), WFS_SUCCESS);
            assert_ne!(parent, ptr::null_mut());
            assert_ne!(child, ptr::null_mut());
            assert_eq!(WFMFreeBuffer(child), WFS_ERR_INVALID_BUFFER);
            assert_eq!(WFMFreeBuffer(parent), WFS_SUCCESS);
            assert_eq!(WFMFreeBuffer(parent), WFS_ERR_INVALID_BUFFER);
        }
        assert_eq!(HEAP.lock().unwrap().allocations.len(), 0);
        assert_eq!(HEAP.lock().unwrap().total_bytes.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_allocate_fail() {
        assert_eq!(WFMAllocateBuffer(20, WFS_MEM_ZEROINIT, ptr::null_mut()), WFS_ERR_INVALID_POINTER);
    }

    #[test]
    fn test_allocate_fail_oom() {
        let mut buffer = ptr::null_mut();
        assert_eq!(WFMAllocateBuffer(MAX_HEAP_SIZE as u32 + 1, WFS_MEM_ZEROINIT, &mut buffer), WFS_ERR_OUT_OF_MEMORY);
    }

    #[test]
    fn test_allocate_more_fail() {
        assert_eq!(WFMAllocateMore(10, ptr::null_mut(), ptr::null_mut()), WFS_ERR_INVALID_POINTER);
        assert_eq!(WFMAllocateMore(10, 1 as *mut _, &mut ptr::null_mut()), WFS_ERR_INVALID_BUFFER);
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
