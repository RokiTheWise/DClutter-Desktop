//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! Runs the cases in `fixtures/` at the repository root. The macOS app runs
//! the same files; see `fixtures/README.md` for the format.

use std::path::{Path, PathBuf};

use dclutter_core::{FileCandidate, QueueContext, QueueScorer, ScoreWeights};
use serde::Deserialize;

const SECONDS_PER_DAY: f64 = 86_400.0;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueFixture {
    schema_version: u32,
    now: f64,
    tolerance: f64,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    name: String,
    #[serde(default)]
    weights: ScoreWeights,
    candidates: Vec<CandidateSpec>,
    expect: Vec<Expectation>,
    expect_rank: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidateSpec {
    id: String,
    name: String,
    bytes: u64,
    #[serde(default)]
    is_directory: bool,
    #[serde(default)]
    created_days_ago: f64,
    modified_days_ago: Option<f64>,
    last_opened_days_ago: Option<f64>,
    #[serde(rename = "sourceURL")]
    source_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Expectation {
    id: String,
    score: Option<f64>,
    redundant_copy: Option<bool>,
    extracted_archive: Option<bool>,
    duplicate_count: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenericNamesFixture {
    schema_version: u32,
    generic: Vec<String>,
    descriptive: Vec<String>,
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn load<T: for<'de> Deserialize<'de>>(file: &str) -> T {
    let path = fixtures_dir().join(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parsing {}: {e}", path.display()))
}

fn named(name: &str) -> FileCandidate {
    FileCandidate {
        id: name.to_owned(),
        path: PathBuf::from("downloads").join(name),
        bytes: 1024,
        last_opened: None,
        created: 0.0,
        modified: None,
        source_url: None,
        is_directory: false,
    }
}

#[test]
fn queue_cases() {
    let fixture: QueueFixture = load("queue.json");
    assert_eq!(fixture.schema_version, 1, "unsupported queue.json schema");

    let days_ago = |days: f64| fixture.now - days * SECONDS_PER_DAY;
    let mut failures = Vec::new();

    for case in &fixture.cases {
        let candidates: Vec<FileCandidate> = case
            .candidates
            .iter()
            .map(|spec| FileCandidate {
                id: spec.id.clone(),
                path: PathBuf::from("downloads").join(&spec.name),
                bytes: spec.bytes,
                last_opened: spec.last_opened_days_ago.map(days_ago),
                created: days_ago(spec.created_days_ago),
                modified: spec.modified_days_ago.map(days_ago),
                source_url: spec.source_url.clone(),
                is_directory: spec.is_directory,
            })
            .collect();
        let ctx = QueueContext::new(&candidates);

        for expected in &case.expect {
            let c = candidates
                .iter()
                .find(|c| c.id == expected.id)
                .unwrap_or_else(|| panic!("{}: no candidate {}", case.name, expected.id));
            let id = &expected.id;

            if let Some(want) = expected.score {
                let got = QueueScorer::score(c, &ctx, fixture.now, &case.weights);
                if (got - want).abs() > fixture.tolerance {
                    failures.push(format!("{}: {id} score {got}, expected {want}", case.name));
                }
            }
            if let Some(want) = expected.redundant_copy {
                let got = ctx.is_redundant_copy(c);
                if got != want {
                    failures.push(format!("{}: {id} redundantCopy {got}, expected {want}", case.name));
                }
            }
            if let Some(want) = expected.extracted_archive {
                let got = ctx.is_extracted_archive(c);
                if got != want {
                    failures.push(format!("{}: {id} extractedArchive {got}, expected {want}", case.name));
                }
            }
            if let Some(want) = expected.duplicate_count {
                let got = ctx.duplicate_count(c);
                if got != want {
                    failures.push(format!("{}: {id} duplicateCount {got}, expected {want}", case.name));
                }
            }
        }

        if let Some(want) = &case.expect_rank {
            let got: Vec<String> = QueueScorer::rank(candidates.clone(), fixture.now, &case.weights)
                .into_iter()
                .map(|s| s.candidate.id)
                .collect();
            if &got != want {
                failures.push(format!("{}: rank {got:?}, expected {want:?}", case.name));
            }
        }
    }

    assert!(failures.is_empty(), "{} failing expectation(s):\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn generic_name_cases() {
    let fixture: GenericNamesFixture = load("generic-names.json");
    assert_eq!(fixture.schema_version, 1, "unsupported generic-names.json schema");

    let mut failures = Vec::new();
    for name in &fixture.generic {
        if !named(name).has_generic_name() {
            failures.push(format!("expected {name:?} to be generic"));
        }
    }
    for name in &fixture.descriptive {
        if named(name).has_generic_name() {
            failures.push(format!("expected {name:?} to be descriptive"));
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
