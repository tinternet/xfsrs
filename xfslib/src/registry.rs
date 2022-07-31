use std::{
    ffi::{CStr, CString},
    ptr,
};

use winapi::{
    shared::{
        minwindef::{DWORD, HKEY, LPDWORD, PFILETIME, PHKEY},
        winerror::{ERROR_FILE_NOT_FOUND, ERROR_INVALID_HANDLE, ERROR_KEY_HAS_CHILDREN, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, HRESULT},
    },
    um::{
        winnt::{KEY_ALL_ACCESS, LPSTR, REG_CREATED_NEW_KEY, REG_OPENED_EXISTING_KEY, REG_OPTION_NON_VOLATILE, REG_SZ},
        winreg::{
            RegCloseKey, RegCreateKeyExA, RegDeleteKeyExA, RegDeleteValueA, RegEnumKeyExA, RegEnumValueA, RegGetValueA, RegOpenKeyA, RegSetValueExA, HKEY_CLASSES_ROOT, HKEY_LOCAL_MACHINE, HKEY_USERS,
            RRF_RT_ANY,
        },
    },
};

use crate::*;

pub unsafe fn close_key(h_key: HKEY) -> HRESULT {
    match RegCloseKey(h_key) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        ERROR_INVALID_HANDLE => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    }
}

pub unsafe fn create_key(h_key: HKEY, lpsz_sub_key: LPSTR, phk_result: PHKEY, lpdw_disposition: LPDWORD) -> HRESULT {
    if lpsz_sub_key.is_null() || lpdw_disposition.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    let dw_disposition: LPDWORD = 0 as LPDWORD;

    let (sub_key, h_key) = match h_key {
        WFS_CFG_HKEY_XFS_ROOT => ("WOSA/XFS_ROOT\\", HKEY_CLASSES_ROOT),
        WFS_CFG_HKEY_MACHINE_XFS_ROOT => ("SOFTWARE\\XFS\\", HKEY_LOCAL_MACHINE),
        WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT => (".DEFAULT\\XFS\\", HKEY_USERS),
        _ => ("", h_key),
    };

    let sub_key = format!("{}{}", sub_key, xfs_unwrap!(CStr::from_ptr(lpsz_sub_key).to_str()));
    let sub_key_cstring = xfs_unwrap!(CString::new(sub_key));

    match RegCreateKeyExA(
        h_key,
        sub_key_cstring.as_ptr(),
        0,
        ptr::null_mut(),
        REG_OPTION_NON_VOLATILE,
        KEY_ALL_ACCESS,
        ptr::null_mut(),
        phk_result,
        dw_disposition,
    ) as u32
    {
        ERROR_SUCCESS => {
            lpdw_disposition.write(match *dw_disposition {
                REG_CREATED_NEW_KEY => WFS_CFG_CREATED_NEW_KEY,
                REG_OPENED_EXISTING_KEY => WFS_CFG_OPENED_EXISTING_KEY,
                _ => 0,
            });
            WFS_SUCCESS
        }
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        ERROR_PATH_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_SUBKEY),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    }
}

pub unsafe fn delete_key(h_key: HKEY, lpsz_sub_key: LPSTR) -> HRESULT {
    if lpsz_sub_key.is_null() {
        return WFS_ERR_INVALID_POINTER;
    }

    match RegDeleteKeyExA(h_key, lpsz_sub_key, 0, 0) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        ERROR_KEY_HAS_CHILDREN => xfs_reject!(WFS_ERR_CFG_KEY_NOT_EMPTY),
        ERROR_PATH_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_SUBKEY),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    }
}

pub unsafe fn delete_value(h_key: HKEY, lpsz_value: LPSTR) -> HRESULT {
    if lpsz_value.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    match RegDeleteValueA(h_key, lpsz_value) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        ERROR_PATH_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_VALUE),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    }
}

pub unsafe fn enum_key(h_key: HKEY, i_sub_key: DWORD, lpsz_name: LPSTR, lpcch_name: LPDWORD, lpft_last_write: PFILETIME) -> HRESULT {
    if lpsz_name.is_null() || lpcch_name.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    match RegEnumKeyExA(h_key, i_sub_key, lpsz_name, lpcch_name, ptr::null_mut(), ptr::null_mut(), ptr::null_mut(), lpft_last_write) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        ERROR_MORE_DATA => xfs_reject!(WFS_ERR_CFG_NAME_TOO_LONG),
        ERROR_NO_MORE_ITEMS => xfs_reject!(WFS_ERR_CFG_NO_MORE_ITEMS),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    }
}

pub unsafe fn enum_value(h_key: HKEY, i_value: DWORD, lpsz_value: LPSTR, lpcch_value: LPDWORD, lpsz_data: LPSTR, lpcch_data: LPDWORD) -> HRESULT {
    if lpsz_value.is_null() || lpcch_value.is_null() || lpsz_data.is_null() || lpcch_data.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    let result = match RegEnumValueA(h_key, i_value, lpsz_value, lpcch_value, ptr::null_mut(), ptr::null_mut(), lpsz_data as *mut _, lpcch_data) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        ERROR_PATH_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_SUBKEY),
        ERROR_MORE_DATA => xfs_reject!(WFS_ERR_CFG_VALUE_TOO_LONG),
        ERROR_NO_MORE_ITEMS => xfs_reject!(WFS_ERR_CFG_NO_MORE_ITEMS),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    };

    // Diebold xfs simply decreases by 1 even if there was an error
    *lpcch_data = *lpcch_data - 1;

    result
}

