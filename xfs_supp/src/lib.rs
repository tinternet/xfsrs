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
pub extern "stdcall" fn WFMAllocateBuffer(ul_size: ULONG, ul_flags: ULONG, lppv_data: *mut LPVOID) -> HRESULT {
    xfslib::supp::allocate_buffer(ul_size, ul_flags, lppv_data)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMAllocateMore(ulSize: ULONG, lpvOriginal: LPVOID, lppvData: *mut LPVOID) -> HRESULT {
    xfslib::supp::allocate_more(ulSize, lpvOriginal, lppvData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMFreeBuffer(lpvData: LPVOID) -> HRESULT {
    xfslib::supp::free_buffer(lpvData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMKillTimer(wTimerID: WORD) -> HRESULT {
    xfslib::supp::kill_timer(wTimerID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMOutputTraceData(lpszData: LPSTR) -> HRESULT {
    xfslib::supp::output_trace_data(lpszData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMSetTimer(hWnd: HWND, lpContext: LPVOID, dwTimeVal: DWORD, lpwTimerID: LPWORD) -> HRESULT {
    xfslib::supp::set_timer(hWnd, lpContext, dwTimeVal, lpwTimerID)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub extern "stdcall" fn WFMSetTraceLevel(hService: HSERVICE, dwTraceLevel: DWORD) -> HRESULT {
    xfslib::supp::set_trace_level(hService, dwTraceLevel)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn DllMain(hinst_dll: HINSTANCE, fdw_reason: DWORD, _: LPVOID) -> bool {
    module_init(hinst_dll, fdw_reason);
    true
}
