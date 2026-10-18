/**
 * Turns touchpad scrolling into triage gestures. Port of the macOS app's
 * SwipeMonitorController, adapted to what Windows gives a web view.
 *
 * Swipes reach the WebView as plain wheel events, and after the fingers
 * lift the scroll keeps coasting (momentum). The app reads the touchpad's
 * finger contacts natively and passes them in through `fingers`, which
 * makes this behave like macOS:
 *
 * - Each touch starts a fresh gesture, so swipes can follow each other
 *   straight away.
 * - A sideways swipe commits as soon as it crosses the threshold,
 *   mid-gesture. The rest of that gesture is ignored, and everything after
 *   the lift is momentum, also ignored.
 * - A swipe that stopped short springs back when the fingers lift.
 * - Swiping up opens the shelf and carries straight on into choosing a bin
 *   by sliding sideways; lifting files into it, and dragging back down
 *   cancels. Once a two-finger scroll starts vertically Windows holds it to
 *   that axis, so the sliding is followed from the fingers' positions, not
 *   from scrolling.
 *
 * Touchpads that don't report contacts fall back to inferring a gesture's
 * end from a quiet moment. Without a lift event the coasting tail would
 * keep moving the highlight, so on the shelf each swipe is then a single
 * step: left, right, up to file, down to close.
 *
 * Pure and clock-free: the caller feeds events and the time, and calls
 * `tick` once things may have gone quiet.
 */

/** What the touchpad reports: fingers down, and their middle as 0 to 1
 *  across the touchpad from its top-left corner (null if unknown). */
export interface TouchInput {
  fingers: number;
  x: number | null;
  y: number | null;
}

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
  | { type: "right" }
  /** Fingers lifted while choosing a bin: file into the highlighted one. */
  | { type: "release" };

export const SWIPE = {
  /** Pixels of travel before deciding whether a gesture is sideways. */
  axisLock: 12,
  /** Pixels of sideways travel that make a full swipe. */
  full: 300,
  /** Fraction of a full swipe that commits, as on macOS. */
  commitAt: 0.4,
  /** Pixels for a discrete swipe: opening the shelf, or a step on it. */
  step: 90,
  /** Sideways pixels per bin while sliding across the shelf, as on macOS. */
  binStep: 60,
  /** Pixels back down that abandon the shelf while choosing. */
  cancelDown: 50,
  /** Without contacts, a gesture that's been quiet this long has ended. */
  quietMs: 150,
  /** Finger travel across the whole touchpad, in the same pixels as
   *  scrolling: a bin is then about a tenth of the touchpad's width. */
  padWidth: 600,
  padHeight: 400,
} as const;

type Phase =
  | { kind: "idle" }
  | { kind: "deciding"; x: number; y: number }
  | { kind: "sideways"; x: number }
  | { kind: "vertical"; y: number }
  /** Opened the shelf this gesture; sliding sideways chooses a bin. */
  | { kind: "steering"; x: number; y: number }
  /** Finished with this gesture; ignore the rest of it. */
  | { kind: "spent" };

export class SwipeTracker {
  private phase: Phase = { kind: "idle" };
  private lastEvent = -Infinity;
  /** Whether the touchpad reports finger contacts at all. */
  private contacts = false;
  private touching = false;
  /** Whether the shelf was already open when this gesture began. */
  private startedOnShelf = false;
  /** Where the fingers were last, while the count stays the same. */
  private lastAt: { x: number; y: number; fingers: number } | null = null;
  /** Whether the touchpad reports positions, so steering follows them. */
  private positions = false;

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

  /** The touchpad reported fingers touching, moving or lifting. */
  touch({ fingers, x, y }: TouchInput): Intent[] {
    const intents = this.fingers(fingers);
    if (fingers === 0 || x === null || y === null) {
      this.lastAt = null;
      return intents;
    }
    this.positions = true;
    const last = this.lastAt;
    this.lastAt = { x, y, fingers };
    // Only compare like with like: the middle jumps when a finger lands.
    if (!last || last.fingers !== fingers || this.phase.kind !== "steering") return intents;
    const phase = this.phase;
    // Positions are where the fingers really are, so unlike scrolling
    // they never need reversing.
    phase.x += (x - last.x) * SWIPE.padWidth;
    phase.y += (last.y - y) * SWIPE.padHeight;
    return [...intents, ...this.steer(phase)];
  }

  /** The number of fingers on the touchpad changed. */
  fingers(count: number): Intent[] {
    this.contacts = true;
    if (count > 0 && !this.touching) {
      // A new touch: whatever came before is over.
      this.touching = true;
      const intents = this.end();
      this.phase = { kind: "idle" };
      return intents;
    }
    if (count === 0 && this.touching) {
      this.touching = false;
      return this.lift();
    }
    return [];
  }

  /** Feeds one touchpad wheel event. */
  wheel({ dx, dy, t }: WheelInput): Intent[] {
    const intents: Intent[] = [];
    if (this.contacts) {
      // After the lift it's only momentum.
      if (!this.touching) return [];
    } else if (t - this.lastEvent > SWIPE.quietMs) {
      // A gap means the previous gesture ended, whatever state it was in.
      intents.push(...this.end());
    }
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
        if (phase.kind === "idle") this.startedOnShelf = this.onShelf();
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
      case "steering":
        // Followed from finger positions where the touchpad gives them.
        if (this.positions) return intents;
        phase.x += right;
        phase.y += up;
        return [...intents, ...this.steer(phase)];
    }
  }

  /** Call when no event has arrived for a while; ends a stopped gesture.
   *  Only needed for touchpads that don't report contacts. */
  tick(t: number): Intent[] {
    if (this.contacts) return [];
    return t - this.lastEvent > SWIPE.quietMs ? this.end() : [];
  }

  /** Abandons any gesture, for when something else takes over input. */
  reset(): Intent[] {
    const intents = this.end();
    // Wait for a fresh touch before tracking again.
    if (this.contacts && this.touching) this.phase = { kind: "spent" };
    return intents;
  }

  private advance(): Intent[] {
    const phase = this.phase;
    if (phase.kind === "sideways") {
      if (this.startedOnShelf) {
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
      // Opening the shelf with a touchpad that reports contacts: the same
      // gesture carries on into choosing a bin, as on macOS.
      if (phase.y > 0 && this.contacts && !this.startedOnShelf) {
        this.phase = { kind: "steering", x: 0, y: 0 };
        return [{ type: "up" }];
      }
      this.phase = { kind: "spent" };
      return [{ type: phase.y > 0 ? "up" : "down" }];
    }
    return [];
  }

  private steer(phase: { x: number; y: number }): Intent[] {
    // Dragging back down abandons the shelf without filing.
    if (phase.y <= -SWIPE.cancelDown) {
      this.phase = { kind: "spent" };
      return [{ type: "down" }];
    }
    const steps: Intent[] = [];
    while (Math.abs(phase.x) >= SWIPE.binStep) {
      const right = phase.x > 0;
      phase.x -= right ? SWIPE.binStep : -SWIPE.binStep;
      steps.push({ type: right ? "right" : "left" });
    }
    return steps;
  }

  private lift(): Intent[] {
    const phase = this.phase;
    this.phase = { kind: "idle" };
    if (phase.kind === "steering") return [{ type: "release" }];
    // A sideways swipe that stopped short puts the card back.
    if (phase.kind === "sideways" && !this.startedOnShelf) return [{ type: "cancel" }];
    return [];
  }

  private end(): Intent[] {
    const wasSideways = this.phase.kind === "sideways" && !this.startedOnShelf;
    this.phase = { kind: "idle" };
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
