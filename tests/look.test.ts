import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import { TERRAIN_ORDER } from "../src/core/terrain";
import {
  facingTip,
  NORTH,
  silhouetteOf,
  spanEnd,
  trimOf,
  workCycle,
} from "../src/rendering/buildingLook";
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
import type { EntitySnapshot } from "../src/core/types";

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
