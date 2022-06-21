use std::ffi::{CStr, CString};
use std::ptr;
use std::{collections::HashMap, sync::Mutex};

use lazy_static::lazy_static;
use libloading::Symbol;
use log::{debug, error, info, trace, LevelFilter};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;
use log_derive::{logfn, logfn_inputs};
use winapi::shared::minwindef::{BYTE, MAX_PATH};
use winapi::shared::winerror::{ERROR_FILE_NOT_FOUND, ERROR_KEY_HAS_CHILDREN, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS};
use winapi::shared::{
    minwindef::{DWORD, HKEY, LPDWORD, PFILETIME, PHKEY},
    ntdef::LPSTR,
    winerror::HRESULT,
};
use winapi::um::winnt::{KEY_ALL_ACCESS, REG_CREATED_NEW_KEY, REG_OPENED_EXISTING_KEY, REG_OPTION_NON_VOLATILE, REG_SZ};
use winapi::um::winreg::{
    RegCloseKey, RegCreateKeyExA, RegDeleteKeyExA, RegDeleteValueA, RegEnumKeyExA, RegEnumValueA, RegGetValueA, RegOpenKeyA, RegQueryValueA, RegSetValueExA, HKEY_CLASSES_ROOT, HKEY_LOCAL_MACHINE,
    HKEY_USERS, RRF_RT_ANY,
};
use winapi::{
    shared::{
        basetsd::{UINT_PTR, ULONG_PTR},
        minwindef::{HINSTANCE, LPARAM, LPVOID, LPWORD, UINT, ULONG, WORD},
        windef::HWND,
    },
    um::{
        winnt::DLL_PROCESS_ATTACH,
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
        error!("XFS_CONF {}", stringify!($l));
        return $l;
    }};
}

