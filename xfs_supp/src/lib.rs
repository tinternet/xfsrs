use log_derive::{logfn, logfn_inputs};
use winapi::{
    shared::{
        minwindef::{DWORD, HINSTANCE, LPVOID, LPWORD, ULONG, WORD},
        windef::HWND,
        winerror::HRESULT,
    },
    um::winnt::LPSTR,
};
use xfslib::*;

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMAllocateBuffer(size: ULONG, flags: ULONG, data: *mut LPVOID) -> HRESULT {
    heap::allocate_buffer(size, flags, data)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMAllocateMore(size: ULONG, original: LPVOID, data: *mut LPVOID) -> HRESULT {
    heap::allocate_more(size, original, data)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMFreeBuffer(data: LPVOID) -> HRESULT {
    heap::free_buffer(data)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMSetTimer(wnd: HWND, context: LPVOID, dw_time_val: DWORD, lpw_timer_id: LPWORD) -> HRESULT {
    timer::set_timer(wnd, context, dw_time_val, lpw_timer_id)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMKillTimer(w_timer_id: WORD) -> HRESULT {
    timer::kill_timer(w_timer_id)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMOutputTraceData(lpszData: LPSTR) -> HRESULT {
    output_trace_data(lpszData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMSetTraceLevel(hService: HSERVICE, dwTraceLevel: DWORD) -> HRESULT {
    WFS_SUCCESS
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn DllMain(hinst_dll: HINSTANCE, fdw_reason: DWORD, _: LPVOID) -> bool {
    module_init(hinst_dll, fdw_reason);
    true
}
