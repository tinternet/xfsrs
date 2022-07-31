use std::{
    ffi::{CStr, CString},
    ptr,
    sync::Mutex,
};

use crate::spi::*;
use lazy_static::lazy_static;
use log::error;
use winapi::{
    shared::{
        minwindef::{DWORD, HKEY, LPDWORD, LPVOID, MAX_PATH},
        windef::HWND,
        winerror::HRESULT,
    },
    um::winnt::LPSTR,
};
use xfslib::*;

lazy_static! {
    // holds service handles
    static ref SERVICES: Mutex<Vec<Option<Service>>> = Mutex::new((0..8192).map(|_| None).collect());

    // holds app handles
    static ref APP_HANDLES: Mutex<[bool; 8192]> = Mutex::new([false; 8192]);
}

struct Service {
    request_id: u32,
    library: libloading::Library,
    trace_level: DWORD,
}

macro_rules! service_request {
    ($function:ty, $service:expr, $services:expr) => {
        match $service {
            0 => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
            _ => {
                let mut service = match $services.get_mut($service as usize - 1) {
                    Some(Some(service)) => service,
                    _ => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
                };
                let symbol = match unsafe { service.library.get::<$function>(stringify!($function).as_bytes()) } {
                    Ok(symbol) => symbol,
                    Err(_) => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
                };
                service.request_id += 1;
                (service.request_id as REQUESTID, symbol)
            }
        }
    };
}

macro_rules! call_api {
    ($function:ty => $service:expr, $wnd:expr, $request_id:expr => $($params:expr),+) => {
        let mut services = xfs_unwrap!(SERVICES.lock());
        let (service_request_id, function) = service_request!($function, $service, services);

        unsafe { $request_id.write(service_request_id) };
        return function($service, $($params),+, $wnd, service_request_id);
    };
    ($function:ty => $service:expr, $wnd:expr, $request_id:expr) => {
        let mut services = xfs_unwrap!(SERVICES.lock());
        let (service_request_id, function) = service_request!($function, $service, services);

        unsafe { $request_id.write(service_request_id) };
        return function($service, $wnd, service_request_id);
    };
}

pub fn async_close(service: HSERVICE, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
    call_api!(WFPClose => service, wnd, request_id);
}

pub fn async_deregister(service: HSERVICE, dw_event_class: DWORD, wnd_reg: HWND, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
    call_api!(WFPDeregister => service, wnd, request_id => dw_event_class, wnd_reg);
}

pub fn async_execute(service: HSERVICE, dw_command: DWORD, lp_cmd_data: LPVOID, dw_time_out: DWORD, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
    call_api!(WFPExecute => service, wnd, request_id => dw_command, lp_cmd_data, dw_time_out);
}

pub fn async_get_info(service: HSERVICE, dw_category: DWORD, lp_query_details: LPVOID, dw_time_out: DWORD, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
    call_api!(WFPGetInfo => service, wnd, request_id => dw_category, lp_query_details, dw_time_out);
}

pub fn async_lock(service: HSERVICE, dw_time_out: DWORD, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
    call_api!(WFPLock => service, wnd, request_id => dw_time_out);
}

pub fn async_register(service: HSERVICE, dw_event_class: DWORD, wnd_reg: HWND, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
    call_api!(WFPRegister => service, wnd, request_id => dw_event_class, wnd_reg);
}

pub fn async_unlock(service: HSERVICE, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
    call_api!(WFPUnlock => service, wnd, request_id);
}

pub fn create_app_handle(lph_app: LPHAPP) -> HRESULT {
    let mut handles = xfs_unwrap!(APP_HANDLES.lock());
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

    let mut handles = xfs_unwrap!(APP_HANDLES.lock());
    let ptr = (&*handles) as *const _ as usize;
    let index = h_app as usize - ptr;

    match handles.get_mut(index) {
        Some(h) => *h = false,
        None => xfs_reject!(WFS_ERR_INVALID_APP_HANDLE),
    }

    WFS_SUCCESS
}

