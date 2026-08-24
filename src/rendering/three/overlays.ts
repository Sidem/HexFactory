import {
  BufferGeometry,
  ConeGeometry,
  Float32BufferAttribute,
  Group,
  InstancedMesh,
  LineSegments,
  Matrix4,
  Mesh,
  Quaternion,
  RingGeometry,
  Vector3,
} from "three";
import {
  axialToPixel,
  pixelToAxial,
  type AxialCoordinate,
} from "@hexlife/embed/hex";

import type {
  FactorySnapshot,
  LinePreviewCell,
  PlacementPreview,
} from "../../core/types";
import { TRANSPORT_DIRECTIONS } from "../../core/directions";
import type { ReachRadii } from "../FactoryRenderer";
import { WORLD_SCALE } from "../landmarks";
import type { WorldMaterials } from "./materials";
import type { TerrainCell } from "./terrainMeshes";
import { cellKey } from "./terrainMeshes";

export interface SpatialOverlayState {
  readonly hover: AxialCoordinate | null;
  readonly selection: AxialCoordinate | null;
  readonly placement: PlacementPreview | null;
  readonly dragPath: readonly LinePreviewCell[];
  readonly buildMode: boolean;
  readonly gridToggled: boolean;
  readonly buildFootprint: readonly AxialCoordinate[];
  readonly buildOrientation: number;
  readonly buildReach: ReachRadii | null;
  readonly gathering: boolean;
}

const OVERLAY_CAPACITY = 512;
export const HEX_RING_START = Math.PI / 6;

export class SpatialOverlays {
  readonly group = new Group();
  private readonly ringGeometry = new RingGeometry(
    0.72,
    0.87,
    6,
    1,
    HEX_RING_START,
  );
  private readonly directionGeometry = new ConeGeometry(0.08, 0.32, 4);
  private readonly legal: InstancedMesh;
  private readonly illegal: InstancedMesh;
  private readonly selected: InstancedMesh;
  private readonly arrows: InstancedMesh;
  private readonly rangeRing: Mesh;
  private readonly reachRings: Mesh[];
  private grid: LineSegments | null = null;
  private gridGeometry: BufferGeometry | null = null;

  constructor(private readonly materials: WorldMaterials) {
    this.group.name = "spatial-overlays";
    this.legal = new InstancedMesh(
      this.ringGeometry,
      materials.overlayLegal,
      OVERLAY_CAPACITY,
    );
    this.illegal = new InstancedMesh(
      this.ringGeometry,
      materials.overlayIllegal,
      OVERLAY_CAPACITY,
    );
    this.selected = new InstancedMesh(
      this.ringGeometry,
      materials.overlaySelection,
      OVERLAY_CAPACITY,
    );
    this.arrows = new InstancedMesh(
      this.directionGeometry,
      materials.overlayLegal,
      OVERLAY_CAPACITY,
    );
    this.rangeRing = ringMesh(materials.overlaySelection);
    this.reachRings = [
      ringMesh(materials.overlayLegal),
      ringMesh(materials.overlaySelection),
      ringMesh(materials.overlayIllegal),
    ];
    for (const object of [
      this.legal,
      this.illegal,
      this.selected,
      this.arrows,
      this.rangeRing,
      ...this.reachRings,
    ]) {
      object.renderOrder = 40;
      this.group.add(object);
    }
  }

  setTerrain(cells: readonly TerrainCell[]): void {
    if (this.grid) this.group.remove(this.grid);
    this.gridGeometry?.dispose();
    const positions: number[] = [];
    for (const cell of cells) {
      for (let corner = 0; corner < 6; corner += 1) {
        const a = hexCorner(cell.x, cell.z, corner);
        const b = hexCorner(cell.x, cell.z, corner + 1);
        const y = cell.height + 0.018;
        positions.push(a.x, y, a.z, b.x, y, b.z);
      }
    }
    this.gridGeometry = new BufferGeometry();
    this.gridGeometry.setAttribute(
      "position",
      new Float32BufferAttribute(positions, 3),
    );
    this.grid = new LineSegments(this.gridGeometry, this.materials.grid);
    this.grid.name = "construction-grid";
    this.grid.renderOrder = 35;
    this.group.add(this.grid);
  }

