//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! The only place in DClutter that may remove a user's file from where it
//! is, and only into the Recycle Bin. Never a permanent delete.

use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum TrashError {
    /// The file is on a drive Windows doesn't recycle from, such as a
    /// network share or a removable drive. Deleting there is permanent, so
    /// DClutter refuses rather than risk it.
    NoRecycleBin(PathBuf),
    Failed { path: PathBuf, reason: String },
}

impl fmt::Display for TrashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRecycleBin(path) => write!(
                f,
                "{} is on a drive without a Recycle Bin, so DClutter won't delete it.",
                display_name(path)
            ),
            Self::Failed { path, reason } => {
                write!(f, "Couldn't move {} to the Recycle Bin: {reason}", display_name(path))
            }
        }
    }
}

impl std::error::Error for TrashError {}

fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// Moves `path` to the Recycle Bin, where the user can still restore it.
pub fn move_to_recycle_bin(path: &Path) -> Result<(), TrashError> {
    if !has_recycle_bin(path) {
        return Err(TrashError::NoRecycleBin(path.to_owned()));
    }
    trash::delete(path).map_err(|e| TrashError::Failed { path: path.to_owned(), reason: e.to_string() })?;

    // The shell can report success for an operation it quietly skipped;
    // only call it done once the file has actually left its folder.
    if path.exists() {
        return Err(TrashError::Failed {
            path: path.to_owned(),
            reason: "it's still in its folder".into(),
        });
    }
    Ok(())
}

/// Whether Windows recycles deletions on the drive holding `path`. Only
/// local fixed drives qualify. Network shares and removable drives delete
/// permanently, and so does anything we can't identify.
#[cfg(windows)]
fn has_recycle_bin(path: &Path) -> bool {
    use std::path::{Component, Prefix};
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;

    const DRIVE_FIXED: u32 = 3;

    let letter = match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter,
            _ => return false,
        },
        _ => return false,
    };
    let root: Vec<u16> = format!("{}:\\", letter as char).encode_utf16().chain([0]).collect();
    // SAFETY: `root` is a valid, NUL-terminated UTF-16 string.
    unsafe { GetDriveTypeW(root.as_ptr()) == DRIVE_FIXED }
}

/// Elsewhere the `trash` crate uses the desktop trash, which always exists.
#[cfg(not(windows))]
fn has_recycle_bin(_path: &Path) -> bool {
    true
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn refuses_network_shares_without_touching_them() {
        let path = Path::new(r"\\server\share\report.pdf");
        assert!(matches!(move_to_recycle_bin(path), Err(TrashError::NoRecycleBin(_))));
    }

    #[test]
    fn refuses_relative_paths() {
        assert!(matches!(move_to_recycle_bin(Path::new("report.pdf")), Err(TrashError::NoRecycleBin(_))));
    }

    #[test]
    fn local_drives_have_a_recycle_bin() {
        assert!(has_recycle_bin(&std::env::temp_dir()));
    }

    #[test]
    fn missing_files_fail_and_say_which_file() {
        let dir = tempfile::tempdir().unwrap();
        let err = move_to_recycle_bin(&dir.path().join("not-here.txt")).unwrap_err();
        assert!(matches!(err, TrashError::Failed { .. }));
        assert!(err.to_string().contains("not-here.txt"));
    }

    /// Round trip through the real Recycle Bin: the file must leave its
    /// folder, be listed in the bin, and come back intact when restored,
    /// which proves it was recycled rather than deleted.
    #[test]
    fn recycles_a_file_so_it_can_be_restored() {
        let dir = tempfile::tempdir().unwrap();
        let name = format!("dclutter-test-{}.txt", std::process::id());
        let path = dir.path().join(&name);
        fs::write(&path, "keep me").unwrap();

        move_to_recycle_bin(&path).unwrap();
        assert!(!path.exists());

        let in_bin: Vec<_> = trash::os_limited::list()
            .unwrap()
            .into_iter()
            .filter(|item| item.name == name.as_str())
            .collect();
        assert_eq!(in_bin.len(), 1, "expected {name} in the Recycle Bin");

        trash::os_limited::restore_all(in_bin).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "keep me");
    }
}
