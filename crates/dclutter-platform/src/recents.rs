//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! The folders the user has chosen to sort, for the folder menu. Port of
//! `SourceFolderStore`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::Destination;

/// Small on purpose: a shortcut back to somewhere you were, not a file
/// browser.
pub const MAX_RECENT_FOLDERS: usize = 5;

pub struct RecentFolders {
    file: PathBuf,
}

impl RecentFolders {
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }

    /// Most recent first. Folders that no longer exist are left out rather
    /// than offered as somewhere that can't be opened.
    pub fn load(&self) -> Vec<PathBuf> {
        self.stored().into_iter().filter(|p| p.is_dir()).collect()
    }

    /// Puts `folder` at the top of the list.
    pub fn remember(&self, folder: &Path) -> io::Result<()> {
        let list = merged(self.stored(), folder);
        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(&list).map_err(io::Error::other)?;
        let temp = self.file.with_extension("json.tmp");
        fs::write(&temp, json)?;
        fs::rename(&temp, &self.file)
    }

    fn stored(&self) -> Vec<PathBuf> {
        fs::read_to_string(&self.file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }
}

/// Newest first, no duplicates (ignoring case, as Windows does), capped.
fn merged(existing: Vec<PathBuf>, folder: &Path) -> Vec<PathBuf> {
    let same = |p: &PathBuf| Destination::new(p.clone()).is_folder(folder);
    let mut list: Vec<PathBuf> = existing.into_iter().filter(|p| !same(p)).collect();
    list.insert(0, folder.to_owned());
    list.truncate(MAX_RECENT_FOLDERS);
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, RecentFolders) {
        let dir = tempfile::tempdir().unwrap();
        let store = RecentFolders::new(dir.path().join("config").join("recent-folders.json"));
        (dir, store)
    }

    fn folder(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn starts_empty() {
        let (_dir, store) = store();
        assert!(store.load().is_empty());
    }

    #[test]
    fn newest_first_without_duplicates() {
        let (dir, store) = store();
        let (a, b) = (folder(dir.path(), "A"), folder(dir.path(), "B"));
        store.remember(&a).unwrap();
        store.remember(&b).unwrap();
        store.remember(&a).unwrap();
        assert_eq!(store.load(), [a, b]);
    }

    #[cfg(windows)]
    #[test]
    fn the_same_folder_spelled_differently_isnt_listed_twice() {
        let (dir, store) = store();
        let a = folder(dir.path(), "Photos");
        store.remember(&a).unwrap();
        store.remember(&PathBuf::from(a.to_string_lossy().to_uppercase())).unwrap();
        assert_eq!(store.load().len(), 1);
    }

    #[test]
    fn keeps_at_most_five() {
        let (dir, store) = store();
        let folders: Vec<_> = (0..7).map(|i| folder(dir.path(), &format!("F{i}"))).collect();
        for f in &folders {
            store.remember(f).unwrap();
        }
        let loaded = store.load();
        assert_eq!(loaded.len(), MAX_RECENT_FOLDERS);
        assert_eq!(loaded[0], folders[6]);
    }

    #[test]
    fn folders_that_are_gone_are_left_out() {
        let (dir, store) = store();
        let (a, b) = (folder(dir.path(), "A"), folder(dir.path(), "B"));
        store.remember(&a).unwrap();
        store.remember(&b).unwrap();
        fs::remove_dir(&b).unwrap();
        assert_eq!(store.load(), [a]);
    }
}
