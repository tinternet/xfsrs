use std::{fmt, ptr};

use winapi::ctypes::c_char;
use winapi::shared::minwindef::{DWORD, LPVOID, WORD};
use winapi::shared::ntdef::{ULONG, USHORT};
use winapi::um::minwinbase::SYSTEMTIME;
use winapi::um::winnt::{HANDLE, HRESULT};

pub use constants::*;
pub use errors::*;
pub use version::*;
pub use window::*;

mod constants;
mod errors;
mod version;
mod window;

/// Unwraps the result of a WFS call and returns the HRESULT on error.
/// The error is logged to the log file.
#[macro_export]
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

/// Rejects with specific error and logs error.
#[macro_export]
macro_rules! xfs_reject {
    ($l:expr) => {{
        error!(stringify!($l));
        return $l;
    }};
}

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

#[repr(C)]
#[allow(non_snake_case)]
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

impl fmt::Debug for WFSRESULT {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            write!(
                f,
                "RequestID: {}, hService: {}, tsTimestamp: {}, hResult: {}, u.dwCommandCode: {}, u.dwEventID: {}, lpBuffer: {:?}",
                *ptr::addr_of!(self.RequestID),
                *ptr::addr_of!(self.hService),
                format!(
                    "{}-{}-{} {}:{}:{}",
                    self.tsTimestamp.wYear, self.tsTimestamp.wMonth, self.tsTimestamp.wDay, self.tsTimestamp.wHour, self.tsTimestamp.wMinute, self.tsTimestamp.wSecond,
                ),
                *ptr::addr_of!(self.hResult),
                self.u.dwCommandCode,
                self.u.dwEventID,
                *self.lpBuffer
            )
        }
    }
}
