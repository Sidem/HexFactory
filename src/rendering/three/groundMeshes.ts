import {
  Color,
  CylinderGeometry,
  Group,
  InstancedMesh,
  Matrix4,
  MeshBasicMaterial,
  Quaternion,
  Vector3,
} from "three";
import { axialToPixel } from "@hexlife/embed/hex";

import type { GroundPreview, GroundPreviewCell } from "../../core/types";
import { GRADE_STEP_HEIGHT } from "../surfaceLook";
import { HEX_RADIUS, cellKey, type TerrainCell } from "./terrainMeshes";

/** Refused, and the whole selection says so at once. */
const BLOCKED = "#ff7a70";
/** A deposit about to be sealed. The one change in here that is not free to walk back. */
const SEALING = "#f0b45a";
/** Ground coming up. */
const CUT = "#7fc9ff";
/** Ground going down. */
const FILL = "#ffd479";
/** A surface being laid over ground that stays where it is. */
const PAVING = "#79e7c0";

/**
 * The ghost of a pending earthworks selection: one lifted disc per hex, drawn at the height the
 * finished grade would sit at, coloured by what is about to happen to it.
 *
 * Cut and fill get their own colours rather than one "selected" colour, because the whole skill of
 * grading is seeing where the spoil comes from and where it goes before committing to it.
 */
export class GroundMeshes {
  readonly group = new Group();
  private readonly geometry = new CylinderGeometry(
    HEX_RADIUS * 0.92,
    HEX_RADIUS * 0.92,
    1,
    6,
    1,
    false,
  );
  private readonly material = new MeshBasicMaterial({
    color: 0xffffff,
    vertexColors: true,
    transparent: true,
    opacity: 0.5,
    depthWrite: false,
  });
  private ghost: InstancedMesh | null = null;
  private preview: GroundPreview | null = null;
  private terrain: ReadonlyMap<string, TerrainCell> | null = null;

  setTerrain(terrain: ReadonlyMap<string, TerrainCell>): void {
    this.terrain = terrain;
    this.setPreview(this.preview);
  }

  setPreview(preview: GroundPreview | null): void {
    this.preview = preview;
    this.drop();
    if (!preview?.cells.length) return;
    this.ghost = new InstancedMesh(
      this.geometry,
      this.material,
      preview.cells.length,
    );
    this.ghost.renderOrder = 24;
    const matrix = new Matrix4();
    const quaternion = new Quaternion();
    const scale = new Vector3(1, 0.07, 1);
    const position = new Vector3();
    const tint = new Color();
    preview.cells.forEach((cell, index) => {
      const centre = axialToPixel(cell, 1, { x: 0, y: 0 });
      const base =
        this.terrain?.get(cellKey(cell.q, cell.r))?.height ??
        cell.elevation * GRADE_STEP_HEIGHT;
      position.set(
        centre.x,
        base + cell.change * GRADE_STEP_HEIGHT + 0.09,
        centre.y,
      );
      matrix.compose(position, quaternion, scale);
      this.ghost!.setMatrixAt(index, matrix);
      this.ghost!.setColorAt(index, tint.set(colorFor(preview, cell)));
    });
    this.ghost.instanceMatrix.needsUpdate = true;
    if (this.ghost.instanceColor) this.ghost.instanceColor.needsUpdate = true;
    this.ghost.computeBoundingSphere();
    this.group.add(this.ghost);
  }

  private drop(): void {
    if (!this.ghost) return;
    this.group.remove(this.ghost);
    this.ghost.dispose();
    this.ghost = null;
  }

  dispose(): void {
    this.drop();
    this.geometry.dispose();
    this.material.dispose();
  }
}

function colorFor(preview: GroundPreview, cell: GroundPreviewCell): string {
  if (preview.error) return BLOCKED;
  if (cell.covers) return SEALING;
  if (cell.retained) return SEALING;
  if (cell.change > 0) return FILL;
  if (cell.change < 0) return CUT;
  return PAVING;
}
