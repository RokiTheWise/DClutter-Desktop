//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! Coping with files another program has open. Windows refuses to move or
//! delete those, which is often brief: a fresh download gets scanned by
//! Defender, indexed by Windows Search or picked up by OneDrive.

use std::io;
use std::path::Path;
use std::thread;
use std::time::Duration;

/// Waits between attempts, about 1.5 s in all: long enough to ride out a
/// scan or an index pass, short enough that a file someone really has
/// open fails quickly with a useful message.
const RETRY_DELAYS_MS: [u64; 5] = [100, 200, 300, 400, 500];

/// Whether `error` means another program has the file open.
pub fn is_in_use(error: &io::Error) -> bool {
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;
    cfg!(windows) && matches!(error.raw_os_error(), Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION))
}

/// Runs `op`, retrying for a moment while the file is in use.
pub fn retry_while_in_use<T>(mut op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut delays = RETRY_DELAYS_MS.iter();
    loop {
        match op() {
            Err(e) if is_in_use(&e) => match delays.next() {
                Some(ms) => thread::sleep(Duration::from_millis(*ms)),
                None => return Err(e),
            },
            result => return result,
        }
    }
}

/// The names of the programs that have `path` open, such as "Microsoft
/// Edge", as Windows' Restart Manager reports them. Empty when none can be
/// named.
#[cfg(windows)]
pub fn apps_using(path: &Path) -> Vec<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::RestartManager::{
        CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources,
        RmStartSession,
    };

    const ERROR_MORE_DATA: u32 = 234;
    let file: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    let mut session = 0u32;

    // SAFETY: every pointer passed is valid for the call: a NUL-terminated
    // path, a key buffer of the documented size, and a process list sized
    // by the previous call. The session is always ended.
    unsafe {
        if RmStartSession(&mut session, 0, key.as_mut_ptr()) != 0 {
            return Vec::new();
        }
        let files = [file.as_ptr()];
        let mut names = Vec::new();
        if RmRegisterResources(session, 1, files.as_ptr(), 0, std::ptr::null(), 0, std::ptr::null()) == 0 {
            let (mut needed, mut count, mut reasons) = (0u32, 0u32, 0u32);
            let first = RmGetList(session, &mut needed, &mut count, std::ptr::null_mut(), &mut reasons);
            if first == ERROR_MORE_DATA && needed > 0 {
                let mut info: Vec<RM_PROCESS_INFO> = vec![std::mem::zeroed(); needed as usize];
                count = needed;
                if RmGetList(session, &mut needed, &mut count, info.as_mut_ptr(), &mut reasons) == 0 {
                    for process in &info[..count as usize] {
                        let len = process.strAppName.iter().position(|&c| c == 0).unwrap_or(process.strAppName.len());
                        let name = String::from_utf16_lossy(&process.strAppName[..len]);
                        if !name.is_empty() && !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
            }
        }
        RmEndSession(session);
        names
    }
}

#[cfg(not(windows))]
pub fn apps_using(_path: &Path) -> Vec<String> {
    Vec::new()
}

/// "Microsoft Edge", "Word and Microsoft Edge", or "another app".
pub fn describe_apps(apps: &[String]) -> String {
    match apps {
        [] => "another app".into(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn in_use() -> io::Error {
        io::Error::from_raw_os_error(32)
    }

    #[test]
    fn describes_apps_in_plain_words() {
        assert_eq!(describe_apps(&[]), "another app");
        assert_eq!(describe_apps(&["Microsoft Edge".into()]), "Microsoft Edge");
        assert_eq!(describe_apps(&["Word".into(), "Excel".into(), "Edge".into()]), "Word, Excel and Edge");
    }

    #[cfg(windows)]
    #[test]
    fn retries_until_the_file_is_free() {
        let attempts = Cell::new(0);
        let result = retry_while_in_use(|| {
            attempts.set(attempts.get() + 1);
            if attempts.get() < 3 { Err(in_use()) } else { Ok("moved") }
        });
        assert_eq!(result.unwrap(), "moved");
        assert_eq!(attempts.get(), 3);
    }

    #[cfg(windows)]
    #[test]
    fn gives_up_after_a_moment() {
        let attempts = Cell::new(0);
        let result: io::Result<()> = retry_while_in_use(|| {
            attempts.set(attempts.get() + 1);
            Err(in_use())
        });
        assert!(is_in_use(&result.unwrap_err()));
        assert_eq!(attempts.get(), RETRY_DELAYS_MS.len() + 1);
    }

    #[test]
    fn other_errors_are_not_retried() {
        let attempts = Cell::new(0);
        let result: io::Result<()> = retry_while_in_use(|| {
            attempts.set(attempts.get() + 1);
            Err(io::ErrorKind::NotFound.into())
        });
        assert!(result.is_err());
        assert_eq!(attempts.get(), 1);
    }

    /// Holds a file open with no sharing, the way Acrobat or Edge do, and
    /// checks Restart Manager names this test process as the holder.
    #[cfg(windows)]
    #[test]
    fn names_the_process_holding_a_file() {
        use std::os::windows::fs::OpenOptionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("open.pdf");
        std::fs::write(&path, "x").unwrap();
        let _held = std::fs::OpenOptions::new().read(true).share_mode(0).open(&path).unwrap();

        assert!(!apps_using(&path).is_empty());
    }

    #[test]
    fn a_file_nobody_has_open_names_no_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("closed.pdf");
        std::fs::write(&path, "x").unwrap();
        assert!(apps_using(&path).is_empty());
    }
}
