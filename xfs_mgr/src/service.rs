use libloading::Library;
use libloading::Symbol;
use winapi::shared::minwindef::DWORD;
use winapi::shared::minwindef::LPVOID;
use winapi::shared::windef::HWND;
use winapi::um::commctrl::HRESULT;
use winapi::um::winnt::LPSTR;
use xfslib::spi::*;
use xfslib::*;

pub struct Service {
    request_id: u32,
    library: Library,
    service_id: HSERVICE,
    trace_level: DWORD,
}

impl Service {
    pub unsafe fn new(service_id: HSERVICE, path: &str, trace_level: DWORD) -> Result<Self, HRESULT> {
        Ok(Self {
            request_id: 0,
            library: xfs_unwrap_err!(libloading::Library::new(path)),
            service_id,
            trace_level,
        })
    }

    pub fn get_trace_level(&self) -> DWORD {
        self.trace_level
    }

    pub unsafe fn open(
        &mut self,
        logical_name: LPSTR,
        app: HAPP,
        app_id: LPSTR,
        trace_level: DWORD,
        time_out: DWORD,
        wnd: HWND,
        srvc_versions_required: DWORD,
        srvc_version: LPWFSVERSION,
        spiversion: LPWFSVERSION,
        request_id: LPREQUESTID,
    ) -> HRESULT {
        let open: Symbol<WFPOpen> = xfs_unwrap!(self.library.get(b"WFPOpen"));
        open(
            self.service_id,
            logical_name,
            app,
            app_id,
            trace_level,
            time_out,
            wnd,
            self.get_request_id(request_id),
            self as *const Service as HPROVIDER,
            VersionRange::new_explicit(Version::new_explicit(3, 0), Version::new_explicit(3, 30)).value(),
            spiversion,
            srvc_versions_required,
            srvc_version,
        )
    }

    pub unsafe fn close(&mut self, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
        let close: Symbol<WFPClose> = xfs_unwrap!(self.library.get(b"WFPClose"));
        close(self.service_id, wnd, self.get_request_id(request_id))
    }

    pub unsafe fn register(&mut self, event_class: DWORD, wnd_reg: HWND, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
        let register: Symbol<WFPRegister> = xfs_unwrap!(self.library.get(b"WFPRegister"));
        register(self.service_id, event_class, wnd_reg, wnd, self.get_request_id(request_id))
    }

    pub unsafe fn deregister(&mut self, event_class: DWORD, wnd_reg: HWND, wnd: HWND, request_id: LPREQUESTID) -> HRESULT {
        let deregister: Symbol<WFPDeregister> = xfs_unwrap!(self.library.get(b"WFPDeregister"));
        deregister(self.service_id, event_class, wnd_reg, wnd, self.get_request_id(request_id))
    }

    pub unsafe fn execute(&mut self, command: DWORD, cmd_data: LPVOID, time_out: DWORD, wnd: HWND, req_id: LPREQUESTID) -> HRESULT {
        let execute: Symbol<WFPExecute> = xfs_unwrap!(self.library.get(b"WFPExecute"));
        execute(self.service_id, command, cmd_data, time_out, wnd, self.get_request_id(req_id))
    }

    pub unsafe fn get_info(&mut self, dw_category: DWORD, lp_query_details: LPVOID, dw_time_out: DWORD, h_wnd: HWND, req_id: LPREQUESTID) -> HRESULT {
        let get_info: Symbol<WFPGetInfo> = xfs_unwrap!(self.library.get(b"WFPGetInfo"));
        get_info(self.service_id, dw_category, lp_query_details, dw_time_out, h_wnd, self.get_request_id(req_id))
    }

    pub unsafe fn set_trace_level(&mut self, trace_level: DWORD) -> HRESULT {
        self.trace_level = trace_level;
        let set_level: Symbol<WFPSetTraceLevel> = xfs_unwrap!(self.library.get(b"WFPSetTraceLevel"));
        set_level(self.service_id, trace_level)
    }

    pub unsafe fn lock(&mut self, dw_time_out: DWORD, h_wnd: HWND, req_id: LPREQUESTID) -> HRESULT {
        let lock: Symbol<WFPLock> = xfs_unwrap!(self.library.get(b"WFPLock"));
        lock(self.service_id, dw_time_out, h_wnd, self.get_request_id(req_id))
    }

    pub unsafe fn unlock(&mut self, h_wnd: HWND, req_id: LPREQUESTID) -> HRESULT {
        let unlock: Symbol<WFPUnlock> = xfs_unwrap!(self.library.get(b"WFPUnlock"));
        unlock(self.service_id, h_wnd, self.get_request_id(req_id))
    }

    pub unsafe fn cancel_async_request(&mut self, req_id: REQUESTID) -> HRESULT {
        let cancel_async_request: Symbol<WFPCancelAsyncRequest> = xfs_unwrap!(self.library.get(b"WFPCancelAsyncRequest"));
        cancel_async_request(self.service_id, req_id)
    }

    pub unsafe fn unload_service(&mut self) -> HRESULT {
        let unload_service: Symbol<WFPUnloadService> = xfs_unwrap!(self.library.get(b"WFPUnloadService"));
        unload_service()
    }

    fn get_request_id(&mut self, request_id: LPREQUESTID) -> u32 {
        let (id, _) = self.request_id.overflowing_add(1);
        if !request_id.is_null() {
            unsafe { *request_id = id };
        }
        self.request_id = id;
        self.request_id
    }
}
