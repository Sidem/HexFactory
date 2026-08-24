import { describe, expect, it } from "vitest";
import { axialToPixel } from "@hexlife/embed/hex";

import type { EntitySnapshot, FactorySnapshot } from "../src/core/types";
import {
  BUILDING_SHAPES,
  partsFor,
  type SilhouetteKey,
} from "../src/rendering/buildingLook";
import { WORLD_SCALE } from "../src/rendering/landmarks";
import type { PartKind, ShapePart } from "../src/rendering/shapeGrammar";
import { HexSceneCamera } from "../src/rendering/three/HexSceneCamera";
import {
  buildPartGeometry,
  machinePartMatrix,
  type MachinePartInstance,
} from "../src/rendering/three/machineMeshes";
import { createWorldMaterials } from "../src/rendering/three/materials";
import { HEX_RING_START } from "../src/rendering/three/overlays";
import {
  QUALITY_SETTINGS,
  parseGraphicsProfile,
} from "../src/rendering/three/quality";
import {
  buildTerrainMeshes,
  HEX_RADIUS,
} from "../src/rendering/three/terrainMeshes";
import {
  TERRAIN_STYLE,
  visualHeight,
} from "../src/rendering/three/terrainStyle";
import { createTransportGeometry } from "../src/rendering/three/transportGeometry";
import {
  fieldVisualColor,
  FIELD_RESOURCE_SHAPES,
} from "../src/rendering/three/worldInstances";

describe("Visual Depth camera", () => {
  it("round-trips native world and axial points at every orbit and zoom extreme", () => {
    const coordinate = { q: 7, r: -4 };
    const world = axialToPixel(coordinate, WORLD_SCALE, { x: 0, y: 0 });
    for (let orbit = 0; orbit < 6; orbit += 1) {
      for (const factor of [0.01, 100]) {
        const camera = new HexSceneCamera();
        camera.resize(1440, 900);
        camera.recenter(world);
        for (let step = 0; step < orbit; step += 1) camera.orbitBy(1);
        camera.zoomAt(720, 450, factor);
        const screen = camera.projectWorld(world);
        const roundTrip = camera.worldAt(screen.x, screen.y);
        expect(roundTrip.x).toBeCloseTo(world.x, 0);
        expect(roundTrip.y).toBeCloseTo(world.y, 0);
        expect(camera.axialAt(screen.x, screen.y)).toEqual(coordinate);
      }
    }
  });

  it("keeps orbit as a six-step presentation value", () => {
    const camera = new HexSceneCamera();
    for (let step = 0; step < 12; step += 1) camera.orbitBy(1);
    expect(camera.orbitIndex).toBe(0);
    camera.orbitBy(-1);
    expect(camera.orbitIndex).toBe(5);
  });

  it("inverse-projects WASD through every camera orbit", () => {
    const directions = [
      { x: 0, y: -1 },
      { x: -1, y: 0 },
      { x: 0, y: 1 },
      { x: 1, y: 0 },
    ];
    const camera = new HexSceneCamera();
    camera.resize(1440, 900);
    for (let orbit = 0; orbit < 6; orbit += 1) {
      const center = camera.projectScene(0, 0, 0);
      for (const direction of directions) {
        const world = camera.screenMovement(direction.x, direction.y);
        const projected = camera.projectScene(world.x, 0, world.y);
        const screenX = projected.x - center.x;
        const screenY = projected.y - center.y;
        expect(screenX * direction.y - screenY * direction.x).toBeCloseTo(0, 8);
        expect(screenX * direction.x + screenY * direction.y).toBeGreaterThan(
          0,
        );
      }
      camera.orbitBy(1);
    }
  });
});

describe("Visual Depth generated geometry", () => {
  const kinds: PartKind[] = [
    "vessel",
    "chamber",
    "stack",
    "rotor",
    "aperture",
    "mast",
    "band",
    "mouth",
  ];

  it("gives every grammar kind finite low-poly geometry", () => {
    for (const kind of kinds) {
      const geometry = buildPartGeometry(kind, 4);
      const positions = geometry.getAttribute("position");
      expect(positions.count, kind).toBeGreaterThan(0);
      for (let index = 0; index < positions.count; index += 1) {
        expect(Number.isFinite(positions.getX(index)), kind).toBe(true);
        expect(Number.isFinite(positions.getY(index)), kind).toBe(true);
        expect(Number.isFinite(positions.getZ(index)), kind).toBe(true);
      }
      geometry.dispose();
    }
  });

  it("keeps every silhouette, tier, hub step, and phase transform finite", () => {
    const building: EntitySnapshot = {
      id: 1,
      definition_id: 1,
      kind: "composer",
      q: 0,
      r: 0,
      orientation: 0,
      scenario_owned: false,
      inventory: [],
      progress: 1,
      progress_total: 8,
      status: "composing",
      footprint: [{ q: 0, r: 0 }],
    };
    for (const key of Object.keys(BUILDING_SHAPES) as SilhouetteKey[]) {
      for (let tier = 0; tier <= 2; tier += 1) {
        for (let growth = 0; growth <= 2; growth += 1) {
          for (const part of partsFor(key, tier, growth)) {
            const instance: MachinePartInstance = {
              building,
              part,
              key: part.part,
              animated: part.phase !== undefined && part.phase !== "still",
              color: "#ffffff",
              glow: part.glow ?? null,
              groundHeight: 0.1,
              x: 0,
              z: 0,
            };
            const elements = machinePartMatrix(instance, 1234, false).elements;
            expect(
              elements.every(Number.isFinite),
              `${key}/${tier}/${growth}`,
            ).toBe(true);
          }
        }
      }
    }
  });

  it("freezes every animated transform under reduced motion", () => {
    const part: ShapePart = {
      part: "rotor",
      x: 0,
      y: 0,
      scale: 0.2,
      count: 3,
      phase: "spin",
    };
    const building: EntitySnapshot = {
      id: 9,
      definition_id: 3,
      kind: "composer",
      q: 0,
      r: 0,
      orientation: 0,
      scenario_owned: false,
      inventory: [],
      progress: 0,
      progress_total: 0,
      status: "composing",
      footprint: [{ q: 0, r: 0 }],
    };
    const instance: MachinePartInstance = {
      building,
      part,
      key: "rotor:3",
      animated: true,
      color: "#fff",
      glow: null,
      groundHeight: 0,
      x: 0,
      z: 0,
    };
    expect(machinePartMatrix(instance, 10, true).elements).toEqual(
      machinePartMatrix(instance, 9_999, true).elements,
    );
  });
});

