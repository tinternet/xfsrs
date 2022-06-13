use lazy_static::lazy_static;
use libloading::Symbol;
use log::{trace, LevelFilter};
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    Config,
};
use winapi::{
    shared::{
        minwindef::{DWORD, HINSTANCE, LPDWORD, LPVOID, LPWORD, ULONG, WORD},
        windef::HWND,
        winerror::HRESULT,
    },
    um::winnt::{DLL_PROCESS_ATTACH, LPSTR},
};
use xfslib::*;

#[allow(non_snake_case)]
#[no_mangle]
pub extern "stdcall" fn DllMain(_hinst_dll: HINSTANCE, fdw_reason: DWORD, _: LPVOID) -> bool {
    if fdw_reason == DLL_PROCESS_ATTACH {
        let logfile = FileAppender::builder().build("C:\\XFS_TRACES.txt").unwrap();
        let config = Config::builder()
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
            .build(Root::builder().appender("logfile").build(LevelFilter::Trace))
            .unwrap();

        log4rs::init_config(config).unwrap();
    }
    true
}

#[allow(non_snake_case)]
struct XFSApi {
    WFSCancelAsyncRequest: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, REQUESTID) -> HRESULT>,
    WFSCancelBlockingCall: Symbol<'static, unsafe extern "stdcall" fn(DWORD) -> HRESULT>,
    WFSCleanUp: Symbol<'static, unsafe extern "stdcall" fn() -> HRESULT>,
    WFSClose: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE) -> HRESULT>,
    WFSAsyncClose: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, HWND, LPREQUESTID) -> HRESULT>,
    WFSCreateAppHandle: Symbol<'static, unsafe extern "stdcall" fn(LPHAPP) -> HRESULT>,
    WFSDeregister: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, DWORD, HWND) -> HRESULT>,
    WFSAsyncDeregister: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, DWORD, HWND, HWND, LPREQUESTID) -> HRESULT>,
    WFSDestroyAppHandle: Symbol<'static, unsafe extern "stdcall" fn(HAPP) -> HRESULT>,
    WFSExecute: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, DWORD, LPVOID, DWORD, *mut LPWFSRESULT) -> HRESULT>,
    WFSAsyncExecute: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, DWORD, LPVOID, DWORD, HWND, LPREQUESTID) -> HRESULT>,
    WFSFreeResult: Symbol<'static, unsafe extern "stdcall" fn(LPWFSRESULT) -> HRESULT>,
    WFSGetInfo: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, DWORD, LPVOID, DWORD, *mut LPWFSRESULT) -> HRESULT>,
    WFSAsyncGetInfo: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, DWORD, LPVOID, DWORD, HWND, LPREQUESTID) -> HRESULT>,
    WFSIsBlocking: Symbol<'static, unsafe extern "stdcall" fn() -> bool>,
    WFSLock: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, DWORD, *mut LPWFSRESULT) -> HRESULT>,
    WFSAsyncLock: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, DWORD, HWND, LPREQUESTID) -> HRESULT>,
    WFSOpen: Symbol<'static, unsafe extern "stdcall" fn(LPSTR, HAPP, LPSTR, DWORD, DWORD, DWORD, LPWFSVERSION, LPWFSVERSION, LPHSERVICE) -> HRESULT>,
    WFSAsyncOpen: Symbol<'static, unsafe extern "stdcall" fn(LPSTR, HAPP, LPSTR, DWORD, DWORD, LPHSERVICE, HWND, DWORD, LPWFSVERSION, LPWFSVERSION, LPREQUESTID) -> HRESULT>,
    WFSRegister: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, DWORD, HWND) -> HRESULT>,
    WFSAsyncRegister: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, DWORD, HWND, HWND, LPREQUESTID) -> HRESULT>,
    WFSSetBlockingHook: Symbol<'static, unsafe extern "stdcall" fn(XFSBLOCKINGHOOK, LPXFSBLOCKINGHOOK) -> HRESULT>,
    WFSStartUp: Symbol<'static, unsafe extern "stdcall" fn(DWORD, LPWFSVERSION) -> HRESULT>,
    WFSUnhookBlockingHook: Symbol<'static, unsafe extern "stdcall" fn() -> HRESULT>,
    WFSUnlock: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE) -> HRESULT>,
    WFSAsyncUnlock: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, HWND, LPREQUESTID) -> HRESULT>,
    WFMAllocateBuffer: Symbol<'static, unsafe extern "stdcall" fn(ULONG, ULONG, *mut LPVOID) -> HRESULT>,
    WFMAllocateMore: Symbol<'static, unsafe extern "stdcall" fn(ULONG, LPVOID, *mut LPVOID) -> HRESULT>,
    WFMFreeBuffer: Symbol<'static, unsafe extern "stdcall" fn(LPVOID) -> HRESULT>,
    WFMGetTraceLevel: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, LPDWORD) -> HRESULT>,
    WFMKillTimer: Symbol<'static, unsafe extern "stdcall" fn(WORD) -> HRESULT>,
    WFMOutputTraceData: Symbol<'static, unsafe extern "stdcall" fn(LPSTR) -> HRESULT>,
    WFMReleaseDLL: Symbol<'static, unsafe extern "stdcall" fn(HPROVIDER) -> HRESULT>,
    WFMSetTimer: Symbol<'static, unsafe extern "stdcall" fn(HWND, LPVOID, DWORD, LPWORD) -> HRESULT>,
    WFMSetTraceLevel: Symbol<'static, unsafe extern "stdcall" fn(HSERVICE, DWORD) -> HRESULT>,
}

