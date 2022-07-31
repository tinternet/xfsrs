use std::path::Path;

use log::{trace, LevelFilter};
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
    Config,
};
use winapi::{
    shared::minwindef::{DWORD, HINSTANCE, HMODULE, MAX_PATH},
    um::{libloaderapi::GetModuleFileNameA, winnt::DLL_PROCESS_ATTACH},
};

pub fn module_init(dll: HINSTANCE, fdw_reason: DWORD) {
    if fdw_reason != DLL_PROCESS_ATTACH {
        return;
    }

    let filename = unsafe { get_module_name(dll) };
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} {l} {L} - {m}\n")))
        .build(format!("$ENV{{Public}}\\{filename}.log"))
        .unwrap();
    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Trace))
        .unwrap();

    log4rs::init_config(config).unwrap();
    let pid = std::process::id();
    trace!("DLL attached: {filename}, process id: {pid}");
}

unsafe fn get_module_name(module: HMODULE) -> String {
    let mut i8slice = [0i8; MAX_PATH];
    let len = GetModuleFileNameA(module, i8slice.as_mut_ptr(), MAX_PATH as u32) as usize;

    if len == 0 {
        return String::new();
    }
    let u8slice = std::slice::from_raw_parts(i8slice.as_ptr() as *const u8, MAX_PATH);
    let dir = std::str::from_utf8(&u8slice[..len]).unwrap_or("");
    Path::new(dir).file_name().unwrap_or_default().to_owned().into_string().unwrap_or_default()
}