describe("Visual Depth terrain and quality contracts", () => {
  it("gives every raw item form its own field silhouette", () => {
    expect(FIELD_RESOURCE_SHAPES).toEqual({
      ore: "faceted-shards",
      lump: "boulder-cluster",
      grains: "low-mounds",
      crystal: "prismatic-spires",
      log: "trunk-and-canopy",
    });
    expect(new Set(Object.values(FIELD_RESOURCE_SHAPES)).size).toBe(5);
  });

  it("keeps instance identity colour instead of multiplying it by a missing vertex colour", () => {
    const materials = createWorldMaterials();
    expect(materials.machine.vertexColors).toBe(false);
    expect(materials.machineDark.vertexColors).toBe(false);
    expect(materials.resource.vertexColors).toBe(false);
    expect(materials.resourceAccent.vertexColors).toBe(false);
    expect(fieldVisualColor("#39404a")).not.toBe("#39404a");
    for (const material of materials.materials) material.dispose();
  });

  it("builds belts from rails and contrasting transverse treads", () => {
    const geometry = createTransportGeometry();
    expect(geometry.belt.getAttribute("position").count).toBeGreaterThan(24);
    expect(geometry.beltDetail.getAttribute("position").count).toBeGreaterThan(
      24,
    );
    geometry.belt.dispose();
    geometry.beltDetail.dispose();
    geometry.bridge.dispose();
  });

  it("uses the exact pointy-top circumradius so three-cell junctions close", () => {
    expect(HEX_RADIUS).toBe(1);
    expect(2 * HEX_RADIUS * Math.cos(Math.PI / 6)).toBeCloseTo(
      Math.sqrt(3),
      12,
    );
  });

  it("starts six-sided interaction rings on the pointy-top tile vertices", () => {
    expect(HEX_RING_START).toBeCloseTo(Math.PI / 6, 12);
  });

  it("has one total height/material row for the pinned terrain union", () => {
    const keys = [
      "deep_water",
      "shallow_water",
      "shore",
      "lowland",
      "hills",
      "highland",
      "cliff",
    ] as const;
    expect(Object.keys(TERRAIN_STYLE)).toEqual(keys);
    expect(keys.map(visualHeight)).toEqual(
      [...keys.map(visualHeight)].sort((a, b) => a - b),
    );
  });

  it("meshes surveyed lowland but ignores published cells outside native coverage", () => {
    const snapshot = minimalSnapshot();
    const materials = createWorldMaterials();
    const built = buildTerrainMeshes(snapshot, materials);
    expect(built.cells.length).toBeGreaterThan(0);
    expect(built.cells.some(({ terrain }) => terrain === "lowland")).toBe(true);
    expect(built.cells.some(({ q, r }) => q === 99 && r === 99)).toBe(false);
    for (const geometry of built.geometries) geometry.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("keeps Low, Medium, and High as deliberate bounded profiles", () => {
    expect(Object.keys(QUALITY_SETTINGS)).toEqual(["low", "medium", "high"]);
    expect(QUALITY_SETTINGS.low.shadows).toBe(false);
    expect(QUALITY_SETTINGS.high.pixelRatioCap).toBeLessThanOrEqual(1.5);
    expect(parseGraphicsProfile("medium")).toBe("medium");
    expect(parseGraphicsProfile("ultra")).toBeNull();
  });
});

function minimalSnapshot(): FactorySnapshot {
  return {
    scenario: "test",
    scenario_name: "Test",
    world_version: 8,
    seed: 1,
    tick: 0,
    checksum: 0,
    delivered: 0,
    delivered_by_item: [],
    insight: 0,
    victory: false,
    contract: {
      key: "test",
      name: "Test",
      stage: 0,
      stages: 1,
      stage_key: "test",
      stage_name: "Test",
      stage_brief: "Test",
      requirements: [],
      complete: false,
    },
    requests: [],
    player: {
      x: 0,
      y: 0,
      facing_x: 1,
      facing_y: 0,
      move_x: 0,
      move_y: 0,
      inventory: {},
      action_cooldown: 0,
      build_range: WORLD_SCALE * 3,
      carry_slots: 8,
      carry_stacks: [],
      radius: 200,
      action_cooldown_total: 0,
      extract_radius: 1,
      creative: false,
    },
    researched: [],
    chunks: [
      {
        chunk_q: 0,
        chunk_r: 0,
        entity_count: 0,
        x: -WORLD_SCALE * 2,
        y: -WORLD_SCALE * 2,
        span: WORLD_SCALE * 4,
      },
    ],
    terrain: [
      {
        q: 99,
        r: 99,
        x: 0,
        y: 0,
        radius: WORLD_SCALE,
        terrain: "cliff",
      },
    ],
    resources: [],
    buildings: [],
    events: [],
  };
}
