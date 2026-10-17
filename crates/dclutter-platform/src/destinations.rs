//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! The folders the user files things into, saved between launches. Port of
//! `DestinationStore`. Windows has no sandbox, so a plain path is enough
//! where the macOS app needs a security-scoped bookmark.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Three, deliberately: with three bins people aim by muscle memory and
/// never read a label; with six they read, and reading breaks the loop.
pub const MAX_DESTINATIONS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Destination {
    pub id: String,
    pub path: PathBuf,
    /// Short label for the bin, the folder's own name by default.
    pub name: String,
}

impl Destination {
    pub fn new(path: PathBuf) -> Self {
        // A drive root like D:\ has no file name; show the drive instead.
        let name = path
            .file_name()
            .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        Self { id: path.to_string_lossy().into_owned(), path, name }
    }

    /// Whether this is the same folder as `path`. Windows paths are case
    /// insensitive, so `C:\Invoices` and `c:\invoices` match.
    pub fn is_folder(&self, path: &Path) -> bool {
        self.path.as_os_str().eq_ignore_ascii_case(path.as_os_str())
    }
}

/// Reads and writes the destination list as JSON in one file.
pub struct DestinationStore {
    file: PathBuf,
}

impl DestinationStore {
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }

    /// The saved destinations. A missing or unreadable file means none,
    /// rather than an error: losing the list costs a few clicks to redo,
    /// but refusing to start would cost the whole app.
    pub fn load(&self) -> Vec<Destination> {
        let Ok(text) = fs::read_to_string(&self.file) else { return Vec::new() };
        let mut destinations: Vec<Destination> = serde_json::from_str(&text).unwrap_or_default();
        destinations.truncate(MAX_DESTINATIONS);
        destinations
    }

    /// Saves `destinations`, dropping any beyond the cap. Writes to a
    /// temporary file first so a crash mid-write can't leave half a list.
    pub fn save(&self, destinations: &[Destination]) -> io::Result<()> {
        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir)?;
        }
        let kept = &destinations[..destinations.len().min(MAX_DESTINATIONS)];
        let json = serde_json::to_string_pretty(kept).map_err(io::Error::other)?;
        let temp = self.file.with_extension("json.tmp");
        fs::write(&temp, json)?;
        fs::rename(&temp, &self.file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, DestinationStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = DestinationStore::new(dir.path().join("config").join("destinations.json"));
        (dir, store)
    }

    #[test]
    fn starts_empty() {
        let (_dir, store) = store();
        assert!(store.load().is_empty());
    }

    #[test]
    fn saves_and_loads_in_order() {
        let (_dir, store) = store();
        let saved = vec![
            Destination::new(PathBuf::from(r"C:\Users\me\Invoices")),
            Destination::new(PathBuf::from(r"D:\Photos")),
        ];
        store.save(&saved).unwrap();
        assert_eq!(store.load(), saved);
    }

    #[test]
    fn keeps_at_most_three() {
        let (_dir, store) = store();
        let four: Vec<_> = ["A", "B", "C", "D"].iter().map(|n| Destination::new(PathBuf::from(n))).collect();
        store.save(&four).unwrap();
        assert_eq!(store.load(), four[..3]);
    }

    #[test]
    fn a_corrupt_file_loads_as_empty() {
        let (_dir, store) = store();
        store.save(&[]).unwrap();
        fs::write(&store.file, "{ not json").unwrap();
        assert!(store.load().is_empty());
    }

    #[test]
    fn names_come_from_the_folder() {
        assert_eq!(Destination::new(PathBuf::from(r"C:\Users\me\Invoices")).name, "Invoices");
        assert_eq!(Destination::new(PathBuf::from(r"D:\")).name, r"D:\");
    }

    #[test]
    fn folder_matching_ignores_case() {
        let d = Destination::new(PathBuf::from(r"C:\Users\me\Invoices"));
        assert!(d.is_folder(Path::new(r"c:\users\ME\invoices")));
        assert!(!d.is_folder(Path::new(r"C:\Users\me\Invoices 2")));
    }
}
