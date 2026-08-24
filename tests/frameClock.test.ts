import { describe, expect, it } from "vitest";

import { FrameClock } from "../src/core/frameClock";

describe("frame clock", () => {
  it("separates scaled factory time from unscaled player work", () => {
    const clock = new FrameClock(1_000);
    expect(
      clock.update(1_200, {
        playing: true,
        speed: 4,
        playerActive: true,
        playerTicksPerSecond: 30,
      }),
    ).toEqual({ elapsed: 200, ticks: 0, playerSteps: 6 });
    clock.consume(0, 6);

    expect(
      clock.update(1_450, {
        playing: true,
        speed: 4,
        playerActive: true,
        playerTicksPerSecond: 30,
      }),
    ).toEqual({ elapsed: 250, ticks: 1, playerSteps: 7 });
  });

  it("pauses only the factory and drops idle player debt", () => {
    const clock = new FrameClock(0);
    clock.update(100, {
      playing: false,
      speed: 16,
      playerActive: true,
      playerTicksPerSecond: 30,
    });
    clock.consume(0, 3);
    expect(
      clock.update(200, {
        playing: false,
        speed: 16,
        playerActive: false,
        playerTicksPerSecond: 30,
      }),
    ).toEqual({ elapsed: 100, ticks: 0, playerSteps: 0 });
  });

  it("bounds a stalled frame to twenty native steps", () => {
    const clock = new FrameClock(0);
    expect(
      clock.update(10_000, {
        playing: true,
        speed: 1_000,
        playerActive: true,
        playerTicksPerSecond: 1_000,
      }),
    ).toEqual({ elapsed: 250, ticks: 20, playerSteps: 20 });
  });
});
