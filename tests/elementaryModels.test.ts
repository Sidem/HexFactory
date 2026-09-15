import { describe, expect, it } from "vitest";
import { Box3, Matrix4, Quaternion, Vector3 } from "three";
import type { InstancedMesh } from "three";
import { axialToPixel } from "@hexlife/embed/hex";
import data from "../src/data/definitions.json";
import type {
  BuildingDefinition,
  Definitions,
  EntitySnapshot,
  FactorySnapshot,
} from "../src/core/types";
import { elementaryParts } from "../src/rendering/three/elementaryModels";
import {
  elementaryCycle,
  extractorPose,
} from "../src/rendering/three/elementaryAnimation";
import {
  collectMachineParts,
  machinePartMatrix,
  PartGeometryLibrary,
  type MachinePartInstance,
} from "../src/rendering/three/machineMeshes";
import { BUILDING_COLORS } from "../src/rendering/FactoryRenderer";
import { createWorldMaterials } from "../src/rendering/three/materials";
import { WorldInstanceLayer } from "../src/rendering/three/worldInstances";
import { PlayerRig } from "../src/rendering/three/playerRig";

const definitions = data as Definitions;
const byId = new Map(definitions.buildings.map((d) => [d.id, d]));
const definition = (key: string): BuildingDefinition =>
  definitions.buildings.find((d) => d.key === key)!;
function building(key: string): EntitySnapshot {
  const d = definition(key);
  return {
    id: d.id,
    definition_id: d.id,
    kind: d.kind,
    q: 0,
    r: 0,
    orientation: 0,
    scenario_owned: false,
    inventory: [],
    progress: 0,
    progress_total: 30,
    status: "idle",
    footprint: d.footprint ?? [{ q: 0, r: 0 }],
  };
}
function snapshot(buildings: EntitySnapshot[], tick = 0): FactorySnapshot {
  return {
    seed: 1,
    scenario: "test",
    tick,
    buildings,
    resources: [],
    ground_items: [],
    belt_transit_ticks: 27,
    contract: { stage: 0 },
    player: {
      x: 0,
      y: 0,
      facing_x: 1,
      facing_y: 0,
      action_cooldown: 0,
      action_cooldown_total: 0,
    },
  } as unknown as FactorySnapshot;
}
function parts(b: EntitySnapshot): MachinePartInstance[] {
  return collectMachineParts(
    snapshot([b]),
    byId,
    (q, r) => (q === -1 && r === 0 ? 0.7 : 0.2),
    BUILDING_COLORS,
  );
}

