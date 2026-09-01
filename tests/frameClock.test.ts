import { describe, expect, it } from "vitest";

import { FrameClock } from "../src/core/frameClock";

describe("frame clock", () => {
  it("advances fixed factory time separately from active player work", () => {
    const clock = new FrameClock(1_000);
    expect(
      clock.update(1_200, {
        playerActive: true,
        playerTicksPerSecond: 30,
      }),
    ).toEqual({ elapsed: 200, ticks: 2, playerSteps: 6 });
    clock.consume(2, 6);

    expect(
      clock.update(1_450, {
        playerActive: true,
        playerTicksPerSecond: 30,
      }),
    ).toEqual({ elapsed: 250, ticks: 2, playerSteps: 7 });
  });

  it("keeps the factory running at its own rate through an idle player and a stalled frame", () => {
    // Going idle drops the player's debt without touching the factory's.
    const idling = new FrameClock(0);
    idling.update(100, { playerActive: true, playerTicksPerSecond: 30 });
    idling.consume(1, 3);
    expect(
      idling.update(200, { playerActive: false, playerTicksPerSecond: 30 }),
    ).toEqual({ elapsed: 100, ticks: 1, playerSteps: 0 });

    // A stalled frame is bounded, and the bound never changes the factory rate.
    expect(
      new FrameClock(0).update(10_000, {
        playerActive: true,
        playerTicksPerSecond: 1_000,
      }),
    ).toEqual({ elapsed: 250, ticks: 2, playerSteps: 20 });
  });
});
