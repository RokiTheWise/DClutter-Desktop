import { invoke } from "@tauri-apps/api/core";

export type DecisionKind = "keep" | "stage" | "skip";

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

/** Opens the file on the card in its default app. */
export function openCurrent(): Promise<void> {
  return invoke("open_current");
}
