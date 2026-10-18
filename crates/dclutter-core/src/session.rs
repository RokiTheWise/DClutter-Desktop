//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::FileCandidate;

/// Where a file stands in the session.
///
/// `Staged` means "marked for trash, not yet trashed". `Trashed` is the
/// terminal state after the Recycle Bin took the file, kept distinct so
/// undo can never bring back a card for a file that is gone. `Moved` means
/// already filed into a destination folder; unlike staging it happens at
/// once, and undo moves the file back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileState {
    Pending,
    Kept,
    Staged,
    Trashed,
    Moved,
}

/// The decisions the triage card offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DecisionKind {
    Keep,
    Stage,
    Skip,
    /// Filed into a destination folder.
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Decision {
    kind: DecisionKind,
    id: String,
    /// Where a filed file now is. Set only for `File`.
    moved_to: Option<PathBuf>,
}

/// Disk work the caller must do for the folder to match the session. The
/// session itself never touches files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskMove {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// What an undo or redo changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub kind: DecisionKind,
    /// For a filed file, the move that undoes or redoes it. If that move
    /// fails, call the opposite of what was just called (`redo` after a
    /// failed undo, `undo` after a failed redo), ignoring its `DiskMove`,
    /// so the session goes back to matching the disk.
    pub disk: Option<DiskMove>,
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

/// A session saved to disk, so quitting mid-session costs nothing: the
/// decisions, the skip order, and undo and redo all come back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    version: u32,
    states: HashMap<String, FileState>,
    deferred: Vec<String>,
    history: Vec<Decision>,
    redo_stack: Vec<Decision>,
    sorted: usize,
    /// Files filed into a destination. A fresh scan no longer finds them in
    /// the folder, so they're kept here to rejoin the queue and stay
    /// undoable after a relaunch. The macOS app drops them instead.
    moved: Vec<FileCandidate>,
}

impl SessionSnapshot {
    pub const VERSION: u32 = 1;

    /// Whether this snapshot was written in a format this build reads.
    pub fn is_current(&self) -> bool {
        self.version == Self::VERSION
    }
}

impl TriageSession {
    pub fn new(queue: Vec<FileCandidate>) -> Self {
        Self { queue, ..Self::default() }
    }

    /// Everything needed to pick this session up again later.
    pub fn snapshot(&self) -> SessionSnapshot {
        let moved = self
            .queue
            .iter()
            .filter(|c| self.state(&c.id) == FileState::Moved)
            .cloned()
            .collect();
        SessionSnapshot {
            version: SessionSnapshot::VERSION,
            states: self.states.clone(),
            deferred: self.deferred.clone(),
            history: self.history.clone(),
            redo_stack: self.redo_stack.clone(),
            sorted: self.sorted,
            moved,
        }
    }