pub fn async_open(
    lpsz_logical_name: LPSTR,
    h_app: HAPP,
    lpsz_app_id: LPSTR,
    dw_trace_level: DWORD,
    dw_time_out: DWORD,
    lph_service: LPHSERVICE,
    h_wnd: HWND,
    dw_srvc_versions_required: DWORD,
    lp_srvc_version: LPWFSVERSION,
    lp_spiversion: LPWFSVERSION,
    lp_request_id: LPREQUESTID,
) -> HRESULT {
    if lpsz_logical_name.is_null() || lp_srvc_version.is_null() || lp_spiversion.is_null() || lph_service.is_null() || lp_request_id.is_null() {
        xfs_reject!(WFS_ERR_INVALID_POINTER);
    }

    fn get_value(root: HKEY, path: CString, name: CString) -> Result<String, HRESULT> {
        let mut key = ptr::null_mut();

        // SAFETY: the path pointer is function argument, and it is not null
        unsafe {
            if xfslib::conf::open_key(root, path.as_ptr() as *mut _, &mut key) != WFS_SUCCESS {
                error!("WFM_OPEN_KEY failed");
                return Err(WFS_ERR_INVALID_SERVPROV);
            }
        }

        let mut value_buffer: Vec<u8> = Vec::with_capacity(MAX_PATH);
        let mut value_len = MAX_PATH as u32;

        // SAFETY:
        // - the key pointer is not null as the WFM_OPEN_KEY call succeeded
        // - the value pointer is function argument, and it is not null
        // - the value buffer pointer is not null as the vector allocated this memory
        unsafe {
            if xfslib::conf::query_value(key, name.as_ptr() as *mut i8, value_buffer.as_mut_ptr() as *mut _, &mut value_len) != WFS_SUCCESS {
                xfslib::conf::close_key(key);
                error!("WFM_QUERY_VALUE failed");
                return Err(WFS_ERR_INVALID_SERVPROV);
            }
        }

        // SAFETY: We know that the buffer is at least as large as the value_len.
        unsafe { value_buffer.set_len(value_len as usize) };

        // SAFETY: the key was opened by WFM_OPEN_KEY, so it is a valid pointer.
        unsafe { xfslib::conf::close_key(key) };

        Ok(String::from_utf8(value_buffer).map_err(|error| {
            error!("{}", error);
            WFS_ERR_INTERNAL_ERROR
        })?)
    }

    let logical_name = xfs_unwrap!(unsafe { CStr::from_ptr(lpsz_logical_name) }.to_str());
    let path = xfs_unwrap!(CString::new(format!("LOGICAL_SERVICES\\{}", logical_name)));
    let lgl_prov_path = match get_value(WFS_CFG_HKEY_USER_DEFAULT_XFS_ROOT, path, CString::new("provider").unwrap()) {
        Ok(lgl_prov_path) => lgl_prov_path,
        Err(error) => return error,
    };

    let path = xfs_unwrap!(CString::new(format!("SERVICE_PROVIDERS\\{}", lgl_prov_path)));
    let phy_prov_path = match get_value(WFS_CFG_HKEY_MACHINE_XFS_ROOT, path, CString::new("dllname").unwrap()) {
        Ok(phy_prov_path) => phy_prov_path,
        Err(error) => return error,
    };

    // SAFETY: The service providers are safe to use.
    let library = unsafe { xfs_unwrap!(libloading::Library::new(phy_prov_path)) };

    let mut services = xfs_unwrap!(SERVICES.lock());
    let service_index = match services.iter().position(|s| s.is_none()) {
        Some(index) => index,
        None => xfs_reject!(WFS_ERR_INTERNAL_ERROR),
    };

    services[service_index] = Some(Service {
        library,
        request_id: 1,
        trace_level: dw_trace_level,
    });
    let service = services[service_index].as_ref().unwrap();

    // SAFETY: The service providers are safe to use. All pointers are checked and not null.
    unsafe {
        *lph_service = service_index as u16 + 1;
        *lp_request_id = 1;

        let wfp_open = xfs_unwrap!(service.library.get::<WfpOpen>(b"WFPOpen"));
        let service_handle = ((&*services) as *const _ as HPROVIDER).add(service_index);

        wfp_open(
            *lph_service,
            lpsz_logical_name,
            h_app,
            lpsz_app_id,
            dw_trace_level,
            dw_time_out,
            h_wnd,
            *lp_request_id,
            service_handle,
            VersionRange::new_explicit(Version::new_explicit(3, 0), Version::new_explicit(3, 30)).value(),
            lp_spiversion,
            dw_srvc_versions_required,
            lp_srvc_version,
        )
    }
}

pub fn cancel_request(h_service: HSERVICE, request_id: REQUESTID) -> HRESULT {
    if h_service == 0 {
        xfs_reject!(WFS_ERR_INVALID_HSERVICE);
    }

    let services = xfs_unwrap!(SERVICES.lock());
    let service = match services.get(h_service as usize - 1).and_then(|service| service.as_ref()) {
        Some(service) => service,
        None => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
    };
    let cancel = unsafe { xfs_unwrap!(service.library.get::<WfpCancelAsyncRequest>(b"WFPCancelAsyncRequest")) };

    cancel(h_service, request_id)
}

// TODO: Close service providers
pub fn clean_up() -> HRESULT {
    xfs_unwrap!(APP_HANDLES.lock()).iter_mut().for_each(|v| *v = false);
    xfs_unwrap!(SERVICES.lock()).iter_mut().filter_map(|s| s.take()).for_each(drop);
    WFS_SUCCESS
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

pub fn get_trace_level(h_service: HSERVICE, lpdw_trace_level: LPDWORD) -> HRESULT {
    let services = xfs_unwrap!(SERVICES.lock());
    if let Some(service) = services.get(h_service as usize - 1).and_then(|service| service.as_ref()) {
        unsafe { lpdw_trace_level.write(service.trace_level) };
        return WFS_SUCCESS;
    }
    xfs_reject!(WFS_ERR_INVALID_HSERVICE);
}

pub fn release_dll(h_provider: HPROVIDER) -> HRESULT {
    let mut services = xfs_unwrap!(SERVICES.lock());
    let service_handle = (&*services) as *const _ as usize;
    let index = h_provider as usize - service_handle;
    services[index] = None;
    WFS_SUCCESS
}

pub fn set_trace_level(h_service: HSERVICE, dw_trace_level: DWORD) -> HRESULT {
    if h_service == 0 {
        xfs_reject!(WFS_ERR_INVALID_HSERVICE);
    }
    let mut services = xfs_unwrap!(SERVICES.lock());
    let service = match services.get_mut(h_service as usize - 1) {
        Some(Some(service)) => service,
        _ => xfs_reject!(WFS_ERR_INVALID_HSERVICE),
    };
    service.trace_level = dw_trace_level;
    unsafe { xfs_unwrap!(service.library.get::<WFPSetTraceLevel>(b"WFPSetTraceLevel"))(h_service, dw_trace_level) }
}
