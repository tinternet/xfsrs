use lazy_static::lazy_static;
use libloading::Symbol;
use winapi::shared::{
    minwindef::{DWORD, HKEY, LPDWORD, PFILETIME, PHKEY},
    ntdef::LPSTR,
    winerror::HRESULT,
};

#[allow(non_snake_case)]
pub struct XFSConfig {
    pub WFMCloseKey: Symbol<'static, unsafe extern "stdcall" fn(HKEY) -> HRESULT>,
    pub WFMCreateKey: Symbol<'static, unsafe extern "stdcall" fn(HKEY, LPSTR, PHKEY, LPDWORD) -> HRESULT>,
    pub WFMDeleteKey: Symbol<'static, unsafe extern "stdcall" fn(HKEY, LPSTR) -> HRESULT>,
    pub WFMDeleteValue: Symbol<'static, unsafe extern "stdcall" fn(HKEY, LPSTR) -> HRESULT>,
    pub WFMEnumKey: Symbol<'static, unsafe extern "stdcall" fn(HKEY, DWORD, LPSTR, LPDWORD, PFILETIME) -> HRESULT>,
    pub WFMEnumValue: Symbol<'static, unsafe extern "stdcall" fn(HKEY, DWORD, LPSTR, LPDWORD, LPSTR, LPDWORD) -> HRESULT>,
    pub WFMOpenKey: Symbol<'static, unsafe extern "stdcall" fn(HKEY, LPSTR, PHKEY) -> HRESULT>,
    pub WFMQueryValue: Symbol<'static, unsafe extern "stdcall" fn(HKEY, LPSTR, LPSTR, LPDWORD) -> HRESULT>,
    pub WFMSetValue: Symbol<'static, unsafe extern "stdcall" fn(HKEY, LPSTR, LPSTR, DWORD) -> HRESULT>,
}

lazy_static! {
    static ref XFS_LIB: libloading::Library = unsafe {
        let lib = libloading::Library::new("xfs_conf.dll").unwrap();
        lib
    };
    pub static ref XFS_CONFIG: XFSConfig = unsafe {
        XFSConfig {
            WFMCloseKey: XFS_LIB.get(b"WFMCloseKey").unwrap(),
            WFMCreateKey: XFS_LIB.get(b"WFMCreateKey").unwrap(),
            WFMDeleteKey: XFS_LIB.get(b"WFMDeleteKey").unwrap(),
            WFMDeleteValue: XFS_LIB.get(b"WFMDeleteValue").unwrap(),
            WFMEnumKey: XFS_LIB.get(b"WFMEnumKey").unwrap(),
            WFMEnumValue: XFS_LIB.get(b"WFMEnumValue").unwrap(),
            WFMOpenKey: XFS_LIB.get(b"WFMOpenKey").unwrap(),
            WFMQueryValue: XFS_LIB.get(b"WFMQueryValue").unwrap(),
            WFMSetValue: XFS_LIB.get(b"WFMSetValue").unwrap(),
        }
    };
}
