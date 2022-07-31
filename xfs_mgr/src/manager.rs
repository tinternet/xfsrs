use std::{
    ffi::{CStr, CString},
    ptr,
    sync::Mutex,
};

use winapi::{
    shared::{
        minwindef::{DWORD, HKEY, LPDWORD, LPVOID, MAX_PATH},
        windef::HWND,
        winerror::HRESULT,
    },
    um::winnt::LPSTR,
};
use xfslib::*;

use crate::service::Service;

lazy_static::lazy_static! {
    // holds service handles
    static ref SERVICES: Mutex<Vec<Option<Service>>> = Mutex::new((0..8192).map(|_| None).collect());

    // holds app handles
    static ref APP_HANDLES: Mutex<[bool; 8192]> = Mutex::new([false; 8192]);
}

macro_rules! services {
    () => {
        SERVICES.lock().unwrap_or_else(|e| e.into_inner())
    };
}

macro_rules! app_handles {
    () => {
        APP_HANDLES.lock().unwrap_or_else(|e| e.into_inner())
    };
}

pub fn clean_up() -> HRESULT {
    services!().iter_mut().for_each(|s| *s = None);
    app_handles!().iter_mut().for_each(|b| *b = false);
    WFS_SUCCESS
}

pub fn create_app_handle(lph_app: LPHAPP) -> HRESULT {
    let mut handles = app_handles!();
    let index = match handles.iter().position(|h| !h) {
        Some(index) => index,
        None => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    };

    handles[index] = true;
    let ptr = (&*handles) as *const _ as usize + index;

    unsafe { lph_app.write(ptr as HAPP) };
    WFS_SUCCESS
}

pub fn destroy_app_handle(h_app: HAPP) -> HRESULT {
    if h_app.is_null() {
        return WFS_ERR_INVALID_APP_HANDLE;
    }

    let mut handles = app_handles!();
    let ptr = (&*handles) as *const _ as usize;
    let index = h_app as usize - ptr;

    match handles.get_mut(index) {
        Some(h) => *h = false,
        None => xfs_reject!(WFS_ERR_INVALID_APP_HANDLE),
    }

    WFS_SUCCESS
}

pub fn get_trace_level(h_service: HSERVICE, lpdw_trace_level: LPDWORD) -> HRESULT {
    let services = services!();
    if let Some(service) = services.get(h_service as usize - 1).and_then(|service| service.as_ref()) {
        unsafe { lpdw_trace_level.write(service.get_trace_level()) };
        return WFS_SUCCESS;
    }
    xfs_reject!(WFS_ERR_INVALID_HSERVICE);
}

pub fn start_up(_dw_versions_required: DWORD, lp_wfsversion: LPWFSVERSION) -> HRESULT {
    // TODO: debug why diebold works with pointer here
    // let range = VersionRange::new(dwVersionsRequired);
    // if range.start > range.end {
    //     xfs_reject!(WFS_ERR_INTERNAL_ERROR);
    // }
    // if range.start > Version::new_explicit(3, 30) {
    //     xfs_reject!(WFS_ERR_API_VER_TOO_HIGH);
    // }
    // if range.end < Version::new_explicit(2, 00) {
    //     xfs_reject!(WFS_ERR_API_VER_TOO_LOW);
    // }
    // if lpWFSVersion.is_null() {
    //     xfs_reject!(WFS_ERR_INVALID_POINTER);
    // }

    if !lp_wfsversion.is_null() {
        let description = "Rust XFS Manager v2.00 to v3.30".as_bytes();
        let mut sz_description = [0i8; WFSDDESCRIPTION_LEN + 1];
        for i in 0..description.len() {
            sz_description[i] = description[i] as i8;
        }
        let version = WFSVERSION {
            w_version: Version::new_explicit(3, 0).value(),
            w_low_version: Version::new_explicit(2, 0).value(),
            w_high_version: Version::new_explicit(3, 30).value(),
            sz_description,
            sz_system_status: [0; WFSDSYSSTATUS_LEN + 1],
        };
        unsafe { lp_wfsversion.write(version) };
    }
    WFS_SUCCESS
}

pub fn release_dll(h_provider: HPROVIDER) -> HRESULT {
    let mut services = services!();
    let service_handle = (&*services) as *const _ as usize;
    let index = h_provider as usize - service_handle;
    services[index] = None;
    WFS_SUCCESS
}

pub fn set_trace_level(h_service: HSERVICE, dw_trace_level: DWORD) -> HRESULT {
    if h_service == 0 {
        xfs_reject!(WFS_ERR_INVALID_HSERVICE);
    }
    let mut services = services!();
    let service = match services.get_mut(h_service as usize - 1) {
        Some(Some(service)) => service,
        _ => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
    };
    unsafe { service.set_trace_level(dw_trace_level) }
}

