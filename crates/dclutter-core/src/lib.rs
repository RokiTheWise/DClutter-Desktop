//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! Pure logic shared by every DClutter desktop build: no filesystem access,
//! no Tauri, no Windows APIs. A port of `DClutterCore` from the macOS app.
//!
//! The scoring model is a contract between the two apps. Both run the same
//! cases from `fixtures/` at the repository root, so a change to a weight or
//! a matching rule has to land in both.

mod candidate;
mod context;
mod scorer;
mod session;

pub use candidate::{FileCandidate, Timestamp};
pub use context::QueueContext;
pub use scorer::{QueueScorer, ScoreWeights, ScoredCandidate};
pub use session::{DecisionKind, DiskMove, FileState, Reversal, SessionSnapshot, Step, TriageSession};
