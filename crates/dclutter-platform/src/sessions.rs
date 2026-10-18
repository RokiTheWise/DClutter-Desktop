//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! Where each folder's triage session is saved between launches. Port of
//! `SessionPersistence`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use dclutter_core::SessionSnapshot;

/// One file per source folder, not one overall, so one folder's saved
/// decisions can never be applied to another folder's files.
#[derive(Clone)]
pub struct SessionStore {
    dir: PathBuf,
}

impl SessionStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// `session-<hash>.json`, named by an FNV-1a hash of the folder's path
    /// rather than the path itself, which has separators and can be too
    /// long for a file name. Stable across runs, which a std hasher isn't.
    pub fn file_for(&self, folder: &Path) -> PathBuf {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in normalise(folder).bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        self.dir.join(format!("session-{hash:x}.json"))
    }

    /// The saved session for `folder`, if there's one this build can read.
    /// A missing, corrupt or older-format file means starting fresh: the
    /// cost is re-deciding some files, never a crash.
    pub fn load(&self, folder: &Path) -> Option<SessionSnapshot> {
        let text = fs::read_to_string(self.file_for(folder)).ok()?;
        serde_json::from_str::<SessionSnapshot>(&text).ok().filter(SessionSnapshot::is_current)
    }

    /// Saves through a temporary file, so a crash mid-write can't leave a
    /// half-written session.
    pub fn save(&self, folder: &Path, snapshot: &SessionSnapshot) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let file = self.file_for(folder);
        let temp = file.with_extension("json.tmp");
        fs::write(&temp, serde_json::to_vec(snapshot).map_err(io::Error::other)?)?;
        fs::rename(&temp, &file)
    }
}

/// The same folder however it's spelled: Windows paths are case
/// insensitive, and a trailing separator doesn't change the folder.
fn normalise(folder: &Path) -> String {
    let path = folder.to_string_lossy();
    let trimmed = path.trim_end_matches(['\\', '/']);
    if cfg!(windows) { trimmed.to_lowercase() } else { trimmed.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutter_core::{DecisionKind, FileCandidate, TriageSession};

    fn store() -> (tempfile::TempDir, SessionStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path().join("sessions"));
        (dir, store)
    }

    fn session() -> TriageSession {
        let queue = ["a", "b"]
            .iter()
            .map(|name| FileCandidate {
                id: (*name).to_owned(),
                path: PathBuf::from(name),
                bytes: 1024,
                last_opened: None,
                created: 0.0,
                modified: None,
                source_url: None,
                is_directory: false,
            })
            .collect();
        let mut s = TriageSession::new(queue);
        s.decide(DecisionKind::Keep);
        s
    }

    #[test]
    fn nothing_saved_loads_as_none() {
        let (_dir, store) = store();
        assert!(store.load(Path::new(r"C:\Users\me\Downloads")).is_none());
    }

    #[test]
    fn saves_and_loads_a_session() {
        let (_dir, store) = store();
        let folder = Path::new(r"C:\Users\me\Downloads");
        let snapshot = session().snapshot();
        store.save(folder, &snapshot).unwrap();
        assert_eq!(store.load(folder), Some(snapshot));
    }

    #[test]
    fn each_folder_has_its_own_session() {
        let (_dir, store) = store();
        store.save(Path::new(r"C:\Users\me\Downloads"), &session().snapshot()).unwrap();
        assert!(store.load(Path::new(r"C:\Users\me\Desktop")).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn the_same_folder_spelled_differently_shares_a_session() {
        let (_dir, store) = store();
        assert_eq!(
            store.file_for(Path::new(r"C:\Users\me\Downloads")),
            store.file_for(Path::new(r"c:\users\ME\downloads\")),
        );
    }

    #[test]
    fn file_names_are_stable_across_runs() {
        let store = SessionStore::new(PathBuf::from("sessions"));
        // Already lowercase, so the expected hash is the same on every
        // platform. Worked out independently of this code.
        let name = store.file_for(Path::new(r"c:\users\me\downloads"));
        assert_eq!(name, PathBuf::from("sessions").join("session-e240aceb6fbfb787.json"));
    }

    #[test]
    fn a_corrupt_file_loads_as_none() {
        let (_dir, store) = store();
        let folder = Path::new(r"C:\Users\me\Downloads");
        store.save(folder, &session().snapshot()).unwrap();
        fs::write(store.file_for(folder), "{ not json").unwrap();
        assert!(store.load(folder).is_none());
    }
}