    /// Starts a session on a freshly scanned and ranked `queue`, picking up
    /// a saved one. The saved state is reconciled with what's on disk now:
    ///
    /// - Files no longer in the folder are dropped, so undo can never reach
    ///   for something that isn't there. New files arrive as pending.
    /// - A filed file rejoins the session, still undoable, only if it's
    ///   still where it was filed (`exists` checks that path).
    /// - Files trashed or filed earlier that are back in the folder, put
    ///   back from the Recycle Bin or by hand, show up as pending again.
    pub fn restore(
        mut queue: Vec<FileCandidate>,
        snapshot: SessionSnapshot,
        exists: impl Fn(&Path) -> bool,
    ) -> Self {
        let mut present: HashSet<String> = queue.iter().map(|c| c.id.clone()).collect();

        // Where each filed file was last recorded as landing.
        let filed_to: HashMap<&str, &Path> = snapshot
            .history
            .iter()
            .filter(|d| d.kind == DecisionKind::File)
            .filter_map(|d| Some((d.id.as_str(), d.moved_to.as_deref()?)))
            .collect();
        let mut still_filed = HashSet::new();
        for candidate in snapshot.moved {
            let landed = filed_to.get(candidate.id.as_str()).copied();
            if !present.contains(&candidate.id) && landed.is_some_and(&exists) {
                still_filed.insert(candidate.id.clone());
                present.insert(candidate.id.clone());
                queue.push(candidate);
            }
        }

        let states: HashMap<String, FileState> = snapshot
            .states
            .into_iter()
            .filter(|(id, state)| match state {
                FileState::Moved => still_filed.contains(id),
                FileState::Trashed => false,
                _ => present.contains(id),
            })
            .collect();

        // The fresh ranking can put a decided file below a pending one. In a
        // live session decided files always come first, which is what makes
        // an undone file the next card; keep that true after a relaunch.
        let (decided, pending): (Vec<_>, Vec<_>) =
            queue.into_iter().partition(|c| states.contains_key(&c.id));
        let queue = decided.into_iter().chain(pending).collect();
        let keeps = |d: &Decision| {
            present.contains(&d.id) && (d.kind != DecisionKind::File || still_filed.contains(&d.id))
        };
        // An undone filing sits in redo with the file back in the folder, so
        // it only needs the file to be present.
        let keeps_redo = |d: &Decision| present.contains(&d.id) && !still_filed.contains(&d.id);

        Self {
            states,
            deferred: snapshot.deferred.into_iter().filter(|id| present.contains(id)).collect(),
            history: snapshot.history.into_iter().filter(keeps).collect(),
            redo_stack: snapshot.redo_stack.into_iter().filter(keeps_redo).collect(),
            sorted: snapshot.sorted,
            queue,
        }
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

    /// Kept, staged or filed decisions since the last commit. Skips don't count:
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
        // Filing goes through `begin_file`, which needs a destination.
        if kind == DecisionKind::File {
            return false;
        }
        let decision = Decision { kind, id, moved_to: None };
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

    /// Records filing the current file to `planned`, before anything moves
    /// on disk, so a move is never left without an undo entry. Returns its
    /// id and current path for the caller to move. Afterwards, call
    /// `amend_last_move` with where it really landed, or
    /// `withdraw_last_move` if the move failed.
    pub fn begin_file(&mut self, planned: PathBuf) -> Option<(String, PathBuf)> {
        let current = self.current()?;
        let (id, path) = (current.id.clone(), current.path.clone());
        let decision = Decision { kind: DecisionKind::File, id: id.clone(), moved_to: Some(planned) };
        self.apply(&decision);
        self.history.push(decision);
        self.redo_stack.clear();
        Some((id, path))
    }

    /// Corrects where the last filed file landed. The folder may have
    /// numbered its name to avoid replacing a file, and undo has to look
    /// where the file really is.
    pub fn amend_last_move(&mut self, id: &str, actual: PathBuf) {
        if let Some(last) = self.history.last_mut() {
            if last.kind == DecisionKind::File && last.id == id {
                last.moved_to = Some(actual);
            }
        }
    }

    /// Takes back a filing whose move failed, so the session never claims
    /// a move that didn't happen. Unlike undo, it leaves nothing to redo.
    pub fn withdraw_last_move(&mut self, id: &str) {
        if self.history.last().is_some_and(|d| d.kind == DecisionKind::File && d.id == id) {
            let decision = self.history.pop().unwrap();
            self.revert(&decision);
        }
    }

    /// Reverts the most recent decision. Says what it was, so the interface
    /// can bring the card back from the way it left, and what to move back
    /// on disk if it was filed.
    pub fn undo(&mut self) -> Option<Step> {
        let decision = self.history.pop()?;
        self.revert(&decision);
        let disk = self.disk_move(&decision, true);
        let step = Step { kind: decision.kind, disk };
        self.redo_stack.push(decision);
        Some(step)
    }

    /// Re-applies the most recently undone decision.
    pub fn redo(&mut self) -> Option<Step> {
        let decision = self.redo_stack.pop()?;
        self.apply(&decision);
        let disk = self.disk_move(&decision, false);
        let step = Step { kind: decision.kind, disk };
        self.history.push(decision);
        Some(step)
    }

    /// The move that undoes (`back`) or redoes a filing.
    fn disk_move(&self, decision: &Decision, back: bool) -> Option<DiskMove> {
        let moved_to = decision.moved_to.clone()?;
        let original = self.queue.iter().find(|c| c.id == decision.id)?.path.clone();
        Some(if back {
            DiskMove { from: moved_to, to: original }
        } else {
            DiskMove { from: original, to: moved_to }
        })
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
            DecisionKind::File => self.set_decided(id, FileState::Moved),
            DecisionKind::Skip => {
                self.deferred.retain(|d| d != id);
                self.deferred.push(id.clone());
            }
        }
    }

