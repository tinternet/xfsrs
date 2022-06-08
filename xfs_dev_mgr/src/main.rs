use std::{ffi::CString, ptr};

use winapi::{
    shared::minwindef::{DWORD, HKEY},
    um::winreg::{RegOpenKeyA, HKEY_LOCAL_MACHINE},
};

fn main() {
    let h_key: HKEY = HKEY_LOCAL_MACHINE;

    let os_string = CString::new("SOFTWARE\\XFS\\SERVICE_PROVIDERS\\serviceprovider").unwrap();
    // let c_path = to_utf16(os_string);
    let mut new_hkey: HKEY = ptr::null_mut();

    let result = unsafe { RegOpenKeyA(h_key, os_string.as_ptr(), &mut new_hkey) as DWORD };

    println!("{}", result);
}
