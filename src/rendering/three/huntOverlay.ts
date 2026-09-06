import { BufferGeometry, Group, Line, LineBasicMaterial, Vector3 } from "three";
import type { HuntPreview } from "../../core/types";
import { WORLD_SCALE } from "../landmarks";
import { heightAtWorld, type TerrainCell } from "./terrainMeshes";

/** Native reach and target, draped over the drawn ground. No host legality test. */
export class HuntOverlay {
  readonly group = new Group();
  private readonly material = new LineBasicMaterial({
    color: "#fff0ac",
    depthTest: false,
  });
  private readonly range = new Line(new BufferGeometry(), this.material);
  private readonly target = new Line(new BufferGeometry(), this.material);
  private preview: HuntPreview | null = null;
  private workPoint: [number, number] | null = null;

  constructor() {
    this.group.name = "hunting-reach-and-herd";
    this.group.add(this.range, this.target);
    this.group.visible = false;
    this.range.renderOrder = this.target.renderOrder = 20;
  }

  set(preview: HuntPreview | null): void {
    this.preview = preview;
    this.group.visible = preview !== null || this.workPoint !== null;
    this.range.visible = this.workPoint === null;
    this.material.color.set(
      preview?.ready || preview?.remaining_ticks ? "#fff0ac" : "#ef9c78",
    );
  }

  setWork(point: [number, number] | null): void {
    this.workPoint = point;
    this.group.visible = point !== null || this.preview !== null;
    this.range.visible = point === null;
    this.material.color.set(point ? "#bde18c" : "#fff0ac");
  }

  update(
    terrain: ReadonlyMap<string, TerrainCell>,
    point?: [number, number],
  ): void {
    if (this.workPoint) {
      this.ring(this.target, this.workPoint, 900, terrain);
      return;
    }
    const p = this.preview;
    if (!p) return;
    this.ring(this.range, p.player, p.reach, terrain);
    this.ring(this.target, point ?? p.point, 650, terrain);
  }

  private ring(
    line: Line,
    centre: [number, number],
    radius: number,
    terrain: ReadonlyMap<string, TerrainCell>,
  ): void {
    const points: Vector3[] = [];
    for (let i = 0; i <= 64; i++) {
      const angle = (i * Math.PI) / 32;
      const x = centre[0] + Math.cos(angle) * radius;
      const y = centre[1] + Math.sin(angle) * radius;
      points.push(
        new Vector3(
          x / WORLD_SCALE,
          heightAtWorld(terrain, { x, y }) + 0.035,
          y / WORLD_SCALE,
        ),
      );
    }
    line.geometry.setFromPoints(points);
    line.geometry.computeBoundingSphere();
  }

  get herdId(): number | undefined {
    return this.preview?.herd_id;
  }

  dispose(): void {
    this.range.geometry.dispose();
    this.target.geometry.dispose();
    this.material.dispose();
  }
}
