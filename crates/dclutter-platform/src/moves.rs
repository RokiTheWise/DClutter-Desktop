//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! Moving files into destination folders and back again, without ever
//! replacing a file that is already there.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum MoveError {
    /// The destination folder is gone, renamed, or on a disconnected drive.
    DestinationUnavailable(PathBuf),
    /// Undo found something else at the file's original path.
    OriginalPathTaken(PathBuf),
    Failed { path: PathBuf, reason: String },
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DestinationUnavailable(folder) => write!(
                f,
                "{} isn't reachable. It may have been moved or renamed, or be on a drive that's disconnected.",
                name_of(folder)
            ),
            Self::OriginalPathTaken(path) => write!(
                f,
                "Something else named {} is in its old folder now, so it wasn't moved back.",
                name_of(path)
            ),
            Self::Failed { path, reason } => write!(f, "Couldn't move {}: {reason}", name_of(path)),
        }
    }
}

impl std::error::Error for MoveError {}

fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
}

/// Moves `path` into `folder` and returns where it landed. If the name is
/// taken there, the file is renamed `Report (2).pdf`, `Report (3).pdf` and
/// so on, the way File Explorer does. Nothing is ever overwritten.
pub fn move_into_folder(path: &Path, folder: &Path) -> Result<PathBuf, MoveError> {
    if !folder.is_dir() {
        return Err(MoveError::DestinationUnavailable(folder.to_owned()));
    }
    let name = path.file_name().ok_or_else(|| MoveError::Failed {
        path: path.to_owned(),
        reason: "it has no file name".into(),
    })?;

    for attempt in 1.. {
        let target = folder.join(numbered_name(Path::new(name), attempt));
        match move_without_replacing(path, &target) {
            Ok(()) => return Ok(target),
            // Taken: try the next number. Checking first and moving second
            // would race with anything else writing to the folder.
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(MoveError::Failed { path: path.to_owned(), reason: e.to_string() }),
        }
    }
    unreachable!()
}

/// Moves a filed file back to exactly where it was, for undo. Refuses if
/// something else has taken that path in the meantime.
pub fn move_back(from: &Path, to: &Path) -> Result<(), MoveError> {
    move_without_replacing(from, to).map_err(|e| match e.kind() {
        io::ErrorKind::AlreadyExists => MoveError::OriginalPathTaken(to.to_owned()),
        _ => MoveError::Failed { path: from.to_owned(), reason: e.to_string() },
    })
}

/// `Report.pdf` for attempt 1, then `Report (2).pdf`, `Report (3).pdf`.
fn numbered_name(name: &Path, attempt: u32) -> PathBuf {
    if attempt == 1 {
        return name.to_owned();
    }
    let stem = name.file_stem().unwrap_or_default().to_string_lossy();
    match name.extension() {
        Some(ext) => format!("{stem} ({attempt}).{}", ext.to_string_lossy()).into(),
        None => format!("{stem} ({attempt})").into(),
    }
}

