use std::fmt;

use winapi::ctypes::c_char;
use winapi::shared::minwindef::{DWORD, LPVOID, WORD};
use winapi::shared::ntdef::{ULONG, USHORT};
use winapi::um::minwinbase::SYSTEMTIME;
use winapi::um::winnt::{HANDLE, HRESULT};

pub use constants::*;
pub use errors::*;
pub use version::*;

mod constants;
mod errors;
mod version;

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

#[derive(Debug)]
#[repr(C)]
pub struct WFSVERSION {
    pub w_version: WORD,
    pub w_low_version: WORD,
    pub w_high_version: WORD,
    pub sz_description: [c_char; WFSDDESCRIPTION_LEN + 1],
    pub sz_system_status: [c_char; WFSDSYSSTATUS_LEN + 1],
}

#[repr(C)]
#[allow(non_snake_case)]
pub union U {
    pub dwCommandCode: DWORD,
    pub dwEventID: DWORD,
}

#[repr(C)]
#[allow(non_snake_case)]
pub struct WFSRESULT {
    pub RequestID: ULONG,
    pub hService: HSERVICE,
    pub tsTimestamp: SYSTEMTIME,
    pub hResult: HRESULT,
    pub u: U,
    pub lpBuffer: LPVOID,
}

impl fmt::Debug for WFSRESULT {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            write!(
                f,
                "RequestID: {}, hService: {}, tsTimestamp: {}, hResult: {}, u.dwCommandCode: {}, u.dwEventID: {}, lpBuffer: {:?}",
                self.RequestID, self.hService, "", self.hResult, self.u.dwCommandCode, self.u.dwEventID, *self.lpBuffer
            )
        }
    }
}
