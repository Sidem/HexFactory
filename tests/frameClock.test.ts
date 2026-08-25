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

  it("keeps the factory running while dropping idle player debt", () => {
    const clock = new FrameClock(0);
    clock.update(100, {
      playerActive: true,
      playerTicksPerSecond: 30,
    });
    clock.consume(1, 3);
    expect(
      clock.update(200, {
        playerActive: false,
        playerTicksPerSecond: 30,
      }),
    ).toEqual({ elapsed: 100, ticks: 1, playerSteps: 0 });
  });

  it("bounds a stalled frame and never changes the factory rate", () => {
    const clock = new FrameClock(0);
    expect(
      clock.update(10_000, {
        playerActive: true,
        playerTicksPerSecond: 1_000,
      }),
    ).toEqual({ elapsed: 250, ticks: 2, playerSteps: 20 });
  });
});
