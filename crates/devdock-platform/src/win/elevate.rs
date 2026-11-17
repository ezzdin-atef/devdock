//! Running one program as administrator (hosts-file updates). Windows shows its UAC
//! prompt; the user's answer there is the confirmation.
#![allow(unsafe_code)]

use std::io;
use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED};
use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
use windows::Win32::UI::Shell::{
    SEE_MASK_NO_CONSOLE, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::PCWSTR;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Starts `program args` elevated, waits for it and returns its exit code.
/// Declining the UAC prompt is `ErrorKind::PermissionDenied`.
pub fn run_elevated(program: &Path, args: &str) -> io::Result<u32> {
    let verb = wide("runas");
    let file = wide(&program.to_string_lossy());
    let params = wide(args);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(std::mem::size_of::<SHELLEXECUTEINFOW>()).unwrap_or(0),
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NO_CONSOLE,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: `info` and the three wide strings it points to outlive the call.
    if let Err(e) = unsafe { ShellExecuteExW(&raw mut info) } {
        if e.code() == ERROR_CANCELLED.to_hresult() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the Windows permission prompt was declined",
            ));
        }
        return Err(io::Error::from_raw_os_error(e.code().0));
    }
    let process = info.hProcess;
    if process.is_invalid() {
        return Err(io::Error::other(
            "Windows didn't return the elevated process",
        ));
    }
    let mut code = 0u32;
    // SAFETY: `process` is a live handle owned here (SEE_MASK_NOCLOSEPROCESS);
    // `code` is a valid out-pointer. The handle is closed exactly once.
    let result = unsafe {
        WaitForSingleObject(process, INFINITE);
        let r = GetExitCodeProcess(process, &raw mut code);
        let _ = CloseHandle(process);
        r
    };
    result.map_err(|e| io::Error::from_raw_os_error(e.code().0))?;
    Ok(code)
}
