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

/**
 * The panel is DOM, but what could be wrong about it is not: a band byte has to reach the colour
 * the map paints that band with, and a world native refuses has to say so. Those are here; the
 * canvas around them is not.
 */
describe("parseHexColor", () => {
  it("reads the three lengths the band table uses", () => {
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
  });

  it("answers anything else with a colour rather than a throw", () => {
    expect(parseHexColor("rebeccapurple").a).toBe(255);
    expect(parseHexColor("")).toEqual({ r: 0, g: 0, b: 0, a: 255 });
  });
});

describe("flatten", () => {
  it("leaves an opaque colour alone and drops a transparent one entirely", () => {
    const under = parseHexColor("#0c1b18");
    const gold = parseHexColor("#f6c85f");
    expect(flatten(gold, under)).toEqual(gold);
    expect(flatten({ ...gold, a: 0 }, under)).toEqual(under);
  });

  it("lands between the two at half alpha", () => {
    const mixed = flatten(
      { r: 200, g: 100, b: 0, a: 128 },
      { r: 0, g: 0, b: 100, a: 255 },
    );
    expect(mixed).toEqual({ r: 100, g: 50, b: 50, a: 255 });
  });
});

describe("terrainPalette", () => {
  it("holds one opaque entry per band, in the order the wire sends", () => {
    const palette = terrainPalette();
    expect(palette).toHaveLength(TERRAIN_ORDER.length * 4);
    for (let band = 0; band < TERRAIN_ORDER.length; band += 1) {
      expect(palette[band * 4 + 3]).toBe(255);
    }
  });

  it("gives every band a colour of its own", () => {
    const palette = terrainPalette();
    const colors = TERRAIN_ORDER.map((_, band) =>
      [...palette.slice(band * 4, band * 4 + 3)].join(","),
    );
    expect(new Set(colors).size).toBe(TERRAIN_ORDER.length);
  });

  it("paints a band the colour the map paints it, over the panel's own ground", () => {
    const palette = terrainPalette();
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
});

describe("previewPixels", () => {
  it("turns one band byte into one opaque pixel", () => {
    const palette = terrainPalette();
    const cells = Uint8Array.from([0, 3, 6]);
    const pixels = previewPixels(cells, palette);
    expect(pixels).toHaveLength(12);
    expect([...pixels.slice(4, 8)]).toEqual([...palette.slice(12, 16)]);
  });

  it("leaves a band this build does not know as a hole rather than another band", () => {
    // A byte off the end of the palette would mean native had grown a band. A hole says so; a
    // silent wrap to band zero would draw deep water where the generator put something else.
    const pixels = previewPixels(Uint8Array.from([200]));
    expect([...pixels]).toEqual([0, 0, 0, 0]);
  });
});

describe("unmetWarning", () => {
  const look = (itemId: number): { name: string; color: string } | undefined =>
    itemId === 4 ? { name: "Iron ore", color: "#b8c4cc" } : undefined;

  it("says nothing when the bootstrap pass was satisfied", () => {
    expect(unmetWarning([], look)).toBeNull();
  });

  it("names the materials native would refuse the world over", () => {
    expect(unmetWarning([4], look)).toBe(
      "No room for Iron ore — this world cannot be started.",
    );
  });

  it("still names an item this build has no definition for", () => {
    expect(unmetWarning([99], look)).toContain("item 99");
  });
});

describe("describeDeposits", () => {
  it("counts what was drawn when everything was drawn", () => {
    expect(describeDeposits({ total: 18, dense: false })).toBe("18 deposits");
    expect(describeDeposits({ total: 1, dense: false })).toBe("1 deposit");
    expect(describeDeposits({ total: 0, dense: false })).toBe("0 deposits");
  });

  it("never reports a crowded window as an empty one", () => {
    // An empty `sites` at a wide zoom means native declined to send them, and a caption reading
    // "0 deposits" over a world full of them is the one reading this has to rule out.
    expect(describeDeposits({ total: 4254, dense: true })).toBe(
      "4254 deposits, too dense to plot at this zoom",
    );
    expect(describeDeposits({ total: 0, dense: true })).not.toContain("0");
  });
});

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

describe("describeNeeds", () => {
  it("says no seed will help when the opening holds none of the ground", () => {
    const [sentence] = describeNeeds(
      [{ item_id: 4, bands: ["highland"], ground: false }],
      look,
    );
    expect(sentence).toContain("Iron ore sits on highland");
    expect(sentence).toContain("no seed will find any");
  });

  it("points at another seed when the ground is there and the patch is not", () => {
    const [sentence] = describeNeeds(
      [{ item_id: 4, bands: ["highland"], ground: true }],
      look,
    );
    expect(sentence).toContain("another seed");
    expect(sentence).toContain("closer deposit spacing");
  });

  it("groups materials that fail the same way into one sentence", () => {
    const sentences = describeNeeds(
      [
        { item_id: 4, bands: ["highland"], ground: false },
        { item_id: 5, bands: ["lowland"], ground: false },
      ],
      look,
    );
    expect(sentences).toHaveLength(1);
    expect(sentences[0]).toContain("Iron ore and Wood sit on");
    expect(sentences[0]).toContain("highland and lowland");
  });
});

describe("joinWords", () => {
  it("holds one, two, or many words in a sentence", () => {
    expect(joinWords(["Iron ore"])).toBe("Iron ore");
    expect(joinWords(["Iron ore", "Wood"])).toBe("Iron ore and Wood");
    expect(joinWords(["Iron ore", "Wood", "Coal"])).toBe(
      "Iron ore, Wood and Coal",
    );
  });
});

describe("describeChange", () => {
  it("uses the form's label and the unit the slider shows", () => {
    expect(
      describeChange({ field: "water_level", from: 50000, to: 24000 }, PARAMS),
    ).toBe("Sea level 50000 → 24000");
    expect(
      describeChange({ field: "site_cell", from: 128, to: 40 }, PARAMS),
    ).toBe("Deposit spacing 128 → 40");
  });

  it("converts a scaled field into the unit on the slider", () => {
    expect(
      describeChange({ field: "river_width", from: 2200, to: 1100 }, PARAMS),
    ).toBe(
      `River width ${riverHexWidth(2200, PARAMS.river_cell)} → ${riverHexWidth(1100, PARAMS.river_cell)}`,
    );
  });

  it("still names a field this build has no row for", () => {
    expect(
      describeChange({ field: "unknown_knob", from: 1, to: 2 }, PARAMS),
    ).toBe("unknown_knob 1 → 2");
  });
});

describe("refusedStatus", () => {
  it("is empty when the bootstrap pass was satisfied", () => {
    expect(refusedStatus({ unmet: [], needs: [] }, look)).toBe("");
  });

  it("puts the verdict and the hint in one announcement", () => {
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

describe("repairLabels", () => {
  it("offers nothing when there is no repair", () => {
    expect(repairLabels(null, PARAMS)).toEqual({ seed: null, params: null });
  });

  it("names the seed and the knobs the player would actually see change", () => {
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
});

describe("applyChanges", () => {
  it("turns only the named knobs, the way native's test applies a repair", () => {
    const next = applyChanges(PARAMS, [
      { field: "site_cell", from: 40, to: 24 },
      { field: "water_level", from: 24000, to: 18000 },
    ]);
    expect(next.site_cell).toBe(24);
    expect(next.water_level).toBe(18000);
    expect(next.shore_level).toBe(PARAMS.shore_level);
  });

  it("ignores a field this build has no row for rather than throwing", () => {
    expect(
      applyChanges(PARAMS, [{ field: "unknown_knob", from: 1, to: 2 }]),
    ).toEqual(PARAMS);
  });
});

describe("PREVIEW_ZOOMS", () => {
  it("offers spans that narrow, so a picker reads as zooming in", () => {
    const spans = PREVIEW_ZOOMS.map((zoom) => zoom.hexesAcross);
    expect(spans).toEqual([...spans].sort((a, b) => b - a));
    expect(new Set(PREVIEW_ZOOMS.map((zoom) => zoom.key)).size).toBe(
      PREVIEW_ZOOMS.length,
    );
  });

  it("keeps every span inside what native will raster", () => {
    // `MAX_PREVIEW_SPAN` in `factory-wasm/src/lib.rs`. A zoom past it would be silently clamped,
    // and the caption would then describe a window nobody was shown.
    for (const zoom of PREVIEW_ZOOMS) {
      expect(zoom.hexesAcross).toBeGreaterThan(0);
      expect(zoom.hexesAcross).toBeLessThanOrEqual(16384);
    }
  });
});
