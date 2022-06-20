// use std::{collections::HashMap, ffi::c_void, sync::Mutex};

// use lazy_static::lazy_static;
// use libloading::Symbol;
// use log::{error, trace, LevelFilter};
// use log4rs::{
//     append::file::FileAppender,
//     config::{Appender, Root},
//     encode::pattern::PatternEncoder,
//     Config,
// };
// use log_derive::{logfn, logfn_inputs};
// use winapi::{
//     shared::{
//         basetsd::{UINT_PTR, ULONG_PTR},
//         minwindef::{DWORD, HINSTANCE, LPARAM, LPVOID, LPWORD, UINT, ULONG, WORD},
//         ntdef::LPSTR,
//         windef::HWND,
//         winerror::HRESULT,
//     },
//     um::{
//         heapapi::{GetProcessHeap, HeapAlloc, HeapFree},
//         winnt::{DLL_PROCESS_ATTACH, HEAP_ZERO_MEMORY},
//         winuser::{KillTimer, PostMessageA, SetTimer},
//     },
// };
// use xfslib::*;

// lazy_static! {
//     static ref XFS_LIB: libloading::Library = unsafe { libloading::Library::new("xfs_supp_orig.dll").unwrap() };
//     pub static ref WFM_ALLOCATE_BUFFER: Symbol<'static, unsafe extern "stdcall" fn(ULONG, ULONG, *mut LPVOID) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMAllocateBuffer").unwrap() };
//     pub static ref WFM_ALLOCATE_MORE: Symbol<'static, unsafe extern "stdcall" fn(ULONG, LPVOID, *mut LPVOID) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMAllocateMore").unwrap() };
//     pub static ref WFM_FREE_BUFFER: Symbol<'static, unsafe extern "stdcall" fn(LPVOID) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMFreeBuffer").unwrap() };
//     pub static ref WFM_KILL_TIMER: Symbol<'static, unsafe extern "stdcall" fn(WORD) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMKillTimer").unwrap() };
//     pub static ref WFM_OUTPUT_TRACE_DATA: Symbol<'static, unsafe extern "stdcall" fn(LPSTR) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMOutputTraceData").unwrap() };
//     pub static ref WFM_SET_TIMER: Symbol<'static, unsafe extern "stdcall" fn(HWND, LPVOID, DWORD, LPWORD) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMSetTimer").unwrap() };
//     pub static ref WFM_SET_TRACE_LEVEL: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, DWORD) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMSetTraceLevel").unwrap() };
// }

// /// Unwraps result, logging error if any and returning xfs internal error value.
// macro_rules! xfs_unwrap {
//     ($l:expr) => {
//         match $l {
//             Ok(result) => result,
//             Err(error) => {
//                 error!("{:?}", error);
//                 return WFS_ERR_INTERNAL_ERROR;
//             }
//         }
//     };
// }

// macro_rules! xfs_reject {
//     ($l:expr) => {{
//         error!("XFS_SUPP {}", stringify!($l));
//         return $l;
//     }};
// }

// lazy_static! {
//     // holds application & service providers buffers
//     static ref BUFFERS: Mutex<HashMap<ULONG_PTR, Vec<ULONG_PTR>>> = Mutex::new(HashMap::new());

//     // holds application timers
//     static ref TIMERS: Mutex<Vec<Option<Timer>>> = Mutex::new((0..65535).map(|_| None).collect());
// }

// struct Timer {
//     hwnd: ULONG_PTR,
//     lpcontext: ULONG_PTR,
//     timer_id: usize,
// }

// #[allow(non_snake_case)]
// #[no_mangle]
// #[logfn(TRACE)]
// #[logfn_inputs(TRACE)]
// pub unsafe extern "stdcall" fn WFMAllocateBuffer(ulSize: ULONG, ulFlags: ULONG, lppvData: *mut LPVOID) -> HRESULT {
//     if lppvData.is_null() {
//         xfs_reject!(WFS_ERR_INVALID_POINTER);
//     }

//     let mut buffers = xfs_unwrap!(BUFFERS.lock());

//     *lppvData = HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, ulSize as usize);

//     if lppvData.is_null() {
//         xfs_reject!(WFS_ERR_OUT_OF_MEMORY);
//     }

//     buffers.insert(*lppvData as ULONG_PTR, Vec::new());

//     WFS_SUCCESS
//     // (WFM_ALLOCATE_BUFFER)(ulSize, ulFlags, lppvData)
// }

