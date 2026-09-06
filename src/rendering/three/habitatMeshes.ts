import {
  Color,
  ConeGeometry,
  Group,
  InstancedMesh,
  Matrix4,
  MeshStandardMaterial,
  Quaternion,
  Vector3,
} from "three";

import type { HabitatSnapshot } from "../../core/types";
import { WORLD_SCALE } from "../landmarks";
import type { TerrainCell } from "./terrainMeshes";
import { heightAt } from "./terrainMeshes";

/**
 * Generated riverbank cover: narrow upright sedge blades arranged as a continuous fringe.
 * Deposits use rocks, mounds, crystals, or trees; this repeated three-sided blade silhouette stays
 * recognizable without colour and costs one instanced draw call.
 */
export class HabitatMeshes {
  readonly group = new Group();
  private readonly geometry = new ConeGeometry(0.045, 0.48, 3);
  private readonly material = new MeshStandardMaterial({
    color: "#ffffff",
    roughness: 1,
    flatShading: true,
  });
  private mesh: InstancedMesh | null = null;
  private identity: readonly HabitatSnapshot[] | null = null;
  private terrainIdentity: ReadonlyMap<string, TerrainCell> | null = null;
  private readonly slots = new Map<string, number>();
  private readonly rows = new Map<string, HabitatSnapshot>();
  private readonly free: number[] = [];
  private capacity = 0;
  private nextSlot = 0;
  private readonly matrix = new Matrix4();
  private readonly position = new Vector3();
  private readonly rotation = new Quaternion();
  private readonly scale = new Vector3();
  private readonly up = new Vector3(0, 1, 0);
  private readonly colour = new Color();

  constructor() {
    this.group.name = "fertile-riverbank-habitat";
  }

  update(
    habitats: readonly HabitatSnapshot[],
    terrain: ReadonlyMap<string, TerrainCell>,
  ): boolean {
    if (habitats === this.identity && terrain === this.terrainIdentity)
      return false;
    const terrainChanged = terrain !== this.terrainIdentity;
    const keys = new Set(habitats.map((h) => `${h.q},${h.r}`));
    for (const [key, slot] of this.slots) {
      if (keys.has(key)) continue;
      this.slots.delete(key);
      this.rows.delete(key);
      this.free.push(slot);
      for (let i = 0; i < 8; i++)
        this.mesh?.setMatrixAt(slot * 8 + i, this.matrix.makeScale(0, 0, 0));
    }
    const changed: HabitatSnapshot[] = [];
    for (const row of habitats) {
      const key = `${row.q},${row.r}`;
      if (!this.slots.has(key))
        this.slots.set(key, this.free.pop() ?? this.nextSlot++);
      if (terrainChanged || this.rows.get(key) !== row) changed.push(row);
      this.rows.set(key, row);
    }
    const grow = !this.mesh || this.nextSlot > this.capacity;
    if (grow) {
      if (this.mesh) {
        this.group.remove(this.mesh);
        this.mesh.dispose();
      }
      this.capacity = Math.max(
        16,
        2 ** Math.ceil(Math.log2(Math.max(1, this.nextSlot))),
      );
      this.mesh = new InstancedMesh(
        this.geometry,
        this.material,
        this.capacity * 8,
      );
      this.mesh.name = "fertile-riverbank-sedge";
      this.mesh.frustumCulled = false;
      for (let i = 0; i < this.capacity * 8; i++)
        this.mesh.setMatrixAt(i, this.matrix.makeScale(0, 0, 0));
      this.group.add(this.mesh);
    }
    this.identity = habitats;
    this.terrainIdentity = terrain;
    for (const habitat of grow ? habitats : changed) {
      const slot = this.slots.get(`${habitat.q},${habitat.r}`)!;
      const cover =
        habitat.grass_limit > 0 ? habitat.grass / habitat.grass_limit : 0;
      const count =
        habitat.grass_limit > 0
          ? Math.ceil(cover * 8)
          : tuftCount(habitat.capacity);
      this.colour.set(
        habitat.fouling > 0 ? "#766342" : cover < 0.4 ? "#9a9857" : "#82a34e",
      );
      for (let tuft = 0; tuft < 8; tuft++) {
        const index = slot * 8 + tuft;
        if (tuft >= count) {
          this.mesh!.setMatrixAt(index, this.matrix.makeScale(0, 0, 0));
          continue;
        }
        const angle = variation(habitat.q, habitat.r, tuft) * Math.PI * 2;
        const distance = 0.25 + 0.08 * (tuft % 5);
        const height =
          habitat.grass_limit > 0
            ? 0.14 + cover * 0.35
            : 0.84 + 0.08 * (tuft % 3);
        this.position.set(
          habitat.x / WORLD_SCALE + Math.cos(angle) * distance,
          heightAt(terrain, habitat.q, habitat.r) + 0.24 * height + 0.018,
          habitat.y / WORLD_SCALE + Math.sin(angle) * distance,
        );
        this.rotation.setFromAxisAngle(this.up, angle);
        this.scale.set(1, height, 1);
        this.mesh!.setMatrixAt(
          index,
          this.matrix.compose(this.position, this.rotation, this.scale),
        );
        this.mesh!.setColorAt(index, this.colour);
      }
    }
    this.mesh!.instanceMatrix.needsUpdate = true;
    if (this.mesh!.instanceColor) this.mesh!.instanceColor.needsUpdate = true;
    return true;
  }

  dispose(): void {
    this.mesh?.dispose();
    this.geometry.dispose();
    this.material.dispose();
  }
}

export function tuftCount(capacity: number): number {
  if (capacity <= 0) return 0;
  return Math.max(2, Math.min(4, Math.ceil(capacity / 50)));
}

function variation(q: number, r: number, index: number): number {
  let value = Math.imul(q + index * 17, 0x45d9f3b);
  value ^= Math.imul(r - index * 29, 0x119de1f3);
  value ^= value >>> 16;
  return (value >>> 0) / 0x1_0000_0000;
}