pub fn open_service(
    logical_name: LPSTR,
    app: HAPP,
    app_id: LPSTR,
    trace_level: DWORD,
    time_out: DWORD,
    service: LPHSERVICE,
    wnd: HWND,
    srvc_versions_required: DWORD,
    srvc_version: LPWFSVERSION,
    spiversion: LPWFSVERSION,
    request_id: LPREQUESTID,
) -> HRESULT {
    if logical_name.is_null() || srvc_version.is_null() || spiversion.is_null() || service.is_null() || request_id.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    let name = xfs_unwrap!(unsafe { CStr::from_ptr(logical_name) }.to_str());
    let path = xfs_unwrap!(CString::new(format!("LOGICAL_SERVICES\\{}", name)));
    let lgl_prov_path = match get_value(WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT, path, CString::new("provider").unwrap()) {
        Ok(lgl_prov_path) => lgl_prov_path,
        Err(error) => return error,
    };

    let path = xfs_unwrap!(CString::new(format!("SERVICE_PROVIDERS\\{}", lgl_prov_path)));
    let phy_prov_path = match get_value(WFS_CFG_HKEY_MACHINE_XFS_ROOT, path, CString::new("dllname").unwrap()) {
        Ok(phy_prov_path) => phy_prov_path,
        Err(error) => return error,
    };

    let mut services = services!();
    let service_index = match services.iter().position(|s| s.is_none()) {
        Some(index) => index,
        None => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    };
    let mut service = xfs_unwrap!(unsafe { Service::new(service_index as HSERVICE, &phy_prov_path, trace_level) });

    let result = unsafe { service.open(logical_name, app, app_id, trace_level, time_out, wnd, srvc_versions_required, srvc_version, spiversion, request_id) };
    if result != WFS_SUCCESS {
        return result;
    }
    services[service_index] = Some(service);
    WFS_SUCCESS
}

pub fn close_service(h_service: HSERVICE, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
    let mut services = services!();
    let service = match services.get_mut(h_service as usize) {
        Some(Some(service)) => service,
        _ => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
    };
    let result = unsafe { service.close(wnd, request_id) };
    services[h_service as usize - 1] = None;
    result
}

pub fn register(h_service: HSERVICE, dw_event_class: DWORD, h_wnd_reg: HWND, h_wnd: HWND, req_id: LPREQUESTID) -> HRESULT {
    let mut services = services!();
    let service = match services.get_mut(h_service as usize) {
        Some(Some(service)) => service,
        _ => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
    };
    unsafe { service.register(dw_event_class, h_wnd_reg, h_wnd, req_id) }
}

pub fn deregister(h_service: HSERVICE, dw_event_class: DWORD, h_wnd_reg: HWND, h_wnd: HWND, req_id: LPREQUESTID) -> HRESULT {
    let mut services = services!();
    let service = match services.get_mut(h_service as usize) {
        Some(Some(service)) => service,
        _ => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
    };
    unsafe { service.deregister(dw_event_class, h_wnd_reg, h_wnd, req_id) }
}

pub fn execute(h_service: HSERVICE, dw_command: DWORD, lp_cmd_data: LPVOID, dw_time_out: DWORD, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
    let mut services = services!();
    let service = match services.get_mut(h_service as usize) {
        Some(Some(service)) => service,
        _ => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
    };
    unsafe { service.execute(dw_command, lp_cmd_data, dw_time_out, wnd, request_id) }
}

pub fn get_info(h_service: HSERVICE, dw_category: DWORD, lp_query_details: LPVOID, dw_time_out: DWORD, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
    let mut services = services!();
    let service = match services.get_mut(h_service as usize) {
        Some(Some(service)) => service,
        _ => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
    };
    unsafe { service.get_info(dw_category, lp_query_details, dw_time_out, wnd, request_id) }
}

pub fn lock(h_service: HSERVICE, dw_time_out: DWORD, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
    let mut services = services!();
    let service = match services.get_mut(h_service as usize) {
        Some(Some(service)) => service,
        _ => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
    };
    unsafe { service.lock(dw_time_out, wnd, request_id) }
}

pub fn unlock(h_service: HSERVICE, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
    let mut services = services!();
    let service = match services.get_mut(h_service as usize) {
        Some(Some(service)) => service,
        _ => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
    };
    unsafe { service.unlock(wnd, request_id) }
}

pub fn cancel_request(h_service: HSERVICE, request_id: REQUESTID) -> HRESULT {
    let mut services = services!();
    let service = match services.get_mut(h_service as usize) {
        Some(Some(service)) => service,
        _ => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
    };
    unsafe { service.cancel_async_request(request_id) }
}

fn get_value(root: HKEY, path: CString, name: CString) -> Result<String, HRESULT> {
    let mut key = ptr::null_mut();

    // SAFETY: the path pointer is function argument, and it is not null
    unsafe {
        if registry::open_key(root, path.as_ptr() as *mut _, &mut key) != WFS_SUCCESS {
            xfs_reject_err!(WFS_ERR_INVALID_SERVPROV);
        }
    }

    let mut value_buffer: Vec<u8> = Vec::with_capacity(MAX_PATH);
    let mut value_len = MAX_PATH as u32;

    // SAFETY:
    // - the key pointer is not null as the WFM_OPEN_KEY call succeeded
    // - the value pointer is function argument, and it is not null
    // - the value buffer pointer is not null as the vector allocated this memory
    unsafe {
        if registry::query_value(key, name.as_ptr() as *mut i8, value_buffer.as_mut_ptr() as *mut _, &mut value_len) != WFS_SUCCESS {
            registry::close_key(key);
            xfs_reject_err!(WFS_ERR_INVALID_SERVPROV);
        }
    }

    // SAFETY: We know that the buffer is at least as large as the value_len.
    unsafe { value_buffer.set_len(value_len as usize) };

    // SAFETY: the key was opened by WFM_OPEN_KEY, so it is a valid pointer.
    unsafe { xfslib::registry::close_key(key) };

    Ok(String::from_utf8(value_buffer).map_err(|_| xfs_reject!(WFS_ERR_INTERNAL_ERROR))?)
}
