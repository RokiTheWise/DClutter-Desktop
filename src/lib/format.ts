/** Matches `UNKNOWN_INTERNET_SOURCE` in dclutter-platform. */
export const UNKNOWN_INTERNET_SOURCE = "about:internet";

const UNITS = ["bytes", "KB", "MB", "GB", "TB"];

/** Sizes the way File Explorer shows them: 1024-based, one decimal under 10. */
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

const dateFormat = new Intl.DateTimeFormat(undefined, {
  day: "numeric",
  month: "short",
  year: "numeric",
});

export function formatDate(seconds: number | null): string {
  return seconds === null ? "—" : dateFormat.format(new Date(seconds * 1000));
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

/** "Folder", "PDF file", or "File" when there's no extension. */
export function fileKind(isDirectory: boolean, extension: string): string {
  if (isDirectory) return "Folder";
  return extension ? `${extension.toUpperCase()} file` : "File";
}
