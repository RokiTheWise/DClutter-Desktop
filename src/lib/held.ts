/**
 * Repeats a decision while its key is held, at a pace the app sets rather
 * than the system key-repeat rate, so a held arrow can't race through
 * dozens of files. Port of HeldDecisionController from the macOS app.
 *
 * No backlog builds up: releasing the key, or the window losing focus,
 * stops the hold.
 */
export class HeldDecision {
  static readonly INITIAL_DELAY_MS = 450;
  static readonly DEFAULT_SPEED = 3; // files per second

  private timer: ReturnType<typeof setTimeout> | null = null;
  private key: string | null = null;

  /**
   * Performs once immediately, then repeats until `stop`. `perform`
   * returns false when there's nothing left to decide, ending the hold.
   */
  start(key: string, perform: () => boolean, speed = HeldDecision.DEFAULT_SPEED) {
    this.stop();
    if (!perform()) return;
    this.key = key;

    const interval = 1000 / speed;
    // Slow paces govern the first repeat too: at 0.5 files/s the second
    // decision waits two seconds, not 450 ms.
    const tick = () => {
      if (perform()) {
        this.timer = setTimeout(tick, interval);
      } else {
        this.stop();
      }
    };
    this.timer = setTimeout(tick, Math.max(HeldDecision.INITIAL_DELAY_MS, interval));
  }

  /** Stops the hold, or only the hold on `key` when given. */
  stop(key?: string) {
    if (key !== undefined && key !== this.key) return;
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
    this.key = null;
  }
}
