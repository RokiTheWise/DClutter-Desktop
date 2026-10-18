/**
 * Turns touchpad scrolling into triage gestures. Port of the macOS app's
 * SwipeMonitorController, adapted to what Windows gives a web view.
 *
 * macOS reports when fingers touch and lift. Windows precision touchpads
 * reach the WebView as plain wheel events, and after the fingers lift the
 * scroll keeps coasting (momentum), so a gesture's end can only be inferred
 * from a quiet moment. Two consequences shape this module:
 *
 * - A swipe commits as soon as it crosses the threshold, mid-gesture, as on
 *   macOS. Everything after that, including the momentum tail, is ignored
 *   until the touchpad goes quiet, so one swipe can never decide two files.
 * - On the shelf each swipe is one discrete step (left, right, up to file,
 *   down to close) rather than "slide across and let go": without a lift
 *   event, the coasting tail would keep moving the highlight.
 *
 * Pure and clock-free: the caller feeds events and the time, and calls
 * `tick` once things may have gone quiet.
 */

export interface WheelInput {
  dx: number;
  dy: number;
  /** Milliseconds, from any steady clock. */
  t: number;
}

export type Intent =
  /** The card should sit at this fraction of a full swipe, -1 to 1. */
  | { type: "progress"; value: number }
  | { type: "keep" }
  | { type: "stage" }
  /** Stopped short: the card goes back to rest. */
  | { type: "cancel" }
  /** A deliberate swipe in a direction, for the shelf and for opening it. */
  | { type: "up" }
  | { type: "down" }
  | { type: "left" }
  | { type: "right" };

export const SWIPE = {
  /** Pixels of travel before deciding whether a gesture is sideways. */
  axisLock: 12,
  /** Pixels of sideways travel that make a full swipe. */
  full: 300,
  /** Fraction of a full swipe that commits, as on macOS. */
  commitAt: 0.4,
  /** Pixels for a discrete swipe: opening the shelf, or a step on it. */
  step: 90,
  /** A gesture that's been quiet this long has ended. */
  quietMs: 150,
} as const;

type Phase =
  | { kind: "idle" }
  | { kind: "deciding"; x: number; y: number }
  | { kind: "sideways"; x: number }
  | { kind: "vertical"; y: number }
  /** Finished with this gesture; ignore input until it goes quiet. */
  | { kind: "spent" };

export class SwipeTracker {
  private phase: Phase = { kind: "idle" };
  private lastEvent = -Infinity;

  /**
   * @param reversed Users who set Windows to reverse touchpad scrolling
   *   would otherwise find every swipe backwards.
   * @param onShelf Whether the destination shelf is open, so sideways
   *   swipes step bins instead of deciding.
   */
  constructor(
    private reversed: () => boolean,
    private onShelf: () => boolean,
  ) {}

  /** Feeds one touchpad wheel event. */
  wheel({ dx, dy, t }: WheelInput): Intent[] {
    const intents: Intent[] = [];
    // A gap means the previous gesture ended, whatever state it was in.
    if (t - this.lastEvent > SWIPE.quietMs) intents.push(...this.end());
    this.lastEvent = t;

    // Windows' default "down motion scrolls up" is natural scrolling:
    // fingers moving right scroll left (negative dx). Flip both axes so
    // positive means fingers moving right and up.
    const sign = this.reversed() ? 1 : -1;
    const right = dx * sign;
    const up = -dy * sign;

    const phase = this.phase;
    switch (phase.kind) {
      case "spent":
        return intents;
      case "idle":
      case "deciding": {
        const x = (phase.kind === "deciding" ? phase.x : 0) + right;
        const y = (phase.kind === "deciding" ? phase.y : 0) + up;
        if (Math.hypot(x, y) < SWIPE.axisLock) {
          this.phase = { kind: "deciding", x, y };
          return intents;
        }
        this.phase = Math.abs(x) >= Math.abs(y) ? { kind: "sideways", x } : { kind: "vertical", y };
        return [...intents, ...this.advance()];
      }
      case "sideways":
        phase.x += right;
        return [...intents, ...this.advance()];
      case "vertical":
        phase.y += up;
        return [...intents, ...this.advance()];
    }
  }

  /** Call when no event has arrived for a while; ends a stopped gesture. */
  tick(t: number): Intent[] {
    return t - this.lastEvent > SWIPE.quietMs ? this.end() : [];
  }

  /** Abandons any gesture, for when something else takes over input. */
  reset(): Intent[] {
    return this.end();
  }

  private advance(): Intent[] {
    const phase = this.phase;
    if (phase.kind === "sideways") {
      if (this.onShelf()) {
        if (Math.abs(phase.x) < SWIPE.step) return [];
        this.phase = { kind: "spent" };
        return [{ type: phase.x > 0 ? "right" : "left" }];
      }
      const value = Math.max(-1, Math.min(1, phase.x / SWIPE.full));
      if (Math.abs(value) < SWIPE.commitAt) return [{ type: "progress", value }];
      this.phase = { kind: "spent" };
      return [{ type: "progress", value }, { type: value > 0 ? "keep" : "stage" }];
    }
    if (phase.kind === "vertical") {
      if (Math.abs(phase.y) < SWIPE.step) return [];
      this.phase = { kind: "spent" };
      return [{ type: phase.y > 0 ? "up" : "down" }];
    }
    return [];
  }

  private end(): Intent[] {
    const wasSideways = this.phase.kind === "sideways" && !this.onShelf();
    this.phase = { kind: "idle" };
    // A sideways swipe that stopped short puts the card back.
    return wasSideways ? [{ type: "cancel" }] : [];
  }
}

/**
 * Whether a wheel event came from a mouse wheel rather than a touchpad.
 * Mouse wheels move in whole notches, reported as multiples of 120 in the
 * legacy wheelDelta fields; touchpads report fine-grained amounts. Mouse
 * wheels never decide files: a quick spin would race through the queue.
 */
export function isMouseWheel(e: { deltaX: number; deltaY: number; wheelDeltaX?: number; wheelDeltaY?: number }) {
  const notched = (v: number | undefined) => v !== undefined && v !== 0 && v % 120 === 0;
  return (notched(e.wheelDeltaY) && e.deltaX === 0) || (notched(e.wheelDeltaX) && e.deltaY === 0);
}