  update(
    snapshot: FactorySnapshot,
    state: SpatialOverlayState,
    terrain: ReadonlyMap<string, TerrainCell>,
  ): void {
    if (this.grid) this.grid.visible = state.buildMode || state.gridToggled;
    const legal: AxialCoordinate[] = [];
    const illegal: AxialCoordinate[] = [];
    const selected: AxialCoordinate[] = [];
    if (state.selection) selected.push(state.selection);
    if (state.hover) {
      const target = state.placement?.legal === false ? illegal : legal;
      target.push(state.hover);
      if (state.buildMode)
        for (const cell of state.buildFootprint)
          target.push({ q: state.hover.q + cell.q, r: state.hover.r + cell.r });
    }
    for (const cell of state.dragPath)
      (cell.legal ? legal : illegal).push(cell);
    this.writeCells(this.legal, legal, terrain);
    this.writeCells(this.illegal, illegal, terrain);
    this.writeCells(this.selected, selected, terrain);
    this.writeArrows(
      state.dragPath,
      state.hover,
      state.buildOrientation,
      terrain,
    );

    const playerAxial = state.gathering
      ? this.axialFromPlayer(snapshot.player.x, snapshot.player.y)
      : null;
    const buildRange = snapshot.player.build_range / WORLD_SCALE;
    this.placeWorldRing(
      this.rangeRing,
      snapshot.player.x / WORLD_SCALE,
      snapshot.player.y / WORLD_SCALE,
      this.heightAt(terrain, playerAxial?.q ?? 0, playerAxial?.r ?? 0) + 0.025,
      buildRange,
      state.buildMode,
    );
    const reachValues = state.buildReach
      ? [
          state.buildReach.extract,
          state.buildReach.supply,
          state.buildReach.link,
        ]
      : [state.gathering ? snapshot.player.extract_radius : null, null, null];
    for (const [index, ring] of this.reachRings.entries()) {
      const radius = reachValues[index];
      const center = state.hover ?? playerAxial;
      if (radius === null || radius === undefined || !center) {
        ring.visible = false;
        continue;
      }
      const point = axialToPixel(center, 1, { x: 0, y: 0 });
      this.placeWorldRing(
        ring,
        point.x,
        point.y,
        this.heightAt(terrain, center.q, center.r) + 0.035,
        Math.max(0.82, radius * Math.sqrt(3) + 0.92),
        true,
      );
    }
  }

  dispose(): void {
    this.ringGeometry.dispose();
    this.directionGeometry.dispose();
    this.rangeRing.geometry.dispose();
    for (const ring of this.reachRings) ring.geometry.dispose();
    this.gridGeometry?.dispose();
  }

  private writeCells(
    mesh: InstancedMesh,
    cells: readonly AxialCoordinate[],
    terrain: ReadonlyMap<string, TerrainCell>,
  ): void {
    const matrix = new Matrix4();
    const quaternion = new Quaternion().setFromAxisAngle(
      new Vector3(1, 0, 0),
      -Math.PI / 2,
    );
    const scale = new Vector3(1, 1, 1);
    let count = 0;
    for (const cell of cells.slice(0, OVERLAY_CAPACITY)) {
      const point = axialToPixel(cell, 1, { x: 0, y: 0 });
      matrix.compose(
        new Vector3(
          point.x,
          this.heightAt(terrain, cell.q, cell.r) + 0.05,
          point.y,
        ),
        quaternion,
        scale,
      );
      mesh.setMatrixAt(count, matrix);
      count += 1;
    }
    mesh.count = count;
    mesh.instanceMatrix.needsUpdate = true;
  }

  private writeArrows(
    dragPath: readonly LinePreviewCell[],
    hover: AxialCoordinate | null,
    orientation: number,
    terrain: ReadonlyMap<string, TerrainCell>,
  ): void {
    const cells: Array<AxialCoordinate & { orientation: number }> =
      dragPath.map((cell) => cell);
    if (!cells.length && hover) cells.push({ ...hover, orientation });
    const matrix = new Matrix4();
    let count = 0;
    for (const cell of cells.slice(0, OVERLAY_CAPACITY)) {
      const point = axialToPixel(cell, 1, { x: 0, y: 0 });
      const direction =
        TRANSPORT_DIRECTIONS[cell.orientation] ?? TRANSPORT_DIRECTIONS[0]!;
      const tip = axialToPixel(direction, 0.25, { x: point.x, y: point.y });
      const angle = Math.atan2(tip.x - point.x, tip.y - point.y);
      matrix.compose(
        new Vector3(
          tip.x,
          this.heightAt(terrain, cell.q, cell.r) + 0.12,
          tip.y,
        ),
        new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), angle),
        new Vector3(1, 1, 1),
      );
      this.arrows.setMatrixAt(count, matrix);
      count += 1;
    }
    this.arrows.count = count;
    this.arrows.instanceMatrix.needsUpdate = true;
  }

  private placeWorldRing(
    ring: Mesh,
    x: number,
    z: number,
    y: number,
    radius: number,
    visible: boolean,
  ): void {
    ring.visible = visible;
    ring.position.set(x, y, z);
    ring.scale.setScalar(radius);
  }

  private axialFromPlayer(x: number, y: number): AxialCoordinate {
    return pixelToAxial({ x, y }, WORLD_SCALE);
  }

  private heightAt(
    terrain: ReadonlyMap<string, TerrainCell>,
    q: number,
    r: number,
  ): number {
    return terrain.get(cellKey(q, r))?.height ?? 0.07;
  }
}

function ringMesh(material: WorldMaterials["overlaySelection"]): Mesh {
  const mesh = new Mesh(new RingGeometry(0.97, 1.03, 48), material);
  mesh.rotateX(-Math.PI / 2);
  mesh.visible = false;
  return mesh;
}

function hexCorner(
  x: number,
  z: number,
  index: number,
): { x: number; z: number } {
  const angle = ((60 * (index % 6) - 30) * Math.PI) / 180;
  return { x: x + Math.cos(angle), z: z + Math.sin(angle) };
}
