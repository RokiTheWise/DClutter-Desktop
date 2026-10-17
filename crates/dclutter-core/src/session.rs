//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::FileCandidate;

/// Where a file stands in the session.
///
/// `Staged` means "marked for trash, not yet trashed". `Trashed` is the
/// terminal state after the Recycle Bin took the file, kept distinct so
/// undo can never bring back a card for a file that is gone. Moving and
/// renaming add their own states later, as in the macOS app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileState {
    Pending,
    Kept,
    Staged,
    Trashed,
}

/// The three decisions the triage card offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DecisionKind {
    Keep,
    Stage,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Decision {
    kind: DecisionKind,
    id: String,
}

/// The triage state machine, ported from `DClutterSession` in the macOS app.
///
/// `queue` order is fixed when the session starts (the caller passes an
/// already-ranked list). Skipping reorders logically through `deferred`
/// and never changes `queue` itself.
#[derive(Debug, Clone, Default)]
pub struct TriageSession {
    queue: Vec<FileCandidate>,
    /// Decided files only; anything missing is pending.
    states: HashMap<String, FileState>,
    /// In deferral order: skipping moves a file to the end, so skipping an
    /// already-deferred file rotates it further back instead of doing
    /// nothing (the macOS app once hung on that case).
    deferred: Vec<String>,
    history: Vec<Decision>,
    /// Decisions undone and not yet re-applied. Any fresh decision clears
    /// it: branching discards the undone future.
    redo_stack: Vec<Decision>,
    /// Decisions since the last commit. A commit is the natural checkpoint:
    /// "67 sorted" means nothing once those files are gone.
    sorted: usize,
}

impl TriageSession {
    pub fn new(queue: Vec<FileCandidate>) -> Self {
        Self { queue, ..Self::default() }
    }

    pub fn state(&self, id: &str) -> FileState {
        self.states.get(id).copied().unwrap_or(FileState::Pending)
    }

    fn is_pending(&self, id: &str) -> bool {
        self.state(id) == FileState::Pending
    }

    /// Pending files in the order they will be shown: undeferred files in
    /// queue order, then deferred files in the order they were skipped.
    fn pending_in_order(&self) -> impl Iterator<Item = &FileCandidate> {
        let undeferred = self.queue.iter().filter(|c| !self.deferred.contains(&c.id));
        let deferred = self
            .deferred
            .iter()
            .filter_map(|id| self.queue.iter().find(|c| &c.id == id));
        undeferred.chain(deferred).filter(|c| self.is_pending(&c.id))
    }

    /// The file on the card now, or `None` when every file has a decision.
    pub fn current(&self) -> Option<&FileCandidate> {
        self.pending_in_order().next()
    }

    /// The file that will be shown after the current one, if any.
    pub fn up_next(&self) -> Option<&FileCandidate> {
        self.pending_in_order().nth(1)
    }

    pub fn remaining_count(&self) -> usize {
        self.queue.iter().filter(|c| self.is_pending(&c.id)).count()
    }

    pub fn total_count(&self) -> usize {
        self.queue.len()
    }

    /// Kept or staged decisions since the last commit. Skips don't count:
    /// a skipped file still needs a decision.
    pub fn sorted_count(&self) -> usize {
        self.sorted
    }

    /// Files moved to the Recycle Bin this session.
    pub fn trashed_count(&self) -> usize {
        self.states.values().filter(|s| **s == FileState::Trashed).count()
    }

    /// Files marked for trash, in queue order.
    pub fn staged(&self) -> Vec<&FileCandidate> {
        self.queue.iter().filter(|c| self.state(&c.id) == FileState::Staged).collect()
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Applies `kind` to the current file. Returns `false`, changing
    /// nothing, when there is no current file.
    pub fn decide(&mut self, kind: DecisionKind) -> bool {
        let Some(id) = self.current().map(|c| c.id.clone()) else { return false };
        let decision = Decision { kind, id };
        self.apply(&decision);
        // A skip that doesn't change the visible file (it was the only one
        // left) shouldn't leave a dead undo entry, or undo appears to do
        // nothing for several presses in a row.
        let changed_card = kind != DecisionKind::Skip || self.current().map(|c| &c.id) != Some(&decision.id);
        if changed_card {
            self.history.push(decision);
        }
        self.redo_stack.clear();
        true
    }

    /// Reverts the most recent decision and returns what it was, so the
    /// interface can bring the card back from the way it left.
    pub fn undo(&mut self) -> Option<DecisionKind> {
        let decision = self.history.pop()?;
        self.revert(&decision);
        let kind = decision.kind;
        self.redo_stack.push(decision);
        Some(kind)
    }

    /// Re-applies the most recently undone decision.
    pub fn redo(&mut self) -> Option<DecisionKind> {
        let decision = self.redo_stack.pop()?;
        self.apply(&decision);
        let kind = decision.kind;
        self.history.push(decision);
        Some(kind)
    }

    /// Un-stages files the user unticked when reviewing the trash. They
    /// become kept, not pending: unticking means "keep this one", not "ask
    /// me again", so the file shouldn't come back as a card.
    pub fn unstage<'a>(&mut self, ids: impl IntoIterator<Item = &'a str>) {
        for id in ids {
            if self.state(id) == FileState::Staged {
                self.states.insert(id.to_owned(), FileState::Kept);
            }
        }
    }

