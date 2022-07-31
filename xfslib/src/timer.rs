use std::sync::{Mutex, MutexGuard};

use winapi::{
    shared::{
        basetsd::UINT_PTR,
        minwindef::{LPARAM, LPWORD, UINT},
        windef::HWND,
    },
    um::winuser::{KillTimer, PostMessageA, SetTimer},
};

use crate::*;

struct Timer {
    wnd: HWND,
    timer_id: UINT_PTR,
}

struct TimerContext {
    context: LPVOID,
    callback: Box<dyn FnOnce() + 'static>,
}

impl Timer {
    pub fn new(wnd: HWND, context: LPVOID, dw_time_val: DWORD, callback: impl FnOnce() + 'static) -> Result<Self, HRESULT> {
        if wnd.is_null() {
            xfs_reject_err!(WFS_ERR_INVALID_HWND);
        }

        if dw_time_val == 0 {
            xfs_reject_err!(WFS_ERR_INVALID_DATA);
        }

        let timer_ctx = TimerContext {
            context,
            callback: Box::new(callback),
        };
        let timer_ctx_ptr = Box::into_raw(Box::new(timer_ctx));
        let result = unsafe { SetTimer(wnd, timer_ctx_ptr as usize, dw_time_val, Some(timer_proc)) };

        if result == 0 {
            // SAFETY: the ptr was created literally just above
            drop(unsafe { Box::from_raw(timer_ctx_ptr) });
            xfs_reject_err!(WFS_ERR_INTERNAL_ERROR);
        }

        unsafe extern "system" fn timer_proc(wnd: HWND, _msg: UINT, id_event: UINT_PTR, _elapsed: DWORD) {
            let ctx = Box::from_raw(id_event as *mut TimerContext);
            PostMessageA(wnd, WFS_TIMER_EVENT, id_event, ctx.context as LPARAM);
            (ctx.callback)();
        }

        Ok(Timer { wnd, timer_id: result as UINT_PTR })
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        unsafe { KillTimer(self.wnd, self.timer_id) };
    }
}

unsafe impl Send for Timer {}

lazy_static::lazy_static! {
    static ref TIMERS: Mutex<Vec<Option<Timer>>> = Mutex::new((1..WORD::MAX).map(|_| None).collect());
}

fn get_timers<'a>() -> MutexGuard<'a, Vec<Option<Timer>>> {
    TIMERS.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn set_timer(wnd: HWND, context: LPVOID, dw_time_val: DWORD, lpw_timer_id: LPWORD) -> HRESULT {
    if wnd.is_null() {
        xfs_reject!(WFS_ERR_INVALID_HWND);
    }
    if lpw_timer_id.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    let mut timers = get_timers();
    let timer_id = match timers.iter().position(|t| t.is_none()) {
        Some(id) => id,
        None => return WFS_ERR_INTERNAL_ERROR,
    };
    let timer = xfs_unwrap!(Timer::new(wnd, context, dw_time_val, move || {
        get_timers()[timer_id] = None;
    }));

    // SAFETY: the pointer was checked to be valid before.
    unsafe { lpw_timer_id.write((timer_id as WORD) + 1) };
    timers[timer_id] = Some(timer);
    WFS_SUCCESS
}

pub fn kill_timer(w_timer_id: WORD) -> HRESULT {
    if w_timer_id == 0 {
        xfs_reject!(WFS_ERR_INVALID_TIMER);
    }

    let mut timers = get_timers();
    let timer_id = w_timer_id as usize - 1;

    if timers[timer_id].is_none() {
        xfs_reject!(WFS_ERR_INVALID_TIMER);
    }
    timers[timer_id] = None;
    WFS_SUCCESS
}

#[cfg(test)]
mod tests {
    use std::{ptr, time::Instant};

    use super::Timer;
    use crate::{
        timer::{kill_timer, set_timer},
        window, SyncWindow, WFS_ERR_INVALID_TIMER, WFS_SUCCESS, WFS_TIMER_EVENT,
    };

    #[test]
    fn test_timer() {
        let window = window::SyncWindow::new(WFS_TIMER_EVENT);
        let context = Box::into_raw(Box::new(94324234));
        let timer = Timer::new(window.handle(), context as *mut _, 1, || {});

        loop {
            match window.try_receive() {
                Ok(Some(message)) => {
                    let context = unsafe { Box::from_raw(message as *mut i32) };
                    assert_eq!(*context, 94324234);
                    break;
                }
                Ok(None) => continue,
                Err(e) => panic!("{:?}", e),
            }
        }

        drop(timer);
    }

    #[test]
    fn test_callback() {
        let window = window::SyncWindow::new(WFS_TIMER_EVENT);

        let (sender, receiver) = std::sync::mpsc::channel();
        let timer_start = Instant::now();
        let timer = Timer::new(window.handle(), ptr::null_mut(), 50, move || {
            assert_eq!(sender.send(1), Ok(()));
        });

        assert_eq!(receiver.recv(), Ok(1));

        let elapsed = timer_start.elapsed().as_millis();
        eprintln!("elapsed {elapsed}");

        // Windows timers are not very accurate, so we need to allow for a bit of slack
        assert!(elapsed > 40);
        drop(timer);
    }

    #[test]
    fn test_kill() {
        let window = window::SyncWindow::new(WFS_TIMER_EVENT);
        let context = Box::into_raw(Box::new(94324234));
        let timer_start = Instant::now();
        let timer = Timer::new(window.handle(), context as *mut _, 50, move || {
            panic!("should not be called");
        });
        drop(timer);
        loop {
            if timer_start.elapsed().as_millis() > 100 {
                break;
            }
            match window.try_receive() {
                Ok(Some(_)) => panic!("should not be called"),
                Ok(None) => continue,
                Err(e) => panic!("{:?}", e),
            }
        }
    }

    #[test]
    fn test_timer_api() {
        let window = SyncWindow::new(WFS_TIMER_EVENT);
        let context = Box::into_raw(Box::new(94324234));
        let mut timer_id = 0;
        assert_eq!(set_timer(window.handle(), context as *mut _, 100, &mut timer_id), WFS_SUCCESS);
        assert_ne!(timer_id, 0);

        assert_eq!(kill_timer(timer_id), WFS_SUCCESS);
        assert_eq!(kill_timer(timer_id), WFS_ERR_INVALID_TIMER);
    }
}
