import { describe, expect, it } from "vitest";

import type { WorldParams } from "../src/core/types";
import {
  BAND_GAP,
  BAND_KEYS,
  bandSegments,
  NOISE_MAX,
  orderBands,
  riverHexWidth,
  riverWidthFor,
  WORLD_PARAMETER_FIELDS,
} from "../src/ui/worldParameters";

/**
 * The form is DOM, but the rules it enforces are not: the band cuts must ascend, the coverage
 * strip is arithmetic on those cuts, and the river slider is a unit conversion. Those are what
 * a wrong value would be wrong about, so they are what is tested here.
 */
const BASE: WorldParams = {
  elevation_coarse_cell: 320,
  elevation_fine_cell: 40,
  elevation_coarse_weight: 70,
  moisture_cell: 160,
  richness_cell: 96,
  water_level: 24000,
  shore_level: 27000,
  hills_level: 42000,
  highland_level: 54000,
  cliff_step: 900,
  deep_water_moisture: 12000,
  site_cell: 40,
  site_jitter: 6,
  river_cell: 180,
  river_width: 2200,
  river_max_elevation: 52000,
  ocean_level: 22000,
  site_rules: [],
};

function cuts(params: WorldParams): number[] {
  return BAND_KEYS.map((key) => params[key]);
}

function ascending(values: number[]): boolean {
  return values.every(
    (value, index) => index === 0 || value >= (values[index - 1] ?? 0) + 1,
  );
}

describe("orderBands", () => {
  it("leaves an already ordered set alone", () => {
    expect(orderBands(BASE, "hills_level")).toEqual(BASE);
  });

  it("pushes the cuts above when one is raised past them", () => {
    const raised = orderBands({ ...BASE, water_level: 60000 }, "water_level");
    expect(ascending(cuts(raised))).toBe(true);
    // The moved cut is where the player put it; the three it passed step up out of its way.
    expect(raised.water_level).toBe(60000);
    expect(raised.shore_level).toBe(60000 + BAND_GAP);
    expect(raised.highland_level).toBe(60000 + 3 * BAND_GAP);
  });

  it("stops the lowest cut short of the room the three above it need", () => {
    const raised = orderBands(
      { ...BASE, water_level: NOISE_MAX },
      "water_level",
    );
    expect(raised.water_level).toBe(NOISE_MAX - 3 * BAND_GAP);
    expect(raised.highland_level).toBe(NOISE_MAX);
    expect(ascending(cuts(raised))).toBe(true);
  });

  it("pushes the cuts below when one is dropped past them", () => {
    const dropped = orderBands(
      { ...BASE, highland_level: 100 },
      "highland_level",
    );
    expect(ascending(cuts(dropped))).toBe(true);
    expect(dropped.highland_level).toBe(3 * BAND_GAP);
    expect(dropped.water_level).toBe(0);
  });

  it("moves only the cuts, never another parameter", () => {
    const moved = orderBands({ ...BASE, shore_level: 5 }, "shore_level");
    expect(moved.river_cell).toBe(BASE.river_cell);
    expect(moved.cliff_step).toBe(BASE.cliff_step);
    expect(moved.site_rules).toBe(BASE.site_rules);
  });

  it("ignores a parameter that is not a band cut", () => {
    expect(
      orderBands({ ...BASE, river_cell: 4 }, "river_cell").river_cell,
    ).toBe(4);
  });
});

describe("bandSegments", () => {
  it("covers the whole height range exactly once", () => {
    const total = bandSegments(BASE).reduce(
      (sum, segment) => sum + segment.share,
      0,
    );
    expect(total).toBeCloseTo(1, 10);
  });

  it("reports water as the share of the range below the sea cut", () => {
    const [water] = bandSegments(BASE);
    expect(water?.terrain).toBe("shallow_water");
    expect(water?.share).toBeCloseTo(BASE.water_level / NOISE_MAX, 10);
  });

  it("reports a band nobody can reach as no share rather than a negative one", () => {
    // Out of order on purpose: the strip is drawn from whatever it is handed, including a set
    // orderBands has not been through yet, and a negative flex-grow is not a drawing.
    const segments = bandSegments({ ...BASE, shore_level: 1000 });
    expect(segments.every((segment) => segment.share >= 0)).toBe(true);
  });
});

describe("river width", () => {
  it("round-trips a width in hexes through the noise half-width", () => {
    for (const hexes of [0, 1, 3, 8, 24]) {
      expect(riverHexWidth(riverWidthFor(hexes, 180), 180)).toBe(hexes);
    }
  });

  it("reads a wider river out of the same half-width when rivers sit further apart", () => {
    expect(riverHexWidth(2200, 360)).toBeGreaterThan(riverHexWidth(2200, 180));
  });

  it("never divides by a zero cell", () => {
    expect(Number.isFinite(riverWidthFor(4, 0))).toBe(true);
    expect(Number.isFinite(riverHexWidth(2200, 0))).toBe(true);
  });
});

describe("WORLD_PARAMETER_FIELDS", () => {
  it("offers every generator scalar exactly once", () => {
    const keys = WORLD_PARAMETER_FIELDS.map((field) => field.key);
    expect(new Set(keys).size).toBe(keys.length);
    const scalars = Object.keys(BASE).filter((key) => key !== "site_rules");
    expect([...keys].sort()).toEqual(scalars.sort());
  });

  it("reads every field at both ends of its own range", () => {
    for (const field of WORLD_PARAMETER_FIELDS) {
      for (const value of [field.min, field.max]) {
        expect(field.read(value, BASE).length).toBeGreaterThan(0);
      }
    }
  });
});
