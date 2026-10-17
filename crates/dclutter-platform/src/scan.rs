//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

use std::fs::{self, Metadata};
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use dclutter_core::{FileCandidate, Timestamp};

#[cfg(windows)]
use crate::zone_identifier::{self, ZoneTransfer};

/// The source URL recorded for a file Windows marks as downloaded from the
/// internet without saying from where — Firefox, for one, writes only the
/// zone. It still counts as a download for scoring; the interface should
/// show it as "the internet", not as a link.
pub const UNKNOWN_INTERNET_SOURCE: &str = "about:internet";

/// Reads the files and folders directly inside `folder` (not recursively)
/// as scoring candidates, skipping hidden and system items.
///
/// `last_opened` is always `None` on Windows. NTFS last-access times are
/// often disabled and are bumped by indexers, antivirus and backups, so
/// they say little about whether the user opened a file — the same problem
/// the macOS app found with `contentAccessDate`. Scoring falls back to the
/// modified date.
pub fn candidates_in(folder: &Path) -> io::Result<Vec<FileCandidate>> {
    let mut candidates = Vec::new();

    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let path = entry.path();
        // Follow symlinks and junctions so a linked folder counts as a
        // folder; fall back to the link itself when its target is gone.
        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => entry.metadata()?,
        };
        if is_hidden(&entry.file_name().to_string_lossy(), &metadata) {
            continue;
        }

        let is_directory = metadata.is_dir();
        candidates.push(FileCandidate {
            id: path.to_string_lossy().into_owned(),
            bytes: if is_directory { 0 } else { metadata.len() },
            last_opened: None,
            created: metadata.created().ok().map(timestamp).unwrap_or_else(now),
            modified: metadata.modified().ok().map(timestamp),
            source_url: if is_directory { None } else { read_source_url(&path) },
            is_directory,
            path,
        });
    }

    Ok(candidates)
}

fn timestamp(time: SystemTime) -> Timestamp {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => after.as_secs_f64(),
        Err(before) => -before.duration().as_secs_f64(),
    }
}

fn now() -> Timestamp {
    timestamp(SystemTime::now())
}

#[cfg(windows)]
fn is_hidden(name: &str, metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
    name.starts_with('.')
        || metadata.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
}

#[cfg(not(windows))]
fn is_hidden(name: &str, _metadata: &Metadata) -> bool {
    name.starts_with('.')
}

/// The download URL from the file's `Zone.Identifier` stream: the file's
/// own URL if recorded, else the page it was downloaded from.
#[cfg(windows)]
fn read_source_url(path: &Path) -> Option<String> {
    let mut stream = path.as_os_str().to_owned();
    stream.push(":Zone.Identifier");
    let bytes = fs::read(stream).ok()?;
    let zone = ZoneTransfer::parse(&zone_identifier::decode(&bytes));
    let from_internet = zone.is_from_internet();
    zone.host_url
        .or(zone.referrer_url)
        .or_else(|| from_internet.then(|| UNKNOWN_INTERNET_SOURCE.to_owned()))
}

