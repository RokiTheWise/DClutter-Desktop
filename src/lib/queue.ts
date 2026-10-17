import { invoke } from "@tauri-apps/api/core";

/** Mirrors `QueueItem` in src-tauri/src/lib.rs. Timestamps are Unix seconds. */
export interface QueueItem {
  id: string;
  path: string;
  name: string;
  bytes: number;
  lastOpened: number | null;
  created: number;
  modified: number | null;
  sourceUrl: string | null;
  isDirectory: boolean;
  score: number;
  duplicateCount: number;
  redundantCopy: boolean;
  extractedArchive: boolean;
}

export interface Queue {
  folder: string;
  items: QueueItem[];
}

/** Matches `UNKNOWN_INTERNET_SOURCE` in dclutter-platform. */
export const UNKNOWN_INTERNET_SOURCE = "about:internet";

/** Scans `folder`, or Downloads when omitted, in triage order. */
export function scanFolder(folder?: string): Promise<Queue> {
  return invoke<Queue>("scan_folder", { folder: folder ?? null });
}

const UNITS = ["bytes", "KB", "MB", "GB", "TB"];

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return bytes === 1 ? "1 byte" : `${bytes} bytes`;
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${UNITS[unit]}`;
}

/** The host a file was downloaded from, e.g. `example.com`. */
export function sourceHost(sourceUrl: string | null): string | null {
  if (sourceUrl === null) return null;
  if (sourceUrl === UNKNOWN_INTERNET_SOURCE) return "the internet";
  try {
    return new URL(sourceUrl).hostname.replace(/^www\./, "") || null;
  } catch {
    return null;
  }
}
