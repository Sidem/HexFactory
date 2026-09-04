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
    color: new Color("#a7bd63"),
    roughness: 0.9,
    metalness: 0,
    flatShading: true,
  });
  private mesh: InstancedMesh | null = null;
  private identity: readonly HabitatSnapshot[] | null = null;
  private terrainIdentity: ReadonlyMap<string, TerrainCell> | null = null;

  constructor() {
    this.group.name = "fertile-riverbank-habitat";
  }

  update(
    habitats: readonly HabitatSnapshot[],
    terrain: ReadonlyMap<string, TerrainCell>,
  ): boolean {
    if (habitats === this.identity && terrain === this.terrainIdentity)
      return false;
    this.identity = habitats;
    this.terrainIdentity = terrain;
    if (this.mesh) this.group.remove(this.mesh);

    const count = habitats.reduce(
      (sum, habitat) => sum + tuftCount(habitat.capacity),
      0,
    );
    const mesh = new InstancedMesh(this.geometry, this.material, count);
    mesh.name = "fertile-riverbank-sedge";
    mesh.castShadow = true;
    const matrix = new Matrix4();
    const position = new Vector3();
    const rotation = new Quaternion();
    const scale = new Vector3();
    let index = 0;
    for (const habitat of habitats) {
      const tufts = tuftCount(habitat.capacity);
      for (let tuft = 0; tuft < tufts; tuft += 1) {
        const angle = variation(habitat.q, habitat.r, tuft) * Math.PI * 2;
        const distance = 0.25 + 0.12 * (tuft % 2);
        const height = 0.84 + 0.08 * (tuft % 3);
        position.set(
          habitat.x / WORLD_SCALE + Math.cos(angle) * distance,
          heightAt(terrain, habitat.q, habitat.r) + (0.48 * height) / 2 + 0.018,
          habitat.y / WORLD_SCALE + Math.sin(angle) * distance,
        );
        rotation.setFromAxisAngle(new Vector3(0, 1, 0), angle);
        scale.set(1, height, 1);
        matrix.compose(position, rotation, scale);
        mesh.setMatrixAt(index, matrix);
        index += 1;
      }
    }
    mesh.instanceMatrix.needsUpdate = true;
    this.mesh = mesh;
    this.group.add(mesh);
    return true;
  }

  dispose(): void {
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
