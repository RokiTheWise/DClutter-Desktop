import { describe, expect, it } from "vitest";
import { SWIPE, SwipeTracker, isMouseWheel, type Intent } from "./swipe";

/** A tracker plus a way to feed it a gesture as a series of small moves. */
function setup({ reversed = false, shelf = false } = {}) {
  const state = { shelf };
  const tracker = new SwipeTracker(() => reversed, () => state.shelf);
  let t = 0;
  const swipe = (dx: number, dy: number, steps = 10, gapMs = 8): Intent[] => {
    const out: Intent[] = [];
    for (let i = 0; i < steps; i++) {
      t += gapMs;
      out.push(...tracker.wheel({ dx: dx / steps, dy: dy / steps, t }));
    }
    return out;
  };
  const pause = (ms = SWIPE.quietMs + 1) => {
    t += ms;
    return tracker.tick(t);
  };
  return { tracker, state, swipe, pause };
}

const types = (intents: Intent[]) => intents.map((i) => i.type);

// With Windows' default natural scrolling, fingers moving right give a
// negative dx and fingers moving up a positive dy.
const FINGERS_RIGHT = -1;
const FINGERS_UP = 1;

describe("sideways swipes on the card", () => {
  it("follow the fingers, then keep once past the threshold", () => {
    const { swipe } = setup();
    const intents = swipe(FINGERS_RIGHT * SWIPE.full * 0.6, 0);

    const progress = intents.filter((i) => i.type === "progress").map((i) => (i as { value: number }).value);
    expect(progress[0]).toBeGreaterThan(0);
    expect(progress.every((v, i) => i === 0 || v >= progress[i - 1])).toBe(true);
    expect(types(intents).filter((t) => t !== "progress")).toEqual(["keep"]);
  });

  it("stage when the fingers move left", () => {
    const { swipe } = setup();
    expect(types(swipe(-FINGERS_RIGHT * SWIPE.full, 0))).toContain("stage");
  });

  it("decide only once per gesture, however far it goes", () => {
    const { swipe } = setup();
    const intents = swipe(FINGERS_RIGHT * SWIPE.full * 3, 0, 40);
    expect(types(intents).filter((t) => t === "keep")).toHaveLength(1);
  });

  it("ignore the momentum tail after a decision", () => {
    const { swipe } = setup();
    swipe(FINGERS_RIGHT * SWIPE.full * 0.6, 0);
    // Coasting continues with no gap, as Windows inertia does.
    const tail = swipe(FINGERS_RIGHT * SWIPE.full * 0.6, 0);
    expect(tail).toEqual([]);
  });

  it("decide again on the next gesture after a quiet moment", () => {
    const { swipe, pause } = setup();
    swipe(FINGERS_RIGHT * SWIPE.full * 0.6, 0);
    pause();
    expect(types(swipe(-FINGERS_RIGHT * SWIPE.full * 0.6, 0))).toContain("stage");
  });

  it("spring back when they stop short", () => {
    const { swipe, pause } = setup();
    const intents = swipe(FINGERS_RIGHT * SWIPE.full * 0.2, 0);
    expect(types(intents)).not.toContain("keep");
    expect(types(pause())).toEqual(["cancel"]);
  });

  it("are reversed for users with reversed touchpad scrolling", () => {
    const { swipe } = setup({ reversed: true });
    expect(types(swipe(FINGERS_RIGHT * SWIPE.full, 0))).toContain("stage");
  });

  it("ignore tiny jitter that never commits to an axis", () => {
    const { swipe, pause } = setup();
    expect(swipe(3, 3, 2)).toEqual([]);
    expect(pause()).toEqual([]);
  });
});

describe("vertical swipes", () => {
  it("fingers up says up once, which opens the shelf", () => {
    const { swipe } = setup();
    expect(types(swipe(0, FINGERS_UP * SWIPE.step * 2))).toEqual(["up"]);
  });

  it("a mostly vertical gesture with a little sideways drift isn't a decision", () => {
    const { swipe } = setup();
    expect(types(swipe(FINGERS_RIGHT * 20, FINGERS_UP * SWIPE.step * 2))).toEqual(["up"]);
  });

  it("fingers down says down", () => {
    const { swipe } = setup();
    expect(types(swipe(0, -FINGERS_UP * SWIPE.step * 2))).toEqual(["down"]);
  });
});

describe("on the shelf", () => {
  it("each sideways swipe is one step, never a decision", () => {
    const { swipe, pause } = setup({ shelf: true });
    expect(types(swipe(FINGERS_RIGHT * SWIPE.full, 0))).toEqual(["right"]);
    pause();
    expect(types(swipe(-FINGERS_RIGHT * SWIPE.full, 0))).toEqual(["left"]);
  });

  it("a long swipe with momentum still moves only one bin", () => {
    const { swipe } = setup({ shelf: true });
    expect(types(swipe(FINGERS_RIGHT * SWIPE.full * 4, 0, 40))).toEqual(["right"]);
  });

  it("stopping short does nothing and doesn't cancel anything", () => {
    const { swipe, pause } = setup({ shelf: true });
    expect(swipe(FINGERS_RIGHT * 30, 0)).toEqual([]);
    expect(pause()).toEqual([]);
  });
});

describe("telling a mouse wheel from a touchpad", () => {
  it("whole notches are a mouse wheel", () => {
    expect(isMouseWheel({ deltaX: 0, deltaY: 100, wheelDeltaX: 0, wheelDeltaY: -120 })).toBe(true);
    expect(isMouseWheel({ deltaX: 0, deltaY: 200, wheelDeltaX: 0, wheelDeltaY: -240 })).toBe(true);
  });

  it("fine-grained amounts are a touchpad", () => {
    expect(isMouseWheel({ deltaX: -3.5, deltaY: 0.5, wheelDeltaX: 4, wheelDeltaY: -1 })).toBe(false);
    expect(isMouseWheel({ deltaX: 0, deltaY: 7, wheelDeltaX: 0, wheelDeltaY: -8 })).toBe(false);
  });
});
