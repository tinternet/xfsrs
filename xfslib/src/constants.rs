use winapi::{
    shared::minwindef::{HKEY, UINT},
    um::winuser::WM_USER,
};

pub const WFSDDESCRIPTION_LEN: usize = 256;
pub const WFSDSYSSTATUS_LEN: usize = 256;

/******* Value of hKey *******************************************************/
pub const WFS_CFG_HKEY_XFS_ROOT: HKEY = 1 as HKEY;
pub const WFS_CFG_HKEY_MACHINE_XFS_ROOT: HKEY = 2 as HKEY;
pub const WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT: HKEY = 3 as HKEY;

/******* Values of lpdwDisposition *******************************************/
pub const WFS_CFG_CREATED_NEW_KEY: u32 = 0;
pub const WFS_CFG_OPENED_EXISTING_KEY: u32 = 1;

/****** Messages ********************************************************/

/* Message-No = (WM_USER + No) */

pub const WFS_OPEN_COMPLETE: UINT = WM_USER + 1;
pub const WFS_CLOSE_COMPLETE: UINT = WM_USER + 2;
pub const WFS_LOCK_COMPLETE: UINT = WM_USER + 3;
pub const WFS_UNLOCK_COMPLETE: UINT = WM_USER + 4;
pub const WFS_REGISTER_COMPLETE: UINT = WM_USER + 5;
pub const WFS_DEREGISTER_COMPLETE: UINT = WM_USER + 6;
pub const WFS_GETINFO_COMPLETE: UINT = WM_USER + 7;
pub const WFS_EXECUTE_COMPLETE: UINT = WM_USER + 8;

pub const WFS_EXECUTE_EVENT: UINT = WM_USER + 20;
pub const WFS_SERVICE_EVENT: UINT = WM_USER + 21;
pub const WFS_USER_EVENT: UINT = WM_USER + 22;
pub const WFS_SYSTEM_EVENT: UINT = WM_USER + 23;

pub const WFS_TIMER_EVENT: UINT = WM_USER + 100;
