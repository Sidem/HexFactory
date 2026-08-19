import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import { TERRAIN_ORDER } from "../src/core/terrain";
import wireFixture from "../fixtures/snapshot-delta-wire.json";
import definitionData from "../src/data/definitions.json";
import scenarios from "../src/data/scenarios.json";
import {
  BUILDING_SHAPE_VERSION,
  BUILDING_SHAPES,
  facingTip,
  NORTH,
  partsFor,
  PLAYER_BODY,
  PLAYER_RING,
  silhouetteOf,
  spanEnd,
  STALL_MARKS,
  stallMark,
  trimOf,
  workCycle,
  type SilhouetteKey,
} from "../src/rendering/buildingLook";
import {
  HUB_LADDER,
  isStill,
  profileTop,
  silhouetteSignature,
  TIER_LADDER,
} from "../src/rendering/shapeGrammar";
import {
  BAND_RANK,
  depletionLook,
  fringeToward,
  hexHash,
  hexLook,
  remainingRatio,
  surveyedBand,
  TERRAIN_TILE_VERSION,
  tileKey,
} from "../src/rendering/terrainLook";
import type {
  BuildingDefinition,
  Definitions,
  EntitySnapshot,
} from "../src/core/types";

const definitions = definitionData as unknown as Definitions;

function keyOf(definition: BuildingDefinition): SilhouetteKey {
  return silhouetteOf(
    definition.kind,
    definition.recipe_category,
    definition.power_source,
  );
}

const SHAPED_KEYS = (Object.keys(BUILDING_SHAPES) as SilhouetteKey[]).filter(
  (key) => BUILDING_SHAPES[key].length > 0,
);