lazy_static! {
    static ref XFS_LIB: libloading::Library = unsafe { libloading::Library::new("xfs_conf_orig.dll").unwrap() };
    pub static ref WFM_QUERY_VALUE: Symbol<'static, unsafe extern "stdcall" fn(HKEY, LPSTR, LPSTR, LPDWORD) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMQueryValue").unwrap() };
    static ref XFS_LIB_MY: libloading::Library = unsafe { libloading::Library::new("xfs_conf_my.dll").unwrap() };
    pub static ref WFM_QUERY_VALUE_MY: Symbol<'static, unsafe extern "stdcall" fn(HKEY, LPSTR, LPSTR, LPDWORD) -> HRESULT> = unsafe { XFS_LIB_MY.get(b"WFMQueryValue").unwrap() };
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMCloseKey(hKey: HKEY) -> HRESULT {
    match RegCloseKey(hKey) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMCreateKey(hKey: HKEY, lpszSubKey: LPSTR, phkResult: PHKEY, lpdwDisposition: LPDWORD) -> HRESULT {
    if lpszSubKey.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    *lpdwDisposition = 0;
    let dw_disposition: LPDWORD = 0 as LPDWORD;

    let (sub_key, h_key) = match hKey {
        WFS_CFG_HKEY_XFS_ROOT => ("WOSA/XFS_ROOT\\", HKEY_CLASSES_ROOT),
        WFS_CFG_HKEY_MACHINE_XFS_ROOT => ("SOFTWARE\\XFS\\", HKEY_LOCAL_MACHINE),
        WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT => (".DEFAULT\\XFS\\", HKEY_USERS),
        _ => ("", hKey),
    };

    let sub_key = format!("{}{}", sub_key, xfs_unwrap!(CStr::from_ptr(lpszSubKey).to_str()));
    let sub_key_cstring = xfs_unwrap!(CString::new(sub_key));

    match RegCreateKeyExA(
        h_key,
        sub_key_cstring.as_ptr(),
        0,
        ptr::null_mut(),
        REG_OPTION_NON_VOLATILE,
        KEY_ALL_ACCESS,
        ptr::null_mut(),
        phkResult,
        dw_disposition,
    ) as u32
    {
        ERROR_SUCCESS => {
            *lpdwDisposition = match *dw_disposition {
                REG_CREATED_NEW_KEY => WFS_CFG_CREATED_NEW_KEY,
                REG_OPENED_EXISTING_KEY => WFS_CFG_OPENED_EXISTING_KEY,
                _ => 0,
            };
            WFS_SUCCESS
        }
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        ERROR_PATH_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_SUBKEY),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMDeleteKey(hKey: HKEY, lpszSubKey: LPSTR) -> HRESULT {
    if lpszSubKey.is_null() {
        return WFS_ERR_INVALID_POINTER;
    }

    match RegDeleteKeyExA(hKey, lpszSubKey, 0, 0) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        ERROR_KEY_HAS_CHILDREN => xfs_reject!(WFS_ERR_CFG_KEY_NOT_EMPTY),
        ERROR_PATH_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_SUBKEY),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMDeleteValue(hKey: HKEY, lpszValue: LPSTR) -> HRESULT {
    if lpszValue.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    match RegDeleteValueA(hKey, lpszValue) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        ERROR_PATH_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_VALUE),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMEnumKey(hKey: HKEY, iSubKey: DWORD, lpszName: LPSTR, lpcchName: LPDWORD, lpftLastWrite: PFILETIME) -> HRESULT {
    if lpszName.is_null() || lpcchName.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    for i in 0..*lpcchName {
        *lpszName.add(i as usize) = 0;
    }

    match RegEnumKeyExA(hKey, iSubKey, lpszName, lpcchName, ptr::null_mut(), ptr::null_mut(), ptr::null_mut(), lpftLastWrite) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        ERROR_MORE_DATA => xfs_reject!(WFS_ERR_CFG_NAME_TOO_LONG),
        ERROR_NO_MORE_ITEMS => xfs_reject!(WFS_ERR_CFG_NO_MORE_ITEMS),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMEnumValue(hKey: HKEY, iValue: DWORD, lpszValue: LPSTR, lpcchValue: LPDWORD, lpszData: LPSTR, lpcchData: LPDWORD) -> HRESULT {
    if lpszValue.is_null() || lpcchValue.is_null() || lpszData.is_null() || lpcchData.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    for i in 0..*lpcchValue {
        *lpszValue.add(i as usize) = 0;
    }
    for i in 0..*lpcchData {
        *lpszData.add(i as usize) = 0;
    }

    let result = match RegEnumValueA(hKey, iValue, lpszValue, lpcchValue, ptr::null_mut(), ptr::null_mut(), lpszData as *mut u8, lpcchData) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        ERROR_PATH_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_SUBKEY),
        ERROR_MORE_DATA => xfs_reject!(WFS_ERR_CFG_VALUE_TOO_LONG),
        ERROR_NO_MORE_ITEMS => xfs_reject!(WFS_ERR_CFG_NO_MORE_ITEMS),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    };

    // Exclude null termination if any
    if result == WFS_SUCCESS {
        if *lpszData.add(*lpcchData as usize) == 0 {
            *lpcchData = *lpcchData - 1;
        }
    }

    result
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMOpenKey(hKey: HKEY, lpszSubKey: LPSTR, phkResult: PHKEY) -> HRESULT {
    if lpszSubKey.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }
    let (sub_key, h_key) = match hKey {
        WFS_CFG_HKEY_XFS_ROOT => ("WOSA/XFS_ROOT\\", HKEY_CLASSES_ROOT),
        WFS_CFG_HKEY_MACHINE_XFS_ROOT => ("SOFTWARE\\XFS\\", HKEY_LOCAL_MACHINE),
        WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT => (".DEFAULT\\XFS\\", HKEY_USERS),
        _ => ("", hKey),
    };

    let sub_key = format!("{}{}", sub_key, xfs_unwrap!(CStr::from_ptr(lpszSubKey).to_str()));
    trace!("OPENING sub_key: {}", sub_key);
    let sub_key_cstring = xfs_unwrap!(CString::new(sub_key));

    match RegOpenKeyA(h_key, sub_key_cstring.as_ptr(), phkResult) as DWORD {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        ERROR_PATH_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_SUBKEY),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    }
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMQueryValue(hKey: HKEY, lpszValueName: LPSTR, lpszData: LPSTR, lpcchData: LPDWORD) -> HRESULT {
    let mut data1 = [0; MAX_PATH];
    let len1 = &mut (MAX_PATH as u32);
    let result = (WFM_QUERY_VALUE)(hKey, lpszValueName, data1.as_mut_ptr(), len1);

    let mut data2 = [0; MAX_PATH];
    let len2 = &mut (MAX_PATH as u32);
    let result2 = (WFM_QUERY_VALUE_MY)(hKey, lpszValueName, data2.as_mut_ptr(), len2);

    info!("CHECKING");

    if result != result2 {
        debug!("WFMQueryValue: result != result2");
    }

    if len1 != len2 {
        debug!("WFMQueryValue: their: {:?}, mine: {:?}", data1, data2);
        debug!("WFMQueryValue: {} {}", len1, len2);
    }

    if data1 != data2 {
        debug!("WFMQueryValue: their: {:?}, mine: {:?}", data1, data2);
    }

    (WFM_QUERY_VALUE_MY)(hKey, lpszValueName, lpszData, lpcchData)
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMSetValue(hKey: HKEY, lpszValueName: LPSTR, lpszData: LPSTR, cchData: DWORD) -> HRESULT {
    if lpszValueName.is_null() || lpszData.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    match RegSetValueExA(hKey, lpszValueName, 0, REG_SZ, lpszData as *mut u8, cchData) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    }
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn DllMain(_hinst_dll: HINSTANCE, fdw_reason: DWORD, _: LPVOID) -> bool {
    if fdw_reason == DLL_PROCESS_ATTACH {
        let logfile = FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} {l} {L} - {m}\n")))
            .build("C:\\XFS_CONF.log")
            .unwrap();
        let config = Config::builder()
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
            .build(Root::builder().appender("logfile").build(LevelFilter::Trace))
            .unwrap();

        log4rs::init_config(config).unwrap();
        trace!("XFS CONF DLL INIT");
    }
    true
}