/// Linux has no equivalent of the Mark of the Web yet.
#[cfg(not(windows))]
fn read_source_url(_path: &Path) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(candidates: &[FileCandidate]) -> Vec<String> {
        let mut names: Vec<String> = candidates.iter().map(|c| c.file_name().to_owned()).collect();
        names.sort();
        names
    }

    #[test]
    fn returns_a_candidate_for_each_item_in_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        fs::write(dir.path().join("b.txt"), "hello world").unwrap();
        fs::create_dir(dir.path().join("Extracted")).unwrap();
        fs::write(dir.path().join("Extracted").join("nested.txt"), "not listed").unwrap();

        let candidates = candidates_in(dir.path()).unwrap();

        assert_eq!(names(&candidates), ["Extracted", "a.txt", "b.txt"]);
    }

    #[test]
    fn reports_sizes_and_flags_folders() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("sized.txt"), "fourteen bytes").unwrap();
        fs::create_dir(dir.path().join("Folder")).unwrap();

        let candidates = candidates_in(dir.path()).unwrap();
        let file = candidates.iter().find(|c| c.file_name() == "sized.txt").unwrap();
        let folder = candidates.iter().find(|c| c.file_name() == "Folder").unwrap();

        assert_eq!(file.bytes, 14);
        assert!(!file.is_directory);
        assert_eq!(folder.bytes, 0);
        assert!(folder.is_directory);
    }

    #[test]
    fn records_created_and_modified_and_never_last_opened() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "x").unwrap();

        let c = &candidates_in(dir.path()).unwrap()[0];

        assert!((now() - c.created).abs() < 60.0);
        assert!((now() - c.modified.unwrap()).abs() < 60.0);
        assert_eq!(c.last_opened, None);
    }

    #[test]
    fn skips_dot_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".hidden"), "x").unwrap();
        fs::write(dir.path().join("shown.txt"), "x").unwrap();

        assert_eq!(names(&candidates_in(dir.path()).unwrap()), ["shown.txt"]);
    }

    #[test]
    fn source_url_is_none_without_a_zone_identifier() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("local.txt"), "x").unwrap();

        assert_eq!(candidates_in(dir.path()).unwrap()[0].source_url, None);
    }

    #[test]
    fn missing_folder_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(candidates_in(&dir.path().join("nope")).is_err());
    }

    #[cfg(windows)]
    mod windows {
        use super::*;
        use std::process::Command;

        fn write_zone_identifier(path: &Path, contents: &str) {
            let mut stream = path.as_os_str().to_owned();
            stream.push(":Zone.Identifier");
            fs::write(stream, contents).unwrap();
        }

        #[test]
        fn skips_hidden_and_system_files() {
            let dir = tempfile::tempdir().unwrap();
            for name in ["hidden.txt", "desktop.ini", "shown.txt"] {
                fs::write(dir.path().join(name), "x").unwrap();
            }
            let status = Command::new("attrib").arg("+h").arg(dir.path().join("hidden.txt")).status().unwrap();
            assert!(status.success());
            let status = Command::new("attrib").arg("+s").arg(dir.path().join("desktop.ini")).status().unwrap();
            assert!(status.success());

            assert_eq!(names(&candidates_in(dir.path()).unwrap()), ["shown.txt"]);
        }

        #[test]
        fn reads_host_url_from_zone_identifier() {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("downloaded.pdf");
            fs::write(&file, "x").unwrap();
            write_zone_identifier(
                &file,
                "[ZoneTransfer]\r\nZoneId=3\r\nReferrerUrl=https://example.com/page\r\nHostUrl=https://example.com/downloaded.pdf\r\n",
            );

            let c = &candidates_in(dir.path()).unwrap()[0];
            assert_eq!(c.source_url.as_deref(), Some("https://example.com/downloaded.pdf"));
        }

        #[test]
        fn falls_back_to_referrer_url() {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("downloaded.pdf");
            fs::write(&file, "x").unwrap();
            write_zone_identifier(&file, "[ZoneTransfer]\r\nZoneId=3\r\nReferrerUrl=https://example.com/page\r\n");

            let c = &candidates_in(dir.path()).unwrap()[0];
            assert_eq!(c.source_url.as_deref(), Some("https://example.com/page"));
        }

        #[test]
        fn internet_zone_without_a_url_still_counts_as_downloaded() {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("firefox.zip");
            fs::write(&file, "x").unwrap();
            write_zone_identifier(&file, "[ZoneTransfer]\r\nZoneId=3\r\n");

            let c = &candidates_in(dir.path()).unwrap()[0];
            assert_eq!(c.source_url.as_deref(), Some(UNKNOWN_INTERNET_SOURCE));
        }

        #[test]
        fn local_zone_without_a_url_is_not_a_download() {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("intranet.txt");
            fs::write(&file, "x").unwrap();
            write_zone_identifier(&file, "[ZoneTransfer]\r\nZoneId=1\r\n");

            assert_eq!(candidates_in(dir.path()).unwrap()[0].source_url, None);
        }
    }
}
