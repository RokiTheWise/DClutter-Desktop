//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

use serde::{Deserialize, Serialize};

use crate::{FileCandidate, QueueContext, Timestamp};

const SECONDS_PER_DAY: f64 = 86_400.0;

/// Tuning knobs. The model is additive by design — an earlier
/// multiplicative version let staleness annihilate size — so every signal
/// contributes points independently; none can zero out another.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScoreWeights {
    pub stale: f64,
    pub unopened: f64,
    pub duplicate: f64,
    pub archive: f64,
    pub ambiguous: f64,
    /// Days of staleness at which the stale term reaches its cap.
    pub half_life: f64,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            stale: 10.0,
            unopened: 2.0,
            duplicate: 8.0,
            archive: 4.0,
            ambiguous: 5.0,
            half_life: 90.0,
        }
    }
}

/// A candidate paired with the score that placed it in the queue.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScoredCandidate {
    pub candidate: FileCandidate,
    pub score: f64,
}

/// Orders `FileCandidate`s by how actionable they are. Port target, not a
/// design surface: weights and rules must match the macOS app, and the
/// shared fixtures enforce that.
pub struct QueueScorer;

impl QueueScorer {
    pub fn score(
        c: &FileCandidate,
        ctx: &QueueContext,
        now: Timestamp,
        weights: &ScoreWeights,
    ) -> f64 {
        let mut points = 0.0;

        // Size: floor at 1 KB, not 1 MB — a 1 MB floor ties too many files.
        points += ((c.bytes.max(1024) as f64).log2() - 10.0).max(0.0);

        let reference = c.last_opened.or(c.modified).unwrap_or(c.created);
        let days = (now - reference) / SECONDS_PER_DAY;
        points += weights.stale * (days / weights.half_life).clamp(0.0, 1.0);

        if c.last_opened.is_none() && c.source_url.is_some() {
            points += weights.unopened;
        }
        if ctx.is_redundant_copy(c) {
            points += weights.duplicate;
        }
        if ctx.is_extracted_archive(c) {
            points += weights.archive;
        }
        if c.source_url.is_none() && c.has_generic_name() {
            points -= weights.ambiguous;
        }

        points
    }

    /// Scores every candidate against a shared `QueueContext` and returns
    /// them ordered highest score first — the presentation order for triage.
    pub fn rank(
        candidates: Vec<FileCandidate>,
        now: Timestamp,
        weights: &ScoreWeights,
    ) -> Vec<ScoredCandidate> {
        let ctx = QueueContext::new(&candidates);
        let mut scored: Vec<ScoredCandidate> = candidates
            .into_iter()
            .map(|candidate| {
                let score = Self::score(&candidate, &ctx, now, weights);
                ScoredCandidate { candidate, score }
            })
            .collect();
        scored.sort_by(|a, b| b.score.total_cmp(&a.score));
        scored
    }
}
