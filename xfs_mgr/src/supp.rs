use lazy_static::lazy_static;
use libloading::Symbol;
use winapi::shared::{
    minwindef::{DWORD, LPVOID, LPWORD, ULONG, WORD},
    ntdef::LPSTR,
    windef::HWND,
    winerror::HRESULT,
};
use xfslib::HSERVICE;

lazy_static! {
    static ref XFS_LIB: libloading::Library = unsafe { libloading::Library::new("xfs_supp.dll").unwrap() };
    pub static ref WFM_ALLOCATE_BUFFER: Symbol<'static, unsafe extern "stdcall" fn(ULONG, ULONG, *mut LPVOID) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMAllocateBuffer").unwrap() };
    pub static ref WFM_ALLOCATE_MORE: Symbol<'static, unsafe extern "stdcall" fn(ULONG, LPVOID, *mut LPVOID) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMAllocateMore").unwrap() };
    pub static ref WFM_FREE_BUFFER: Symbol<'static, unsafe extern "stdcall" fn(LPVOID) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMFreeBuffer").unwrap() };
    pub static ref WFM_KILL_TIMER: Symbol<'static, unsafe extern "stdcall" fn(WORD) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMKillTimer").unwrap() };
    pub static ref WFM_OUTPUT_TRACE_DATA: Symbol<'static, unsafe extern "stdcall" fn(LPSTR) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMOutputTraceData").unwrap() };
    pub static ref WFM_SET_TIMER: Symbol<'static, unsafe extern "stdcall" fn(HWND, LPVOID, DWORD, LPWORD) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMSetTimer").unwrap() };
    pub static ref WFM_SET_TRACE_LEVEL: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, DWORD) -> HRESULT> = unsafe { XFS_LIB.get(b"WFMSetTraceLevel").unwrap() };
    pub static ref XFS_SUPP_CLEANUP: Symbol<'static, unsafe extern "stdcall" fn() -> HRESULT> = unsafe { XFS_LIB.get(b"CleanUp").unwrap() };
}
