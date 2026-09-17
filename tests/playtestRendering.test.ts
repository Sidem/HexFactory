import { describe, expect, it, vi } from "vitest";
import {
  InstancedMesh,
  BoxGeometry,
  MeshBasicMaterial,
  Color,
  Matrix4,
  OrthographicCamera,
  Ray,
  Vector3,
} from "three";
import { FrameVisibility } from "../src/rendering/three/frameVisibility";
import { PopulationMeshes } from "../src/rendering/three/populationMeshes";
import { GroundMeshes } from "../src/rendering/three/groundMeshes";
import { HEIGHT_UNIT_HEIGHT } from "../src/rendering/sceneScale";
import { GroundTool } from "../src/ui/ground";
import { updateAnimatedParts } from "../src/rendering/three/animatedParts";
import {
  machinePartMatrix,
  type MachinePartInstance,
} from "../src/rendering/three/machineMeshes";
import type {
  FactorySnapshot,
  GroundPreview,
  HerdSnapshot,
} from "../src/core/types";

describe("playtest rendering and brush feedback", () => {
  it("uploads only visible animation ranges and catches up when a part returns to view", () => {
    const mesh = new InstancedMesh(
      new BoxGeometry(),
      new MeshBasicMaterial(),
      2,
    );
    const instances = [1, 2].map((id) => ({
      building: {
        id,
        definition_id: 14,
        kind: "generator",
        q: 0,
        r: 0,
        orientation: 0,
        scenario_owned: false,
        inventory: [],
        progress: 1,
        progress_total: 10,
        status: "composing",
        footprint: [{ q: 0, r: 0 }],
      },
      part: {
        part: "rotor",
        x: 0,
        y: -0.56,
        scale: 0.38,
        count: 3,
        phase: "spin",
        upright: true,
      },
      key: "rotor:3",
      animated: true,
      color: "#fff",
      glow: null,
      material: "ceramic",
      groundHeight: 0,
      footprintScale: 1,
      visualScale: 1,
      baseLift: 0,
      x: id * 2,
      z: 0,
    })) as MachinePartInstance[];
    const bucket = [{ mesh, instances, animated: true }];
    const matrix = new Matrix4(),
      color = new Color();
    const before = Array.from(mesh.instanceMatrix.array).slice(16);
    updateAnimatedParts(
      bucket,
      new Set([1]),
      250,
      false,
      new Map(),
      matrix,
      color,
    );
    expect(mesh.instanceMatrix.updateRanges).toEqual([{ start: 0, count: 16 }]);
    expect(Array.from(mesh.instanceMatrix.array).slice(16)).toEqual(before);
    const version = mesh.instanceMatrix.version;
    updateAnimatedParts(
      bucket,
      new Set(),
      500,
      false,
      new Map(),
      matrix,
      color,
    );
    expect(mesh.instanceMatrix.version).toBe(version);
    updateAnimatedParts(
      bucket,
      new Set([2]),
      750,
      false,
      new Map(),
      matrix,
      color,
    );
    mesh.getMatrixAt(1, matrix);
    const expected = machinePartMatrix(instances[1]!, 750, false);
    matrix.elements.forEach((value, index) =>
      expect(value).toBeCloseTo(expected.elements[index]!, 5),
    );
    expect(mesh.instanceMatrix.updateRanges).toEqual([
      { start: 16, count: 16 },
    ]);
    mesh.dispose();
    mesh.geometry.dispose();
    (mesh.material as MeshBasicMaterial).dispose();
  });
  it("culls distant herds, keeps pick IDs aligned, and restores them when the camera moves", () => {
    const camera = new OrthographicCamera(-5, 5, 5, -5, 0.1, 100);
    camera.position.set(0, 20, 0);
    camera.up.set(0, 0, -1);
    camera.lookAt(0, 0, 0);
    const view = new FrameVisibility();
    view.update(camera);
    expect(view.includes(5.5, 0, 0, 1)).toBe(true);
    expect(view.includes(50, 0, 0, 1)).toBe(false);
    const herds = [
      {
        id: 1,
        count: 2,
        from: [0, 0],
        to: [0, 0],
        left_tick: 0,
        arrive_tick: 0,
      },
      {
        id: 2,
        count: 3,
        from: [100000, 0],
        to: [100000, 0],
        left_tick: 0,
        arrive_tick: 0,
      },
    ] as HerdSnapshot[];
    const layer = new PopulationMeshes();
    layer.update(herds, 0, new Map(), 0);
    layer.animate(0, view);
    const mesh = layer.group.getObjectByName("grazing-herds") as InstancedMesh;
    expect(mesh.count).toBe(2);
    const matrix = new Matrix4();
    mesh.getMatrixAt(0, matrix);
    const origin = new Vector3().setFromMatrixPosition(matrix);
    origin.y = 10;
    expect(layer.pick(new Ray(origin, new Vector3(0, -1, 0)))).toBe(1);
    camera.position.x = 100;
    camera.lookAt(100, 0, 0);
    view.update(camera);
    layer.animate(16, view);
    expect(mesh.count).toBe(3);
    mesh.getMatrixAt(0, matrix);
    origin.setFromMatrixPosition(matrix);
    origin.y = 10;
    expect(layer.pick(new Ray(origin, new Vector3(0, -1, 0)))).toBe(2);
    layer.update(herds, 1, new Map(), 100);
    expect(layer.group.getObjectByName("grazing-herds")).toBe(mesh);
    camera.position.x = 200;
    camera.lookAt(200, 0, 0);
    view.update(camera);
    const version = mesh.instanceMatrix.version;
    layer.animate(200, view);
    expect(mesh.count).toBe(0);
    expect(mesh.instanceMatrix.version).toBe(version);
    layer.dispose();
  });

  it("shows both current and target ground heights for a cut", () => {
    const layer = new GroundMeshes();
    layer.setPreview({
      cells: [
        {
          q: 0,
          r: 0,
          change: -2,
          elevation: 0,
          surface: 0,
          retained: false,
          covers: false,
          blocked: null,
        },
      ],
      error: null,
    } as GroundPreview);
    const current = layer.group.getObjectByName(
      "ground-current-height",
    ) as InstancedMesh;
    const ghost = layer.group.children[0] as InstancedMesh;
    const before = new Matrix4(),
      after = new Matrix4();
    current.getMatrixAt(0, before);
    ghost.getMatrixAt(0, after);
    expect(before.elements[13]! - after.elements[13]!).toBeCloseTo(
      2 * HEIGHT_UNIT_HEIGHT,
      6,
    );
    layer.setPreview(null);
    expect(layer.group.children).toHaveLength(0);
    layer.dispose();
  });

  it("lets a rejected earthwork stroke be retried without reopening the tool", () => {
    const enqueue = vi.fn(() => true);
    const tool = Object.assign(Object.create(GroundTool.prototype), {
      opened: true,
      mode: "mound",
      radius: 0,
      depth: 1,
      surface: 0,
      cover: false,
      brush: null,
      workQueued: false,
      lastMessage: "",
      inventorySignature: "",
      refreshPreview: vi.fn(),
      drawSpoil: vi.fn(),
      buildPalette: vi.fn(),
      enqueue,
      status: {
        textContent: "",
        classList: { add: vi.fn(), remove: vi.fn(), toggle: vi.fn() },
      },
    }) as GroundTool;
    tool.beginBrush(1, { q: 0, r: 0 });
    tool.endBrush(1);
    tool.update({
      player: { action_cooldown: 0, inventory: {}, creative: false },
      events: ["Not enough spoil"],
      researched: [],
      spoil: 0,
    } as unknown as FactorySnapshot);
    tool.beginBrush(2, { q: 1, r: 0 });
    expect(enqueue).toHaveBeenCalledTimes(2);
  });
});
