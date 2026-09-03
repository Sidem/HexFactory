import { describe, expect, it } from "vitest";
import { Mesh } from "three";

import type { LandscapeLod } from "../src/core/types";
import { HEIGHT_UNIT_HEIGHT } from "../src/rendering/sceneScale";
import { DistantTerrain } from "../src/rendering/three/distantTerrain";

describe("distant terrain LOD", () => {
  it("does not expose coarse landform in the zoomed-out survey view", () => {
    const terrain = new DistantTerrain();
    terrain.setZoom(0.99);
    expect(terrain.group.visible).toBe(false);
    terrain.setZoom(1);
    expect(terrain.group.visible).toBe(true);
    terrain.dispose();
  });

  it("triangulates native coarse samples without creating a pick surface", () => {
    const lod: LandscapeLod = {
      step: 8,
      anchor_q: 0,
      anchor_r: 0,
      cells: [
        { q: 0, r: 0, x: 0, y: 0, height: 4, terrain: "lowland" },
        { q: 8, r: 0, x: 14192, y: 0, height: 8, terrain: "hills" },
        { q: 0, r: 8, x: 7096, y: 12288, height: 12, terrain: "highland" },
        {
          q: 8,
          r: 8,
          x: 21288,
          y: 12288,
          height: 16,
          terrain: "cliff",
        },
      ],
    };
    const terrain = new DistantTerrain();
    terrain.set(lod);
    const mesh = terrain.group.getObjectByName("distant-terrain-surface");
    expect(mesh).toBeInstanceOf(Mesh);
    const position = (mesh as Mesh).geometry.getAttribute("position");
    expect(position.count).toBe(6);
    expect(position.getY(5)).toBeCloseTo(16 * HEIGHT_UNIT_HEIGHT - 0.015);
    expect(terrain.group.name).toBe("distant-terrain-lod");
    terrain.dispose();
    expect(terrain.group.children).toHaveLength(0);
  });
});