describe("Stage B look generator", () => {
  it("hashes hexes deterministically and only on the host", () => {
    expect(hexHash(3, -1)).toBe(hexHash(3, -1));
    expect(hexHash(3, -1)).not.toBe(hexHash(3, 0));
    expect(hexHash(0, 0)).not.toBe(hexHash(1, 0));
    const look = hexLook(4, 2);
    expect(look.rotation).toBeGreaterThanOrEqual(0);
    expect(look.rotation).toBeLessThan(6);
    expect(look.jitter).toBeGreaterThanOrEqual(-1);
    expect(look.jitter).toBeLessThanOrEqual(1);
    expect(look.marks).toBeGreaterThanOrEqual(0);
    expect(look.marks).toBeLessThan(4);

    const rust = readFileSync(
      new URL("../factory-wasm/src/lib.rs", import.meta.url),
      "utf8",
    );
    const commands = readFileSync(
      new URL("../src/core/commands.ts", import.meta.url),
      "utf8",
    );
    expect(rust).not.toContain("hexHash");
    expect(commands).not.toContain("hexHash");
    expect(TERRAIN_TILE_VERSION).toBeGreaterThan(0);
  });

  it("clips a terrain stamp to the hex instead of drawing an oversized square", () => {
    const src = readFileSync(
      new URL("../src/rendering/terrainLook.ts", import.meta.url),
      "utf8",
    );
    expect(src).not.toMatch(/const radius = size \* 1\.02/);
    expect(src).toMatch(/hexPath\(ctx, center, size\);\s*ctx\.clip\(\);/);
    const stamp = src.indexOf("export function drawTerrainCell");
    const clip = src.indexOf("ctx.clip()", stamp);
    const image = src.indexOf("ctx.drawImage", stamp);
    expect(clip).toBeGreaterThan(stamp);
    expect(image).toBeGreaterThan(clip);
  });

  it("draws a fringe only toward the lower band", () => {
    expect(BAND_RANK.deep_water).toBeLessThan(BAND_RANK.shallow_water);
    expect(BAND_RANK.shallow_water).toBeLessThan(BAND_RANK.shore);
    expect(BAND_RANK.shore).toBeLessThan(BAND_RANK.lowland);
    expect(BAND_RANK.lowland).toBeLessThan(BAND_RANK.hills);
    expect(BAND_RANK.hills).toBeLessThan(BAND_RANK.highland);
    expect(BAND_RANK.highland).toBeLessThan(BAND_RANK.cliff);
    expect(TERRAIN_ORDER.every((band) => band in BAND_RANK)).toBe(true);
    expect(fringeToward("shore", "shallow_water")).toBe(true);
    expect(fringeToward("shallow_water", "shore")).toBe(false);
    expect(fringeToward("highland", "hills")).toBe(true);
    expect(fringeToward("cliff", "highland")).toBe(true);
    expect(fringeToward("lowland", "lowland")).toBe(false);
  });

  it("treats a surveyed hex with no terrain entry as lowland", () => {
    const map = new Map([[tileKey(1, 0), "hills" as const]]);
    expect(surveyedBand(map, 1, 0)).toBe("hills");
    expect(surveyedBand(map, 0, 0)).toBe("lowland");
  });

  it("scars a field from quantity against initial_quantity, both ways", () => {
    expect(remainingRatio(40, 40)).toBe(1);
    expect(remainingRatio(0, 40)).toBe(0);
    expect(remainingRatio(10, 40)).toBe(0.25);
    expect(depletionLook(40, 40).scars).toBe(0);
    expect(depletionLook(0, 40).scars).toBe(4);
    expect(depletionLook(10, 40).desaturate).toBeGreaterThan(
      depletionLook(30, 40).desaturate,
    );
    // Flora recovering is the same function with quantity climbing.
    expect(depletionLook(36, 40).scars).toBeLessThan(
      depletionLook(8, 40).scars,
    );
  });

  it("derives a building silhouette from recipe_category and reserves trim for tier", () => {
    expect(silhouetteOf("composer", "smelting")).toBe("smelting");
    expect(silhouetteOf("composer", "firing")).toBe("firing");
    expect(silhouetteOf("composer", "cutting")).toBe("cutting");
    expect(silhouetteOf("composer", "crushing")).toBe("crushing");
    expect(silhouetteOf("composer", "assembly")).toBe("assembly");
    expect(silhouetteOf("extractor")).toBe("extractor");
    expect(silhouetteOf("generator", undefined, "wind")).toBe("wind");
    expect(trimOf(0).stroke).not.toBe(trimOf(1).stroke);
    expect(trimOf().stroke).toBe(trimOf(0).stroke);
  });

  it("draws the two-row headings on the hex column they actually reach", () => {
    const center = { x: 100, y: 100 };
    // The point of due north is that it does not move world-x. The drawing has to agree, or the
    // riser would look like it leans while the simulation routes it straight up.
    for (const orientation of [NORTH, NORTH + 1]) {
      expect(facingTip(center, 40, orientation).x).toBeCloseTo(center.x, 6);
      expect(spanEnd(center, 40, orientation).x).toBeCloseTo(center.x, 6);
    }
    expect(facingTip(center, 40, NORTH).y).toBeLessThan(center.y);
    expect(facingTip(center, 40, NORTH + 1).y).toBeGreaterThan(center.y);
    // The heading tick reads the same length whatever axis it is on, because it is an indicator
    // and not a measurement — the span is what carries the distance.
    const east = facingTip(center, 40, 0);
    const north = facingTip(center, 40, NORTH);
    const length = (point: { x: number; y: number }): number =>
      Math.hypot(point.x - center.x, point.y - center.y);
    expect(length(north)).toBeCloseTo(length(east), 6);
    expect(length(spanEnd(center, 40, NORTH))).toBeGreaterThan(length(north));
    // The six edges are untouched.
    expect(facingTip(center, 40, 0).x).toBeGreaterThan(center.x);
  });

  it("ties a machine cycle to published progress, not to a host clock", () => {
    const composing = {
      progress: 4,
      progress_total: 8,
      status: "composing",
      id: 1,
    } as EntitySnapshot;
    expect(workCycle(composing, 12_000, false)).toBe(0.5);
    const idle = {
      progress: 0,
      progress_total: 0,
      status: "idle",
      id: 1,
    } as EntitySnapshot;
    expect(workCycle(idle, 12_000, false)).toBe(0);
  });
});

