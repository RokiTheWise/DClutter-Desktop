//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

use std::path::PathBuf;
use std::sync::LazyLock;

use regex::{Regex, RegexSet};
use serde::{Deserialize, Serialize};

/// Seconds since the Unix epoch, fractional, matching Foundation's `Date`.
pub type Timestamp = f64;

/// A single file under consideration, stripped of any platform specifics.
///
/// `last_opened` is optional by design: Windows does not reliably track when
/// a file was last opened, so the Windows metadata provider usually leaves it
/// `None` and scoring falls back to `modified`, then `created`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCandidate {
    pub id: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub last_opened: Option<Timestamp>,
    pub created: Timestamp,
    /// Filesystem modification time. Fallback for staleness when
    /// `last_opened` is unavailable — see `QueueScorer`.
    pub modified: Option<Timestamp>,
    /// Where the file was downloaded from. On Windows this comes from the
    /// `Zone.Identifier` stream; on macOS from `kMDItemWhereFroms`.
    pub source_url: Option<String>,
    pub is_directory: bool,
}

static NUMERIC_STEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+$").unwrap());

static GENERIC_STEMS: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r"(?i)^img[_-]?\d+$",
        r"(?i)^img[_-].*$",
        r"(?i)^dsc[_-]?\d+$",
        r"(?i)^screenshot.*$",
        r"(?i)^screen ?shot.*$",
        r"(?i)^download(s)?( ?\(\d+\))?$",
        r"(?i)^untitled( \d+)?$",
        r"(?i)^new document( \d+)?$",
        r"(?i)^document( \d+)?$",
        r"(?i)^unnamed( \d+)?$",
        r"(?i)^scan ?\d*.*$",
        r"(?i)^file( \d+)?$",
    ])
    .unwrap()
});

impl FileCandidate {
    /// The last path component, e.g. `Report (2).pdf`.
    pub fn file_name(&self) -> &str {
        self.path.file_name().and_then(|n| n.to_str()).unwrap_or("")
    }

    /// The file name without its final extension, e.g. `archive.tar` for
    /// `archive.tar.gz`.
    pub fn stem(&self) -> &str {
        self.path.file_stem().and_then(|n| n.to_str()).unwrap_or("")
    }

    /// The final extension without the dot, or `""` when there is none.
    pub fn extension(&self) -> &str {
        self.path.extension().and_then(|n| n.to_str()).unwrap_or("")
    }

    /// Whether the filename looks auto-generated rather than user-chosen —
    /// camera/screenshot patterns, bare "download"/"untitled"/"document",
    /// or a purely numeric name. Feeds the ambiguous-name penalty, which
    /// only applies alongside a missing source URL.
    pub fn has_generic_name(&self) -> bool {
        let stem = self.stem().trim_matches(is_horizontal_whitespace);
        stem.is_empty() || NUMERIC_STEM.is_match(stem) || GENERIC_STEMS.is_match(stem)
    }
}

/// Spaces and tabs, but not line breaks — what Foundation's
/// `CharacterSet.whitespaces` trims.
fn is_horizontal_whitespace(c: char) -> bool {
    c == '\t' || (c.is_whitespace() && !c.is_control() && c != '\u{2028}' && c != '\u{2029}')
}
