import {
  BoxGeometry,
  type BufferGeometry,
  Group,
  InstancedMesh,
  Matrix4,
  MeshStandardMaterial,
  Raycaster,
  type Ray,
  Quaternion,
  SphereGeometry,
  Vector3,
} from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";
import type { HerdSnapshot, HuntPreview } from "../../core/types";
import { WORLD_SCALE } from "../landmarks";
import { heightAtWorld, type TerrainCell } from "./terrainMeshes";
import { HuntOverlay } from "./huntOverlay";

/** Render a native leg at frame time; no host animal decisions or quantities. */
export function herdPosition(
  herd: HerdSnapshot,
  tick: number,
): [number, number] {
  const duration = herd.arrive_tick - herd.left_tick;
  const t =
    duration <= 0
      ? 1
      : Math.max(0, Math.min(1, (tick - herd.left_tick) / duration));
  return [
    herd.from[0] + (herd.to[0] - herd.from[0]) * t,
    herd.from[1] + (herd.to[1] - herd.from[1]) * t,
  ];
}

export class PopulationMeshes {
  readonly group = new Group();
  private readonly huntOverlay = new HuntOverlay();
  private readonly pastureOverlay = new HuntOverlay();
  private readonly material = new MeshStandardMaterial({
    color: "#c1ad7e",
    roughness: 1,
    flatShading: true,
  });
  private readonly geometry = grazerGeometry();
  private mesh: InstancedMesh | undefined;
  private readonly raycaster = new Raycaster();
  private readonly bodyHerdIds: number[] = [];
  private identity: readonly HerdSnapshot[] = [];
  private terrain: ReadonlyMap<string, TerrainCell> = new Map();
  private tick = 0;
  private tickAt = 0;
  private tickMs = 100;
  private readonly matrix = new Matrix4();
  private readonly position = new Vector3();
  private readonly rotation = new Quaternion();
  private readonly scale = new Vector3(1, 1, 1);
  private readonly up = new Vector3(0, 1, 0);

  constructor() {
    this.group.add(this.huntOverlay.group, this.pastureOverlay.group);
  }

  setPastureWork(point: [number, number] | null): void {
    this.pastureOverlay.setWork(point);
    this.pastureOverlay.update(this.terrain);
  }

  setHuntPreview(preview: HuntPreview | null): void {
    this.huntOverlay.set(preview);
    this.huntOverlay.update(this.terrain);
  }

  update(
    herds: readonly HerdSnapshot[],
    tick: number,
    terrain: ReadonlyMap<string, TerrainCell>,
    now = performance.now(),
  ): boolean {
    if (tick !== this.tick) {
      const elapsed = (now - this.tickAt) / (tick - this.tick);
      if (tick > this.tick && elapsed >= 16 && elapsed <= 4000)
        this.tickMs = elapsed;
      this.tickAt = now;
      this.tick = tick;
    }
    const changed = herds !== this.identity || terrain !== this.terrain;
    this.identity = herds;
    this.terrain = terrain;
    const count = herds.reduce((sum, h) => sum + grazerCount(h.count), 0);
    if (!this.mesh || this.mesh.count !== count) {
      if (this.mesh) {
        this.group.remove(this.mesh);
        this.mesh.dispose();
      }
      this.mesh = new InstancedMesh(this.geometry, this.material, count);
      this.mesh.name = "grazing-herds";
      this.mesh.castShadow = true;
      this.mesh.frustumCulled = false;
      this.group.add(this.mesh);
    }
    this.animate(now);
    return changed;
  }

  animate(now: number): void {
    if (!this.mesh) return;
    const tick =
      this.tick + Math.max(0, Math.min(1, (now - this.tickAt) / this.tickMs));
    let index = 0;
    this.bodyHerdIds.length = 0;
    for (const herd of this.identity) {
      const [x, y] = herdPosition(herd, tick);
      if (herd.id === this.huntOverlay.herdId)
        this.huntOverlay.update(this.terrain, [x, y]);
      const moving = herd.from[0] !== herd.to[0] || herd.from[1] !== herd.to[1];
      const heading = moving
        ? -Math.atan2(herd.to[1] - herd.from[1], herd.to[0] - herd.from[0])
        : herd.id * 2.4;
      for (let animal = 0; animal < herd.count; animal++) {
        const angle = herd.id * 0.73 + animal * 2.4;
        const px = x + Math.cos(angle) * 300,
          py = y + Math.sin(angle) * 300;
        this.position.set(
          px / WORLD_SCALE,
          heightAtWorld(this.terrain, { x: px, y: py }) +
            0.02 +
            (moving ? Math.abs(Math.sin(tick * 1.2 + animal)) * 0.018 : 0),
          py / WORLD_SCALE,
        );
        this.rotation.setFromAxisAngle(this.up, heading);
        this.matrix.compose(this.position, this.rotation, this.scale);
        this.bodyHerdIds.push(herd.id);
        this.mesh.setMatrixAt(index++, this.matrix);
      }
    }
    this.mesh.instanceMatrix.needsUpdate = true;
  }

  pick(ray: Ray): number | null {
    if (!this.mesh) return null;
    this.raycaster.ray.copy(ray);
    this.mesh.updateMatrixWorld();
    this.mesh.computeBoundingSphere();
    const hit = this.raycaster.intersectObject(this.mesh)[0];
    return hit?.instanceId === undefined
      ? null
      : (this.bodyHerdIds[hit.instanceId] ?? null);
  }

  dispose(): void {
    this.huntOverlay.dispose();
    this.pastureOverlay.dispose();
    this.mesh?.dispose();
    this.geometry.dispose();
    this.material.dispose();
  }
}

export function grazerCount(population: number): number {
  return Math.max(0, Math.floor(population));
}

function grazerGeometry() {
  const pieces: BufferGeometry[] = [
    new SphereGeometry(1, 8, 6)
      .scale(0.13, 0.075, 0.065)
      .translate(0, 0.115, 0),
    new SphereGeometry(1, 6, 4)
      .scale(0.065, 0.05, 0.048)
      .translate(0.115, 0.13, 0),
  ];
  for (const x of [-0.07, 0.07])
    for (const z of [-0.035, 0.035])
      pieces.push(new BoxGeometry(0.024, 0.08, 0.024).translate(x, 0.04, z));
  for (const z of [-0.03, 0.03])
    pieces.push(new BoxGeometry(0.025, 0.055, 0.016).translate(0.09, 0.19, z));
  const geometry = mergeGeometries(pieces, false)!;
  for (const piece of pieces) piece.dispose();
  return geometry.scale(GRAZER_SCALE, GRAZER_SCALE, GRAZER_SCALE);
}

/** What the silhouette above is multiplied by to stand 1.2 m at the shoulder. */
const GRAZER_SCALE = 1.8;
