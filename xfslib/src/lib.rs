use winapi::ctypes::c_char;
use winapi::shared::minwindef::{DWORD, LPVOID, WORD};
use winapi::shared::ntdef::{ULONG, USHORT};
use winapi::um::minwinbase::SYSTEMTIME;
use winapi::um::winnt::{HANDLE, HRESULT};

pub use constants::*;
pub use errors::*;
pub use util::*;
pub use version::*;
pub use window::*;

pub mod conf;
mod constants;
mod errors;
pub mod supp;
mod util;
mod version;
mod window;

pub type HSERVICE = USHORT;
pub type LPHSERVICE = *mut HSERVICE;
pub type LPWFSVERSION = *mut WFSVERSION;
pub type LPWFSRESULT = *mut WFSRESULT;
pub type REQUESTID = ULONG;
pub type HAPP = HANDLE;
pub type LPHAPP = *mut HAPP;
pub type LPREQUESTID = *mut ULONG;
pub type HPROVIDER = HANDLE;
pub type XFSBLOCKINGHOOK = unsafe extern "stdcall" fn() -> bool;

#[repr(C, packed)]
pub struct WFSVERSION {
    pub w_version: WORD,
    pub w_low_version: WORD,
    pub w_high_version: WORD,
    pub sz_description: [c_char; WFSDDESCRIPTION_LEN + 1],
    pub sz_system_status: [c_char; WFSDSYSSTATUS_LEN + 1],
}

#[allow(non_snake_case)]
#[repr(C)]
pub union U {
    pub dwCommandCode: DWORD,
    pub dwEventID: DWORD,
}

#[allow(non_snake_case)]
#[repr(C, packed)]
pub struct WFSRESULT {
    pub RequestID: ULONG,
    pub hService: HSERVICE,
    pub tsTimestamp: SYSTEMTIME,
    pub hResult: HRESULT,
    pub u: U,
    pub lpBuffer: LPVOID,
}
