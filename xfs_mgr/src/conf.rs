use lazy_static::lazy_static;
use libloading::Symbol;
use winapi::shared::{
    minwindef::{DWORD, HKEY, LPDWORD, PFILETIME, PHKEY},
    ntdef::LPSTR,
    winerror::HRESULT,
};

lazy_static! {
    static ref XFS_LIB: libloading::Library = unsafe { libloading::Library::new("xfs_conf.dll").unwrap() };
    pub static ref WFM_CLOSE_KEY: Symbol<'static, unsafe extern "stdcall" fn(HKEY) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMCloseKey").unwrap() };
    pub static ref WFM_CREATE_KEY: Symbol<'static, unsafe extern "stdcall" fn(HKEY, LPSTR, PHKEY, LPDWORD) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMCreateKey").unwrap() };
    pub static ref WFM_DELETE_KEY: Symbol<'static, unsafe extern "stdcall" fn(HKEY, LPSTR) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMDeleteKey").unwrap() };
    pub static ref WFM_DELETE_VALUE: Symbol<'static, unsafe extern "stdcall" fn(HKEY, LPSTR) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMDeleteValue").unwrap() };
    pub static ref WFM_ENUM_KEY: Symbol<'static, unsafe extern "stdcall" fn(HKEY, DWORD, LPSTR, LPDWORD, PFILETIME) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMEnumKey").unwrap() };
    pub static ref WFM_ENUM_VALUE: Symbol<'static, unsafe extern "stdcall" fn(HKEY, DWORD, LPSTR, LPDWORD, LPSTR, LPDWORD) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMEnumValue").unwrap() };
    pub static ref WFM_OPEN_KEY: Symbol<'static, unsafe extern "stdcall" fn(HKEY, LPSTR, PHKEY) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMOpenKey").unwrap() };
    pub static ref WFM_QUERY_VALUE: Symbol<'static, unsafe extern "stdcall" fn(HKEY, LPSTR, LPSTR, LPDWORD) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMQueryValue").unwrap() };
    pub static ref WFM_SET_VALUE: Symbol<'static, unsafe extern "stdcall" fn(HKEY, LPSTR, LPSTR, DWORD) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMSetValue").unwrap() };
}
