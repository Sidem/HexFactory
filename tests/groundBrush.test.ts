import { describe, expect, it } from "vitest";
import {
  brushDistance,
  brushLine,
  groundBrushEdit,
  MAX_BRUSH_RUN,
  movesEarth,
  takesGroundwork,
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

  it("digs and raises by the chosen depth, and keeps that depth off every other verb", () => {
    const dig = groundBrushEdit(
      { q: 0, r: 0 },
      { q: 0, r: 0 },
      0,
      "dig",
      0,
      false,
      3,
    );
    expect(dig).toMatchObject({ action: "lower", steps: 3, shape: "cell" });
    // A cut has no datum: a trench is not a slope blended toward a sampled height, it is a hole.
    expect(dig.datum).toBeUndefined();
    expect(
      groundBrushEdit({ q: 0, r: 0 }, { q: 0, r: 0 }, 0, "mound", 0, false, 2),
    ).toMatchObject({ action: "raise", steps: 2 });
    // Native clamps too, but a tray that can only offer 1–3 should never be the thing sending 9.
    expect(
      groundBrushEdit({ q: 0, r: 0 }, { q: 0, r: 0 }, 0, "dig", 0, false, 9)
        .steps,
    ).toBe(3);
    // Surface and Strip move by one fixed thing, so the depth tray cannot reach them.
    for (const mode of ["grade", "surface", "strip"] as const) {
      expect(movesEarth(mode)).toBe(false);
      expect(
        groundBrushEdit({ q: 0, r: 0 }, { q: 0, r: 0 }, 0, mode, 4, false, 3)
          .steps,
      ).toBe(1);
    }
  });

  it("puts every earth-moving mode on the field-work clock", () => {
    expect(takesGroundwork("grade")).toBe(true);
    expect(takesGroundwork("dig")).toBe(true);
    expect(takesGroundwork("mound")).toBe(true);
    expect(takesGroundwork("surface")).toBe(false);
    expect(takesGroundwork("strip")).toBe(false);
  });
});
