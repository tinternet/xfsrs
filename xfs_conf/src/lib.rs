use log_derive::{logfn, logfn_inputs};
use winapi::{
    shared::{
        minwindef::{DWORD, HINSTANCE, HKEY, LPDWORD, LPVOID, PFILETIME, PHKEY},
        winerror::HRESULT,
    },
    um::winnt::LPSTR,
};
use xfslib::{module_init, registry::*};

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMCloseKey(h_key: HKEY) -> HRESULT {
    close_key(h_key)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMCreateKey(hKey: HKEY, lpszSubKey: LPSTR, phkResult: PHKEY, lpdwDisposition: LPDWORD) -> HRESULT {
    create_key(hKey, lpszSubKey, phkResult, lpdwDisposition)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMDeleteKey(hKey: HKEY, lpszSubKey: LPSTR) -> HRESULT {
    delete_key(hKey, lpszSubKey)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMDeleteValue(hKey: HKEY, lpszValue: LPSTR) -> HRESULT {
    delete_value(hKey, lpszValue)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMEnumKey(hKey: HKEY, iSubKey: DWORD, lpszName: LPSTR, lpcchName: LPDWORD, lpftLastWrite: PFILETIME) -> HRESULT {
    enum_key(hKey, iSubKey, lpszName, lpcchName, lpftLastWrite)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMEnumValue(hKey: HKEY, iValue: DWORD, lpszValue: LPSTR, lpcchValue: LPDWORD, lpszData: LPSTR, lpcchData: LPDWORD) -> HRESULT {
    enum_value(hKey, iValue, lpszValue, lpcchValue, lpszData, lpcchData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMOpenKey(hKey: HKEY, lpszSubKey: LPSTR, phkResult: PHKEY) -> HRESULT {
    open_key(hKey, lpszSubKey, phkResult)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMQueryValue(hKey: HKEY, lpszValueName: LPSTR, lpszData: LPSTR, lpcchData: LPDWORD) -> HRESULT {
    query_value(hKey, lpszValueName, lpszData, lpcchData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMSetValue(hKey: HKEY, lpszValueName: LPSTR, lpszData: LPSTR, cchData: DWORD) -> HRESULT {
    set_value(hKey, lpszValueName, lpszData, cchData)
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn DllMain(hinst_dll: HINSTANCE, fdw_reason: DWORD, _: LPVOID) -> bool {
    module_init(hinst_dll, fdw_reason);
    true
}