// #[allow(non_snake_case)]
// #[no_mangle]
// #[logfn(TRACE)]
// #[logfn_inputs(TRACE)]
// pub unsafe extern "stdcall" fn WFMAllocateMore(ulSize: ULONG, lpvOriginal: LPVOID, lppvData: *mut LPVOID) -> HRESULT {
//     if lppvData.is_null() {
//         xfs_reject!(WFS_ERR_INVALID_POINTER);
//     }

//     let mut buffers = xfs_unwrap!(BUFFERS.lock());
//     let list = match buffers.get_mut(&(lpvOriginal as ULONG_PTR)) {
//         Some(list) => list,
//         None => xfs_reject!(WFS_ERR_INVALID_BUFFER),
//     };

//     unsafe {
//         *lppvData = HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, ulSize as usize);
//     }

//     if lppvData.is_null() {
//         xfs_reject!(WFS_ERR_OUT_OF_MEMORY);
//     }

//     unsafe {
//         list.push(*lppvData as ULONG_PTR);
//     }

//     WFS_SUCCESS
//     // (WFM_ALLOCATE_MORE)(ulSize, lpvOriginal, lppvData)
// }

// #[allow(non_snake_case)]
// #[no_mangle]
// #[logfn(TRACE)]
// #[logfn_inputs(TRACE)]
// pub unsafe extern "stdcall" fn WFMFreeBuffer(lpvData: LPVOID) -> HRESULT {
//     if lpvData.is_null() {
//         xfs_reject!(WFS_ERR_INVALID_POINTER);
//     }

//     let mut buffers = xfs_unwrap!(BUFFERS.lock());
//     let list = match buffers.get_mut(&(lpvData as ULONG_PTR)) {
//         Some(list) => list,
//         None => xfs_reject!(WFS_ERR_INVALID_BUFFER),
//     };

//     for &ptr in list.iter() {
//         unsafe {
//             HeapFree(GetProcessHeap(), 0, ptr as *mut c_void);
//         }
//     }

//     buffers.remove(&(lpvData as ULONG_PTR));

//     unsafe {
//         HeapFree(GetProcessHeap(), 0, lpvData);
//     }

//     WFS_SUCCESS
//     // (WFM_FREE_BUFFER)(lpvData)
// }

// #[allow(non_snake_case)]
// #[no_mangle]
// #[logfn(TRACE)]
// #[logfn_inputs(TRACE)]
// pub unsafe extern "stdcall" fn WFMKillTimer(wTimerID: WORD) -> HRESULT {
//     (WFM_KILL_TIMER)(wTimerID)
// }

// #[allow(non_snake_case)]
// #[no_mangle]
// #[logfn(TRACE)]
// #[logfn_inputs(TRACE)]
// pub unsafe extern "stdcall" fn WFMOutputTraceData(lpszData: LPSTR) -> HRESULT {
//     (WFM_OUTPUT_TRACE_DATA)(lpszData)
// }

// #[allow(non_snake_case)]
// #[no_mangle]
// #[logfn(TRACE)]
// #[logfn_inputs(TRACE)]
// pub unsafe extern "stdcall" fn WFMSetTimer(hWnd: HWND, lpContext: LPVOID, dwTimeVal: DWORD, lpwTimerID: LPWORD) -> HRESULT {
//     (WFM_SET_TIMER)(hWnd, lpContext, dwTimeVal, lpwTimerID)
// }

// #[allow(non_snake_case)]
// #[no_mangle]
// #[logfn(TRACE)]
// #[logfn_inputs(TRACE)]
// pub unsafe extern "stdcall" fn WFMSetTraceLevel(_hService: HSERVICE, _dwTraceLevel: DWORD) -> HRESULT {
//     (WFM_SET_TRACE_LEVEL)(_hService, _dwTraceLevel)
// }

// #[allow(non_snake_case)]
// #[no_mangle]
// pub extern "stdcall" fn DllMain(_hinst_dll: HINSTANCE, fdw_reason: DWORD, _: LPVOID) -> bool {
//     if fdw_reason == DLL_PROCESS_ATTACH {
//         let logfile = FileAppender::builder()
//             .encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} {l} {L} - {m}\n")))
//             .build("C:\\XFS_SUPP.log")
//             .unwrap();
//         let config = Config::builder()
//             .appender(Appender::builder().build("logfile", Box::new(logfile)))
//             .build(Root::builder().appender("logfile").build(LevelFilter::Trace))
//             .unwrap();

//         log4rs::init_config(config).unwrap();
//         trace!("XFS SUPP DLL INIT");
//     }
//     true
// }
