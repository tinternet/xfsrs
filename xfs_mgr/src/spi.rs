use winapi::{
    shared::{minwindef::DWORD, windef::HWND, winerror::HRESULT},
    um::winnt::LPSTR,
};
use xfslib::{HAPP, HPROVIDER, HSERVICE, LPWFSVERSION, REQUESTID};

pub type WfpOpen = unsafe extern "stdcall" fn(
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
