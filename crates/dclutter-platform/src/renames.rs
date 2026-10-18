//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! Renaming a file where it is. Port of `FileActions.rename`.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::locks;
use crate::moves::move_without_replacing;

#[derive(Debug)]
pub enum RenameError {
    /// Something else in the folder already has the name.
    NameTaken(String),
    /// Windows won't accept it as a file name.
    InvalidName(String),
    InUse { name: String, apps: Vec<String> },
    Failed { name: String, reason: String },
}

impl fmt::Display for RenameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NameTaken(name) => write!(f, "{name} already exists in this folder."),
            Self::InvalidName(name) if name.is_empty() => write!(f, "Type a name first."),
            Self::InvalidName(name) => write!(f, "“{name}” can't be used as a file name on Windows."),
            Self::InUse { name, apps } => write!(
                f,
                "{name} is open in {}. Close it there and try again.",
                locks::describe_apps(apps)
            ),
            Self::Failed { name, reason } => write!(f, "Couldn't rename {name}: {reason}"),
        }
    }
}

impl std::error::Error for RenameError {}

/// Renames `path` to `new_name` in the same folder and returns the new path.
///
/// `new_name` is a bare file name: anything that looks like a folder is
/// stripped, so `..\elsewhere.pdf` can't move the file out of its folder.
/// If it has no extension, the original one is kept: dropping it leaves the
/// bytes intact but Windows can no longer tell what the file is, which
/// looks exactly like corruption to whoever renamed it. Nothing is ever
/// replaced.
pub fn rename_in_place(path: &Path, new_name: &str) -> Result<PathBuf, RenameError> {
    let current = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let typed = bare_name(new_name);
    check_windows_name(&typed)?;

    let extension = path.extension().map(|e| e.to_string_lossy().into_owned());
    let name = match (Path::new(&typed).extension(), extension) {
        (None, Some(ext)) if !path.is_dir() => format!("{typed}.{ext}"),
        _ => typed,
    };
    if name == current {
        return Ok(path.to_owned());
    }

    let target = path.with_file_name(&name);
    locks::retry_while_in_use(|| move_without_replacing(path, &target)).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            RenameError::NameTaken(name.clone())
        } else if locks::is_in_use(&e) {
            RenameError::InUse { name: current.clone(), apps: locks::apps_using(path) }
        } else {
            RenameError::Failed { name: current.clone(), reason: e.to_string() }
        }
    })?;
    Ok(target)
}

/// The last component of whatever was typed, trimmed.
fn bare_name(typed: &str) -> String {
    let last = typed.rsplit(['\\', '/']).next().unwrap_or_default();
    last.trim().to_owned()
}

/// Rejects what Windows would refuse or silently change.
fn check_windows_name(name: &str) -> Result<(), RenameError> {
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let invalid = || Err(RenameError::InvalidName(name.to_owned()));

    if name.is_empty() || name == "." || name == ".." {
        return invalid();
    }
    if name.chars().any(|c| c.is_control() || "<>:\"/\\|?*".contains(c)) {
        return invalid();
    }
    // Windows quietly drops trailing dots, which would rename the file to
    // something other than what was typed.
    if name.ends_with('.') {
        return invalid();
    }
    // CON, nul.txt and the like name devices, whatever the extension.
    let stem = name.split('.').next().unwrap_or_default().trim_end();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        return invalid();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn file(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, name).unwrap();
        path
    }

    #[test]
    fn renames_in_place_and_keeps_the_extension() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(dir.path(), "IMG_1234.jpg");

        let renamed = rename_in_place(&path, "Beach day").unwrap();

        assert_eq!(renamed, dir.path().join("Beach day.jpg"));
        assert!(!path.exists());
        assert_eq!(fs::read_to_string(&renamed).unwrap(), "IMG_1234.jpg");
    }

    #[test]
    fn a_typed_extension_is_used_as_is() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(dir.path(), "notes.txt");
        assert_eq!(rename_in_place(&path, "notes.md").unwrap(), dir.path().join("notes.md"));
    }

    #[test]
    fn folders_never_gain_an_extension() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Cards.v2");
        fs::create_dir(&folder).unwrap();
        assert_eq!(rename_in_place(&folder, "Cards").unwrap(), dir.path().join("Cards"));
    }

    #[test]
    fn folder_parts_of_the_name_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("Downloads");
        fs::create_dir(&inner).unwrap();
        let path = file(&inner, "report.pdf");

        let renamed = rename_in_place(&path, r"..\escaped").unwrap();

        assert_eq!(renamed, inner.join("escaped.pdf"));
        assert!(!dir.path().join("escaped.pdf").exists());
    }

    #[test]
    fn never_replaces_another_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(dir.path(), "a.txt");
        let other = file(dir.path(), "b.txt");

        let err = rename_in_place(&path, "b").unwrap_err();

        assert_eq!(err.to_string(), "b.txt already exists in this folder.");
        assert_eq!(fs::read_to_string(&other).unwrap(), "b.txt");
        assert!(path.exists());
    }

    #[test]
    fn the_same_name_is_a_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(dir.path(), "report.pdf");
        assert_eq!(rename_in_place(&path, " report ").unwrap(), path);
    }

    #[test]
    fn changing_only_the_case_works() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(dir.path(), "report.pdf");

        let renamed = rename_in_place(&path, "Report").unwrap();

        assert_eq!(renamed, dir.path().join("Report.pdf"));
        let names: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, ["Report.pdf"]);
    }

    #[test]
    fn names_windows_refuses_are_rejected_before_touching_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(dir.path(), "report.pdf");
        for bad in ["", "  ", "what?", "a:b", "x*y", "<tag>", "trailing.", "CON", "nul.txt", "Com1", "."] {
            let err = rename_in_place(&path, bad).unwrap_err();
            assert!(matches!(err, RenameError::InvalidName(_)), "{bad:?} gave {err}");
        }
        assert!(path.exists());
        assert_eq!(
            rename_in_place(&path, "what?").unwrap_err().to_string(),
            "“what?” can't be used as a file name on Windows."
        );
        assert_eq!(rename_in_place(&path, "").unwrap_err().to_string(), "Type a name first.");
    }

    #[test]
    fn device_like_words_inside_a_name_are_fine() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(dir.path(), "a.txt");
        assert!(rename_in_place(&path, "CON tract").is_ok());
    }
}
