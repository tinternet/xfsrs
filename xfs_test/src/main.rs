use std::{ffi::CString, mem::size_of, ptr};

use libloading::Symbol;
use log::LevelFilter;
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    Config,
};
use winapi::{
    ctypes::c_void,
    shared::{
        minwindef::{DWORD, LPDWORD, LPVOID, LPWORD, ULONG, WORD},
        windef::HWND,
        winerror::HRESULT,
    },
    um::{minwinbase::SYSTEMTIME, winnt::LPSTR},
};
use xfslib::*;

#[allow(non_snake_case, dead_code)]
struct XFSApi<'a> {
    WFSCancelAsyncRequest: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, REQUESTID) -> HRESULT>,
    WFSCancelBlockingCall: Symbol<'a, unsafe extern "stdcall" fn(DWORD) -> HRESULT>,
    WFSCleanUp: Symbol<'a, unsafe extern "stdcall" fn() -> HRESULT>,
    WFSClose: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE) -> HRESULT>,
    WFSAsyncClose: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, HWND, LPREQUESTID) -> HRESULT>,
    WFSCreateAppHandle: Symbol<'a, unsafe extern "stdcall" fn(LPHAPP) -> HRESULT>,
    WFSDeregister: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, DWORD, HWND) -> HRESULT>,
    WFSAsyncDeregister: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, DWORD, HWND, HWND, LPREQUESTID) -> HRESULT>,
    WFSDestroyAppHandle: Symbol<'a, unsafe extern "stdcall" fn(HAPP) -> HRESULT>,
    WFSExecute: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, DWORD, LPVOID, DWORD, LPWFSRESULT) -> HRESULT>,
    WFSAsyncExecute: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, DWORD, LPVOID, DWORD, HWND, LPREQUESTID) -> HRESULT>,
    WFSFreeResult: Symbol<'a, unsafe extern "stdcall" fn(LPWFSRESULT) -> HRESULT>,
    WFSGetInfo: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, DWORD, LPVOID, DWORD, LPWFSRESULT) -> HRESULT>,
    WFSAsyncGetInfo: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, DWORD, LPVOID, DWORD, HWND, LPREQUESTID) -> HRESULT>,
    WFSIsBlocking: Symbol<'a, unsafe extern "stdcall" fn() -> bool>,
    WFSLock: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, DWORD, LPWFSRESULT) -> HRESULT>,
    WFSAsyncLock: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, DWORD, HWND, LPREQUESTID) -> HRESULT>,
    WFSOpen: Symbol<'a, unsafe extern "stdcall" fn(LPSTR, HAPP, LPSTR, DWORD, DWORD, DWORD, LPWFSVERSION, LPWFSVERSION, LPHSERVICE) -> HRESULT>,
    WFSAsyncOpen: Symbol<'a, unsafe extern "stdcall" fn(LPSTR, HAPP, LPSTR, DWORD, DWORD, LPHSERVICE, HWND, DWORD, LPWFSVERSION, LPWFSVERSION, LPREQUESTID) -> HRESULT>,
    WFSRegister: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, DWORD, HWND) -> HRESULT>,
    WFSAsyncRegister: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, DWORD, HWND, HWND, LPREQUESTID) -> HRESULT>,
    WFSSetBlockingHook: Symbol<'a, unsafe extern "stdcall" fn(XFSBLOCKINGHOOK, *mut XFSBLOCKINGHOOK) -> HRESULT>,
    WFSStartUp: Symbol<'a, unsafe extern "stdcall" fn(DWORD, LPWFSVERSION) -> HRESULT>,
    WFSUnhookBlockingHook: Symbol<'a, unsafe extern "stdcall" fn() -> HRESULT>,
    WFSUnlock: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE) -> HRESULT>,
    WFSAsyncUnlock: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, HWND, LPREQUESTID) -> HRESULT>,
    WFMAllocateBuffer: Symbol<'a, unsafe extern "stdcall" fn(ULONG, ULONG, *mut LPVOID) -> HRESULT>,
    WFMAllocateMore: Symbol<'a, unsafe extern "stdcall" fn(ULONG, LPVOID, *mut LPVOID) -> HRESULT>,
    WFMFreeBuffer: Symbol<'a, unsafe extern "stdcall" fn(LPVOID) -> HRESULT>,
    WFMGetTraceLevel: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, LPDWORD) -> HRESULT>,
    WFMKillTimer: Symbol<'a, unsafe extern "stdcall" fn(WORD) -> HRESULT>,
    WFMOutputTraceData: Symbol<'a, unsafe extern "stdcall" fn(LPSTR) -> HRESULT>,
    WFMReleaseDLL: Symbol<'a, unsafe extern "stdcall" fn(HPROVIDER) -> HRESULT>,
    WFMSetTimer: Symbol<'a, unsafe extern "stdcall" fn(HWND, LPVOID, DWORD, LPWORD) -> HRESULT>,
    WFMSetTraceLevel: Symbol<'a, unsafe extern "stdcall" fn(HSERVICE, DWORD) -> HRESULT>,
}