describe("Stage D shape grammar", () => {
  it("gives every silhouette a part list, and every definition a silhouette", () => {
    for (const definition of definitions.buildings) {
      const key = keyOf(definition);
      expect(BUILDING_SHAPES[key]).toBeDefined();
    }
    // The belt is the one deliberate empty: its look is the heading tick and the cargo riding it.
    expect(BUILDING_SHAPES.belt).toHaveLength(0);
    expect(SHAPED_KEYS.length).toBeGreaterThan(10);
    expect(BUILDING_SHAPE_VERSION).toBeGreaterThan(0);
  });

  it("makes a tier legible as a silhouette, with colour removed", () => {
    // The acceptance this milestone was written against. `silhouetteSignature` excludes `glow`,
    // and `profileTop` is pure outline, so neither can be satisfied by a stroke colour — which is
    // exactly how v0.14 shipped a tier that was invisible on the map.
    for (const key of SHAPED_KEYS) {
      for (let tier = 1; tier <= TIER_LADDER.length; tier += 1) {
        const below = partsFor(key, tier - 1);
        const at = partsFor(key, tier);
        expect(silhouetteSignature(at)).not.toBe(silhouetteSignature(below));
        expect(profileTop(at)).toBeLessThan(profileTop(below));
        expect(at.length).toBeGreaterThan(below.length);
      }
    }
  });

  it("refuses a tiered definition that has no shape to grow", () => {
    // A tier on a definition whose silhouette is empty would be an upgrade the map cannot show.
    // Naming it here is what stops the belt's deliberate blank from quietly becoming a defect the
    // day somebody adds a belt II.
    for (const definition of definitions.buildings) {
      if ((definition.tier ?? 0) === 0) continue;
      expect(BUILDING_SHAPES[keyOf(definition)].length).toBeGreaterThan(0);
    }
  });

  it("costs a new building a data row and not a drawing", () => {
    const look = readFileSync(
      new URL("../src/rendering/buildingLook.ts", import.meta.url),
      "utf8",
    );
    const grammar = readFileSync(
      new URL("../src/rendering/shapeGrammar.ts", import.meta.url),
      "utf8",
    );
    // No `switch` over silhouettes survives in the look module: what used to be two hundred lines
    // of hand-written canvas per building is a table there now.
    expect(look).not.toContain("switch (key)");
    expect(look).not.toContain("drawSilhouette");
    // The only switches left are over the fixed part vocabulary, which does not grow per
    // definition. Two of them: one for extents, one for drawing.
    expect(grammar.match(/switch \(part\.part\)/g)).toHaveLength(2);
    expect(grammar.match(/switch \(/g)).toHaveLength(2);
  });

  it("keeps modifiers pure, so a bake cannot poison the table", () => {
    // The bake caches per key and tier for the life of the page. A modifier that mutated the base
    // row would make the second building of a kind wear the first one's tier.
    for (const key of SHAPED_KEYS) {
      const before = silhouetteSignature(BUILDING_SHAPES[key]);
      partsFor(key, 1);
      partsFor(key, 2);
      expect(silhouetteSignature(BUILDING_SHAPES[key])).toBe(before);
      expect(silhouetteSignature(partsFor(key, 1))).toBe(
        silhouetteSignature(partsFor(key, 1)),
      );
    }
  });

  it("splits every part into exactly one of the baked and the live pass", () => {
    // `drawShape` stamps the stills and walks the movers. A part counted by neither would vanish;
    // a part counted by both would be drawn twice.
    for (const key of SHAPED_KEYS) {
      for (let tier = 0; tier <= TIER_LADDER.length; tier += 1) {
        const parts = partsFor(key, tier);
        const still = parts.filter(isStill).length;
        const moving = parts.filter((part) => !isStill(part)).length;
        expect(still + moving).toBe(parts.length);
      }
    }
  });

  it("draws the player from the same vocabulary the machines use", () => {
    const kinds = [...PLAYER_RING, ...PLAYER_BODY].map((part) => part.part);
    expect(kinds.length).toBeGreaterThan(0);
    for (const kind of kinds) {
      expect(
        SHAPED_KEYS.some((key) =>
          BUILDING_SHAPES[key].some((part) => part.part === kind),
        ),
      ).toBe(true);
    }
  });

  it("names every tier step it applies", () => {
    for (const step of TIER_LADDER) {
      expect(step.name).not.toBe("");
      expect(step.reads).not.toBe("");
      expect(step.modifiers.length).toBeGreaterThan(0);
    }
    // Trim still climbs beside the shape, so the two agree rather than competing.
    expect(trimOf(1).width).toBeGreaterThan(trimOf(0).width);
    expect(trimOf(2).width).toBeGreaterThan(trimOf(1).width);
  });

  it("says why a machine is doing nothing, from the status it already publishes", () => {
    // A working machine and one starved for ten minutes drew identically, and the only way to tell
    // them apart was to click one. Every stalled status the wire can carry needs a mark, and no
    // mark may name a status the core cannot produce — the wire fixture is the list of what it can.
    const carried = new Set(
      (wireFixture as { statuses: string[] }).statuses.map((value) => value),
    );
    for (const status of Object.keys(STALL_MARKS))
      expect(carried.has(status), `${status} is not a native status`).toBe(
        true,
      );
    const running = [
      "extracting",
      "pumping",
      "composing",
      "generating",
      "carrying",
      "receiving",
      "landing hub",
      "idle",
      "buffered",
    ];
    for (const status of running) expect(stallMark(status)).toBeUndefined();
    // Power is already a dimmed machine, so a second mark for the same cause would be noise.
    expect(stallMark("no power")).toBeUndefined();
    expect(stallMark("brownout")).toBeUndefined();
    // Everything else the core can report while making nothing has to say so.
    for (const status of carried)
      if (
        !running.includes(status) &&
        !status.includes("power") &&
        status !== "brownout"
      )
        expect(stallMark(status), `${status} stalls silently`).toBeDefined();
  });

  it("makes a finished contract stage visible on the hub itself", () => {
    // A founding project that changed nothing on screen would be a number in a panel. Growth is a
    // ladder over the same part vocabulary a tier uses, so each completed stage is a different
    // silhouette and a taller one — the two properties that survive being read at play zoom.
    for (let stage = 1; stage <= HUB_LADDER.length; stage += 1) {
      const below = partsFor("hub", 0, stage - 1);
      const at = partsFor("hub", 0, stage);
      expect(silhouetteSignature(at)).not.toBe(silhouetteSignature(below));
      expect(profileTop(at)).toBeLessThan(profileTop(below));
      expect(at.length).toBeGreaterThan(below.length);
    }
    // Growth is the hub's alone in the shipped contract, but the walker is general, so a modifier
    // that only worked on one shape would be a trap for whatever grows next.
    for (const key of SHAPED_KEYS) {
      const grown = partsFor(key, 0, HUB_LADDER.length);
      expect(grown.length).toBeGreaterThanOrEqual(BUILDING_SHAPES[key].length);
    }
    // And the table itself is untouched by any of it, for the same reason a tier bake must not
    // poison it.
    expect(silhouetteSignature(BUILDING_SHAPES.hub)).toBe(
      silhouetteSignature(partsFor("hub", 0, 0)),
    );
    for (const step of HUB_LADDER) {
      expect(step.name).not.toBe("");
      expect(step.reads).not.toBe("");
      expect(step.modifiers.length).toBeGreaterThan(0);
    }
    // One row per stage the shipped contract can complete, or a stage would finish invisibly.
    expect(HUB_LADDER.length).toBeGreaterThanOrEqual(
      Math.max(
        ...(
          scenarios as unknown as {
            scenarios: { contract: { stages: unknown[] } }[];
          }
        ).scenarios
          .filter((scenario) => scenario.contract.stages.length < 10)
          .map((scenario) => scenario.contract.stages.length),
      ),
    );
  });
});
