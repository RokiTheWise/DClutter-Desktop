/**
 * Interaction preferences, kept in the WebView's local storage. They're
 * conveniences: if storage is unavailable or cleared, the defaults apply
 * and nothing else is lost.
 */

const HOLD_SPEED_KEY = "dclutter.holdSpeed";
const SWIPE_REVERSED_KEY = "dclutter.swipeReversed";

/** Files per second while an arrow key is held, as on macOS. */
export const HOLD_SPEED = { min: 0.5, max: 8, step: 0.5, default: 3 } as const;

export function boundedSpeed(value: number): number {
  if (!Number.isFinite(value)) return HOLD_SPEED.default;
  return Math.min(Math.max(value, HOLD_SPEED.min), HOLD_SPEED.max);
}

export function loadHoldSpeed(): number {
  try {
    const saved = localStorage.getItem(HOLD_SPEED_KEY);
    return saved === null ? HOLD_SPEED.default : boundedSpeed(Number(saved));
  } catch {
    return HOLD_SPEED.default;
  }
}

/** Whether touchpad swipes run the other way, for people who set Windows
 *  to reverse touchpad scrolling. */
export function loadSwipeReversed(): boolean {
  try {
    return localStorage.getItem(SWIPE_REVERSED_KEY) === "true";
  } catch {
    return false;
  }
}

export function saveSwipeReversed(reversed: boolean) {
  try {
    localStorage.setItem(SWIPE_REVERSED_KEY, String(reversed));
  } catch {
    // Not remembered this time; the setting still applies until closing.
  }
}

export function saveHoldSpeed(speed: number) {
  try {
    localStorage.setItem(HOLD_SPEED_KEY, String(boundedSpeed(speed)));
  } catch {
    // Not remembered this time; the setting still applies until closing.
  }
}