fn init_log() {
    let logfile = FileAppender::builder().build("output.log").unwrap();
    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Trace))
        .unwrap();

    log4rs::init_config(config).unwrap();
}

pub type WfpOpen = extern "stdcall" fn(
    hService: HSERVICE,
    lpszLogicalName: LPSTR,
    hApp: HAPP,
    lpszAppID: LPSTR,
    dwTraceLevel: DWORD,
    dwTimeOut: DWORD,
    hWnd: HWND,
    reqId: REQUESTID,
    hProvider: HPROVIDER,
    dwSPIVersionsRequired: DWORD,
    lpSPIVersion: LPWFSVERSION,
    dwSrvcVersionsRequired: DWORD,
    lpSrvcVersion: LPWFSVERSION,
) -> HRESULT;

mod window;

fn main() {
    unsafe {
        let lib = libloading::Library::new(r"C:\Users\bloc4\Downloads\OpenXFS_V0.0.0.5\dll\SPI_LQ2090.dll").unwrap();
        let open = lib.get::<WfpOpen>(b"WFPOpen").unwrap();
        let mut lp_spi_version = WFSVERSION {
            w_version: 0,
            w_low_version: 0,
            w_high_version: 0,
            sz_description: [0; 257],
            sz_system_status: [0; 257],
        };
        let mut lp_srvc_version = WFSVERSION {
            w_version: 0,
            w_low_version: 0,
            w_high_version: 0,
            sz_description: [0; 257],
            sz_system_status: [0; 257],
        };
        let window = window::SyncWindow::new(0x401);
        let result = open(
            32134,
            "LQ2090\0".as_ptr() as LPSTR,
            0 as HAPP,
            "LQ2090\0".as_ptr() as LPSTR,
            0,
            0,
            window.handle(),
            666666666,
            std::ptr::null_mut(),
            0,
            &mut lp_spi_version,
            0,
            &mut lp_srvc_version,
        );
        println!("{:?}", result);

        loop {
            if let Some(result) = window.try_receive().unwrap() {
                let wfs_result = result as *const WFSRESULT;
                let wfs_result = std::mem::transmute::<*const WFSRESULT, &WFSRESULT>(wfs_result);
                assert_eq!(-54, ptr::addr_of!(wfs_result.hResult).read_unaligned());
                // let wfs_result = result as *mut WFSRESULT;
                // let wfs_result = &*wfs_result;

                // let brw = std::ptr::addr_of!(wfs_result.RequestID);
                // let brw2 = std::ptr::addr_of!(wfs_result.hResult);
                // println!("reques {:#034b}; {}", *brw, *brw);
                // println!("result {:#034b}; {}", *brw2, *brw2);
                // println!("expect {:#034b}", -54);
                // println!("{:?}", size_of::<SYSTEMTIME>());

                // let hmm = result as *mut u8;
                // let ptr = hmm.add(size_of::<ULONG>()).add(size_of::<HSERVICE>()).add(size_of::<SYSTEMTIME>()).add(2);
                // println!("ptr    {:#034b}", *(ptr as *mut HRESULT));

                // let another = result as *mut [u8; size_of::<WFSRESULT>()];
                // let work: WFSRESULT = std::mem::transmute(*another);
                // println!("hmm    {:#034b}; {}", work.hResult, work.hResult);

                // let bytes = result as *mut [u8; 36];
                // for byte in &*bytes {
                //     println!("{:#010b}", byte);
                // }

                break;
            }
        }
    }
    // init_log();
    // let s: &str = "123";
    // let ptr: *const u8 = s.as_ptr();

    // unsafe {
    //     println!("{}", *ptr.add(1) as char);
    //     println!("{}", *ptr.add(2) as char);
    //     println!("{}", *ptr as char);
    // }
}

// 0xFFFFFFEA = 11111111 11111111 11111111 11101010
// 0xFFFFFFFC = 11111111 11111111 11111111 11111100
// res 0x0000FFFF = 00000000 00000000 11111111 11111111

unsafe fn test_buffers() {
    let lib = libloading::Library::new("msxfs.dll").unwrap();
    let allocate = lib.get::<unsafe extern "stdcall" fn(ULONG, ULONG, *mut LPVOID) -> HRESULT>(b"WFMAllocateBuffer").unwrap();
    let allocate_more = lib.get::<unsafe extern "stdcall" fn(ULONG, LPVOID, *mut LPVOID) -> HRESULT>(b"WFMAllocateMore").unwrap();
    let free_buffer = lib.get::<unsafe extern "stdcall" fn(LPVOID) -> HRESULT>(b"WFMFreeBuffer").unwrap();

    let mut buffer: LPVOID = ptr::null_mut();
    let result = allocate(100, 0, &mut buffer);

    println!("{:?} {:?}", result, buffer);

    let mut more_buffer: LPVOID = ptr::null_mut();
    let result = allocate_more(100, buffer, &mut more_buffer);

    println!("{:?} {:?}", result, more_buffer);

    let result = free_buffer(buffer);

    println!("{:?}", result);
}

