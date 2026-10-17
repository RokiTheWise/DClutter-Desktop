//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! Prints a folder's triage queue, for checking scores against real data.
//!
//!     cargo run -p dclutter-platform --example rank -- "C:\Users\me\Downloads" [limit]

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use dclutter_core::{QueueContext, QueueScorer, ScoreWeights};

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let folder = PathBuf::from(args.next().expect("usage: rank <folder> [limit]"));
    let limit: usize = args.next().map_or(usize::MAX, |n| n.parse().expect("limit must be a number"));

    let candidates = dclutter_platform::candidates_in(&folder)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64();
    let ctx = QueueContext::new(&candidates);

    let downloaded = candidates.iter().filter(|c| c.source_url.is_some()).count();
    let copies = candidates.iter().filter(|c| ctx.is_redundant_copy(c)).count();
    let extracted = candidates.iter().filter(|c| ctx.is_extracted_archive(c)).count();
    println!(
        "{}: {} items, {downloaded} with a download source, {copies} redundant copies, {extracted} extracted archives\n",
        folder.display(),
        candidates.len()
    );

    for scored in QueueScorer::rank(candidates, now, &ScoreWeights::default()).iter().take(limit) {
        let c = &scored.candidate;
        let mut flags = Vec::new();
        if c.is_directory {
            flags.push("folder");
        }
        if c.source_url.is_some() {
            flags.push("downloaded");
        }
        if ctx.is_redundant_copy(c) {
            flags.push("copy");
        }
        if ctx.is_extracted_archive(c) {
            flags.push("extracted");
        }
        if c.source_url.is_none() && c.has_generic_name() {
            flags.push("generic");
        }
        println!("{:>6.1}  {:<50}  {}", scored.score, c.file_name(), flags.join(", "));
    }
    Ok(())
}
