use std::{
    ffi::{CStr, CString},
    ptr,
};

use log::{error, trace, LevelFilter};
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    Config,
};
use log_derive::{logfn, logfn_inputs};
use winapi::{
    shared::{
        minwindef::{DWORD, HINSTANCE, HKEY, LPDWORD, LPVOID, PFILETIME, PHKEY},
        winerror::{ERROR_FILE_NOT_FOUND, ERROR_KEY_HAS_CHILDREN, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, HRESULT},
    },
    um::{
        winnt::{DLL_PROCESS_ATTACH, KEY_ALL_ACCESS, LPSTR, REG_CREATED_NEW_KEY, REG_OPENED_EXISTING_KEY, REG_OPTION_NON_VOLATILE, REG_SZ},
        winreg::{
            RegCloseKey, RegCreateKeyExA, RegDeleteKeyExA, RegDeleteValueA, RegEnumKeyExA, RegEnumValueA, RegGetValueA, RegOpenKeyA, RegSetValueExA, HKEY_CLASSES_ROOT, HKEY_LOCAL_MACHINE, HKEY_USERS,
            RRF_RT_ANY,
        },
    },
};
use xfslib::*;

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

    let result = match RegEnumValueA(hKey, iValue, lpszValue, lpcchValue, ptr::null_mut(), ptr::null_mut(), lpszData as *mut _, lpcchData) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        ERROR_PATH_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_SUBKEY),
        ERROR_MORE_DATA => xfs_reject!(WFS_ERR_CFG_VALUE_TOO_LONG),
        ERROR_NO_MORE_ITEMS => xfs_reject!(WFS_ERR_CFG_NO_MORE_ITEMS),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    };

    // Diebold xfs simply decreases by 1 even if there was an error
    *lpcchData = *lpcchData - 1;

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
    if lpszValueName.is_null() || lpcchData.is_null() || ((*lpcchData > 0) && lpszData.is_null()) {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    let result = match RegGetValueA(hKey, std::ptr::null_mut(), lpszValueName, RRF_RT_ANY, std::ptr::null_mut(), lpszData as *mut _, lpcchData) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => WFS_ERR_CFG_INVALID_NAME,
        ERROR_PATH_NOT_FOUND => WFS_ERR_CFG_INVALID_HKEY,
        ERROR_MORE_DATA => WFS_ERR_CFG_VALUE_TOO_LONG,
        _ => WFS_ERR_INTERNAL_ERROR,
    };

    // Diebold xfs simply decreases by 1 even if there was an error
    *lpcchData = *lpcchData - 1;
    result
}

#[allow(non_snake_case)]
#[no_mangle]
#[logfn(TRACE)]
#[logfn_inputs(TRACE)]
pub unsafe extern "stdcall" fn WFMSetValue(hKey: HKEY, lpszValueName: LPSTR, lpszData: LPSTR, cchData: DWORD) -> HRESULT {
    if lpszValueName.is_null() || lpszData.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    match RegSetValueExA(hKey, lpszValueName, 0, REG_SZ, lpszData as *mut _, cchData) as u32 {
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
            .build("$ENV{Public}\\XFS_CONF.log")
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

#[cfg(test)]
mod tests {
    use super::*;
    use winapi::shared::minwindef::MAX_PATH;

    #[test]
    fn test_open_key() {
        let mut key: HKEY = ptr::null_mut();
        let path = CString::new("LOGICAL_SERVICES\\cwd").unwrap();
        let result = unsafe { WFMOpenKey(WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT, path.as_ptr() as *mut i8, &mut key) };
        assert_eq!(result, 0);
    }
    #[test]
    fn test_open_key_fail() {
        let mut key: HKEY = ptr::null_mut();
        let path = CString::new("LOGICAL_SERVICES\\gfdggfshfgsdfgs").unwrap();
        let result = unsafe { WFMOpenKey(WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT, path.as_ptr() as *mut i8, &mut key) };
        assert_eq!(result, WFS_ERR_CFG_INVALID_HKEY);
    }
    #[test]
    fn test_query_value() {
        let mut lgl_prov_path = [0u8; MAX_PATH];
        let mut key: HKEY = ptr::null_mut();
        let path = CString::new("LOGICAL_SERVICES\\cwd").unwrap();

        let result = unsafe { WFMOpenKey(WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT, path.as_ptr() as *mut i8, &mut key) };
        assert_eq!(result, WFS_SUCCESS);

        let name = CString::new("provider").unwrap();
        let len = &mut (MAX_PATH as u32);
        let result = unsafe { WFMQueryValue(key, name.as_ptr() as *mut _, lgl_prov_path.as_mut_ptr() as *mut _, len) };
        assert_eq!(result, WFS_SUCCESS);
        assert_eq!(&lgl_prov_path[..*len as usize], b"serviceprovider");
    }
    #[test]
    fn test_query_value_fail() {
        let mut lgl_prov_path = [0u8; MAX_PATH];
        let mut key: HKEY = ptr::null_mut();
        let path = CString::new("LOGICAL_SERVICES\\cwd").unwrap();

        let result = unsafe { WFMOpenKey(WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT, path.as_ptr() as *mut i8, &mut key) };
        assert_eq!(result, WFS_SUCCESS);

        let name = CString::new("54gfdgfdgdfgsgsfg").unwrap();
        let len = &mut (MAX_PATH as u32);
        let result = unsafe { WFMQueryValue(key, name.as_ptr() as *mut _, lgl_prov_path.as_mut_ptr() as *mut _, len) };
        assert_eq!(result, WFS_ERR_CFG_INVALID_NAME);
    }
}
