//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::FileCandidate;

/// Precomputed cross-candidate facts the scorer needs: which files are
/// redundant copies of another, and which archives have already been
/// extracted beside themselves.
#[derive(Debug, Clone, Default)]
pub struct QueueContext {
    redundant_copy_ids: HashSet<String>,
    extracted_archive_ids: HashSet<String>,
    cluster_size_by_id: HashMap<String, usize>,
}

impl QueueContext {
    pub fn new(candidates: &[FileCandidate]) -> Self {
        let (redundant_copy_ids, cluster_size_by_id) = find_duplicate_info(candidates);
        Self {
            redundant_copy_ids,
            extracted_archive_ids: find_extracted_archives(candidates),
            cluster_size_by_id,
        }
    }

    pub fn is_redundant_copy(&self, candidate: &FileCandidate) -> bool {
        self.redundant_copy_ids.contains(&candidate.id)
    }

    pub fn is_extracted_archive(&self, candidate: &FileCandidate) -> bool {
        self.extracted_archive_ids.contains(&candidate.id)
    }

    /// How many files share this one's duplicate cluster, itself included.
    pub fn duplicate_count(&self, candidate: &FileCandidate) -> usize {
        self.cluster_size_by_id.get(&candidate.id).copied().unwrap_or(1)
    }
}

// Duplicate detection (filename + size, no hashing)

static PAREN_SUFFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s\(\d+\)$").unwrap());
static COPY_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s+copy(\s+\d+)?$").unwrap());

/// Strips copy suffixes: `foo (2)` -> `foo`, `foo copy 3` -> `foo`.
/// Returns the stripped stem and whether a suffix was removed.
fn normalize(stem: &str) -> (&str, bool) {
    let mut result = stem;
    let mut had_suffix = false;

    if let Some(m) = PAREN_SUFFIX.find(result) {
        result = &result[..m.start()];
        had_suffix = true;
    }
    if let Some(m) = COPY_SUFFIX.find(result) {
        result = &result[..m.start()];
        had_suffix = true;
    }

    (result, had_suffix)
}

fn find_duplicate_info(
    candidates: &[FileCandidate],
) -> (HashSet<String>, HashMap<String, usize>) {
    // Members keep input order within a cluster so keeper tie-breaks match
    // the macOS app, which also takes the first of equally old files.
    let mut clusters: HashMap<(String, u64), Vec<(&FileCandidate, bool)>> = HashMap::new();

    for c in candidates.iter().filter(|c| !c.is_directory) {
        let (stem, had_copy_suffix) = normalize(c.stem());
        let ext = c.extension();
        let normalized_name = if ext.is_empty() {
            stem.to_lowercase()
        } else {
            format!("{}.{}", stem.to_lowercase(), ext.to_lowercase())
        };
        // Floor, not round: rounding can put two members of the same
        // duplicate pair on opposite sides of a .5 KB boundary and split
        // them into separate clusters.
        let bucketed_kb = c.bytes / 1024;
        clusters
            .entry((normalized_name, bucketed_kb))
            .or_default()
            .push((c, had_copy_suffix));
    }

    let mut redundant = HashSet::new();
    let mut cluster_size = HashMap::new();
    for members in clusters.values().filter(|m| m.len() > 1) {
        let unsuffixed: Vec<&FileCandidate> = members
            .iter()
            .filter(|(_, had_suffix)| !had_suffix)
            .map(|(c, _)| *c)
            .collect();
        // `min_by` returns the first of equal minimums, like Swift's `min`.
        let keeper = match unsuffixed.as_slice() {
            [only] => *only,
            [] => members
                .iter()
                .map(|(c, _)| *c)
                .min_by(|a, b| a.created.total_cmp(&b.created))
                .unwrap(),
            several => several
                .iter()
                .copied()
                .min_by(|a, b| a.created.total_cmp(&b.created))
                .unwrap(),
        };
        for (member, _) in members {
            cluster_size.insert(member.id.clone(), members.len());
            if member.id != keeper.id {
                redundant.insert(member.id.clone());
            }
        }
    }
    (redundant, cluster_size)
}

// Extracted archive detection

fn find_extracted_archives(candidates: &[FileCandidate]) -> HashSet<String> {
    let directory_names: HashSet<String> = candidates
        .iter()
        .filter(|c| c.is_directory)
        .map(|c| c.file_name().to_lowercase())
        .collect();

    candidates
        .iter()
        .filter(|c| !c.is_directory && c.extension().eq_ignore_ascii_case("zip"))
        .filter(|c| directory_names.contains(&c.stem().to_lowercase()))
        .map(|c| c.id.clone())
        .collect()
}
