import { describe, expect, it } from "vitest";

import { TERRAIN_INFO, TERRAIN_ORDER } from "../src/core/terrain";
import type { WorldParams } from "../src/core/types";
import { riverHexWidth } from "../src/ui/worldParameters";
import {
  applyChanges,
  describeChange,
  describeDeposits,
  describeNeeds,
  flatten,
  joinWords,
  parseHexColor,
  previewPixels,
  PREVIEW_BACKDROP,
  PREVIEW_ZOOMS,
  refusedStatus,
  repairLabels,
  terrainPalette,
  unmetWarning,
} from "../src/ui/worldPreview";

const look = (itemId: number): { name: string; color: string } | undefined => {
  if (itemId === 4) return { name: "Iron ore", color: "#b8c4cc" };
  if (itemId === 5) return { name: "Wood", color: "#6b8f4e" };
  return undefined;
};

const PARAMS: WorldParams = {
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

/**
 * The panel is DOM, but what could be wrong about it is not: a band byte has to reach the colour
 * the map paints that band with, and a world native refuses has to say so. Those are here; the
 * canvas around them is not.
 */
describe("a band byte reaching the colour the map paints it", () => {
  it("parses every hex length the band table uses, and never throws on a bad one", () => {
    expect(parseHexColor("#1a5474dd")).toEqual({
      r: 0x1a,
      g: 0x54,
      b: 0x74,
      a: 0xdd,
    });
    expect(parseHexColor("#0c1b18")).toEqual({
      r: 0x0c,
      g: 0x1b,
      b: 0x18,
      a: 255,
    });
    expect(parseHexColor("#abc")).toEqual({
      r: 0xaa,
      g: 0xbb,
      b: 0xcc,
      a: 255,
    });
    expect(parseHexColor("rebeccapurple").a).toBe(255);
    expect(parseHexColor("")).toEqual({ r: 0, g: 0, b: 0, a: 255 });
  });

  it("flattens alpha against the ground under it at both ends and in the middle", () => {
    const under = parseHexColor("#0c1b18");
    const gold = parseHexColor("#f6c85f");
    expect(flatten(gold, under)).toEqual(gold);
    expect(flatten({ ...gold, a: 0 }, under)).toEqual(under);
    expect(
      flatten({ r: 200, g: 100, b: 0, a: 128 }, { r: 0, g: 0, b: 100, a: 255 }),
    ).toEqual({ r: 100, g: 50, b: 50, a: 255 });
  });

  it("gives each band one opaque entry of its own, in the order the wire sends", () => {
    const palette = terrainPalette();
    expect(palette).toHaveLength(TERRAIN_ORDER.length * 4);
    for (let band = 0; band < TERRAIN_ORDER.length; band += 1) {
      expect(palette[band * 4 + 3]).toBe(255);
    }

    const colors = TERRAIN_ORDER.map((_, band) =>
      [...palette.slice(band * 4, band * 4 + 3)].join(","),
    );
    expect(new Set(colors).size).toBe(TERRAIN_ORDER.length);

    // And the entry is the colour the map paints that band, over the panel's own ground.
    const index = TERRAIN_ORDER.indexOf("shore");
    const expected = flatten(
      parseHexColor(TERRAIN_INFO.shore.fill),
      parseHexColor(PREVIEW_BACKDROP),
    );
    expect([...palette.slice(index * 4, index * 4 + 4)]).toEqual([
      expected.r,
      expected.g,
      expected.b,
      255,
    ]);
  });

  it("turns one band byte into one pixel, and an unknown band into a hole", () => {
    const palette = terrainPalette();
    const pixels = previewPixels(Uint8Array.from([0, 3, 6]), palette);
    expect(pixels).toHaveLength(12);
    expect([...pixels.slice(4, 8)]).toEqual([...palette.slice(12, 16)]);

    // A byte off the end of the palette would mean native had grown a band. A hole says so; a
    // silent wrap to band zero would draw deep water where the generator put something else.
    expect([...previewPixels(Uint8Array.from([200]))]).toEqual([0, 0, 0, 0]);
  });
});

describe("telling the player why a world was refused", () => {
  it("names the materials native would refuse the world over, and only then", () => {
    expect(unmetWarning([], look)).toBeNull();
    expect(unmetWarning([4], look)).toBe(
      "No room for Iron ore — this world cannot be started.",
    );
    // Still names an item this build has no definition for.
    expect(unmetWarning([99], look)).toContain("item 99");
  });

  it("counts deposits, and never reports a crowded window as an empty one", () => {
    expect(describeDeposits({ total: 18, dense: false })).toBe("18 deposits");
    expect(describeDeposits({ total: 1, dense: false })).toBe("1 deposit");
    expect(describeDeposits({ total: 0, dense: false })).toBe("0 deposits");

    // An empty `sites` at a wide zoom means native declined to send them, and a caption reading
    // "0 deposits" over a world full of them is the one reading this has to rule out.
    expect(describeDeposits({ total: 4254, dense: true })).toBe(
      "4254 deposits, too dense to plot at this zoom",
    );
    expect(describeDeposits({ total: 0, dense: true })).not.toContain("0");
  });

  it("separates ground that no seed will supply from a patch another seed might", () => {
    const [absent] = describeNeeds(
      [{ item_id: 4, bands: ["highland"], ground: false }],
      look,
    );
    expect(absent).toContain("Iron ore sits on highland");
    expect(absent).toContain("no seed will find any");

    const [reseedable] = describeNeeds(
      [{ item_id: 4, bands: ["highland"], ground: true }],
      look,
    );
    expect(reseedable).toContain("another seed");
    expect(reseedable).toContain("closer deposit spacing");

    // Materials that fail the same way are one sentence, not two.
    const grouped = describeNeeds(
      [
        { item_id: 4, bands: ["highland"], ground: false },
        { item_id: 5, bands: ["lowland"], ground: false },
      ],
      look,
    );
    expect(grouped).toHaveLength(1);
    expect(grouped[0]).toContain("Iron ore and Wood sit on");
    expect(grouped[0]).toContain("highland and lowland");
  });

  it("holds one, two, or many words in a sentence", () => {
    expect(joinWords(["Iron ore"])).toBe("Iron ore");
    expect(joinWords(["Iron ore", "Wood"])).toBe("Iron ore and Wood");
    expect(joinWords(["Iron ore", "Wood", "Coal"])).toBe(
      "Iron ore, Wood and Coal",
    );
  });

  it("puts the verdict and the hint in one announcement, and nothing when satisfied", () => {
    expect(refusedStatus({ unmet: [], needs: [] }, look)).toBe("");
    const copy = refusedStatus(
      {
        unmet: [4],
        needs: [{ item_id: 4, bands: ["highland"], ground: false }],
      },
      look,
    );
    expect(copy.startsWith("No room for Iron ore")).toBe(true);
    expect(copy).toContain("no seed will find any");
  });
});

describe("offering the player a repair", () => {
  it("names a change by the form's label and the unit the slider shows", () => {
    expect(
      describeChange({ field: "water_level", from: 50000, to: 24000 }, PARAMS),
    ).toBe("Sea level 50000 → 24000");
    expect(
      describeChange({ field: "site_cell", from: 128, to: 40 }, PARAMS),
    ).toBe("Deposit spacing 128 → 40");
    // A scaled field is converted into the unit on the slider.
    expect(
      describeChange({ field: "river_width", from: 2200, to: 1100 }, PARAMS),
    ).toBe(
      `River width ${riverHexWidth(2200, PARAMS.river_cell)} → ${riverHexWidth(1100, PARAMS.river_cell)}`,
    );
    // And a field this build has no row for is still named rather than dropped.
    expect(
      describeChange({ field: "unknown_knob", from: 1, to: 2 }, PARAMS),
    ).toBe("unknown_knob 1 → 2");
  });

  it("labels only the seed and knobs the player would actually see change", () => {
    expect(repairLabels(null, PARAMS)).toEqual({ seed: null, params: null });
    expect(
      repairLabels(
        {
          seed: 8,
          changes: [{ field: "site_cell", from: 128, to: 40 }],
        },
        PARAMS,
      ),
    ).toEqual({
      seed: "Try seed 8",
      params: "Fix Deposit spacing 128 → 40",
    });
  });

  it("turns only the named knobs, the way native's test applies a repair", () => {
    const next = applyChanges(PARAMS, [
      { field: "site_cell", from: 40, to: 24 },
      { field: "water_level", from: 24000, to: 18000 },
    ]);
    expect(next.site_cell).toBe(24);
    expect(next.water_level).toBe(18000);
    expect(next.shore_level).toBe(PARAMS.shore_level);
    // A field this build has no row for is ignored rather than thrown over.
    expect(
      applyChanges(PARAMS, [{ field: "unknown_knob", from: 1, to: 2 }]),
    ).toEqual(PARAMS);
  });

  it("offers zoom spans that narrow, and none past what native will raster", () => {
    const spans = PREVIEW_ZOOMS.map((zoom) => zoom.hexesAcross);
    expect(spans).toEqual([...spans].sort((a, b) => b - a));
    expect(new Set(PREVIEW_ZOOMS.map((zoom) => zoom.key)).size).toBe(
      PREVIEW_ZOOMS.length,
    );
    // `MAX_PREVIEW_SPAN` in `factory-wasm/src/lib.rs`. A zoom past it would be silently clamped,
    // and the caption would then describe a window nobody was shown.
    for (const zoom of PREVIEW_ZOOMS) {
      expect(zoom.hexesAcross).toBeGreaterThan(0);
      expect(zoom.hexesAcross).toBeLessThanOrEqual(16384);
    }
  });
});
