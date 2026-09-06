import { readFileSync } from "node:fs";
import { Matrix4, type InstancedMesh } from "three";
import { describe, expect, it } from "vitest";

import { applyHabitatsPatch } from "../src/core/snapshotDelta";
import type { HabitatSnapshot } from "../src/core/types";
import { HabitatMeshes, tuftCount } from "../src/rendering/three/habitatMeshes";
import type { TerrainCell } from "../src/rendering/three/terrainMeshes";
import { readAppSource } from "./sourceGraph";
import {
  grazerCount,
  PopulationMeshes,
} from "../src/rendering/three/populationMeshes";
import type { HerdSnapshot } from "../src/core/types";

describe("fertile riverbank habitat", () => {
  it("patches exact capacity and removes zero-capacity tombstones", () => {
    const first: HabitatSnapshot = {
      q: 7,
      r: -2,
      x: 10644,
      y: -3072,
      radius: 1024,
      capacity: 100,
      grass: 0,
      grass_limit: 0,
      fouling: 0,
      discharge: 4,
    };
    const second: HabitatSnapshot = {
      ...first,
      q: 8,
      x: 12418,
      capacity: 150,
      grass: 0,
      grass_limit: 0,
      fouling: 0,
      discharge: 6,
    };
    const added = applyHabitatsPatch([first], { changed: [second] });
    expect(added).toEqual([first, second]);
    expect(
      applyHabitatsPatch(added, {
        changed: [{ ...first, capacity: 0, discharge: 0 }],
      }),
    ).toEqual([second]);
    expect(applyHabitatsPatch([first], { changed: [] })).toEqual([first]);
  });

  it("draws a capacity-scaled fringe of slender three-sided blades", () => {
    const habitats = new HabitatMeshes();
    const bank = (q: number): TerrainCell => ({
      q,
      r: 0,
      terrain: "shore",
      x: q * Math.sqrt(3),
      z: 0,
      height: 0,
      elevation: 0,
      surface: 0,
      substrate: "sand",
      waterDepth: 0,
      waterHeight: 0,
      discharge: 0,
    });
    const terrain = new Map([
      ["0,0", bank(0)],
      ["1,0", bank(1)],
    ]);
    const cells = [
      {
        q: 0,
        r: 0,
        x: 0,
        y: 0,
        radius: 1024,
        capacity: 50,
        grass: 0,
        grass_limit: 0,
        fouling: 0,
        discharge: 2,
      },
      {
        q: 1,
        r: 0,
        x: 1774,
        y: 0,
        radius: 1024,
        capacity: 175,
        grass: 0,
        grass_limit: 0,
        fouling: 0,
        discharge: 7,
      },
    ];
    expect(habitats.update(cells, terrain)).toBe(true);
    const mesh = habitats.group.getObjectByName(
      "fertile-riverbank-sedge",
    ) as InstancedMesh;
    const matrix = new Matrix4();
    let visible = 0;
    for (let i = 0; i < mesh.count; i++) {
      mesh.getMatrixAt(i, matrix);
      if (matrix.determinant() !== 0) visible++;
    }
    expect(visible).toBe(tuftCount(50) + tuftCount(175));
    expect(mesh.geometry.type).toBe("ConeGeometry");
    expect(
      (mesh.geometry as unknown as { parameters: { radialSegments: number } })
        .parameters.radialSegments,
    ).toBe(3);
    expect(habitats.update(cells, terrain)).toBe(false);
    habitats.dispose();
  });

  it("draws one body per animal, wherever the herd has walked to", () => {
    const populations = new PopulationMeshes();
    const grass = (q: number, r: number): TerrainCell => ({
      q,
      r,
      terrain: "lowland",
      x: q * Math.sqrt(3),
      z: r * 1.5,
      height: 0,
      elevation: 0,
      surface: 0,
      substrate: "soil",
      waterDepth: 0,
      waterHeight: 0,
      discharge: 0,
    });
    const terrain = new Map([
      ["0,0", grass(0, 0)],
      ["4,-9", grass(4, -9)],
    ]);
    const row = (id: number, count: number): HerdSnapshot => ({
      id,
      count,
      species: 1,
      from: [0, 0],
      to: [1774, 0],
      left_tick: 0,
      arrive_tick: 30,
      drive: "Rest",
      hunger: 0,
      thirst: 0,
      alarm: 0,
      healthy_ticks: 0,
      shortage_ticks: 0,
      hunt_tick: 0,
    });
    const resources = [row(1, 4), row(2, 2)];
    expect(populations.update(resources, 0, terrain)).toBe(true);
    const herds = populations.group.getObjectByName(
      "grazing-herds",
    ) as InstancedMesh;
    expect(herds.count).toBe(6);
    expect(grazerCount(0)).toBe(0);
    expect(grazerCount(4)).toBe(4);
    expect(populations.update(resources, 0, terrain)).toBe(false);
    populations.dispose();
  });

  it("names the habitat and explains its exact native causes", () => {
    const main = readAppSource();
    const html = readFileSync(
      new URL("../index.html", import.meta.url),
      "utf8",
    );
    expect(html).toContain('id="inspect-habitat"');
    expect(html).toContain('id="inspect-habitat-name"');
    expect(main).toContain('title.textContent = "Fertile riverbank"');
    expect(main).toContain("habitat.capacity");
    expect(main).toContain("habitat.discharge");
    expect(main).toContain("fresh standing water in its ring");
    expect(main).toContain(
      "A cut canal waters ground the same way a river does",
    );
  });
});
