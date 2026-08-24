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

/**
 * How many route hexes the ribbon can draw, matching `MAX_WALK_PATH_CELLS` in the core. Native
 * cannot hand over a longer route, so the buffer is allocated once at this size and refilled in
 * place rather than regrown per frame.
 */
const ROUTE_CAPACITY = 512;

/** Clear of the construction grid at 0.018 and the hover and selection rings at 0.05. */
const ROUTE_LIFT = 0.09;

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
  private readonly routeGeometry = new BufferGeometry();
  // Two vertices per segment, one segment per route hex plus the one joining the player to the
  // first of them. Written in place every frame because the leading segment moves with the player.
  private readonly routePositions = new Float32Array((ROUTE_CAPACITY + 1) * 6);
  private readonly routeLine: LineSegments;
  private readonly routeGoal: Mesh;
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
    this.routeGeometry.setAttribute(
      "position",
      new Float32BufferAttribute(this.routePositions, 3),
    );
    this.routeLine = new LineSegments(this.routeGeometry, materials.route);
    this.routeLine.name = "walk-route";
    // The tail of the buffer holds stale points from whatever the last route was, so a bounding
    // sphere computed from it would be wrong. The ribbon is a few hundred segments at most and only
    // exists while a walk does, so it is cheaper to draw it than to bound it.
    this.routeLine.frustumCulled = false;
    this.routeGoal = ringMesh(materials.routeGoal);
    for (const object of [
      this.legal,
      this.illegal,
      this.selected,
      this.arrows,
      this.rangeRing,
      ...this.reachRings,
      this.routeLine,
      this.routeGoal,
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
    this.writeRoute(snapshot, terrain);

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
    this.routeGeometry.dispose();
    this.routeGoal.geometry.dispose();
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
    // InstancedMesh caches the first bounding sphere Three.js computes. These
    // overlays move with the pointer, so leaving that sphere near the landing
    // hub makes the renderer cull a valid hover ring once the camera follows
    // the player far enough away. Keep the culling bound aligned with the
    // matrices we just wrote.
    mesh.computeBoundingSphere();
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
    this.arrows.computeBoundingSphere();
  }

  /**
   * The ribbon along the route an autonomous walk is following, and the ring on the hex it ends at.
   *
   * Every point on it comes from `snapshot.player.walk_path`, which is native's own remaining route
   * — the hexes the steering will actually consume, replanned natively whenever the world changes
   * under it. Nothing here searches, smooths, or extrapolates: if the drawn ribbon and the walk ever
   * disagreed, the picture would be promising a way through that the simulation would not take.
   *
   * It starts at the player rather than at the first route hex, so the line is anchored under their
   * feet and shortens as they walk instead of jumping a hex at a time.
   */
  private writeRoute(
    snapshot: FactorySnapshot,
    terrain: ReadonlyMap<string, TerrainCell>,
  ): void {
    const { walk_goal: goal, walk_path: path } = snapshot.player;
    if (!goal || !path.length) {
      this.routeLine.visible = false;
      this.routeGoal.visible = false;
      return;
    }
    let previousX = snapshot.player.x / WORLD_SCALE;
    let previousZ = snapshot.player.y / WORLD_SCALE;
    const start = this.axialFromPlayer(snapshot.player.x, snapshot.player.y);
    let previousY = this.heightAt(terrain, start.q, start.r) + ROUTE_LIFT;
    let offset = 0;
    for (const cell of path.slice(0, ROUTE_CAPACITY)) {
      const point = axialToPixel(cell, 1, { x: 0, y: 0 });
      const y = this.heightAt(terrain, cell.q, cell.r) + ROUTE_LIFT;
      this.routePositions[offset] = previousX;
      this.routePositions[offset + 1] = previousY;
      this.routePositions[offset + 2] = previousZ;
      this.routePositions[offset + 3] = point.x;
      this.routePositions[offset + 4] = y;
      this.routePositions[offset + 5] = point.y;
      offset += 6;
      previousX = point.x;
      previousY = y;
      previousZ = point.y;
    }
    this.routeGeometry.getAttribute("position").needsUpdate = true;
    this.routeGeometry.setDrawRange(0, offset / 3);
    this.routeLine.visible = true;

    const destination = axialToPixel(goal, 1, { x: 0, y: 0 });
    this.placeWorldRing(
      this.routeGoal,
      destination.x,
      destination.y,
      this.heightAt(terrain, goal.q, goal.r) + ROUTE_LIFT,
      0.62,
      true,
    );
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