#[allow(non_snake_case)]
unsafe fn test() {
    let lib = libloading::Library::new("msxfs.dll").unwrap();
    let api = XFSApi {
        WFSCancelAsyncRequest: lib.get(b"WFSCancelAsyncRequest").unwrap(),
        WFSCancelBlockingCall: lib.get(b"WFSCancelBlockingCall").unwrap(),
        WFSCleanUp: lib.get(b"WFSCleanUp").unwrap(),
        WFSClose: lib.get(b"WFSClose").unwrap(),
        WFSAsyncClose: lib.get(b"WFSAsyncClose").unwrap(),
        WFSCreateAppHandle: lib.get(b"WFSCreateAppHandle").unwrap(),
        WFSDeregister: lib.get(b"WFSDeregister").unwrap(),
        WFSAsyncDeregister: lib.get(b"WFSAsyncDeregister").unwrap(),
        WFSDestroyAppHandle: lib.get(b"WFSDestroyAppHandle").unwrap(),
        WFSExecute: lib.get(b"WFSExecute").unwrap(),
        WFSAsyncExecute: lib.get(b"WFSAsyncExecute").unwrap(),
        WFSFreeResult: lib.get(b"WFSFreeResult").unwrap(),
        WFSGetInfo: lib.get(b"WFSGetInfo").unwrap(),
        WFSAsyncGetInfo: lib.get(b"WFSAsyncGetInfo").unwrap(),
        WFSIsBlocking: lib.get(b"WFSIsBlocking").unwrap(),
        WFSLock: lib.get(b"WFSLock").unwrap(),
        WFSAsyncLock: lib.get(b"WFSAsyncLock").unwrap(),
        WFSOpen: lib.get(b"WFSOpen").unwrap(),
        WFSAsyncOpen: lib.get(b"WFSAsyncOpen").unwrap(),
        WFSRegister: lib.get(b"WFSRegister").unwrap(),
        WFSAsyncRegister: lib.get(b"WFSAsyncRegister").unwrap(),
        WFSSetBlockingHook: lib.get(b"WFSSetBlockingHook").unwrap(),
        WFSStartUp: lib.get(b"WFSStartUp").unwrap(),
        WFSUnhookBlockingHook: lib.get(b"WFSUnhookBlockingHook").unwrap(),
        WFSUnlock: lib.get(b"WFSUnlock").unwrap(),
        WFSAsyncUnlock: lib.get(b"WFSAsyncUnlock").unwrap(),
        WFMAllocateBuffer: lib.get(b"WFMAllocateBuffer").unwrap(),
        WFMAllocateMore: lib.get(b"WFMAllocateMore").unwrap(),
        WFMFreeBuffer: lib.get(b"WFMFreeBuffer").unwrap(),
        WFMGetTraceLevel: lib.get(b"WFMGetTraceLevel").unwrap(),
        WFMKillTimer: lib.get(b"WFMKillTimer").unwrap(),
        WFMOutputTraceData: lib.get(b"WFMOutputTraceData").unwrap(),
        WFMReleaseDLL: lib.get(b"WFMReleaseDLL").unwrap(),
        WFMSetTimer: lib.get(b"WFMSetTimer").unwrap(),
        WFMSetTraceLevel: lib.get(b"WFMSetTraceLevel").unwrap(),
    };

    let mut version = WFSVERSION {
        w_version: 0,
        w_low_version: 0,
        w_high_version: 0,
        sz_description: [0; WFSDDESCRIPTION_LEN + 1],
        sz_system_status: [0; WFSDSYSSTATUS_LEN + 1],
    };
    let result = (api.WFSStartUp)(3, &mut version);
    println!("{}, {:?}", result, version);

    let lpSrvcVersion: LPWFSVERSION = &mut WFSVERSION {
        w_version: 0,
        w_low_version: 0,
        w_high_version: 0,
        sz_description: [0; WFSDDESCRIPTION_LEN + 1],
        sz_system_status: [0; WFSDSYSSTATUS_LEN + 1],
    };
    let lpSPIVersion: LPWFSVERSION = &mut WFSVERSION {
        w_version: 0,
        w_low_version: 0,
        w_high_version: 0,
        sz_description: [0; WFSDDESCRIPTION_LEN + 1],
        sz_system_status: [0; WFSDSYSSTATUS_LEN + 1],
    };
    let mut lphService: HSERVICE = 0;
    let service = CString::new("cwd").unwrap();
    let app_id = CString::new("cwd").unwrap();
    let result = (api.WFSOpen)(
        service.as_ptr() as *mut i8,
        1 as *mut c_void,
        app_id.as_ptr() as *mut i8,
        0,
        0,
        3,
        lpSrvcVersion,
        lpSPIVersion,
        &mut lphService,
    );
    println!("{}, {:?}, {:?}, {:?}", result, lpSrvcVersion, lpSPIVersion, lphService);
}
