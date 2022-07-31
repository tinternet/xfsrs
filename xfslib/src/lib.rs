use std::ffi::CStr;

use winapi::ctypes::c_char;
use winapi::shared::minwindef::{DWORD, LPVOID, WORD};
use winapi::shared::ntdef::{ULONG, USHORT};
use winapi::um::minwinbase::SYSTEMTIME;
use winapi::um::winnt::{HANDLE, HRESULT, LPSTR};

pub use constants::*;
pub use errors::*;
pub use module::*;
pub use version::*;
pub use window::*;

mod constants;
mod errors;
pub mod heap;
mod macros;
mod module;
pub mod registry;
pub mod spi;
pub mod timer;
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
#[repr(C, packed)]
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

pub fn output_trace_data(lpsz_data: LPSTR) -> HRESULT {
    let data = unsafe { CStr::from_ptr(lpsz_data).to_str() };
    tracing::trace!("XFS TRACE --- {}", xfs_unwrap!(data));
    WFS_SUCCESS
}
