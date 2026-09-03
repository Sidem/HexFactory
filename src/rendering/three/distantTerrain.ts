import {
  BufferGeometry,
  Color,
  Float32BufferAttribute,
  Group,
  Mesh,
  MeshBasicMaterial,
} from "three";

import type { LandscapeLod } from "../../core/types";
import { WORLD_SCALE } from "../landmarks";
import { HEIGHT_UNIT_HEIGHT } from "../sceneScale";
import { TERRAIN_STYLE } from "./terrainStyle";

const SKY = new Color("#142129");
/**
 * The coarse mesh is a close-view horizon aid, not a strategic map layer. Below the natural 1:1
 * view its broad triangles are large enough to read as surveyed terrain beyond the frontier.
 */
const HORIZON_MIN_ZOOM = 1;

/** Native's coarse generated landform, drawn only at the horizon and never used for picking. */
export class DistantTerrain {
  readonly group = new Group();
  private readonly material = new MeshBasicMaterial({
    vertexColors: true,
    // The ordinary camera fog ends with the detailed screenful. A mountain may project into that
    // screen from much farther away, so this mesh carries its own axial fade instead.
    fog: false,
  });
  private mesh: Mesh | null = null;

  constructor() {
    this.group.name = "distant-terrain-lod";
  }

  setZoom(zoom: number): void {
    this.group.visible = zoom >= HORIZON_MIN_ZOOM;
  }

  set(lod: LandscapeLod): void {
    this.clear();
    const byKey = new Map(
      lod.cells.map((cell) => [`${cell.q},${cell.r}`, cell]),
    );
    const positions: number[] = [];
    const colors: number[] = [];
    const append = (cell: LandscapeLod["cells"][number]): void => {
      positions.push(
        cell.x / WORLD_SCALE,
        cell.height * HEIGHT_UNIT_HEIGHT - 0.015,
        cell.y / WORLD_SCALE,
      );
      const dq = (cell.q - lod.anchor_q) / lod.step;
      const dr = (cell.r - lod.anchor_r) / lod.step;
      const distance = Math.max(Math.abs(dq), Math.abs(dr), Math.abs(dq + dr));
      const fade = Math.min(1, Math.max(0, (distance - 3) / 13));
      const colour = new Color(TERRAIN_STYLE[cell.terrain].color).lerp(
        SKY,
        0.35 + fade * 0.65,
      );
      colors.push(colour.r, colour.g, colour.b);
    };
    for (const cell of lod.cells) {
      const east = byKey.get(`${cell.q + lod.step},${cell.r}`);
      const southEast = byKey.get(`${cell.q},${cell.r + lod.step}`);
      const diagonal = byKey.get(`${cell.q + lod.step},${cell.r + lod.step}`);
      if (east && southEast) {
        append(cell);
        append(southEast);
        append(east);
      }
      if (east && southEast && diagonal) {
        append(east);
        append(southEast);
        append(diagonal);
      }
    }
    const geometry = new BufferGeometry();
    geometry.setAttribute("position", new Float32BufferAttribute(positions, 3));
    geometry.setAttribute("color", new Float32BufferAttribute(colors, 3));
    geometry.computeBoundingSphere();
    this.mesh = new Mesh(geometry, this.material);
    this.mesh.name = "distant-terrain-surface";
    this.mesh.renderOrder = -20;
    this.group.add(this.mesh);
  }

  dispose(): void {
    this.clear();
    this.material.dispose();
  }

  private clear(): void {
    if (!this.mesh) return;
    this.group.remove(this.mesh);
    this.mesh.geometry.dispose();
    this.mesh = null;
  }
}