pub unsafe fn open_key(h_key: HKEY, lpsz_sub_key: LPSTR, phk_result: PHKEY) -> HRESULT {
    if lpsz_sub_key.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }
    let (sub_key, h_key) = match h_key {
        WFS_CFG_HKEY_XFS_ROOT => ("WOSA/XFS_ROOT\\", HKEY_CLASSES_ROOT),
        WFS_CFG_HKEY_MACHINE_XFS_ROOT => ("SOFTWARE\\XFS\\", HKEY_LOCAL_MACHINE),
        WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT => (".DEFAULT\\XFS\\", HKEY_USERS),
        _ => ("", h_key),
    };

    let sub_key = format!("{}{}", sub_key, xfs_unwrap!(CStr::from_ptr(lpsz_sub_key).to_str()));
    let sub_key_cstring = xfs_unwrap!(CString::new(sub_key));

    match RegOpenKeyA(h_key, sub_key_cstring.as_ptr(), phk_result) as DWORD {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        ERROR_PATH_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_SUBKEY),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    }
}

pub unsafe fn query_value(h_key: HKEY, lpsz_value_name: LPSTR, lpsz_data: LPSTR, lpcch_data: LPDWORD) -> HRESULT {
    if lpsz_value_name.is_null() || lpcch_data.is_null() || ((*lpcch_data > 0) && lpsz_data.is_null()) {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    let result = match RegGetValueA(h_key, ptr::null_mut(), lpsz_value_name, RRF_RT_ANY, ptr::null_mut(), lpsz_data as *mut _, lpcch_data) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => WFS_ERR_CFG_INVALID_NAME,
        ERROR_PATH_NOT_FOUND => WFS_ERR_CFG_INVALID_HKEY,
        ERROR_MORE_DATA => WFS_ERR_CFG_VALUE_TOO_LONG,
        _ => WFS_ERR_INTERNAL_ERROR,
    };

    // Diebold xfs simply decreases by 1 even if there was an error
    *lpcch_data = *lpcch_data - 1;
    result
}

pub unsafe fn set_value(h_key: HKEY, lpsz_value_name: LPSTR, lpsz_data: LPSTR, cch_data: DWORD) -> HRESULT {
    if lpsz_value_name.is_null() || lpsz_data.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    match RegSetValueExA(h_key, lpsz_value_name, 0, REG_SZ, lpsz_data as *mut _, cch_data) as u32 {
        ERROR_SUCCESS => WFS_SUCCESS,
        ERROR_FILE_NOT_FOUND => xfs_reject!(WFS_ERR_CFG_INVALID_HKEY),
        _ => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    }
}

#[cfg(test)]
mod tests {
    use crate::registry::*;
    use winapi::shared::minwindef::MAX_PATH;

    #[test]
    fn test_open_key() {
        let mut key: HKEY = ptr::null_mut();
        let path = CString::new("LOGICAL_SERVICES\\cwd").unwrap();
        let result = unsafe { open_key(WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT, path.as_ptr() as *mut i8, &mut key) };
        assert_eq!(result, WFS_SUCCESS);

        let result = unsafe { close_key(key) };
        assert_eq!(result, WFS_SUCCESS);
    }

    #[test]
    fn test_open_key_fail() {
        let mut key: HKEY = ptr::null_mut();
        let path = CString::new("LOGICAL_SERVICES\\gfdggfshfgsdfgs").unwrap();
        let result = unsafe { open_key(WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT, path.as_ptr() as *mut i8, &mut key) };
        assert_eq!(result, WFS_ERR_CFG_INVALID_HKEY);

        let result = unsafe { close_key(key) };
        assert_eq!(result, WFS_ERR_CFG_INVALID_HKEY);
    }

    #[test]
    fn test_query_value() {
        let mut lgl_prov_path = [0u8; MAX_PATH];
        let mut key: HKEY = ptr::null_mut();
        let path = CString::new("LOGICAL_SERVICES\\cwd").unwrap();

        let result = unsafe { open_key(WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT, path.as_ptr() as *mut i8, &mut key) };
        assert_eq!(result, WFS_SUCCESS);

        let name = CString::new("provider").unwrap();
        let len = &mut (MAX_PATH as u32);
        let result = unsafe { query_value(key, name.as_ptr() as *mut _, lgl_prov_path.as_mut_ptr() as *mut _, len) };
        assert_eq!(result, WFS_SUCCESS);
        assert_eq!(&lgl_prov_path[..*len as usize], b"serviceprovider");

        let result = unsafe { close_key(key) };
        assert_eq!(result, WFS_SUCCESS);
    }

    #[test]
    fn test_query_value_fail() {
        let mut lgl_prov_path = [0u8; MAX_PATH];
        let mut key: HKEY = ptr::null_mut();
        let path = CString::new("LOGICAL_SERVICES\\cwd").unwrap();

        let result = unsafe { open_key(WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT, path.as_ptr() as *mut i8, &mut key) };
        assert_eq!(result, WFS_SUCCESS);

        let name = CString::new("54gfdgfdgdfgsgsfg").unwrap();
        let len = &mut (MAX_PATH as u32);
        let result = unsafe { query_value(key, name.as_ptr() as *mut _, lgl_prov_path.as_mut_ptr() as *mut _, len) };
        assert_eq!(result, WFS_ERR_CFG_INVALID_NAME);

        let result = unsafe { close_key(key) };
        assert_eq!(result, WFS_SUCCESS);
    }
}
