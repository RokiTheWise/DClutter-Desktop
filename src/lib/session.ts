import { invoke } from "@tauri-apps/api/core";

export type DecisionKind = "keep" | "stage" | "skip" | "file" | "rename";

/** Mirrors `CardItem` in src-tauri/src/lib.rs. Timestamps are Unix seconds. */
export interface CardItem {
  id: string;
  path: string;
  name: string;
  extension: string;
  bytes: number;
  lastOpened: number | null;
  created: number;
  modified: number | null;
  sourceUrl: string | null;
  isDirectory: boolean;
  score: number;
  duplicateCount: number;
  extractedArchive: boolean;
}

/** Mirrors `SessionView` in src-tauri/src/lib.rs. */
export interface SessionView {
  folder: string;
  current: CardItem | null;
  upNext: CardItem | null;
  remaining: number;
  total: number;
  sorted: number;
  staged: number;
  trashed: number;
  canUndo: boolean;
  canRedo: boolean;
}

/** A view plus the decision an undo or redo reversed or re-applied. */
export interface Change {
  view: SessionView;
  kind: DecisionKind | null;
}

/** Starts a session on `folder`, or on Downloads when omitted. */
export function openQueue(folder?: string): Promise<SessionView> {
  return invoke("open_queue", { folder: folder ?? null });
}

export function decide(kind: DecisionKind): Promise<SessionView> {
  return invoke("decide", { kind });
}

export function undo(): Promise<Change> {
  return invoke("undo");
}

export function redo(): Promise<Change> {
  return invoke("redo");
}

/** Mirrors `StagedItem` in src-tauri/src/lib.rs. */
export interface StagedItem {
  id: string;
  name: string;
  bytes: number;
  isDirectory: boolean;
}

/** Mirrors `CommitOutcome` in src-tauri/src/lib.rs. */
export interface CommitOutcome {
  view: SessionView;
  trashed: number;
  error: string | null;
}

/** The files staged for trash, for the review list. */
export function stagedFiles(): Promise<StagedItem[]> {
  return invoke("staged_files");
}

/** Keeps the `excluded` files and recycles every other staged file. */
export function commitTrash(excluded: string[]): Promise<CommitOutcome> {
  return invoke("commit_trash", { excluded });
}

/** Mirrors `DestinationItem` in src-tauri/src/lib.rs. */
export interface DestinationItem {
  id: string;
  name: string;
  path: string;
}

/** The most destinations the shelf holds. Matches `MAX_DESTINATIONS`. */
export const MAX_DESTINATIONS = 3;

export function listDestinations(): Promise<DestinationItem[]> {
  return invoke("list_destinations");
}

/** Opens the folder picker and adds the choice to the shelf. */
export function addDestination(): Promise<DestinationItem[]> {
  return invoke("add_destination");
}

/** Takes a folder off the shelf; the folder itself is left alone. */
export function removeDestination(id: string): Promise<DestinationItem[]> {
  return invoke("remove_destination", { id });
}

/** Files the current card's file into destination `id` right away. */
export function fileCurrent(id: string): Promise<SessionView> {
  return invoke("file_current", { id });
}

/** Mirrors `PreviewImage` in src-tauri/src/lib.rs. */
export interface PreviewImage {
  /** A picture of the content, or only the file type's icon. */
  kind: "thumbnail" | "icon";
  width: number;
  height: number;
  /** A PNG data URL. */
  src: string;
}

/** The shell's preview for the current or next card, if it has one. */
export function thumbnail(id: string): Promise<PreviewImage | null> {
  return invoke("thumbnail", { id });
}

/** Renames the card's file; the extension is kept if `name` has none. */
export function renameCurrent(name: string): Promise<SessionView> {
  return invoke("rename_current", { name });
}

/** Opens File Explorer with the card's file selected. */
export function revealCurrent(): Promise<void> {
  return invoke("reveal_current");
}

/** Opens the file on the card in its default app. */
export function openCurrent(): Promise<void> {
  return invoke("open_current");
}
