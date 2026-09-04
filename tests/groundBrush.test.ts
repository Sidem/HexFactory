import { describe, expect, it } from "vitest";
import {
  brushDistance,
  brushLine,
  groundBrushEdit,
  MAX_BRUSH_RUN,
} from "../src/ui/groundBrush";

describe("live ground brush", () => {
  it("fills skipped pointer positions with one adjacent stamp per crossed hex", () => {
    const line = brushLine({ q: 0, r: 0 }, { q: 3, r: -2 });
    expect(line).toHaveLength(4);
    expect(line.at(-1)).toEqual({ q: 3, r: -2 });
    expect(
      line
        .slice(1)
        .every((cell, index) => brushDistance(line[index]!, cell) === 1),
    ).toBe(true);
  });

  it("refuses to fill a jump, so a zoom mid-stroke cannot paint the hexes it skipped over", () => {
    const jump = brushLine({ q: 0, r: 0 }, { q: 40, r: -20 });
    expect(jump).toEqual([
      { q: 0, r: 0 },
      { q: 40, r: -20 },
    ]);
    expect(brushLine({ q: 0, r: 0 }, { q: MAX_BRUSH_RUN, r: 0 })).toHaveLength(
      MAX_BRUSH_RUN + 1,
    );
  });

  it("centres a bounded disc under the pointer and keeps the sampled grade datum", () => {
    expect(
      groundBrushEdit({ q: 5, r: 2 }, { q: 1, r: -1 }, 2, "grade", 7, false),
    ).toMatchObject({
      q: 5,
      r: 2,
      to_q: 7,
      to_r: 2,
      datum: [1, -1],
      shape: "disc",
      action: "smooth",
    });
  });
});