/// `std::fs::rename` replaces an existing file on Windows, so call
/// `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`. `MOVEFILE_COPY_ALLOWED`
/// lets a file move to another drive (Windows copies it, then removes the
/// original only once the copy succeeded). Folders can't cross drives.
#[cfg(windows)]
fn move_without_replacing(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_COPY_ALLOWED, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let wide = |p: &Path| p.as_os_str().encode_wide().chain([0]).collect::<Vec<u16>>();
    let (from_w, to_w) = (wide(from), wide(to));
    // SAFETY: both are valid, NUL-terminated UTF-16 paths.
    let ok = unsafe {
        MoveFileExW(from_w.as_ptr(), to_w.as_ptr(), MOVEFILE_COPY_ALLOWED | MOVEFILE_WRITE_THROUGH)
    };
    if ok != 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

#[cfg(not(windows))]
fn move_without_replacing(from: &Path, to: &Path) -> io::Result<()> {
    if to.symlink_metadata().is_ok() {
        return Err(io::ErrorKind::AlreadyExists.into());
    }
    std::fs::rename(from, to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Folders {
        _root: tempfile::TempDir,
        source: PathBuf,
        destination: PathBuf,
    }

    fn folders() -> Folders {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("Downloads");
        let destination = root.path().join("Invoices");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        Folders { _root: root, source, destination }
    }

    #[test]
    fn moves_a_file_into_the_folder() {
        let f = folders();
        let file = f.source.join("Report.pdf");
        fs::write(&file, "report").unwrap();

        let landed = move_into_folder(&file, &f.destination).unwrap();

        assert_eq!(landed, f.destination.join("Report.pdf"));
        assert!(!file.exists());
        assert_eq!(fs::read_to_string(&landed).unwrap(), "report");
    }

    #[test]
    fn never_overwrites_and_numbers_the_name_like_explorer() {
        let f = folders();
        fs::write(f.destination.join("Report.pdf"), "existing").unwrap();
        fs::write(f.destination.join("Report (2).pdf"), "existing 2").unwrap();
        let file = f.source.join("Report.pdf");
        fs::write(&file, "new").unwrap();

        let landed = move_into_folder(&file, &f.destination).unwrap();

        assert_eq!(landed, f.destination.join("Report (3).pdf"));
        assert_eq!(fs::read_to_string(f.destination.join("Report.pdf")).unwrap(), "existing");
        assert_eq!(fs::read_to_string(f.destination.join("Report (2).pdf")).unwrap(), "existing 2");
        assert_eq!(fs::read_to_string(&landed).unwrap(), "new");
    }

    #[test]
    fn numbers_names_without_an_extension() {
        let f = folders();
        fs::write(f.destination.join("LICENSE"), "existing").unwrap();
        let file = f.source.join("LICENSE");
        fs::write(&file, "new").unwrap();

        assert_eq!(move_into_folder(&file, &f.destination).unwrap(), f.destination.join("LICENSE (2)"));
    }

    #[test]
    fn moves_folders_too() {
        let f = folders();
        let folder = f.source.join("Extracted");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("inside.txt"), "x").unwrap();

        let landed = move_into_folder(&folder, &f.destination).unwrap();

        assert!(landed.join("inside.txt").exists());
        assert!(!folder.exists());
    }

    #[test]
    fn a_missing_destination_is_reported_and_nothing_moves() {
        let f = folders();
        let file = f.source.join("Report.pdf");
        fs::write(&file, "report").unwrap();

        let err = move_into_folder(&file, &f.destination.join("Gone")).unwrap_err();

        assert!(matches!(err, MoveError::DestinationUnavailable(_)));
        assert!(err.to_string().starts_with("Gone isn't reachable"));
        assert!(file.exists());
    }

    #[test]
    fn move_back_returns_the_file_to_its_exact_path() {
        let f = folders();
        let file = f.source.join("Report.pdf");
        fs::write(&file, "report").unwrap();
        let landed = move_into_folder(&file, &f.destination).unwrap();

        move_back(&landed, &file).unwrap();

        assert_eq!(fs::read_to_string(&file).unwrap(), "report");
        assert!(!landed.exists());
    }

    #[test]
    fn move_back_refuses_to_overwrite_a_new_file_at_the_old_path() {
        let f = folders();
        let file = f.source.join("Report.pdf");
        fs::write(&file, "report").unwrap();
        let landed = move_into_folder(&file, &f.destination).unwrap();
        fs::write(&file, "a newer download").unwrap();

        let err = move_back(&landed, &file).unwrap_err();

        assert!(matches!(err, MoveError::OriginalPathTaken(_)));
        assert_eq!(fs::read_to_string(&file).unwrap(), "a newer download");
        assert_eq!(fs::read_to_string(&landed).unwrap(), "report");
    }
}
