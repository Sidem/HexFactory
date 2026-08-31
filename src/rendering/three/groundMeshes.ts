import {
  Color,
  CylinderGeometry,
  Group,
  InstancedMesh,
  Matrix4,
  MeshBasicMaterial,
  Quaternion,
  RingGeometry,
  Vector3,
} from "three";
import { axialToPixel } from "@hexlife/embed/hex";

import type { GroundPreview, GroundPreviewCell } from "../../core/types";
import { GRADE_STEP_HEIGHT } from "../surfaceLook";
import { HEX_RING_START } from "./overlays";
import { HEX_RADIUS, cellKey, type TerrainCell } from "./terrainMeshes";

/** Nothing in this selection will happen: the edit as a whole was refused. A wash, not an alarm. */
const REFUSED = "#ff9a92";
/** This one hex is the obstacle. Hot, because it is the thing the player has to go and look at. */
const OBSTRUCTED = "#ff4d3d";
/** A deposit about to be sealed. The one change in here that is not free to walk back. */
const SEALING = "#f0b45a";
/** Ground coming up. */
const CUT = "#7fc9ff";
/** Ground going down. */
const FILL = "#ffd479";
/** A surface being laid over ground that stays where it is. */
const PAVING = "#79e7c0";

/**
 * How far above the finished grade the ghost floats. Depth testing is off, so this is not clearance
 * from z-fighting — it is only enough separation to read the plate as sitting *on* the grade.
 */
const LIFT = 0.02;

/**
 * The ghost of a pending earthworks selection: one lifted plate per hex, drawn at the height the
 * finished grade would sit at, coloured by what is about to happen to it, with a bright rim around
 * the shape's outer boundary.
 *
 * Three rules earn their keep here.
 *
 * Depth testing is off, matching every other spatial overlay. A cut is drawn *below* the ground it
 * is cutting, so a depth-tested ghost buried itself the moment the player chose the one verb whose
 * result they most needed to see.
 *
 * The rim is drawn on the cells of the selection that touch something outside it, so a rectangle
 * reads as a rectangle and a circle as a circle rather than as a heap of hexes. That perimeter is
 * pure presentation — it decides where to draw a line, never what native will grade.
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
  private readonly rimGeometry = new RingGeometry(
    HEX_RADIUS * 0.8,
    HEX_RADIUS * 0.98,
    6,
    1,
    HEX_RING_START,
  );
  private readonly material = new MeshBasicMaterial({
    color: 0xffffff,
    transparent: true,
    opacity: 0.4,
    depthTest: false,
    depthWrite: false,
  });
  private readonly rimMaterial = new MeshBasicMaterial({
    color: 0xffffff,
    transparent: true,
    opacity: 0.95,
    depthTest: false,
    depthWrite: false,
  });
  private ghost: InstancedMesh | null = null;
  private rim: InstancedMesh | null = null;
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
    const edge = perimeter(preview.cells);
    this.ghost = new InstancedMesh(
      this.geometry,
      this.material,
      preview.cells.length,
    );
    this.ghost.renderOrder = 24;
    if (edge.length > 0) {
      this.rim = new InstancedMesh(
        this.rimGeometry,
        this.rimMaterial,
        edge.length,
      );
      this.rim.renderOrder = 25;
    }
    const matrix = new Matrix4();
    const flat = new Quaternion().setFromAxisAngle(
      new Vector3(1, 0, 0),
      -Math.PI / 2,
    );
    const upright = new Quaternion();
    const scale = new Vector3(1, 0.07, 1);
    const one = new Vector3(1, 1, 1);
    const position = new Vector3();
    const tint = new Color();
    let rimIndex = 0;
    preview.cells.forEach((cell, index) => {
      const centre = axialToPixel(cell, 1, { x: 0, y: 0 });
      const base =
        this.terrain?.get(cellKey(cell.q, cell.r))?.height ??
        cell.elevation * GRADE_STEP_HEIGHT;
      const top = base + cell.change * GRADE_STEP_HEIGHT + LIFT;
      const color = tint.set(colorFor(preview, cell));
      position.set(centre.x, top, centre.y);
      matrix.compose(position, upright, scale);
      this.ghost!.setMatrixAt(index, matrix);
      this.ghost!.setColorAt(index, color);
      if (this.rim && edge[rimIndex] === index) {
        position.set(centre.x, top + LIFT, centre.y);
        matrix.compose(position, flat, one);
        this.rim.setMatrixAt(rimIndex, matrix);
        this.rim.setColorAt(rimIndex, color);
        rimIndex += 1;
      }
    });
    this.ghost.instanceMatrix.needsUpdate = true;
    if (this.ghost.instanceColor) this.ghost.instanceColor.needsUpdate = true;
    this.ghost.computeBoundingSphere();
    this.group.add(this.ghost);
    if (this.rim) {
      this.rim.instanceMatrix.needsUpdate = true;
      if (this.rim.instanceColor) this.rim.instanceColor.needsUpdate = true;
      this.rim.computeBoundingSphere();
      this.group.add(this.rim);
    }
  }

  private drop(): void {
    for (const mesh of [this.ghost, this.rim]) {
      if (!mesh) continue;
      this.group.remove(mesh);
      mesh.dispose();
    }
    this.ghost = null;
    this.rim = null;
  }

  dispose(): void {
    this.drop();
    this.geometry.dispose();
    this.rimGeometry.dispose();
    this.material.dispose();
    this.rimMaterial.dispose();
  }
}

/** Neighbour offsets in the axial plane, in the shipped clockwise E/SE/SW/W/NW/NE order. */
const NEIGHBOURS: readonly (readonly [number, number])[] = [
  [1, 0],
  [0, 1],
  [-1, 1],
  [-1, 0],
  [0, -1],
  [1, -1],
];

/**
 * The indices of the cells that touch something outside the selection — the shape's own boundary.
 * A selection one hex thick is entirely its own boundary, which is what makes a frame or a ring
 * draw as a solid outline without a second rule for it.
 */
function perimeter(cells: readonly GroundPreviewCell[]): number[] {
  const inside = new Set(cells.map((cell) => `${cell.q},${cell.r}`));
  const edge: number[] = [];
  cells.forEach((cell, index) => {
    const open = NEIGHBOURS.some(
      ([dq, dr]) => !inside.has(`${cell.q + dq},${cell.r + dr}`),
    );
    if (open) edge.push(index);
  });
  return edge;
}

function colorFor(preview: GroundPreview, cell: GroundPreviewCell): string {
  if (cell.blocked) return OBSTRUCTED;
  if (preview.error) return REFUSED;
  if (cell.covers) return SEALING;
  if (cell.retained) return SEALING;
  if (cell.change > 0) return FILL;
  if (cell.change < 0) return CUT;
  return PAVING;
}
