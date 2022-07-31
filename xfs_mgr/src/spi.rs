use libloading::{Library, Symbol};
use winapi::{
    shared::{
        minwindef::{DWORD, LPVOID},
        windef::HWND,
        winerror::HRESULT,
    },
    um::winnt::LPSTR,
};
use xfslib::{xfs_unwrap, HAPP, HPROVIDER, HSERVICE, LPREQUESTID, LPWFSVERSION, REQUESTID, WFS_ERR_INTERNAL_ERROR};

pub type WfpCancelAsyncRequest = extern "stdcall" fn(hService: HSERVICE, RequestId: REQUESTID) -> HRESULT;
pub type WFPClose = extern "stdcall" fn(hService: HSERVICE, hWnd: HWND, ReqID: REQUESTID) -> HRESULT;
pub type WFPDeregister = extern "stdcall" fn(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, ReqID: REQUESTID) -> HRESULT;
pub type WFPExecute = extern "stdcall" fn(hService: HSERVICE, dwCommand: DWORD, lpCmdData: LPVOID, dwTimeOut: DWORD, hWnd: HWND, ReqID: REQUESTID) -> HRESULT;
pub type WFPGetInfo = extern "stdcall" fn(hService: HSERVICE, dwCategory: DWORD, lpQueryDetails: LPVOID, dwTimeOut: DWORD, hWnd: HWND, ReqID: REQUESTID) -> HRESULT;
pub type WFPLock = extern "stdcall" fn(hService: HSERVICE, dwTimeOut: DWORD, hWnd: HWND, ReqID: REQUESTID) -> HRESULT;
pub type WFPRegister = extern "stdcall" fn(hService: HSERVICE, dwEventClass: DWORD, hWndReg: HWND, hWnd: HWND, ReqID: REQUESTID) -> HRESULT;
pub type WFPSetTraceLevel = extern "stdcall" fn(hService: HSERVICE, dwTraceLevel: DWORD) -> HRESULT;
#[allow(dead_code)]
pub type WFPUnloadService = extern "stdcall" fn() -> HRESULT;
pub type WFPUnlock = extern "stdcall" fn(hService: HSERVICE, hWnd: HWND, ReqID: REQUESTID) -> HRESULT;
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