    fn revert(&mut self, decision: &Decision) {
        let id = &decision.id;
        match decision.kind {
            DecisionKind::Keep | DecisionKind::Stage | DecisionKind::File => {
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

    use DecisionKind::{File, Keep, Skip, Stage};

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

    fn undone(s: &mut TriageSession) -> Option<DecisionKind> {
        s.undo().map(|step| step.kind)
    }

    fn redone(s: &mut TriageSession) -> Option<DecisionKind> {
        s.redo().map(|step| step.kind)
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
        assert_eq!(undone(&mut s), Some(Keep));
        assert!(!s.can_undo());
    }

    #[test]
    fn undo_reverts_a_keep() {
        let mut s = session(&["a", "b"]);
        s.decide(Keep);
        assert_eq!(undone(&mut s), Some(Keep));
        assert_eq!(s.state("a"), FileState::Pending);
        assert_eq!(current(&s), Some("a"));
    }

    #[test]
    fn undo_reverts_a_skip() {
        let mut s = session(&["a", "b"]);
        s.decide(Skip);
        assert_eq!(undone(&mut s), Some(Skip));
        assert_eq!(current(&s), Some("a"));
    }

    #[test]
    fn undo_with_no_history_does_nothing() {
        let mut s = session(&["a"]);
        assert_eq!(undone(&mut s), None);
        assert_eq!(current(&s), Some("a"));
    }

    #[test]
    fn undo_walks_back_through_several_decisions() {
        let mut s = session(&["a", "b", "c"]);
        s.decide(Keep);
        s.decide(Stage);
        assert_eq!(undone(&mut s), Some(Stage));
        assert_eq!(current(&s), Some("b"));
        assert_eq!(undone(&mut s), Some(Keep));
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

        assert_eq!(redone(&mut s), Some(Keep));
        assert_eq!(s.state("a"), FileState::Kept);
        assert_eq!(redone(&mut s), Some(Stage));
        assert_eq!(s.state("b"), FileState::Staged);
        assert_eq!(redone(&mut s), Some(Skip));
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
        assert_eq!(redone(&mut s), None);
    }

    #[test]
    fn filing_moves_the_file_out_of_the_queue() {
        let mut s = session(&["a", "b"]);
        let (id, path) = s.begin_file(PathBuf::from("Invoices/a")).unwrap();
        assert_eq!((id.as_str(), path), ("a", PathBuf::from("a")));
        assert_eq!(s.state("a"), FileState::Moved);
        assert_eq!(current(&s), Some("b"));
        assert_eq!(s.sorted_count(), 1);
        assert!(s.staged().is_empty());
    }

    #[test]
    fn decide_cannot_file_without_a_destination() {
        let mut s = session(&["a"]);
        assert!(!s.decide(File));
        assert_eq!(s.state("a"), FileState::Pending);
    }

    #[test]
    fn undoing_a_filing_asks_for_the_file_to_be_moved_back() {
        let mut s = session(&["a", "b"]);
        s.begin_file(PathBuf::from("Invoices/a"));
        let step = s.undo().unwrap();
        assert_eq!(step.kind, File);
        assert_eq!(step.disk, Some(DiskMove { from: PathBuf::from("Invoices/a"), to: PathBuf::from("a") }));
        assert_eq!(current(&s), Some("a"));
        assert_eq!(s.sorted_count(), 0);
    }

    #[test]
    fn redoing_a_filing_asks_for_the_file_to_be_moved_again() {
        let mut s = session(&["a", "b"]);
        s.begin_file(PathBuf::from("Invoices/a"));
        s.undo();
        let step = s.redo().unwrap();
        assert_eq!(step.disk, Some(DiskMove { from: PathBuf::from("a"), to: PathBuf::from("Invoices/a") }));
        assert_eq!(s.state("a"), FileState::Moved);
    }

    #[test]
    fn other_decisions_need_no_disk_work() {
        let mut s = session(&["a", "b"]);
        s.decide(Keep);
        assert_eq!(s.undo().unwrap().disk, None);
    }

    #[test]
    fn amending_a_move_changes_where_undo_looks() {
        let mut s = session(&["a", "b"]);
        s.begin_file(PathBuf::from("Invoices/a"));
        s.amend_last_move("a", PathBuf::from("Invoices/a (2)"));
        assert_eq!(s.undo().unwrap().disk.unwrap().from, PathBuf::from("Invoices/a (2)"));
    }

    #[test]
    fn withdrawing_a_failed_move_leaves_nothing_to_undo_or_redo() {
        let mut s = session(&["a", "b"]);
        s.decide(Keep);
        s.begin_file(PathBuf::from("Invoices/b"));
        s.withdraw_last_move("b");
        assert_eq!(s.state("b"), FileState::Pending);
        assert_eq!(current(&s), Some("b"));
        assert!(!s.can_redo());
        assert_eq!(s.sorted_count(), 1);
        assert_eq!(undone(&mut s), Some(Keep));
    }

    #[test]
    fn redo_after_a_failed_undo_puts_the_filing_back_on_record() {
        let mut s = session(&["a", "b"]);
        s.begin_file(PathBuf::from("Invoices/a"));
        s.undo(); // pretend moving it back failed
        s.redo();
        assert_eq!(s.state("a"), FileState::Moved);
        assert!(s.can_undo());
        assert!(!s.can_redo());
    }

    fn file(name: &str) -> FileCandidate {
        FileCandidate {
            id: name.to_owned(),
            path: PathBuf::from(name),
            bytes: 1024,
            last_opened: None,
            created: 0.0,
            modified: None,
            source_url: None,
            is_directory: false,
        }
    }

    fn files(names: &[&str]) -> Vec<FileCandidate> {
        names.iter().map(|n| file(n)).collect()
    }

    /// Saves through JSON, as the app does, and restores onto `rescanned`.
    fn relaunch(s: &TriageSession, rescanned: &[&str], exists: impl Fn(&Path) -> bool) -> TriageSession {
        let json = serde_json::to_string(&s.snapshot()).unwrap();
        let snapshot: SessionSnapshot = serde_json::from_str(&json).unwrap();
        assert!(snapshot.is_current());
        TriageSession::restore(files(rescanned), snapshot, exists)
    }

    #[test]
    fn decisions_skips_and_undo_survive_a_relaunch() {
        let mut s = session(&["a", "b", "c", "d"]);
        s.decide(Keep);
        s.decide(Stage);
        s.decide(Skip);
        s.decide(Keep);
        s.undo();

        let mut s = relaunch(&s, &["a", "b", "c", "d"], |_| false);

        assert_eq!(s.state("a"), FileState::Kept);
        assert_eq!(s.state("b"), FileState::Staged);
        assert_eq!(current(&s), Some("d"));
        assert_eq!(s.sorted_count(), 2);
        assert!(s.can_redo());
        assert_eq!(redone(&mut s), Some(Keep));
        assert_eq!(current(&s), Some("c"));
        assert_eq!(undone(&mut s), Some(Keep));
        assert_eq!(undone(&mut s), Some(Skip));
        assert_eq!(undone(&mut s), Some(Stage));
    }

    #[test]
    fn an_undone_file_comes_straight_back_even_if_it_now_ranks_lower() {
        let mut s = session(&["a", "b", "c"]);
        s.decide(Keep);

        // After a relaunch, a now ranks last.
        let mut s = relaunch(&s, &["b", "c", "a"], |_| false);

        assert_eq!(current(&s), Some("b"));
        assert_eq!(undone(&mut s), Some(Keep));
        assert_eq!(current(&s), Some("a"));
    }

    #[test]
    fn files_that_left_the_folder_are_dropped_and_new_ones_are_pending() {
        let mut s = session(&["a", "b"]);
        s.decide(Keep);
        s.decide(Stage);

        let s = relaunch(&s, &["b", "new"], |_| false);

        assert_eq!(s.state("a"), FileState::Pending);
        assert_eq!(s.state("b"), FileState::Staged);
        assert_eq!(current(&s), Some("new"));
        assert_eq!(s.total_count(), 2);
        // Only b's stage is still undoable; a's keep went with a.
        let mut s = s;
        assert_eq!(undone(&mut s), Some(Stage));
        assert!(!s.can_undo());
    }

    #[test]
    fn a_filed_file_stays_undoable_after_a_relaunch() {
        let mut s = session(&["a", "b"]);
        s.begin_file(PathBuf::from("Invoices/a"));

        let mut s = relaunch(&s, &["b"], |p| p == Path::new("Invoices/a"));

        assert_eq!(s.state("a"), FileState::Moved);
        assert_eq!(current(&s), Some("b"));
        let step = s.undo().unwrap();
        assert_eq!(step.disk, Some(DiskMove { from: PathBuf::from("Invoices/a"), to: PathBuf::from("a") }));
        assert_eq!(current(&s), Some("a"));
    }

    #[test]
    fn a_filed_file_that_has_since_moved_on_is_let_go() {
        let mut s = session(&["a", "b"]);
        s.begin_file(PathBuf::from("Invoices/a"));

        let s = relaunch(&s, &["b"], |_| false);

        assert_eq!(s.state("a"), FileState::Pending);
        assert_eq!(s.total_count(), 1);
        assert!(!s.can_undo());
    }

    #[test]
    fn a_filed_file_put_back_by_hand_is_pending_again() {
        let mut s = session(&["a", "b"]);
        s.begin_file(PathBuf::from("Invoices/a"));

        // The user dragged it back into Downloads themselves.
        let s = relaunch(&s, &["a", "b"], |_| false);

        assert_eq!(s.state("a"), FileState::Pending);
        assert_eq!(current(&s), Some("a"));
        assert!(!s.can_undo());
    }

    #[test]
    fn an_undone_filing_can_still_be_redone_after_a_relaunch() {
        let mut s = session(&["a", "b"]);
        s.begin_file(PathBuf::from("Invoices/a"));
        s.undo();

        let mut s = relaunch(&s, &["a", "b"], |_| false);

        let step = s.redo().unwrap();
        assert_eq!(step.disk, Some(DiskMove { from: PathBuf::from("a"), to: PathBuf::from("Invoices/a") }));
    }

    #[test]
    fn a_file_restored_from_the_recycle_bin_is_pending_again() {
        let mut s = session(&["a", "b"]);
        s.decide(Stage);
        s.commit_trashed(["a"]);

        let s = relaunch(&s, &["a", "b"], |_| false);

        assert_eq!(s.state("a"), FileState::Pending);
        assert_eq!(current(&s), Some("a"));
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
        assert_eq!(undone(&mut s), None);
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