lazy_static! {
    static ref XFS_LIB: libloading::Library = unsafe {
        let lib = libloading::Library::new("xfs.dll").unwrap();
        lib
    };
    static ref XFS: XFSApi = unsafe {
        XFSApi {
            WFSCancelAsyncRequest: XFS_LIB.get(b"WFSCancelAsyncRequest").unwrap(),
            WFSCancelBlockingCall: XFS_LIB.get(b"WFSCancelBlockingCall").unwrap(),
            WFSCleanUp: XFS_LIB.get(b"WFSCleanUp").unwrap(),
            WFSClose: XFS_LIB.get(b"WFSClose").unwrap(),
            WFSAsyncClose: XFS_LIB.get(b"WFSAsyncClose").unwrap(),
            WFSCreateAppHandle: XFS_LIB.get(b"WFSCreateAppHandle").unwrap(),
            WFSDeregister: XFS_LIB.get(b"WFSDeregister").unwrap(),
            WFSAsyncDeregister: XFS_LIB.get(b"WFSAsyncDeregister").unwrap(),
            WFSDestroyAppHandle: XFS_LIB.get(b"WFSDestroyAppHandle").unwrap(),
            WFSExecute: XFS_LIB.get(b"WFSExecute").unwrap(),
            WFSAsyncExecute: XFS_LIB.get(b"WFSAsyncExecute").unwrap(),
            WFSFreeResult: XFS_LIB.get(b"WFSFreeResult").unwrap(),
            WFSGetInfo: XFS_LIB.get(b"WFSGetInfo").unwrap(),
            WFSAsyncGetInfo: XFS_LIB.get(b"WFSAsyncGetInfo").unwrap(),
            WFSIsBlocking: XFS_LIB.get(b"WFSIsBlocking").unwrap(),
            WFSLock: XFS_LIB.get(b"WFSLock").unwrap(),
            WFSAsyncLock: XFS_LIB.get(b"WFSAsyncLock").unwrap(),
            WFSOpen: XFS_LIB.get(b"WFSOpen").unwrap(),
            WFSAsyncOpen: XFS_LIB.get(b"WFSAsyncOpen").unwrap(),
            WFSRegister: XFS_LIB.get(b"WFSRegister").unwrap(),
            WFSAsyncRegister: XFS_LIB.get(b"WFSAsyncRegister").unwrap(),
            WFSSetBlockingHook: XFS_LIB.get(b"WFSSetBlockingHook").unwrap(),
            WFSStartUp: XFS_LIB.get(b"WFSStartUp").unwrap(),
            WFSUnhookBlockingHook: XFS_LIB.get(b"WFSUnhookBlockingHook").unwrap(),
            WFSUnlock: XFS_LIB.get(b"WFSUnlock").unwrap(),
            WFSAsyncUnlock: XFS_LIB.get(b"WFSAsyncUnlock").unwrap(),
            WFMAllocateBuffer: XFS_LIB.get(b"WFMAllocateBuffer").unwrap(),
            WFMAllocateMore: XFS_LIB.get(b"WFMAllocateMore").unwrap(),
            WFMFreeBuffer: XFS_LIB.get(b"WFMFreeBuffer").unwrap(),
            WFMGetTraceLevel: XFS_LIB.get(b"WFMGetTraceLevel").unwrap(),
            WFMKillTimer: XFS_LIB.get(b"WFMKillTimer").unwrap(),
            WFMOutputTraceData: XFS_LIB.get(b"WFMOutputTraceData").unwrap(),
            WFMReleaseDLL: XFS_LIB.get(b"WFMReleaseDLL").unwrap(),
            WFMSetTimer: XFS_LIB.get(b"WFMSetTimer").unwrap(),
            WFMSetTraceLevel: XFS_LIB.get(b"WFMSetTraceLevel").unwrap(),
        }
    };
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSCancelAsyncRequest(hService: HSERVICE, RequestID: REQUESTID) -> HRESULT {
    trace!("WFSCancelAsyncRequest: {}, {}", hService, RequestID);
    (XFS.WFSCancelAsyncRequest)(hService, RequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSCancelBlockingCall(dwThreadID: DWORD) -> HRESULT {
    trace!("WFSCancelBlockingCall: {}", dwThreadID);
    (XFS.WFSCancelBlockingCall)(dwThreadID)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSCleanUp() -> HRESULT {
    trace!("WFSCleanUp");
    (XFS.WFSCleanUp)()
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSClose(hService: HSERVICE) -> HRESULT {
    trace!("WFSClose: {}", hService);
    (XFS.WFSClose)(hService)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSAsyncClose(hService: HSERVICE, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncClose: {}", hService);
    (XFS.WFSAsyncClose)(hService, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSCreateAppHandle(lphApp: LPHAPP) -> HRESULT {
    trace!("WFSCreateAppHandle");
    let result = (XFS.WFSCreateAppHandle)(lphApp);
    trace!("WFSCreateAppHandle: {}, handle: {:?}, handlev: {:?}", result, *lphApp, **lphApp);
    result
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSDeregister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND) -> HRESULT {
    trace!("WFSDeregister");
    (XFS.WFSDeregister)(hService, dwEventClass, hWndReg)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSAsyncDeregister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncDeregister");
    (XFS.WFSAsyncDeregister)(hService, dwEventClass, hWndReg, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSDestroyAppHandle(hApp: HAPP) -> HRESULT {
    trace!("WFSDestroyAppHandle");
    (XFS.WFSDestroyAppHandle)(hApp)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSExecute(hService: HSERVICE, dwCommand: DWORD, lpCmdData: LPVOID, dwTimeOut: DWORD, lppResult: *mut LPWFSRESULT) -> HRESULT {
    trace!("WFSExecute");
    (XFS.WFSExecute)(hService, dwCommand, lpCmdData, dwTimeOut, lppResult)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSAsyncExecute(hService: HSERVICE, dwCommand: DWORD, lpCmdData: LPVOID, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncExecute");
    (XFS.WFSAsyncExecute)(hService, dwCommand, lpCmdData, dwTimeOut, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSFreeResult(lpResult: LPWFSRESULT) -> HRESULT {
    trace!("WFSFreeResult");
    (XFS.WFSFreeResult)(lpResult)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSGetInfo(hService: HSERVICE, dwCategory: DWORD, lpQueryDetails: LPVOID, dwTimeOut: DWORD, lppResult: *mut LPWFSRESULT) -> HRESULT {
    trace!(
        "WFSGetInfo CAL: hService: {}, dwCategory: {}, lpQueryDetails: {:?}, dwTimeOut: {}, lppResult: {:?}",
        hService,
        dwCategory,
        *lpQueryDetails,
        dwTimeOut,
        *lppResult,
    );
    // CreateWindowEx();
    // let a = HWND_MESSAGE;
    (XFS.WFSGetInfo)(hService, dwCategory, lpQueryDetails, dwTimeOut, lppResult)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSAsyncGetInfo(hService: HSERVICE, dwCategory: DWORD, lpQueryDetails: LPVOID, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!(
        "WFSAsyncGetInfo CAL: hService: {}, dwCategory: {}, lpQueryDetails: {:?}, dwTimeOut: {}, lpRequestID: {:?}",
        hService,
        dwCategory,
        *lpQueryDetails,
        dwTimeOut,
        lpRequestID
    );

    (XFS.WFSAsyncGetInfo)(hService, dwCategory, lpQueryDetails, dwTimeOut, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSIsBlocking() -> bool {
    trace!("WFSIsBlocking");
    (XFS.WFSIsBlocking)()
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSLock(hService: HSERVICE, dwTimeOut: DWORD, lppResult: *mut LPWFSRESULT) -> HRESULT {
    trace!("WFSLock");
    (XFS.WFSLock)(hService, dwTimeOut, lppResult)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSAsyncLock(hService: HSERVICE, dwTimeOut: DWORD, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncLock");
    (XFS.WFSAsyncLock)(hService, dwTimeOut, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSOpen(
    lpszLogicalName: LPSTR,
    hApp: HAPP,
    lpszAppID: LPSTR,
    dwTraceLevel: DWORD,
    dwTimeOut: DWORD,
    dwSrvcVersionsRequired: DWORD,
    lpSrvcVersion: LPWFSVERSION,
    lpSPIVersion: LPWFSVERSION,
    lphService: LPHSERVICE,
) -> HRESULT {
    trace!(
        "WFSOpen CAL: lpszLogicalName: {:?}, hApp: {:?}, lpszAppID: {:?}, dwTraceLevel: {}, dwTimeOut: {}, dwSrvcVersionsRequired: {}, lpSrvcVersion: {:?}, lpSPIVersion: {:?}, lphService: {:?}",
        *lpszLogicalName,
        hApp,
        *lpszAppID,
        dwTraceLevel,
        dwTimeOut,
        dwSrvcVersionsRequired,
        *lpSrvcVersion,
        *lpSPIVersion,
        *lphService
    );
    let res = (XFS.WFSOpen)(
        lpszLogicalName,
        hApp,
        lpszAppID,
        dwTraceLevel,
        dwTimeOut,
        dwSrvcVersionsRequired,
        lpSrvcVersion,
        lpSPIVersion,
        lphService,
    );
    trace!(
        "WFSOpen RES: lpszLogicalName: {:?}, hApp: {:?}, lpszAppID: {:?}, dwTraceLevel: {}, dwTimeOut: {}, dwSrvcVersionsRequired: {}, lpSrvcVersion: {:?}, lpSPIVersion: {:?}, lphService: {:?}",
        *lpszLogicalName,
        hApp,
        *lpszAppID,
        dwTraceLevel,
        dwTimeOut,
        dwSrvcVersionsRequired,
        *lpSrvcVersion,
        *lpSPIVersion,
        *lphService
    );
    res
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSAsyncOpen(
    lpszLogicalName: LPSTR,
    hApp: HAPP,
    lpszAppID: LPSTR,
    dwTraceLevel: DWORD,
    dwTimeOut: DWORD,
    lphService: LPHSERVICE,
    hWnd: HWND,
    dwSrvcVersionsRequired: DWORD,
    lpSrvcVersion: LPWFSVERSION,
    lpSPIVersion: LPWFSVERSION,
    lpRequestID: LPREQUESTID,
) -> HRESULT {
    trace!("WFSAsyncOpen");
    (XFS.WFSAsyncOpen)(
        lpszLogicalName,
        hApp,
        lpszAppID,
        dwTraceLevel,
        dwTimeOut,
        lphService,
        hWnd,
        dwSrvcVersionsRequired,
        lpSrvcVersion,
        lpSPIVersion,
        lpRequestID,
    )
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSRegister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND) -> HRESULT {
    trace!("WFSRegister");
    (XFS.WFSRegister)(hService, dwEventClass, hWndReg)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSAsyncRegister(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncRegister");
    (XFS.WFSAsyncRegister)(hService, dwEventClass, hWndReg, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSSetBlockingHook(lpBlockFunc: XFSBLOCKINGHOOK, lppPrevFunc: LPXFSBLOCKINGHOOK) -> HRESULT {
    trace!("WFSSetBlockingHook");
    (XFS.WFSSetBlockingHook)(lpBlockFunc, lppPrevFunc)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSStartUp(dwVersionsRequired: DWORD, lpWFSVersion: LPWFSVERSION) -> HRESULT {
    trace!("WFSStartUp CAL: dwVersionsRequired: {}, lpWFSVersion: {:?}", dwVersionsRequired, *lpWFSVersion);
    let res = (XFS.WFSStartUp)(dwVersionsRequired, lpWFSVersion);
    trace!("WFSStartUp RES: dwVersionsRequired: {}, lpWFSVersion: {:?}", dwVersionsRequired, *lpWFSVersion);
    res
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSUnhookBlockingHook() -> HRESULT {
    trace!("WFSUnhookBlockingHook");
    (XFS.WFSUnhookBlockingHook)()
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSUnlock(hService: HSERVICE) -> HRESULT {
    trace!("WFSUnlock");
    (XFS.WFSUnlock)(hService)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFSAsyncUnlock(hService: HSERVICE, hWnd: HWND, lpRequestID: LPREQUESTID) -> HRESULT {
    trace!("WFSAsyncUnlock");
    (XFS.WFSAsyncUnlock)(hService, hWnd, lpRequestID)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFMAllocateBuffer(ulSize: ULONG, ulFlags: ULONG, lppvData: *mut LPVOID) -> HRESULT {
    trace!("WFMAllocateBuffer");
    (XFS.WFMAllocateBuffer)(ulSize, ulFlags, lppvData)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFMAllocateMore(ulSize: ULONG, lpvOriginal: LPVOID, lppvData: *mut LPVOID) -> HRESULT {
    trace!("WFMAllocateMore");
    (XFS.WFMAllocateMore)(ulSize, lpvOriginal, lppvData)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFMFreeBuffer(lpvData: LPVOID) -> HRESULT {
    trace!("WFMFreeBuffer");
    (XFS.WFMFreeBuffer)(lpvData)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFMGetTraceLevel(hService: HSERVICE, lpdwTraceLevel: LPDWORD) -> HRESULT {
    trace!("WFMGetTraceLevel");
    (XFS.WFMGetTraceLevel)(hService, lpdwTraceLevel)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFMKillTimer(wTimerID: WORD) -> HRESULT {
    trace!("WFMKillTimer");
    (XFS.WFMKillTimer)(wTimerID)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFMOutputTraceData(lpszData: LPSTR) -> HRESULT {
    trace!("WFMOutputTraceData");
    (XFS.WFMOutputTraceData)(lpszData)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFMReleaseDLL(hProvider: HPROVIDER) -> HRESULT {
    trace!("WFMReleaseDLL");
    (XFS.WFMReleaseDLL)(hProvider)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFMSetTimer(hWnd: HWND, lpContext: LPVOID, dwTimeVal: DWORD, lpwTimerID: LPWORD) -> HRESULT {
    trace!("WFMSetTimer");
    (XFS.WFMSetTimer)(hWnd, lpContext, dwTimeVal, lpwTimerID)
}

#[allow(non_snake_case)]
#[no_mangle]
pub unsafe extern "stdcall" fn WFMSetTraceLevel(hService: HSERVICE, dwTraceLevel: DWORD) -> HRESULT {
    trace!("WFMSetTraceLevel");
    (XFS.WFMSetTraceLevel)(hService, dwTraceLevel)
}
