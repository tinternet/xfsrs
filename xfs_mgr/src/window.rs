use std::{
    ffi::CString,
    ptr,
    sync::mpsc::{Receiver, Sender},
    thread,
};

use winapi::{
    ctypes::c_void,
    shared::{
        minwindef::{LPARAM, LRESULT, WPARAM},
        windef::{HWND, POINT},
    },
    um::{
        libloaderapi::GetModuleHandleW,
        winuser::{
            CreateWindowExA, DefWindowProcA, DestroyWindow, DispatchMessageA, GetMessageA, GetWindowLongPtrA, PostMessageA, PostQuitMessage, RegisterClassExA, SetWindowLongPtrA, CREATESTRUCTW,
            GWLP_USERDATA, HWND_MESSAGE, MSG, WM_CLOSE, WM_CREATE, WM_DESTROY, WM_GETMINMAXINFO, WM_NCCALCSIZE, WM_NCCREATE, WM_NCDESTROY, WNDCLASSEXA,
        },
    },
};

struct Message {
    message: u32,
    // w_param: u32,
    l_param: u32,
}

pub struct SyncWindow {
    hwnd: HWND,
    receiver: Receiver<u32>,
}

struct HwndResult {
    hwnd: HWND,
}

unsafe impl Send for HwndResult {}

impl SyncWindow {
    pub fn new(message: u32) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel::<Message>();
        let (sender_res, receiver_res) = std::sync::mpsc::channel();
        let (sender_hwnd, receiver_hwnd) = std::sync::mpsc::channel();

        thread::spawn(move || unsafe {
            let instance = GetModuleHandleW(ptr::null());
            let class_name = CString::new("XFS_MSG_WINDOW").unwrap();
            let wx = WNDCLASSEXA {
                cbSize: std::mem::size_of::<WNDCLASSEXA>() as u32,
                style: 0,
                lpfnWndProc: Some(wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: ptr::null_mut(),
                hCursor: ptr::null_mut(),
                hbrBackground: ptr::null_mut(),
                lpszMenuName: ptr::null_mut(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: ptr::null_mut(),
            };

            RegisterClassExA(&wx);

            let lparam: *mut Sender<Message> = Box::leak(Box::new(sender));
            let hwnd = CreateWindowExA(0, class_name.as_ptr(), ptr::null(), 0, 0, 0, 0, 0, HWND_MESSAGE, std::ptr::null_mut(), instance, lparam as *mut c_void);

            sender_hwnd.send(HwndResult { hwnd }).unwrap();

            let mut message = MSG {
                hwnd: std::ptr::null_mut(),
                message: 0,
                wParam: 0,
                lParam: 0,
                time: 0,
                pt: POINT { x: 0, y: 0 },
            };

            while GetMessageA(&mut message, std::ptr::null_mut(), 0, 0) != 0 {
                DispatchMessageA(&message);
            }
        });

        thread::spawn(move || loop {
            let received = match receiver.recv() {
                Ok(message) => message,
                Err(_) => break,
            };
            if received.message == message {
                sender_res.send(received.l_param).unwrap();
            }
        });

        let hwnd = receiver_hwnd.recv().unwrap();

        Self {
            hwnd: hwnd.hwnd,
            receiver: receiver_res,
        }
    }

    pub fn try_receive(&self) -> Result<Option<u32>, Box<dyn std::error::Error>> {
        match self.receiver.try_recv() {
            Ok(message) => Ok(Some(message)),
            Err(std::sync::mpsc::TryRecvError::Empty) => Ok(None),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Disconnected",
            ))),
        }
    }

    pub fn handle(&self) -> HWND {
        self.hwnd
    }
}

impl Drop for SyncWindow {
    fn drop(&mut self) {
        unsafe {
            PostMessageA(self.hwnd, WM_CLOSE, 0, 0);
        }
    }
}

extern "system" fn wndproc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as u32 {
            WM_GETMINMAXINFO => DefWindowProcA(window, message, wparam, lparam),
            WM_NCCREATE => {
                let createstruct: *mut CREATESTRUCTW = lparam as *mut _;
                if createstruct.is_null() {
                    return 0;
                }
                let sender_ptr = (*createstruct).lpCreateParams;
                SetWindowLongPtrA(window, GWLP_USERDATA, sender_ptr as i32);
                return 1;
            }
            WM_NCDESTROY => DefWindowProcA(window, message, wparam, lparam),
            WM_NCCALCSIZE => DefWindowProcA(window, message, wparam, lparam),
            WM_CREATE => DefWindowProcA(window, message, wparam, lparam),
            WM_DESTROY => {
                let ptr = GetWindowLongPtrA(window, GWLP_USERDATA) as *mut Sender<Message>;
                drop(Box::from_raw(ptr));
                PostQuitMessage(0);
                0
            }
            WM_CLOSE => {
                DestroyWindow(window);
                0
            }
            _ => {
                let ptr = GetWindowLongPtrA(window, GWLP_USERDATA) as *mut Sender<Message>;
                let sender = &*ptr;
                sender
                    .send(Message {
                        message,
                        // w_param: wparam as u32,
                        l_param: lparam as u32,
                    })
                    .unwrap();
                1
            }
        }
    }
}