describe("elementary buildings in the world", () => {
  it("seats each new body on the native platform, with finite animated transforms", () => {
    const library = new PartGeometryLibrary();
    const keys = [
      "landing-hub",
      "container",
      "burner-generator",
      "pole",
      "extractor",
      "smelter",
      "manual-workshop",
      "composer",
      "primitive-furnace",
      "kiln",
    ];
    for (const key of keys) {
      const b = building(key);
      expect(elementaryParts(definition(key))).toBeDefined();
      let lowest = Infinity;
      for (const p of parts(b)) {
        const geometry = library.get(p.part);
        geometry.computeBoundingBox();
        for (const progress of [0, 8, 16, 29]) {
          b.progress = progress;
          const matrix = machinePartMatrix(p, 1200, false);
          expect(matrix.elements.every(Number.isFinite), key).toBe(true);
          if (!p.animated)
            lowest = Math.min(
              lowest,
              new Box3().copy(geometry.boundingBox!).applyMatrix4(matrix).min.y,
            );
        }
      }
      expect(lowest, key).toBeCloseTo(parts(b)[0]!.groundHeight + 0.18, 5);
    }
    library.dispose();
  });

  it("keeps specialized fluid and livestock models out of the first-set replacements", () => {
    for (const key of [
      "oil-well",
      "water-tank",
      "oil-tank",
      "herd-station",
      "pasture-tender",
      "extractor-ii",
    ])
      expect(elementaryParts(definition(key)), key).toBeUndefined();
  });

  it("touches the exact native depot and exterior output, with connected arm joints", () => {
    const b = building("extractor");
    b.status = "extracting";
    b.extraction_source = { q: -1, r: 0, item_id: 4 };
    b.extraction_output = { q: 1, r: 0, direction: 0 };
    const instances = parts(b);
    const arm = instances.find((p) => p.part.model?.motion === "upper-arm")!;
    const forearm = instances.find((p) => p.part.model?.motion === "forearm")!;
    const pickup = extractorPose(arm, 0.5);
    const depot = axialToPixel(b.extraction_source, 1, { x: 0, y: 0 });
    expect(pickup.wrist.x).toBeCloseTo(depot.x);
    expect(pickup.wrist.z).toBeCloseTo(depot.y);
    expect(pickup.source.y).toBeCloseTo(0.97);
    const delivery = extractorPose(arm, 1);
    expect(delivery.wrist.x).toBeCloseTo(Math.sqrt(3) + 0.79);
    expect(delivery.wrist.z).toBeCloseTo(0);
    expect(delivery.carrying).toBe(false);
    for (const progress of [1, 8, 15, 23, 29]) {
      b.progress = progress;
      const upperTip = new Vector3(0, 0.5, 0).applyMatrix4(
        machinePartMatrix(arm, 0, false),
      );
      const foreStart = new Vector3(0, -0.5, 0).applyMatrix4(
        machinePartMatrix(forearm, 0, false),
      );
      expect(upperTip.distanceTo(foreStart)).toBeLessThan(1e-8);
    }
    // Orientation changes the body, never reinterprets either native endpoint.
    for (let orientation = 0; orientation < 6; orientation++) {
      b.orientation = orientation;
      const pose = extractorPose(arm, 0.5);
      expect(pose.wrist.x).toBeCloseTo(depot.x);
      expect(pose.wrist.z).toBeCloseTo(depot.y);
    }
  });

  it("interpolates only reported work and holds stopped and reduced-motion poses", () => {
    const b = building("extractor");
    b.status = "extracting";
    b.progress = 15;
    const instance = parts(b)[0]!;
    instance.previousBuilding = { ...b, progress: 12 };
    instance.receivedAt = 1000;
    instance.tickDuration = 100;
    expect(elementaryCycle(instance, 1050, false)).toBeCloseTo(0.45);
    expect(elementaryCycle(instance, 9000, false)).toBe(0.5);
    expect(elementaryCycle(instance, 1050, true)).toBe(0.5);
    b.status = "no power";
    expect(elementaryCycle(instance, 1000, false)).toBe(0.5);
  });

  it("refreshes working state without rebuilding meshes when a new immutable snapshot arrives", () => {
    const materials = createWorldMaterials();
    const layer = new WorldInstanceLayer(definitions, materials);
    const b = building("burner-generator");
    b.status = "generating";
    layer.setSnapshot(snapshot([b]), new Map(), 0);
    layer.update(300, false);
    const heat = layer.group.getObjectByName(
      "machine-part-elementary-box:glow:animated",
    ) as InstancedMesh;
    const before = new Matrix4();
    const after = new Matrix4();
    heat.getMatrixAt(0, before);
    const stopped = { ...b, status: "switched off" };
    expect(layer.setSnapshot(snapshot([stopped], 1), new Map(), 100)).toBe(
      false,
    );
    layer.update(400, false);
    expect(layer.group.getObjectByName(heat.name)).toBe(heat);
    heat.getMatrixAt(0, after);
    expect(before.determinant()).toBeGreaterThan(0);
    expect(after.determinant()).toBe(0);
    layer.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("shows crate fill and waiting extractor output only from actual native inventories", () => {
    const b = building("container");
    const fill = parts(b).find((p) => p.part.model?.motion === "stock")!;
    expect(machinePartMatrix(fill, 0, false).determinant()).toBe(0);
    b.inventory = [
      { item_id: 1, quantity: definition("container").capacity! / 2 },
    ];
    const scale = new Vector3();
    machinePartMatrix(fill, 0, false).decompose(
      new Vector3(),
      new Quaternion(),
      scale,
    );
    expect(scale.y).toBeCloseTo(0.3);
    const extractor = building("extractor");
    extractor.extraction_output = { q: 1, r: 0, direction: 0 };
    extractor.output_inventory = [{ item_id: 1, quantity: 1 }];
    const load = parts(extractor).find(
      (p) => p.part.model?.motion === "stored-output",
    )!;
    expect(machinePartMatrix(load, 0, false).determinant()).toBeGreaterThan(0);
    extractor.output_inventory = [];
    expect(machinePartMatrix(load, 0, false).determinant()).toBe(0);
  });

  it("puts the tool in the worker's hand only while native reports an attended batch", () => {
    const materials = createWorldMaterials();
    const rig = new PlayerRig(materials);
    const workshop = building("manual-workshop");
    workshop.status = "composing";
    workshop.progress = 5;
    rig.update(snapshot([]), 100, true, 100, () => 0, workshop);
    const tool = rig.group.getObjectByName("work-tool")!;
    expect(tool.visible).toBe(true);
    expect(tool.parent?.name).toBe("right-arm");
    rig.update(snapshot([]), 200, true, 100, () => 0);
    expect(tool.visible).toBe(false);
    rig.dispose();
    for (const material of materials.materials) material.dispose();
  });
});
