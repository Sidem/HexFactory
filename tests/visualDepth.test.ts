import { BoundaryMeshes } from "../src/rendering/three/boundaryMeshes";
import definitions from "../src/data/definitions.json";
import { describe, expect, it } from "vitest";
import { axialToPixel } from "@hexlife/embed/hex";
import {
  Color,
  InstancedMesh,
  Matrix4,
  Quaternion,
  ShaderLib,
  Vector3,
  type CylinderGeometry,
  type Mesh,
  type RingGeometry,
  type WebGLProgramParametersWithUniforms,
  type WebGLRenderer,
} from "three";

import type {
  BoundaryDefinition,
  BuildingDefinition,
  EntitySnapshot,
  FactorySnapshot,
  SurfaceDefinition,
  Terrain,
  TerrainSnapshot,
} from "../src/core/types";
import { TRANSPORT_DIRECTIONS } from "../src/core/directions";
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
  MACHINE_SILHOUETTE_SCALE,
  MACHINE_VISUAL_SCALE,
  machinePartMatrix,
  type MachinePartInstance,
} from "../src/rendering/three/machineMeshes";
import { createWorldMaterials } from "../src/rendering/three/materials";
import {
  pavingSource,
  pavingStyle,
  UNKNOWN_PAVING,
} from "../src/rendering/three/pavingSurface";
import { SURFACE_LOOK } from "../src/rendering/surfaceLook";
import {
  HEX_RING_START,
  RANGE_RING_WIDTH,
  SpatialOverlays,
} from "../src/rendering/three/overlays";
import {
  QUALITY_SETTINGS,
  parseGraphicsProfile,
} from "../src/rendering/three/quality";
import {
  buildTerrainMeshes,
  FOG_HEIGHT,
  heightAt,
  heightAtWorld,
  HEX_RADIUS,
  pickTerrainCell,
  TerrainMeshCache,
  terrainAt,
  type TerrainCell,
} from "../src/rendering/three/terrainMeshes";
import {
  HEIGHT_UNIT_HEIGHT,
  RELIEF_CEILING,
  RELIEF_FLOOR,
  RELIEF_SPAN,
} from "../src/rendering/sceneScale";
import { TERRAIN_STYLE } from "../src/rendering/three/terrainStyle";
import {
  surfaceSource,
  TERRAIN_SURFACE,
  type SurfaceFamily,
} from "../src/rendering/three/terrainSurface";
import {
  createCurvedTransportGeometry,
  createTransportGeometry,
} from "../src/rendering/three/transportGeometry";
import { directionAngle } from "../src/rendering/three/directionAngle";
import {
  fieldShade,
  fieldVisualColor,
  FIELD_RESOURCE_SHAPES,
  plumeFor,
  powerWireLinks,
  WorldInstanceLayer,
} from "../src/rendering/three/worldInstances";

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
              material: part.material ?? "structure",
              groundHeight: 0.1,
              footprintScale: 1,
              visualScale: MACHINE_SILHOUETTE_SCALE[key],
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
      material: "ceramic",
      groundHeight: 0,
      footprintScale: 1,
      visualScale: MACHINE_SILHOUETTE_SCALE.wind,
      x: 0,
      z: 0,
    };
    expect(machinePartMatrix(instance, 10, true).elements).toEqual(
      machinePartMatrix(instance, 9_999, true).elements,
    );
  });

  // The Wayfinder's own half of this claim lives in `tests/playerRig.test.ts`, beside the rig.
  it("gives the wind turbine the skyline", () => {
    expect(MACHINE_SILHOUETTE_SCALE.wind).toBeGreaterThan(
      MACHINE_SILHOUETTE_SCALE.pole * 3,
    );
    expect(MACHINE_SILHOUETTE_SCALE.extractor).toBeGreaterThan(
      MACHINE_SILHOUETTE_SCALE.pole,
    );
  });

  it("spins an upright turbine in its vertical rotor disc", () => {
    const part: ShapePart = {
      part: "rotor",
      x: 0,
      y: -0.56,
      scale: 0.38,
      count: 3,
      phase: "spin",
      upright: true,
    };
    const building: EntitySnapshot = {
      id: 14,
      definition_id: 14,
      kind: "generator",
      q: 0,
      r: 0,
      orientation: 0,
      scenario_owned: false,
      inventory: [],
      progress: 0,
      progress_total: 0,
      status: "generating",
      footprint: [{ q: 0, r: 0 }],
    };
    const instance: MachinePartInstance = {
      building,
      part,
      key: "rotor:3",
      animated: true,
      color: "#fff",
      glow: null,
      material: "ceramic",
      groundHeight: 0,
      footprintScale: 1,
      visualScale: MACHINE_SILHOUETTE_SCALE.wind,
      x: 0,
      z: 0,
    };
    const position = new Vector3();
    const scale = new Vector3();
    const first = new Quaternion();
    const second = new Quaternion();
    machinePartMatrix(instance, 0, false).decompose(position, first, scale);
    machinePartMatrix(instance, 175, false).decompose(position, second, scale);
    const firstNormal = new Vector3(0, 1, 0).applyQuaternion(first);
    const secondNormal = new Vector3(0, 1, 0).applyQuaternion(second);
    expect(Math.abs(firstNormal.y)).toBeLessThan(1e-6);
    expect(firstNormal.distanceTo(secondNormal)).toBeLessThan(1e-6);
    const firstBlade = new Vector3(0, 0, 1).applyQuaternion(first);
    const secondBlade = new Vector3(0, 0, 1).applyQuaternion(second);
    expect(firstBlade.dot(secondBlade)).toBeLessThan(0.9);
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
    expect(materials.machineCeramic.vertexColors).toBe(false);
    expect(materials.machineBrass.vertexColors).toBe(false);
    expect(materials.machineDark.vertexColors).toBe(false);
    expect(materials.resource.vertexColors).toBe(false);
    expect(materials.resourceCoal.vertexColors).toBe(false);
    for (const material of materials.paving.all())
      expect(material.vertexColors).toBe(false);
    expect(materials.terrain.lowland.vertexColors).toBe(false);
    for (const material of materials.materials) material.dispose();
  });

  it("gives the bounded machine surfaces distinct physical and shader treatments", () => {
    const materials = createWorldMaterials();
    expect(materials.machineCeramic.roughness).toBeGreaterThan(
      materials.machineBrass.roughness,
    );
    expect(materials.machineBrass.metalness).toBeGreaterThan(
      materials.machine.metalness,
    );
    const keys = [
      materials.machine,
      materials.machineCeramic,
      materials.machineBrass,
      materials.machineDark,
    ].map((material) => material.customProgramCacheKey());
    expect(new Set(keys).size).toBe(keys.length);
    const shader = {
      uniforms: {},
      vertexShader: ShaderLib.physical.vertexShader,
      fragmentShader: ShaderLib.physical.fragmentShader,
    } as unknown as WebGLProgramParametersWithUniforms;
    materials.machineCeramic.onBeforeCompile(
      shader,
      undefined as unknown as WebGLRenderer,
    );
    expect(shader.vertexShader).toContain("hfMachineLocal");
    expect(shader.fragmentShader).toContain("hfMachineGrain");
    for (const material of materials.materials) material.dispose();
  });

  it("pools status-driven plumes and freezes them under reduced motion", () => {
    const materials = createWorldMaterials();
    const burner = {
      ...beltDefinition(),
      id: 13,
      key: "burner-generator",
      name: "Burner generator",
      kind: "generator" as const,
      power_source: "burner" as const,
    };
    const layer = new WorldInstanceLayer(
      {
        boundaries: [],
        surfaces: [],
        version: 1,
        items: [],
        recipes: [],
        requests: [],
        buildings: [burner],
      },
      materials,
    );
    const snapshot = minimalSnapshot();
    const generator = entity(1, burner.id, "generator", 0, 0, 0);
    generator.status = "generating";
    snapshot.buildings.push(generator);
    expect(plumeFor(generator, burner)).toBe("smoke");
    layer.setSnapshot(snapshot, new Map(), 0);
    layer.update(300, false);
    const plumes = layer.group.getObjectByName(
      "machine-plumes",
    ) as InstancedMesh;
    expect(plumes.count).toBe(3);
    const before = new Matrix4();
    const after = new Matrix4();
    layer.update(300, true);
    plumes.getMatrixAt(0, before);
    expect(plumes.count).toBe(1);
    layer.update(9_000, true);
    plumes.getMatrixAt(0, after);
    expect(after.elements).toEqual(before.elements);
    generator.status = "idle";
    expect(plumeFor(generator, burner)).toBeNull();
    layer.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("gives coal, stone, and sand different field materials and does not flatten their colours", () => {
    const materials = createWorldMaterials();
    expect(materials.resourceCoal.metalness).toBeGreaterThan(0.6);
    expect(materials.resourceCoal.roughness).toBeLessThan(0.3);
    expect(materials.resourceStone.roughness).toBeGreaterThan(0.7);
    expect(materials.resourceStone.metalness).toBeLessThan(
      materials.resourceCoal.metalness,
    );
    expect(materials.resourceSand.metalness).toBe(0);
    expect(materials.resourceSand.roughness).toBeGreaterThan(
      materials.resourceStone.roughness,
    );

    const hslOf = (hex: string): { h: number; s: number; l: number } => {
      const hsl = { h: 0, s: 0, l: 0 };
      new Color(hex).getHSL(hsl);
      return hsl;
    };
    const coal = hslOf(fieldVisualColor("#000000"));
    const stone = hslOf(fieldVisualColor("#8b9098"));
    const sand = hslOf(fieldVisualColor("#e6d197"));
    expect(coal.l).toBeLessThan(0.28);
    expect(stone.l).toBeGreaterThan(coal.l);
    expect(stone.s).toBeLessThan(0.15);
    expect(sand.l).toBeGreaterThan(0.55);
    expect(sand.s).toBeGreaterThan(stone.s);
    expect(fieldShade("#8b9098", 0.12)).not.toBe(fieldShade("#8b9098", -0.14));
    for (const material of materials.materials) material.dispose();
  });

  it("renders every occupied cell of a multi-cell building as one connected platform", () => {
    const materials = createWorldMaterials();
    const layer = new WorldInstanceLayer(
      {
        boundaries: [],
        surfaces: [],
        version: 1,
        items: [],
        recipes: [],
        requests: [],
        buildings: [
          {
            id: 6,
            key: "landing-hub",
            name: "Landing hub",
            kind: "hub",
            description: "Test hub",
            icon: "HUB",
            construction_cost: [],
            placement_rule: "ground",
            buildable: false,
            blocks_movement: true,
            footprint: [
              { q: 0, r: 0 },
              { q: 0, r: 1 },
              { q: -1, r: 1 },
            ],
          },
        ],
      },
      materials,
    );
    const snapshot = minimalSnapshot();
    snapshot.buildings.push({
      id: 1,
      q: 0,
      r: 0,
      definition_id: 6,
      kind: "hub",
      orientation: 0,
      scenario_owned: true,
      inventory: [],
      progress: 0,
      progress_total: 0,
      status: "landing hub",
      footprint: [
        { q: 0, r: 0 },
        { q: 0, r: 1 },
        { q: -1, r: 1 },
      ],
    });
    layer.setSnapshot(snapshot, new Map());

    const decks = layer.group.getObjectByName("multi-cell-decks");
    const links = layer.group.getObjectByName("multi-cell-links");
    expect(decks).toBeInstanceOf(InstancedMesh);
    expect((decks as InstancedMesh).count).toBe(3);
    expect(links).toBeInstanceOf(InstancedMesh);
    expect((links as InstancedMesh).count).toBe(3);

    layer.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("builds belts from rails and contrasting transverse treads", () => {
    const geometry = createTransportGeometry();
    const straightPositions = geometry.belt.getAttribute("position").count;
    expect(straightPositions).toBeGreaterThan(24);
    expect(geometry.beltDetail.getAttribute("position").count).toBeGreaterThan(
      24,
    );
    geometry.belt.dispose();
    geometry.beltDetail.dispose();
    expect(geometry.pipe.getAttribute("position").count).toBeGreaterThan(24);
    expect(geometry.pipeDetail.getAttribute("position").count).toBeGreaterThan(
      24,
    );
    expect(geometry.portal.getAttribute("position").count).toBeGreaterThan(24);
    geometry.pipe.dispose();
    geometry.pipeDetail.dispose();
    geometry.portal.dispose();
    geometry.portalDetail.dispose();
    geometry.bridge.dispose();

    const curve = createCurvedTransportGeometry(Math.PI / 3);
    curve.frame.computeBoundingBox();
    expect(curve.frame.getAttribute("position").count).toBeGreaterThan(
      straightPositions,
    );
    expect(
      curve.frame.boundingBox!.max.z - curve.frame.boundingBox!.min.z,
    ).toBeGreaterThan(0.4);
    curve.frame.dispose();
    curve.detail.dispose();
  });

  it("draws fluid pipes as coupled conduits and underpasses as guarded portal pairs", () => {
    const materials = createWorldMaterials();
    const underpass = {
      ...beltDefinition(),
      id: 33,
      key: "pipe-underpass",
      name: "Pipe underpass",
      transport_medium: "fluid" as const,
      underpass_span: 4,
    };
    const layer = new WorldInstanceLayer(
      {
        boundaries: [],
        surfaces: [],
        version: 1,
        items: [],
        recipes: [],
        requests: [],
        buildings: [underpass],
      },
      materials,
    );
    const snapshot = minimalSnapshot();
    snapshot.buildings.push(
      entity(1, 33, "belt", 0, 0, 0, 2),
      entity(2, 33, "belt", 2, 0, 0),
    );
    layer.setSnapshot(snapshot, new Map(), 0);

    expect(layer.group.getObjectByName("fluid-pipes")).toBeInstanceOf(
      InstancedMesh,
    );
    expect(
      layer.group.getObjectByName("fluid-pipe-connections"),
    ).toBeInstanceOf(InstancedMesh);
    const portals = layer.group.getObjectByName(
      "underpass-portals",
    ) as InstancedMesh;
    const caution = layer.group.getObjectByName(
      "underpass-caution-panels",
    ) as InstancedMesh;
    expect(portals.count).toBe(2);
    expect(caution.count).toBe(2);

    layer.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("points transport along the native heading and connects it to belts with stable treads", () => {
    // Along the heading, not ninety degrees across it.
    expect(directionAngle(0)).toBeCloseTo(0, 12);
    expect(directionAngle(1)).toBeCloseTo(-Math.PI / 3, 12);
    expect(Math.abs(directionAngle(3))).toBeCloseTo(Math.PI, 12);

    const materials = createWorldMaterials();
    const layer = new WorldInstanceLayer(
      {
        boundaries: [],
        surfaces: [],
        version: 1,
        items: [],
        recipes: [],
        requests: [],
        buildings: [beltDefinition()],
      },
      materials,
    );
    const snapshot = minimalSnapshot();
    snapshot.buildings.push(
      entity(1, 2, "composer", 0, 0, 0, 2),
      entity(2, 2, "belt", 1, 0, 0),
    );
    layer.setSnapshot(snapshot, new Map(), 0);
    const connections = layer.group.getObjectByName("transport-connections");
    const connectionTreads = layer.group.getObjectByName(
      "transport-connection-treads",
    );
    const indicators = layer.group.getObjectByName(
      "building-output-indicators",
    );
    const treads = layer.group.getObjectByName(
      "transport-treads",
    ) as InstancedMesh;
    expect(connections).toBeInstanceOf(InstancedMesh);
    expect((connections as InstancedMesh).count).toBe(1);
    expect(connectionTreads).toBeInstanceOf(InstancedMesh);
    expect((connectionTreads as InstancedMesh).count).toBe(1);
    expect((indicators as InstancedMesh).count).toBe(2);
    const before = new Matrix4();
    const after = new Matrix4();
    layer.update(0, false);
    treads.getMatrixAt(0, before);
    layer.update(120, false);
    treads.getMatrixAt(0, after);
    expect(after.elements).toEqual(before.elements);

    const feet = layer.group.getObjectByName("building-feet") as InstancedMesh;
    expect(feet.count).toBe(1);
    feet.getMatrixAt(0, after);
    expect(after.elements[0]).toBeGreaterThan(1.1);
    expect(MACHINE_VISUAL_SCALE).toBeGreaterThanOrEqual(1.35);

    layer.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("draws separate item-coloured outlets from the chosen multi-cell footprint ports", () => {
    const materials = createWorldMaterials();
    const composer = { ...beltDefinition(), id: 30, kind: "composer" as const };
    const layer = new WorldInstanceLayer(
      {
        boundaries: [],
        surfaces: [],
        version: 1,
        items: [
          {
            id: 29,
            key: "bitumen",
            name: "Bitumen",
            color: "#a58d72",
            icon: "lump",
            description: "",
            stack_size: 40,
          },
          {
            id: 30,
            key: "fuel",
            name: "Fuel",
            color: "#efbb66",
            icon: "droplet",
            description: "",
            stack_size: 40,
          },
        ],
        recipes: [],
        requests: [],
        buildings: [beltDefinition(), composer],
      },
      materials,
    );
    const snapshot = minimalSnapshot();
    const refinery = entity(1, 30, "composer", 0, 0, 0);
    refinery.footprint = [
      { q: 0, r: 0 },
      { q: -1, r: 0 },
    ];
    refinery.output_routes = [
      { item_id: 29, q: 0, r: 0, direction: 0, target_id: 2 },
      { item_id: 30, q: -1, r: 0, direction: 2, target_id: 3 },
    ];
    snapshot.buildings.push(
      refinery,
      entity(2, 2, "belt", 1, 0, 0),
      entity(3, 2, "belt", -2, 1, 2),
    );
    layer.setSnapshot(snapshot, new Map(), 0);

    const indicators = layer.group.getObjectByName(
      "building-output-indicators",
    ) as InstancedMesh;
    const connections = layer.group.getObjectByName(
      "transport-connections",
    ) as InstancedMesh;
    expect(indicators.count).toBe(4);
    expect(connections.count).toBe(2);
    const first = new Matrix4();
    const second = new Matrix4();
    indicators.getMatrixAt(0, first);
    indicators.getMatrixAt(1, second);
    expect(first.elements[12]).not.toBeCloseTo(second.elements[12]!, 4);

    layer.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("slopes transport links between terrain heights and lets cargo settle once", () => {
    const materials = createWorldMaterials();
    const layer = new WorldInstanceLayer(
      {
        boundaries: [],
        surfaces: [],
        version: 1,
        items: [
          {
            id: 1,
            key: "ore",
            name: "Ore",
            description: "Test ore",
            color: "#fff",
            icon: "ore",
            stack_size: 20,
          },
        ],
        recipes: [],
        requests: [],
        buildings: [beltDefinition()],
      },
      materials,
    );
    const snapshot = minimalSnapshot();
    snapshot.tick = 13;
    const from = entity(1, 2, "belt", 0, 0, 0, 2);
    from.lane = [{ cargo: { item_id: 1, quantity: 1 }, entered: 0 }];
    from.status = "carrying";
    snapshot.buildings.push(from, entity(2, 2, "belt", 1, 0, 0));
    const terrain = new Map([
      ["0,0", sceneCell(0, 0, "lowland", 0, 0, 0.1)],
      ["1,0", sceneCell(1, 0, "highland", 1, 0, 0.62)],
    ]);
    layer.setSnapshot(snapshot, terrain, 0);

    const link = layer.group.getObjectByName(
      "transport-connections",
    ) as InstancedMesh;
    const linkMatrix = new Matrix4();
    link.getMatrixAt(0, linkMatrix);
    expect(Math.abs(linkMatrix.elements[1]!)).toBeGreaterThan(0.1);

    const cargo = layer.group.getObjectByName("moving-cargo") as InstancedMesh;
    const start = new Matrix4();
    const later = new Matrix4();
    layer.update(0, false);
    cargo.getMatrixAt(0, start);
    // One loaded belt draws one item, and it is drawn large enough and high enough to be the thing
    // the eye follows. A speck sunk into the deck reads as an empty belt, and "is anything actually
    // moving?" is the first question a factory has to answer at a glance. The belt's deck stands
    // 0.23 over its hex, and its cargo rides on top of that rather than inside it.
    expect(cargo.count).toBe(1);
    cargo.geometry.computeBoundingSphere();
    expect(cargo.geometry.boundingSphere?.radius ?? 0).toBeGreaterThan(0.15);
    expect(start.elements[13]!).toBeGreaterThan(0.1 + 0.23);
    // An item halfway across a 27-tick hex creeps toward the next belt between snapshots.
    layer.update(250, false);
    cargo.getMatrixAt(0, later);
    expect(later.elements[12]!).toBeGreaterThan(start.elements[12]!);

    from.lane = [];
    from.cargo = { item_id: 1, quantity: 1 };
    from.status = "output blocked";
    layer.setSnapshot(snapshot, terrain, 0);
    const blocked = layer.group.getObjectByName(
      "moving-cargo",
    ) as InstancedMesh;
    const settled = new Matrix4();
    const settledLater = new Matrix4();
    layer.update(0, false);
    blocked.getMatrixAt(0, settled);
    layer.update(250, false);
    blocked.getMatrixAt(0, settledLater);
    expect(settledLater.elements).toEqual(settled.elements);
    expect(settled.elements[12]!).toBeGreaterThan(later.elements[12]!);

    from.lane = [
      { cargo: { item_id: 1, quantity: 1 }, entered: 0 },
      { cargo: { item_id: 1, quantity: 1 }, entered: 5 },
    ];
    layer.setSnapshot(snapshot, terrain, 0);
    const jammed = layer.group.getObjectByName("moving-cargo") as InstancedMesh;
    layer.update(0, false);
    expect(jammed.count).toBe(3);

    layer.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("joins a changing belt heading with a generated rail-and-tread curve", () => {
    const materials = createWorldMaterials();
    const layer = new WorldInstanceLayer(
      {
        boundaries: [],
        surfaces: [],
        version: 1,
        items: [],
        recipes: [],
        requests: [],
        buildings: [beltDefinition()],
      },
      materials,
    );
    const snapshot = minimalSnapshot();
    snapshot.buildings.push(
      entity(1, 2, "belt", 0, 0, 0, 2),
      entity(2, 2, "belt", 1, 0, 1, 3),
      entity(3, 2, "belt", 1, 1, 1),
    );
    layer.setSnapshot(snapshot, new Map(), 0);

    const curves = layer.group.getObjectByName("transport-curves");
    expect(curves).toBeDefined();
    expect(curves!.getObjectByName("transport-curve-rails")).toBeInstanceOf(
      InstancedMesh,
    );
    expect(curves!.getObjectByName("transport-curve-treads")).toBeInstanceOf(
      InstancedMesh,
    );
    expect(
      (layer.group.getObjectByName("transport-connections") as InstancedMesh)
        .count,
    ).toBe(2);

    layer.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("joins every input and output heading for belts and pipes", () => {
    const materials = createWorldMaterials();
    const pipe = {
      ...beltDefinition(),
      id: 3,
      key: "pipe",
      name: "Pipe",
      transport_medium: "fluid" as const,
    };
    const layer = new WorldInstanceLayer(
      {
        boundaries: [],
        surfaces: [],
        version: 1,
        items: [],
        recipes: [],
        requests: [],
        buildings: [beltDefinition(), pipe],
      },
      materials,
    );
    const snapshot = minimalSnapshot();
    let id = 1;
    for (const [mediumIndex, definition] of [
      beltDefinition(),
      pipe,
    ].entries()) {
      for (const incoming of TRANSPORT_DIRECTIONS) {
        for (const outgoing of TRANSPORT_DIRECTIONS) {
          const targetQ =
            mediumIndex * 2_000 + incoming.index * 100 + outgoing.index * 5;
          const targetR = mediumIndex * 2_000;
          const targetId = id + 1;
          snapshot.buildings.push(
            entity(
              id,
              definition.id,
              "belt",
              targetQ - incoming.q,
              targetR - incoming.r,
              incoming.index,
              targetId,
            ),
            entity(
              targetId,
              definition.id,
              "belt",
              targetQ,
              targetR,
              outgoing.index,
            ),
          );
          id += 2;
        }
      }
    }
    layer.setSnapshot(snapshot, new Map(), 0);

    const instanceCount = (name: string): number => {
      let count = 0;
      layer.group.traverse((object) => {
        if (object.name === name && object instanceof InstancedMesh)
          count += object.count;
      });
      return count;
    };
    const pairsPerMedium = TRANSPORT_DIRECTIONS.length ** 2;
    const collinearTargets = TRANSPORT_DIRECTIONS.length * 2;
    const curvedTargets = pairsPerMedium - collinearTargets;
    expect(instanceCount("transport-connections")).toBe(pairsPerMedium);
    expect(instanceCount("fluid-pipe-connections")).toBe(pairsPerMedium);
    expect(instanceCount("transport-rails")).toBe(
      pairsPerMedium + collinearTargets,
    );
    expect(instanceCount("fluid-pipes")).toBe(
      pairsPerMedium + collinearTargets,
    );
    expect(instanceCount("transport-curve-rails")).toBe(curvedTargets);
    expect(instanceCount("fluid-pipe-curve-bodies")).toBe(curvedTargets);

    layer.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("keeps closed turning loops connected when no segment stays straight", () => {
    for (const medium of ["solid", "fluid"] as const) {
      const materials = createWorldMaterials();
      const definition = {
        ...beltDefinition(),
        transport_medium: medium,
      };
      const layer = new WorldInstanceLayer(
        {
          boundaries: [],
          surfaces: [],
          version: 1,
          items: [],
          recipes: [],
          requests: [],
          buildings: [definition],
        },
        materials,
      );
      const snapshot = minimalSnapshot();
      snapshot.buildings.push(
        entity(1, definition.id, "belt", 0, 0, 0, 2),
        entity(2, definition.id, "belt", 1, 0, 2, 3),
        entity(3, definition.id, "belt", 0, 1, 4, 1),
      );
      layer.setSnapshot(snapshot, new Map(), 0);

      const count = (name: string): number => {
        let total = 0;
        layer.group.traverse((object) => {
          if (object.name === name && object instanceof InstancedMesh)
            total += object.count;
        });
        return total;
      };
      expect(
        count(
          medium === "fluid"
            ? "fluid-pipe-connections"
            : "transport-connections",
        ),
      ).toBe(3);
      expect(
        count(
          medium === "fluid"
            ? "fluid-pipe-curve-bodies"
            : "transport-curve-rails",
        ),
      ).toBe(3);
      expect(
        count(medium === "fluid" ? "fluid-pipes" : "transport-rails"),
      ).toBe(0);

      layer.dispose();
      for (const material of materials.materials) material.dispose();
    }
  });

  it("draws one sagging instanced wire for every native pole link", () => {
    const pole = {
      ...beltDefinition(),
      id: 12,
      key: "pole",
      name: "Pole",
      kind: "pole" as const,
      supply_radius: 3,
      pole_reach: 6,
    };
    const machine = {
      ...beltDefinition(),
      id: 3,
      key: "composer",
      name: "Composer",
      kind: "composer" as const,
      power_draw: 8,
    };
    const definitions = new Map<number, BuildingDefinition>([
      [pole.id, pole],
      [machine.id, machine],
    ]);
    const buildings = [
      entity(1, 12, "pole", 0, 0, 0),
      entity(2, 12, "pole", 4, 0, 0),
      entity(3, 3, "composer", 1, 0, 0),
    ];
    expect(powerWireLinks(buildings, definitions)).toEqual([
      { fromId: 1, toId: 2 },
      { fromId: 1, toId: 3 },
      { fromId: 2, toId: 3 },
    ]);

    const materials = createWorldMaterials();
    const layer = new WorldInstanceLayer(
      {
        boundaries: [],
        surfaces: [],
        version: 1,
        items: [],
        recipes: [],
        requests: [],
        buildings: [pole, machine],
      },
      materials,
    );
    const snapshot = minimalSnapshot();
    snapshot.buildings.push(...buildings);
    layer.setSnapshot(snapshot, new Map());
    const wires = layer.group.getObjectByName("pole-wires");
    expect(wires).toBeInstanceOf(InstancedMesh);
    expect((wires as InstancedMesh).count).toBe(21);

    layer.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("stands the scene on the exact pointy-top lattice", () => {
    // The circumradius three-cell junctions close on...
    expect(HEX_RADIUS).toBe(1);
    expect(2 * HEX_RADIUS * Math.cos(Math.PI / 6)).toBeCloseTo(
      Math.sqrt(3),
      12,
    );
    // ...and the vertex a six-sided interaction ring starts on.
    expect(HEX_RING_START).toBeCloseTo(Math.PI / 6, 12);
  });

  it("changes the world hex under a stationary pointer when the followed player moves", () => {
    const camera = new HexSceneCamera();
    camera.resize(1440, 900);
    camera.follow({ x: 0, y: 0 });
    const before = camera.axialAt(610, 420);
    camera.follow({ x: WORLD_SCALE * 24, y: WORLD_SCALE * 12 });
    expect(camera.axialAt(610, 420)).not.toEqual(before);
  });

  it("moves the hover culling bound with far-away tiles", () => {
    const materials = createWorldMaterials();
    const overlays = new SpatialOverlays(materials);
    const hover = { q: 40, r: 20 };
    overlays.update(
      minimalSnapshot(),
      {
        hover,
        selection: null,
        placement: null,
        dragPath: [],
        buildMode: false,
        gridToggled: false,
        buildFootprint: [{ q: 0, r: 0 }],
        buildOrientation: 0,
        buildReach: null,
        gathering: false,
      },
      new Map(),
    );

    const legal = overlays.group.children.find(
      (child): child is InstancedMesh => child instanceof InstancedMesh,
    );
    const point = axialToPixel(hover, 1, { x: 0, y: 0 });
    expect(legal?.boundingSphere?.center.x).toBeCloseTo(point.x, 5);
    expect(legal?.boundingSphere?.center.z).toBeCloseTo(point.y, 5);

    overlays.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("keeps range rims thin at every radius and separates build from pole colours", () => {
    const materials = createWorldMaterials();
    const overlays = new SpatialOverlays(materials);
    const state = {
      hover: { q: 0, r: 0 },
      selection: null,
      placement: null,
      dragPath: [],
      buildMode: true,
      gridToggled: false,
      buildFootprint: [{ q: 0, r: 0 }],
      buildOrientation: 0,
      buildReach: { extract: null, supply: 3, link: 6 },
      gathering: false,
    };
    overlays.update(minimalSnapshot(), state, new Map());

    const build = overlays.group.getObjectByName(
      "build-range-ring",
    ) as Mesh<RingGeometry>;
    const supply = overlays.group.getObjectByName(
      "pole-supply-range-ring",
    ) as Mesh<RingGeometry>;
    const link = overlays.group.getObjectByName(
      "pole-link-range-ring",
    ) as Mesh<RingGeometry>;
    const width = (mesh: Mesh<RingGeometry>): number =>
      mesh.geometry.parameters.outerRadius -
      mesh.geometry.parameters.innerRadius;

    expect(width(build)).toBeCloseTo(RANGE_RING_WIDTH.build, 8);
    expect(width(supply)).toBeCloseTo(RANGE_RING_WIDTH.supply, 8);
    expect(width(link)).toBeCloseTo(RANGE_RING_WIDTH.link, 8);
    expect(width(link)).toBeLessThan(width(supply));
    expect(build.material).toBe(materials.buildRange);
    expect(supply.material).toBe(materials.poleSupplyRange);
    expect(materials.buildRange.color.getHex()).not.toBe(
      materials.poleSupplyRange.color.getHex(),
    );

    state.buildReach.link = 12;
    overlays.update(minimalSnapshot(), state, new Map());
    expect(width(link)).toBeCloseTo(RANGE_RING_WIDTH.link, 8);

    overlays.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("pins the two ordered tables the renderer reads: the bands and the quality profiles", () => {
    // The band order is what `terrainMeshes` uses as a band's draw-group look, so reordering this
    // record silently repaints the world. It is also the order a legend reads in.
    expect(Object.keys(TERRAIN_STYLE)).toEqual([
      "deep_water",
      "shallow_water",
      "shore",
      "lowland",
      "hills",
      "highland",
      "cliff",
    ]);
    // And Low, Medium and High are deliberate bounded profiles, not a free-form dial.
    expect(Object.keys(QUALITY_SETTINGS)).toEqual(["low", "medium", "high"]);
    expect(QUALITY_SETTINGS.low.shadows).toBe(false);
    expect(QUALITY_SETTINGS.high.pixelRatioCap).toBeLessThanOrEqual(1.5);
    expect(parseGraphicsProfile("medium")).toBe("medium");
    expect(parseGraphicsProfile("ultra")).toBeNull();
  });

  it("meshes exactly the cells native published and infers none", () => {
    const snapshot = minimalSnapshot();
    const materials = createWorldMaterials();
    const built = buildTerrainMeshes(snapshot, materials);
    expect(built.cells.map(({ q, r }) => [q, r])).toEqual(
      [...SURVEYED_CELLS].sort((a, b) => a[1] - b[1] || a[0] - b[0]),
    );
    // `(-1, 0)` sits inside the published chunk rectangle and is still not drawn: the old builder
    // scanned that rectangle and called every gap surveyed lowland, and nothing does now.
    expect(terrainAt(built.cellByKey, -1, 0)).toBeUndefined();
    for (const geometry of built.geometries) geometry.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("bounds exploration rebuilds to the new native chunk and its neighbours", () => {
    const materials = createWorldMaterials();
    const cache = new TerrainMeshCache(materials);
    const chunk = (chunk_q: number, chunk_r: number) => ({
      chunk_q,
      chunk_r,
      entity_count: 0,
      x: chunk_q * WORLD_SCALE * 2,
      y: chunk_r * WORLD_SCALE * 2,
      span: WORLD_SCALE * 2,
    });
    const tiles = (chunk_q: number, chunk_r: number) => {
      const result: TerrainSnapshot[] = [];
      for (let r = chunk_r * 2; r < chunk_r * 2 + 2; r += 1)
        for (let q = chunk_q * 2; q < chunk_q * 2 + 2; q += 1)
          result.push(lowlandTile(q, r));
      return result;
    };
    const initial = {
      ...minimalSnapshot(),
      chunks: [chunk(0, 0), chunk(1, 0), chunk(4, 0)],
      terrain: [...tiles(0, 0), ...tiles(1, 0), ...tiles(4, 0)],
    };
    cache.update(initial);
    const untouched = cache.surface.getObjectByName(
      "terrain-chunk-surface-0,0",
    );
    const neighbour = cache.surface.getObjectByName(
      "terrain-chunk-surface-1,0",
    );

    const explored = {
      ...initial,
      chunks: [...initial.chunks, chunk(2, 0)],
      // The host's terrain patch preserves every old object and appends the 64 production cells.
      terrain: [...initial.terrain, ...tiles(2, 0)],
    };
    expect(cache.update(explored)).toBe(2);
    expect(cache.surface.getObjectByName("terrain-chunk-surface-0,0")).toBe(
      untouched,
    );
    expect(cache.surface.getObjectByName("terrain-chunk-surface-1,0")).not.toBe(
      neighbour,
    );
    expect(
      cache.surface.getObjectByName("terrain-chunk-surface-2,0"),
    ).toBeTruthy();
    const distant = cache.surface.getObjectByName("terrain-chunk-surface-4,0");

    const far = {
      ...explored,
      chunks: [...explored.chunks, chunk(10, 8)],
      terrain: [...explored.terrain, ...tiles(10, 8)],
    };
    expect(cache.update(far)).toBe(1);
    expect(cache.surface.getObjectByName("terrain-chunk-surface-0,0")).toBe(
      untouched,
    );
    expect(cache.surface.getObjectByName("terrain-chunk-surface-4,0")).toBe(
      distant,
    );

    cache.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("does not keep fog dissolve on the seams of fully surveyed chunks", () => {
    const materials = createWorldMaterials();
    const cache = new TerrainMeshCache(materials);
    const chunk = (chunk_q: number, chunk_r: number) => ({
      chunk_q,
      chunk_r,
      entity_count: 0,
      x: chunk_q * WORLD_SCALE * 2,
      y: chunk_r * WORLD_SCALE * 2,
      span: WORLD_SCALE * 2,
    });
    const tiles = (chunk_q: number, chunk_r: number) => {
      const result: TerrainSnapshot[] = [];
      for (let r = chunk_r * 2; r < chunk_r * 2 + 2; r += 1)
        for (let q = chunk_q * 2; q < chunk_q * 2 + 2; q += 1)
          result.push(lowlandTile(q, r));
      return result;
    };
    const chunks: ReturnType<typeof chunk>[] = [];
    const terrain: TerrainSnapshot[] = [];
    // Five chunks on a side leaves the centre more than two cells from the real survey rim, so
    // the two-ring dissolve cannot reach it. A 3x3 of these 2-cell chunks still sits inside that
    // fade and would pass even if seams were wrongly painted.
    for (let chunkR = -2; chunkR <= 2; chunkR += 1)
      for (let chunkQ = -2; chunkQ <= 2; chunkQ += 1) {
        chunks.push(chunk(chunkQ, chunkR));
        terrain.push(...tiles(chunkQ, chunkR));
      }
    cache.update({ ...minimalSnapshot(), chunks, terrain });

    const interior = cache.surface.getObjectByName("terrain-chunk-surface-0,0");
    const ground = interior?.getObjectByName("terrain-ground-0,0") as
      | Mesh
      | undefined;
    expect(ground).toBeTruthy();
    const fade = [
      ...(ground!.geometry.getAttribute("frontierFade").array as Float32Array),
    ];
    expect(Math.min(...fade)).toBe(1);
    expect(
      interior?.getObjectByName("terrain-frontier-skirt-0,0"),
    ).toBeUndefined();
    expect(
      cache.surface
        .getObjectByName("terrain-chunk-surface--2,-2")
        ?.getObjectByName("terrain-frontier-skirt--2,-2"),
    ).toBeTruthy();

    cache.dispose();
    for (const material of materials.materials) material.dispose();
  });
});

describe("Terrain surfaces", () => {
  it("gives every band a surface from the four families, centred on the band's identity colour", () => {
    expect(Object.keys(TERRAIN_SURFACE)).toEqual(Object.keys(TERRAIN_STYLE));
    const families = Object.values(TERRAIN_SURFACE).map(({ family }) => family);
    expect(new Set(families)).toEqual(
      new Set<SurfaceFamily>(["water", "sand", "meadow", "rock"]),
    );
    expect(TERRAIN_SURFACE.deep_water.family).toBe("water");
    expect(TERRAIN_SURFACE.shore.family).toBe("sand");
    expect(TERRAIN_SURFACE.lowland.family).toBe("meadow");
    expect(TERRAIN_SURFACE.cliff.family).toBe("rock");

    // The procedural range a band varies across has to straddle the flat colour the rest of the UI
    // draws it in, or the shaded world and the minimap disagree about what a band looks like.
    for (const [key, surface] of Object.entries(TERRAIN_SURFACE)) {
      const identity = new Color(TERRAIN_STYLE[key as Terrain].color);
      const middle = new Color(surface.low).lerp(new Color(surface.high), 0.5);
      for (const channel of ["r", "g", "b"] as const)
        expect(Math.abs(middle[channel] - identity[channel])).toBeLessThan(
          0.09,
        );
    }
  });

  it("anchors on chunk includes the shipped three build still emits once", () => {
    const anchors = [
      [ShaderLib.physical.vertexShader, "#include <common>"],
      [ShaderLib.physical.vertexShader, "#include <beginnormal_vertex>"],
      [ShaderLib.physical.vertexShader, "#include <begin_vertex>"],
      [ShaderLib.physical.fragmentShader, "#include <common>"],
      [ShaderLib.physical.fragmentShader, "#include <color_fragment>"],
      [ShaderLib.physical.fragmentShader, "#include <roughnessmap_fragment>"],
      [ShaderLib.physical.fragmentShader, "#include <normal_fragment_maps>"],
      [ShaderLib.physical.fragmentShader, "#include <emissivemap_fragment>"],
      [ShaderLib.physical.fragmentShader, "#include <fog_fragment>"],
    ] as const;
    for (const [source, anchor] of anchors)
      expect(source.split(anchor).length - 1).toBe(1);
  });

  it("injects the surface at every stage the standard material offers", () => {
    const { materials, shader } = compileTerrain("shallow_water");
    expect(shader.vertexShader).toContain("hfNormal = objectNormal;");
    expect(shader.vertexShader).toContain(
      "hfWorld = ( modelMatrix * hfInstanced ).xyz;",
    );
    expect(shader.fragmentShader).toContain("void hfSurface()");
    expect(shader.fragmentShader).toContain("void hfRelief()");
    expect(shader.fragmentShader).toContain("hfRelief();");
    expect(shader.fragmentShader).toContain("diffuseColor.rgb *= hfAlbedo;");
    // The frontier dissolves by colour and stays opaque. Surveyed ground uses the colour captured
    // before camera-depth fog, so altitude cannot masquerade as unexplored terrain; the explicit
    // frontier weight is the only route to the background colour.
    expect(shader.fragmentShader).toContain(
      "vec3 hfSurveyed = gl_FragColor.rgb;\n#include <fog_fragment>",
    );
    expect(shader.fragmentShader).toContain(
      "gl_FragColor.rgb = mix( fogColor, hfSurveyed, clamp( hfFrontier, 0.0, 1.0 ) );",
    );
    expect(shader.fragmentShader).not.toContain(
      "hfHash12( floor( gl_FragCoord",
    );
    expect(shader.fragmentShader).toContain(
      "roughnessFactor = clamp( hfRough, 0.04, 1.0 );",
    );
    expect(shader.fragmentShader).toContain(
      "normal = normalize( normal + mat3( viewMatrix ) * hfBend );",
    );
    expect(shader.fragmentShader).toContain(
      "totalEmissiveRadiance = hfAlbedo * hfFill + hfGlow;",
    );
    expect(uniformValue(shader, "hfWave")).toBe(
      TERRAIN_SURFACE.shallow_water.wave,
    );
    expect(uniformValue(shader, "hfGrain")).toBe(
      TERRAIN_SURFACE.shallow_water.grain,
    );
    for (const material of materials.materials) material.dispose();
  });

  it("gives each band its own program key so seven closures cannot share a shader", () => {
    const materials = createWorldMaterials();
    const keys = Object.values(materials.terrain).map((material) =>
      material.customProgramCacheKey(),
    );
    expect(new Set(keys).size).toBe(keys.length);
    for (const material of materials.materials) material.dispose();
  });

  // The three runtime knobs on `terrainSurfaces`, on one compiled band: the clock the swell rides,
  // the reduced-motion switch that stops it, and the detail budget the quality profile sets.
  it("wraps the clock, holds the swell still under reduced motion, and spends detail by profile", () => {
    const { materials, shader } = compileTerrain("deep_water");
    materials.terrainSurfaces.setTime(3601.5);
    expect(uniformValue(shader, "hfTime")).toBeCloseTo(1.5, 6);
    materials.terrainSurfaces.setMotion(false);
    expect(uniformValue(shader, "hfMotion")).toBe(0);
    materials.terrainSurfaces.setMotion(true);
    expect(uniformValue(shader, "hfMotion")).toBe(1);

    const water = materials.terrain.deep_water;
    materials.terrainSurfaces.setDetail(
      QUALITY_SETTINGS.low.terrainDetail,
      QUALITY_SETTINGS.low.waterDetail,
    );
    expect(water.defines?.HF_OCTAVES).toBe(2);
    expect(water.defines?.HF_WATER_DETAIL).toBe(0);
    materials.terrainSurfaces.setDetail(
      QUALITY_SETTINGS.high.terrainDetail,
      QUALITY_SETTINGS.high.waterDetail,
    );
    expect(water.defines?.HF_OCTAVES).toBe(4);
    expect(water.defines?.HF_DETAIL).toBe(2);
    expect(water.defines?.HF_WATER_DETAIL).toBe(2);
    for (const material of materials.materials) material.dispose();
  });

  it("moves the water and nothing else", () => {
    expect(surfaceBody("water")).toContain("hfTime");
    for (const family of ["sand", "meadow", "rock"] as const)
      expect(surfaceBody(family)).not.toContain("hfTime");
  });

  /**
   * The lattice test. A paved yard used to be one tint per hex with a per-hex luminance jitter on
   * top, and that jitter drew the honeycomb: every hex a slightly different brightness, so a
   * finished yard read as tiles rather than as ground. Two things remove it, and both are pinned
   * here because either one coming back alone brings the grid back with it.
   */
  it("lays paving as continuous ground rather than a hex per tile", () => {
    const snapshot = minimalSnapshot();
    snapshot.ground = [
      { q: 0, r: 0, surface: 1, elevation: 0, paid: [] },
      { q: 1, r: 0, surface: 1, elevation: 0, paid: [] },
      { q: 0, r: 1, surface: 2, elevation: 0, paid: [] },
    ];
    const surfaces: SurfaceDefinition[] = [
      {
        id: 1,
        key: "concrete-slab",
        name: "Concrete slab",
        description: "",
        movement: 100,
        construction_cost: [],
      },
      {
        id: 2,
        key: "brick-pavers",
        name: "Brick pavers",
        description: "",
        movement: 100,
        construction_cost: [],
      },
    ];
    const materials = createWorldMaterials();
    const built = buildTerrainMeshes(snapshot, materials, surfaces);
    const caps = built.group.children.filter(
      (child): child is InstancedMesh =>
        child instanceof InstancedMesh && child.name.startsWith("prepared-"),
    );
    // One draw call per material, not per hex: the two concrete cells share an instanced mesh.
    expect(caps.map(({ name }) => name).sort()).toEqual([
      "prepared-ground-brick-pavers",
      "prepared-ground-concrete-slab",
    ]);
    expect(caps.map(({ count }) => count).sort()).toEqual([1, 2]);
    for (const cap of caps) {
      // No per-instance tint at all. Colour, courses and joints come out of the material, sampled
      // from world space, so a course runs across a hex boundary without knowing one is there.
      expect(cap.instanceColor).toBeNull();
      // Full radius, so neighbouring caps meet edge to edge with no groove of bare ground between
      // them. An inset cap outlines every hex however seamless the pattern on top of it is.
      const geometry = cap.geometry as CylinderGeometry;
      expect(geometry.parameters.radiusTop).toBeCloseTo(HEX_RADIUS, 8);
      expect(geometry.parameters.radiusBottom).toBeCloseTo(HEX_RADIUS, 8);
    }
    expect(caps[0]?.material).not.toBe(caps[1]?.material);
    for (const geometry of built.geometries) geometry.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("samples every paving from world space, gives each its own program, and spends detail by profile", () => {
    const materials = createWorldMaterials();
    const keys = materials.paving
      .all()
      .map((material) => material.customProgramCacheKey());
    expect(new Set(keys).size).toBe(keys.length);
    for (const pattern of [
      "earth",
      "gravel",
      "timber",
      "brick",
      "concrete",
      "asphalt",
    ] as const) {
      const source = pavingSource(pattern);
      expect(source).toContain("hfWorld");
      // A UV or an instance attribute is per-hex by construction, and either one reintroduces the
      // lattice however carefully the rest of the pattern is written.
      expect(source).not.toContain("vUv");
      expect(source).not.toContain("vColor");
    }

    materials.paving.setDetail(QUALITY_SETTINGS.low.terrainDetail);
    for (const material of materials.paving.all()) {
      expect(material.defines?.HF_OCTAVES).toBe(2);
      expect(material.defines?.HF_PAVE_DETAIL).toBe(0);
    }
    materials.paving.setDetail(QUALITY_SETTINGS.high.terrainDetail);
    for (const material of materials.paving.all()) {
      expect(material.defines?.HF_OCTAVES).toBe(4);
      expect(material.defines?.HF_PAVE_DETAIL).toBe(2);
    }
    for (const material of materials.materials) material.dispose();
  });

  it("keeps the laid palette anchored on the one the flat renderers draw", () => {
    // The 3D yard, the minimap and the 2D renderer have to be the same material. `surfaceLook` is
    // still the one place that colour is decided, and the pattern brackets it rather than replacing
    // it, so a palette change lands everywhere at once instead of in two places out of three.
    for (const [key, look] of Object.entries(SURFACE_LOOK)) {
      const style = pavingStyle(key);
      expect(style.roughness).toBeCloseTo(look.roughness, 8);
      const anchor = new Color(look.color);
      const low = new Color(style.low);
      const high = new Color(style.high);
      expect(low.getHSL({ h: 0, s: 0, l: 0 }).l).toBeLessThan(
        anchor.getHSL({ h: 0, s: 0, l: 0 }).l + 0.06,
      );
      expect(high.getHSL({ h: 0, s: 0, l: 0 }).l).toBeGreaterThan(
        anchor.getHSL({ h: 0, s: 0, l: 0 }).l - 0.06,
      );
    }
    // An unrecognised surface still draws as worked earth rather than as nothing.
    expect(pavingStyle("no-such-surface")).toBe(UNKNOWN_PAVING);
    expect(pavingStyle(undefined)).toBe(UNKNOWN_PAVING);
  });
});

describe("picking the drawn landform", () => {
  // Both halves are the same claim from opposite sides: the pick answers for the surface that was
  // drawn, not for the logical plane under it. One raises a cell until the plane picker names its
  // neighbour; the other stands a low cell in front of a rise the plane picker would have preferred.
  it("names the cell whose drawn surface the pointer is over, raised or low", () => {
    const snapshot = minimalSnapshot();
    snapshot.terrain = [cliffTile(0, 0), ...surveyedTiles().slice(1)];
    snapshot.ground = [{ q: 0, r: 0, surface: 0, elevation: 32, paid: [] }];
    const materials = createWorldMaterials();
    const built = buildTerrainMeshes(snapshot, materials);
    const raised = terrainAt(built.cellByKey, 0, 0);
    expect(raised).toBeDefined();
    // Native's generated bed for a cliff band plus the full eight-metre physical earthwork limit,
    // added in native's own unit and converted once.
    expect(raised?.height).toBeCloseTo(35 * HEIGHT_UNIT_HEIGHT, 6);
    expect(built.ceiling).toBeCloseTo(raised?.height ?? 0, 6);

    const camera = new HexSceneCamera();
    camera.resize(1280, 800);
    camera.recenter({ x: 0, y: 0 });
    // The screen point where the top of the rise is actually drawn. That is what the player aims at.
    const screen = camera.projectScene(
      raised?.x ?? 0,
      raised?.height ?? 0,
      raised?.z ?? 0,
    );
    const hit = pickTerrainCell(built, camera.rayAt(screen.x, screen.y));
    expect(hit?.cell.q).toBe(0);
    expect(hit?.cell.r).toBe(0);
    expect(hit?.height).toBeCloseTo(raised?.height ?? 0, 2);
    // The bug this replaces: a column standing at the earthwork ceiling draws more than a hex away
    // from the plane point beneath it, so the old picker handed native the cell in front.
    expect(camera.axialAt(screen.x, screen.y)).not.toEqual({ q: 0, r: 0 });

    const shallow = minimalSnapshot();
    shallow.terrain = [cliffTile(0, 0), ...surveyedTiles().slice(1)];
    shallow.ground = [{ q: 0, r: 0, surface: 0, elevation: 3, paid: [] }];
    const front = buildTerrainMeshes(shallow, materials);
    // `(0, 1)` sits toward the camera at this orbit, so its surface is drawn clear of the cliff.
    const low = terrainAt(front.cellByKey, 0, 1);
    expect(low?.terrain).toBe("lowland");
    const lowScreen = camera.projectScene(
      low?.x ?? 0,
      low?.height ?? 0,
      low?.z ?? 0,
    );
    const lowHit = pickTerrainCell(
      front,
      camera.rayAt(lowScreen.x, lowScreen.y),
    );
    expect(lowHit?.cell.q).toBe(0);
    expect(lowHit?.cell.r).toBe(1);

    for (const geometry of built.geometries) geometry.dispose();
    for (const geometry of front.geometries) geometry.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("draws standing water at the generated depth plus the published departure", () => {
    const snapshot = minimalSnapshot();
    snapshot.terrain = surveyedTiles();
    snapshot.water = [{ q: 0, r: 0, departure: 6 }];
    const materials = createWorldMaterials();
    const built = buildTerrainMeshes(snapshot, materials);
    const dry = terrainAt(built.cellByKey, 1, 0);
    const flooded = terrainAt(built.cellByKey, 0, 0);
    expect(dry?.waterDepth).toBe(0);
    expect(flooded?.waterDepth).toBe(6);
    expect(flooded?.waterHeight).toBeCloseTo(
      (flooded?.height ?? 0) + 6 * HEIGHT_UNIT_HEIGHT,
      6,
    );
    const waterMesh = built.group.getObjectByName("terrain-water") as Mesh;
    const waterMaterials = Array.isArray(waterMesh.material)
      ? waterMesh.material
      : [waterMesh.material];
    expect(waterMaterials).toContain(materials.terrain.deep_water);
    expect(waterMaterials).not.toContain(materials.terrain.lowland);

    snapshot.water = [{ q: 0, r: 0, departure: -6 }];
    const drained = buildTerrainMeshes(snapshot, materials);
    expect(terrainAt(drained.cellByKey, 0, 0)?.waterDepth).toBe(0);

    for (const geometry of built.geometries) geometry.dispose();
    for (const geometry of drained.geometries) geometry.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("agrees with the plane picker on flat ground and meets nothing over fog", () => {
    const snapshot = minimalSnapshot();
    const materials = createWorldMaterials();
    const built = buildTerrainMeshes(snapshot, materials);
    expect(built.cells.length).toBeGreaterThan(0);
    const camera = new HexSceneCamera();
    camera.resize(1280, 800);
    camera.recenter({ x: 0, y: 0 });
    for (const cell of built.cells) {
      const screen = camera.projectScene(cell.x, cell.height, cell.z);
      const hit = pickTerrainCell(built, camera.rayAt(screen.x, screen.y));
      expect({ q: hit?.cell.q, r: hit?.cell.r }).toEqual({
        q: cell.q,
        r: cell.r,
      });
      expect(camera.axialAt(screen.x, screen.y)).toEqual({
        q: cell.q,
        r: cell.r,
      });
    }

    // Far outside the one published chunk: unsurveyed ground has no landform to meet, so the
    // logical plane stays pointable there.
    const fog = camera.projectScene(40, 0, 40);
    expect(pickTerrainCell(built, camera.rayAt(fog.x, fog.y))).toBeNull();

    for (const geometry of built.geometries) geometry.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("still meets the tallest ground the world can raise, seen from its lowest floor", () => {
    const snapshot = minimalSnapshot();
    // The worst case the scale contract allows: a summit at the top of the declared relief with
    // the camera down on the bottom of it. Under an orthographic projection a pick ray starts at
    // the camera's own position plane, so ground standing this far above it is exactly what an
    // unbacked-off ray would begin behind and never meet.
    snapshot.terrain = [
      {
        ...lowlandTile(0, 0),
        terrain: "cliff",
        substrate: "rock",
        height: Math.round(RELIEF_CEILING / HEIGHT_UNIT_HEIGHT),
      },
      ...surveyedTiles().slice(1),
    ];
    const materials = createWorldMaterials();
    const built = buildTerrainMeshes(snapshot, materials);
    const summit = terrainAt(built.cellByKey, 0, 0);
    expect(summit?.height).toBeCloseTo(RELIEF_CEILING, 6);

    const camera = new HexSceneCamera();
    camera.resize(1280, 800);
    camera.recenter({ x: 0, y: 0 }, RELIEF_FLOOR);
    const screen = camera.projectScene(
      summit?.x ?? 0,
      summit?.height ?? 0,
      summit?.z ?? 0,
    );
    const hit = pickTerrainCell(built, camera.rayAt(screen.x, screen.y));
    expect({ q: hit?.cell.q, r: hit?.cell.r }).toEqual({ q: 0, r: 0 });
    // And it is inside the clip planes, so what the player just pointed at is drawn as well as
    // pickable. Orthographic depth is linear, so bracketing the whole relief costs only precision.
    const depth = new Vector3(
      summit?.x ?? 0,
      summit?.height ?? 0,
      summit?.z ?? 0,
    ).project(camera.camera).z;
    expect(Math.abs(depth)).toBeLessThanOrEqual(1);

    for (const geometry of built.geometries) geometry.dispose();
    for (const material of materials.materials) material.dispose();
  });
});

describe("a camera that follows the landform", () => {
  it("carries the whole rig, and the plane a drag is measured against, up to the height it looks at", () => {
    const camera = new HexSceneCamera();
    camera.resize(1280, 800);
    camera.recenter({ x: 0, y: 0 });
    const sealevel = camera.camera.position.y;
    const framed = camera.projectScene(0, 0, 0);

    camera.recenter({ x: 0, y: 0 }, RELIEF_CEILING);
    expect(camera.camera.position.y).toBeCloseTo(sealevel + RELIEF_CEILING, 6);
    // The player on a summit is framed exactly where the player on the plain was. Moving the
    // target alone would have tilted the whole scene and walked them toward the top of the view.
    const climbed = camera.projectScene(0, RELIEF_CEILING, 0);
    expect(climbed.x).toBeCloseTo(framed.x, 6);
    expect(climbed.y).toBeCloseTo(framed.y, 6);

    // The drag plane rises with the target too, so the point under the pointer stays under it.
    // Against a fixed sea-level plane a drag on a hilltop would overshoot.
    expect(camera.groundAt(640, 400).y).toBeCloseTo(RELIEF_CEILING, 6);
  });

  it("fades distance by the screenful rather than by a fixed reach", () => {
    const camera = new HexSceneCamera();
    camera.resize(1280, 800);
    const near = camera.hazeRange;
    camera.zoomAt(640, 400, 2);
    const close = camera.hazeRange;
    // Zoomed in there is less world on screen, so the haze closes in with it. A constant range
    // would have hazed everything at one end of the zoom and nothing at the other.
    expect(close.near).toBeLessThan(near.near);
    expect(close.far).toBeLessThan(near.far);
    expect(close.far).toBeGreaterThan(close.near);
  });
});

describe("one height route", () => {
  it("answers for a hex and for a world point with the same ground", () => {
    const snapshot = minimalSnapshot();
    snapshot.terrain = [cliffTile(0, 0), ...surveyedTiles().slice(1)];
    const materials = createWorldMaterials();
    const built = buildTerrainMeshes(snapshot, materials);

    const cell = terrainAt(built.cellByKey, 0, 0);
    expect(cell?.height).toBeCloseTo(3 * HEIGHT_UNIT_HEIGHT, 6);
    expect(heightAt(built.cellByKey, 0, 0)).toBe(cell?.height);
    expect(heightAtWorld(built.cellByKey, { x: 0, y: 0 })).toBe(cell?.height);
    // Fog is the logical plane: nothing is drawn there, so nothing standing there is lifted off it.
    expect(heightAt(built.cellByKey, 40, 40)).toBe(FOG_HEIGHT);
    expect(
      heightAtWorld(built.cellByKey, {
        x: WORLD_SCALE * 40,
        y: WORLD_SCALE * 40,
      }),
    ).toBe(FOG_HEIGHT);

    for (const geometry of built.geometries) geometry.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("brackets the relief the ground source can actually publish", () => {
    // The renderer's reach is native's own, read off `fixtures/scene-scale.json` — the file Rust
    // asserts against the source production constructs. A camera that bracketed a guess would clip
    // a summit native is entitled to generate.
    expect(RELIEF_FLOOR).toBeLessThan(0);
    expect(RELIEF_CEILING).toBeGreaterThan(0);
    expect(RELIEF_SPAN).toBeCloseTo(RELIEF_CEILING - RELIEF_FLOOR, 12);
  });
});

/** The family's own `hfSurface`, without the shared declarations every family carries. */
function surfaceBody(family: SurfaceFamily): string {
  const source = surfaceSource(family);
  return source.slice(source.indexOf("void hfSurface()"));
}

/** One injected uniform, asserted present rather than read through an optional chain. */
function uniformValue(
  shader: WebGLProgramParametersWithUniforms,
  name: string,
): unknown {
  const uniform = shader.uniforms[name];
  expect(uniform).toBeDefined();
  return uniform?.value;
}

/** Runs a terrain material's injection over the real shipped standard-material source. */
function compileTerrain(terrain: Terrain) {
  const materials = createWorldMaterials();
  const shader = {
    uniforms: {},
    vertexShader: ShaderLib.physical.vertexShader,
    fragmentShader: ShaderLib.physical.fragmentShader,
  } as unknown as WebGLProgramParametersWithUniforms;
  materials.terrain[terrain].onBeforeCompile(
    shader,
    undefined as unknown as WebGLRenderer,
  );
  return { materials, shader };
}

/** One surveyed cell of rock, standing at the height the legacy generator gives a cliff band. */
function cliffTile(q: number, r: number): TerrainSnapshot {
  return {
    ...lowlandTile(q, r),
    terrain: "cliff",
    height: 3,
    substrate: "rock",
  };
}

/** One surveyed cell of flat dry meadow at sea level. */
function lowlandTile(q: number, r: number): TerrainSnapshot {
  return {
    q,
    r,
    x: 0,
    y: 0,
    radius: WORLD_SCALE,
    terrain: "lowland",
    height: 0,
    substrate: "meadow",
    water_depth: 0,
    discharge: 0,
  };
}

/**
 * The cells `minimalSnapshot` publishes: the origin and five of its six neighbours.
 *
 * `(-1, 0)` is deliberately left out even though it falls inside the published chunk rectangle. The
 * terrain group carries every surveyed cell now, so an unpublished centre is unsurveyed ground —
 * there is no rectangle to scan and no omitted row to default to lowland, and the missing cell is
 * what proves it.
 */
const SURVEYED_CELLS: readonly (readonly [number, number])[] = [
  [0, 0],
  [1, 0],
  [0, 1],
  [-1, 1],
  [0, -1],
  [1, -1],
];

function surveyedTiles(): TerrainSnapshot[] {
  return SURVEYED_CELLS.map(([q, r]) => lowlandTile(q, r));
}

/** A drawn cell as `buildTerrainMeshes` produces one, for the layers that consume its map. */
function sceneCell(
  q: number,
  r: number,
  terrain: Terrain,
  x: number,
  z: number,
  height: number,
): TerrainCell {
  return {
    q,
    r,
    terrain,
    x,
    z,
    height,
    elevation: 0,
    surface: 0,
    substrate: "meadow",
    waterDepth: 0,
    waterHeight: height,
    discharge: 0,
  };
}

function minimalSnapshot(): FactorySnapshot {
  return {
    boundaries: [],
    ground: [],
    water: [],
    spoil: 0,
    scenario: "test",
    scenario_name: "Test",
    world_version: 8,
    seed: 1,
    tick: 0,
    checksum: 0,
    belt_transit_ticks: 27,
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
      walk_goal: null,
      walk_path: [],
    },
    researched: [],
    skills: {
      points: 0,
      purchased: [],
      granted: [],
      completed: [],
      sandbox: false,
      availability: [],
    },
    research_availability: [],
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
    terrain: surveyedTiles(),
    habitats: [],
    resources: [],
    buildings: [],
    ground_items: [],
    events: [],
  };
}

function beltDefinition() {
  return {
    id: 2,
    key: "belt",
    name: "Belt",
    kind: "belt" as const,
    description: "Test belt",
    icon: "BLT",
    construction_cost: [],
    placement_rule: "ground" as const,
    buildable: true,
    blocks_movement: false,
    footprint: [{ q: 0, r: 0 }],
  };
}

function entity(
  id: number,
  definitionId: number,
  kind: EntitySnapshot["kind"],
  q: number,
  r: number,
  orientation: number,
  nextId?: number,
): EntitySnapshot {
  return {
    id,
    q,
    r,
    definition_id: definitionId,
    kind,
    orientation,
    scenario_owned: false,
    inventory: [],
    progress: 0,
    progress_total: 0,
    status: "idle",
    next_id: nextId ?? null,
    footprint: [{ q, r }],
  };
}

it("draws sandbox gates by definition, reuses quiet meshes, and releases replaced geometry", () => {
  const layer = new BoundaryMeshes(
    definitions.boundaries as BoundaryDefinition[],
  );
  const terrain = new Map();
  const boundary = {
    q: -2,
    r: 1,
    chord: 0,
    definition_id: 2,
    open: false,
    paid: [],
  };
  const state = [boundary];
  expect(layer.update(state, terrain)).toBe(true);
  const closed = layer.group.children[0] as InstancedMesh;
  expect(closed.count).toBe(5);
  expect(layer.update(state, terrain)).toBe(false);
  expect(layer.group.children[0]).toBe(closed);
  let disposed = false;
  closed.addEventListener("dispose", () => {
    disposed = true;
  });
  layer.update([{ ...boundary, open: true }], terrain);
  expect(disposed).toBe(true);
  const open = layer.group.children[0] as InstancedMesh;
  const closedMatrix = new Matrix4();
  const openMatrix = new Matrix4();
  closed.getMatrixAt(2, closedMatrix);
  open.getMatrixAt(2, openMatrix);
  expect(openMatrix.equals(closedMatrix)).toBe(false);
  layer.setPreview({
    segments: [boundary],
    changes: 1,
    cost: [],
    refund: [],
    error: null,
  });
  expect(layer.group.children).toHaveLength(2);
  layer.setPreview(null);
  expect(layer.group.children).toHaveLength(1);
  // The pins are their own mesh: the first click of a two-vertex selection has no run to preview
  // yet, and the player still has to see where it landed.
  layer.setAnchors([{ q: -2, r: 1, corner: 1 }]);
  expect(layer.group.children).toHaveLength(2);
  layer.setAnchors([]);
  expect(layer.group.children).toHaveLength(1);
  layer.dispose();
  expect(layer.group.children).toHaveLength(0);
});

it("measures every rail off the chord it spans, not off a hex edge", () => {
  const layer = new BoundaryMeshes(
    definitions.boundaries as BoundaryDefinition[],
  );
  const base = { q: 0, r: 0, definition_id: 1, open: false, paid: [] };
  // Chord 0 is a hex edge, chord 12 a long diagonal across the whole hex. A layer that assumed a
  // unit edge would draw the diagonal at edge length and leave it hanging in the air short of its
  // own post, so the two are compared by the width the instance is actually scaled to.
  const widthOf = (chord: number): number => {
    const meshes = new BoundaryMeshes(
      definitions.boundaries as BoundaryDefinition[],
    );
    meshes.update([{ ...base, chord }], new Map());
    const matrix = new Matrix4();
    (meshes.group.children[0] as InstancedMesh).getMatrixAt(2, matrix);
    return new Vector3().setFromMatrixScale(matrix).x;
  };
  expect(widthOf(12)).toBeGreaterThan(widthOf(0) * 1.9);
  layer.dispose();
});