    /// Records that these staged files are now in the Recycle Bin. Clears
    /// undo and redo entirely: a trashed file can't be brought back by
    /// undo, and older entries make no sense to reverse on their own once
    /// the commit has closed the session.
    pub fn commit_trashed<'a>(&mut self, ids: impl IntoIterator<Item = &'a str>) {
        let mut trashed_any = false;
        for id in ids {
            if self.state(id) == FileState::Staged {
                self.states.insert(id.to_owned(), FileState::Trashed);
                trashed_any = true;
            }
        }
        if trashed_any {
            self.history.clear();
            self.redo_stack.clear();
            self.sorted = 0;
        }
    }

    fn apply(&mut self, decision: &Decision) {
        let id = &decision.id;
        match decision.kind {
            DecisionKind::Keep => self.set_decided(id, FileState::Kept),
            DecisionKind::Stage => self.set_decided(id, FileState::Staged),
            DecisionKind::Skip => {
                self.deferred.retain(|d| d != id);
                self.deferred.push(id.clone());
            }
        }
    }

    fn revert(&mut self, decision: &Decision) {
        let id = &decision.id;
        match decision.kind {
            DecisionKind::Keep | DecisionKind::Stage => {
                self.states.remove(id);
                self.sorted = self.sorted.saturating_sub(1);
            }
            DecisionKind::Skip => self.deferred.retain(|d| d != id),
        }
    }

    fn set_decided(&mut self, id: &str, state: FileState) {
        self.states.insert(id.to_owned(), state);
        self.sorted += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use DecisionKind::{Keep, Skip, Stage};

    fn session(names: &[&str]) -> TriageSession {
        TriageSession::new(
            names
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
                .collect(),
        )
    }

    fn current(s: &TriageSession) -> Option<&str> {
        s.current().map(|c| c.id.as_str())
    }

    #[test]
    fn current_is_the_first_file_in_queue_order() {
        let s = session(&["a", "b", "c"]);
        assert_eq!(current(&s), Some("a"));
        assert_eq!(s.up_next().map(|c| c.id.as_str()), Some("b"));
        assert_eq!(s.remaining_count(), 3);
        assert_eq!(s.total_count(), 3);
    }

    #[test]
    fn an_empty_queue_has_no_current_file() {
        let mut s = session(&[]);
        assert_eq!(current(&s), None);
        assert!(!s.decide(Keep));
        assert!(!s.can_undo());
    }

    #[test]
    fn keep_marks_the_file_and_advances() {
        let mut s = session(&["a", "b"]);
        assert!(s.decide(Keep));
        assert_eq!(s.state("a"), FileState::Kept);
        assert_eq!(current(&s), Some("b"));
        assert_eq!(s.remaining_count(), 1);
    }

    #[test]
    fn stage_marks_the_file_and_advances() {
        let mut s = session(&["a", "b"]);
        s.decide(Stage);
        assert_eq!(s.state("a"), FileState::Staged);
        assert_eq!(s.staged().iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["a"]);
        assert_eq!(current(&s), Some("b"));
    }

    #[test]
    fn skip_defers_to_the_end_without_deciding() {
        let mut s = session(&["a", "b", "c"]);
        s.decide(Skip);
        assert_eq!(s.state("a"), FileState::Pending);
        assert_eq!(current(&s), Some("b"));
        s.decide(Keep);
        s.decide(Keep);
        assert_eq!(current(&s), Some("a"));
        assert_eq!(s.remaining_count(), 1);
    }

    #[test]
    fn files_skipped_first_come_back_first() {
        let mut s = session(&["a", "b", "c"]);
        s.decide(Skip);
        s.decide(Skip);
        s.decide(Keep); // c
        assert_eq!(current(&s), Some("a"));
        s.decide(Keep);
        assert_eq!(current(&s), Some("b"));
    }

    #[test]
    fn skipping_an_already_deferred_file_rotates_it_further_back() {
        let mut s = session(&["a", "b"]);
        s.decide(Skip); // a deferred
        s.decide(Skip); // b deferred, a comes back
        assert_eq!(current(&s), Some("a"));
        s.decide(Skip); // a re-deferred behind b
        assert_eq!(current(&s), Some("b"));
    }

    #[test]
    fn skipping_the_only_pending_file_adds_no_undo_entry() {
        let mut s = session(&["a", "b"]);
        s.decide(Keep);
        s.decide(Skip);
        s.decide(Skip);
        assert_eq!(current(&s), Some("b"));
        assert_eq!(s.undo(), Some(Keep));
        assert!(!s.can_undo());
    }

    #[test]
    fn undo_reverts_a_keep() {
        let mut s = session(&["a", "b"]);
        s.decide(Keep);
        assert_eq!(s.undo(), Some(Keep));
        assert_eq!(s.state("a"), FileState::Pending);
        assert_eq!(current(&s), Some("a"));
    }

    #[test]
    fn undo_reverts_a_skip() {
        let mut s = session(&["a", "b"]);
        s.decide(Skip);
        assert_eq!(s.undo(), Some(Skip));
        assert_eq!(current(&s), Some("a"));
    }

    #[test]
    fn undo_with_no_history_does_nothing() {
        let mut s = session(&["a"]);
        assert_eq!(s.undo(), None);
        assert_eq!(current(&s), Some("a"));
    }

    #[test]
    fn undo_walks_back_through_several_decisions() {
        let mut s = session(&["a", "b", "c"]);
        s.decide(Keep);
        s.decide(Stage);
        assert_eq!(s.undo(), Some(Stage));
        assert_eq!(current(&s), Some("b"));
        assert_eq!(s.undo(), Some(Keep));
        assert_eq!(current(&s), Some("a"));
        assert!(!s.can_undo());
    }

    #[test]
    fn redo_reapplies_undone_decisions() {
        let mut s = session(&["a", "b", "c", "d"]);
        s.decide(Keep);
        s.decide(Stage);
        s.decide(Skip);
        s.undo();
        s.undo();
        s.undo();
        assert_eq!(current(&s), Some("a"));
        assert!(!s.can_undo());

        assert_eq!(s.redo(), Some(Keep));
        assert_eq!(s.state("a"), FileState::Kept);
        assert_eq!(s.redo(), Some(Stage));
        assert_eq!(s.state("b"), FileState::Staged);
        assert_eq!(s.redo(), Some(Skip));
        assert_eq!(current(&s), Some("d"));
        assert!(!s.can_redo());
    }

    #[test]
    fn a_new_decision_clears_redo() {
        let mut s = session(&["a", "b"]);
        s.decide(Keep);
        s.undo();
        assert!(s.can_redo());
        s.decide(Stage);
        assert!(!s.can_redo());
        assert_eq!(s.redo(), None);
    }

    #[test]
    fn committing_trashes_staged_files_and_closes_the_session() {
        let mut s = session(&["a", "b", "c"]);
        s.decide(Stage);
        s.decide(Keep);
        s.undo();
        s.commit_trashed(["a"]);

        assert_eq!(s.state("a"), FileState::Trashed);
        assert_eq!(s.trashed_count(), 1);
        assert!(s.staged().is_empty());
        assert!(!s.can_undo());
        assert!(!s.can_redo());
        assert_eq!(s.sorted_count(), 0);
        assert_eq!(s.remaining_count(), 2);
    }

    #[test]
    fn committing_ignores_files_that_were_not_staged() {
        let mut s = session(&["a", "b"]);
        s.decide(Keep);
        s.commit_trashed(["a", "b"]);
        assert_eq!(s.state("a"), FileState::Kept);
        assert_eq!(s.state("b"), FileState::Pending);
    }

    #[test]
    fn committing_nothing_keeps_undo_history() {
        let mut s = session(&["a", "b"]);
        s.decide(Stage);
        s.commit_trashed([]);
        assert!(s.can_undo());
        assert_eq!(s.sorted_count(), 1);
    }

    #[test]
    fn a_trashed_file_never_comes_back() {
        let mut s = session(&["a", "b"]);
        s.decide(Stage);
        s.commit_trashed(["a"]);
        assert_eq!(s.undo(), None);
        assert_eq!(current(&s), Some("b"));
        s.decide(Keep);
        assert_eq!(current(&s), None);
    }

    #[test]
    fn unstaging_keeps_the_file_instead_of_requeueing_it() {
        let mut s = session(&["a", "b"]);
        s.decide(Stage);
        s.decide(Keep);
        s.unstage(["a", "b"]);
        assert_eq!(s.state("a"), FileState::Kept);
        assert_eq!(s.state("b"), FileState::Kept);
        assert!(s.staged().is_empty());
        assert_eq!(current(&s), None);
    }

    #[test]
    fn sorted_count_follows_decisions_undo_and_redo_but_not_skips() {
        let mut s = session(&["a", "b", "c"]);
        s.decide(Keep);
        s.decide(Skip);
        s.decide(Stage);
        assert_eq!(s.sorted_count(), 2);
        s.undo();
        assert_eq!(s.sorted_count(), 1);
        s.undo(); // the skip
        assert_eq!(s.sorted_count(), 1);
        s.redo();
        s.redo();
        assert_eq!(s.sorted_count(), 2);
    }
}
