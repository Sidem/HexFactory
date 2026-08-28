import {
  BufferGeometry,
  Color,
  CylinderGeometry,
  Float32BufferAttribute,
  Group,
  InstancedMesh,
  LineSegments,
  Matrix4,
  Quaternion,
  Vector3,
} from "three";
import { axialToPixel, pixelToAxial } from "@hexlife/embed/hex";

import { TRANSPORT_DIRECTIONS } from "../../core/directions";
import type {
  ChunkSnapshot,
  FactorySnapshot,
  GroundCell,
  SurfaceDefinition,
  Terrain,
} from "../../core/types";
import { WORLD_SCALE } from "../landmarks";
import { GRADE_STEP_HEIGHT } from "../surfaceLook";
import type { WorldMaterials } from "./materials";
import { TERRAIN_STYLE, visualHeight } from "./terrainStyle";

const WORLD_FLOOR = -0.34;
/** How thick a laid surface reads. Thin enough to be a skin, thick enough to catch the key light. */
const SURFACE_CAP_DEPTH = 0.05;
const ADJACENCY_DIRECTIONS = TRANSPORT_DIRECTIONS.slice(0, 6);
/** Exact circumradius for the public pointy-top axial projection. A smaller or rotated prism leaves
 * triangular holes where three cells meet; height may vary, but the logical plane is closed. */
export const HEX_RADIUS = 1;

export interface TerrainCell {
  readonly q: number;
  readonly r: number;
  readonly terrain: Terrain;
  readonly x: number;
  readonly z: number;
  /** Scene height of the walked surface, natural landform plus whatever grading has moved it. */
  readonly height: number;
  /** Native's grade in steps, signed. Zero on untouched ground. */
  readonly elevation: number;
  /** The surface definition id laid here, or 0 for untreated ground. */
  readonly surface: number;
}

export interface TerrainBuild {
  readonly group: Group;
  readonly cells: readonly TerrainCell[];
  readonly cellByKey: ReadonlyMap<string, TerrainCell>;
  readonly geometries: readonly BufferGeometry[];
}

/**
 * Builds only axial centres inside native-published chunk rectangles. An omitted terrain row is
 * surveyed lowland; a centre outside every rectangle is fog and never enters the mesh.
 */
export function buildTerrainMeshes(
  snapshot: FactorySnapshot,
  materials: WorldMaterials,
  surfaces: readonly SurfaceDefinition[] = [],
): TerrainBuild {
  const terrainByKey = new Map(
    snapshot.terrain.map((cell) => [cellKey(cell.q, cell.r), cell.terrain]),
  );
  const groundByKey = new Map(
    snapshot.ground.map((cell) => [cellKey(cell.q, cell.r), cell]),
  );
  const cells = surveyedCells(snapshot.chunks, terrainByKey, groundByKey);
  const group = new Group();
  group.name = "surveyed-terrain";
  const column = new CylinderGeometry(HEX_RADIUS, HEX_RADIUS, 1, 6, 1, false);
  const matrix = new Matrix4();
  const quaternion = new Quaternion();
  const scale = new Vector3();
  const position = new Vector3();
  const tint = new Color();
  for (const terrain of Object.keys(TERRAIN_STYLE) as Terrain[]) {
    const bucket = cells.filter((cell) => cell.terrain === terrain);
    if (!bucket.length) continue;
    const mesh = new InstancedMesh(
      column,
      materials.terrain[terrain],
      bucket.length,
    );
    mesh.name = `terrain-${terrain}`;
    mesh.receiveShadow = true;
    for (const [index, cell] of bucket.entries()) {
      const depth = Math.max(0.01, cell.height - WORLD_FLOOR);
      position.set(cell.x, WORLD_FLOOR + depth / 2, cell.z);
      scale.set(1, depth, 1);
      matrix.compose(position, quaternion, scale);
      mesh.setMatrixAt(index, matrix);
      // A luminance jitter, not a colour: the band's hue now comes from the procedural surface in
      // `terrainSurface.ts`, and tinting the instance as well would fight it.
      tint.setScalar(0.94 + stableVariation(cell.q, cell.r) * 0.12);
      mesh.setColorAt(index, tint);
    }
    mesh.instanceMatrix.needsUpdate = true;
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
    group.add(mesh);
  }
  const frontier = frontierLines(cells, materials);
  group.add(frontier.mesh);
  const caps = surfaceCaps(cells, surfaces, materials);
  if (caps) for (const mesh of caps.meshes) group.add(mesh);
  return {
    group,
    cells,
    cellByKey: new Map(cells.map((cell) => [cellKey(cell.q, cell.r), cell])),
    geometries: [column, frontier.geometry, ...(caps ? [caps.geometry] : [])],
  };
}

/**
 * Every prepared hex, laid as a skin on the landform rather than as a replacement for it, so the
 * band underneath still shows at the rim and a paved shore still reads as shore.
 *
 * Two things here are what make a finished yard read as one continuous surface instead of a grid of
 * tiles. The cap is the prism's full radius, so neighbouring hexes meet edge to edge with no groove
 * of bare ground between them. And nothing is tinted per instance: the colour, the courses and the
 * joints all come out of the material, sampled from world-space metres, so a pattern runs straight
 * across a hex boundary without knowing one is there. A per-hex luminance jitter was the lattice.
 *
 * One draw call per surface material — six at the very most, and one for the yard almost everyone
 * actually builds — however much of the map is finished. Nothing is computed per hex per frame.
 */
function surfaceCaps(
  cells: readonly TerrainCell[],
  surfaces: readonly SurfaceDefinition[],
  materials: WorldMaterials,
): { meshes: InstancedMesh[]; geometry: BufferGeometry } | null {
  const paved = cells.filter((cell) => cell.surface !== 0);
  if (!paved.length) return null;
  const keyById = new Map(surfaces.map((surface) => [surface.id, surface.key]));
  const buckets = new Map<number, TerrainCell[]>();
  for (const cell of paved) {
    const bucket = buckets.get(cell.surface);
    if (bucket) bucket.push(cell);
    else buckets.set(cell.surface, [cell]);
  }
  const geometry = new CylinderGeometry(HEX_RADIUS, HEX_RADIUS, 1, 6, 1, false);
  const matrix = new Matrix4();
  const quaternion = new Quaternion();
  const scale = new Vector3(1, SURFACE_CAP_DEPTH, 1);
  const position = new Vector3();
  const meshes: InstancedMesh[] = [];
  for (const [surface, bucket] of buckets) {
    const key = keyById.get(surface);
    const mesh = new InstancedMesh(
      geometry,
      materials.paving.material(key),
      bucket.length,
    );
    mesh.name = `prepared-ground-${key ?? surface}`;
    mesh.receiveShadow = true;
    for (const [index, cell] of bucket.entries()) {
      // The cap's top sits a hair above the column's so the two never fight for the same pixel.
      position.set(cell.x, cell.height + 0.004 - SURFACE_CAP_DEPTH / 2, cell.z);
      matrix.compose(position, quaternion, scale);
      mesh.setMatrixAt(index, matrix);
    }
    mesh.instanceMatrix.needsUpdate = true;
    meshes.push(mesh);
  }
  return { meshes, geometry };
}

export function terrainAt(
  cellByKey: ReadonlyMap<string, TerrainCell>,
  q: number,
  r: number,
): TerrainCell | undefined {
  return cellByKey.get(cellKey(q, r));
}

export function cellKey(q: number, r: number): string {
  return `${q},${r}`;
}

export function stableVariation(q: number, r: number): number {
  let value = Math.imul(q, 0x45d9f3b) ^ Math.imul(r, 0x27d4eb2d);
  value ^= value >>> 16;
  value = Math.imul(value, 0x45d9f3b);
  value ^= value >>> 16;
  return (value >>> 0) / 0xffffffff;
}

function surveyedCells(
  chunks: readonly ChunkSnapshot[],
  terrainByKey: ReadonlyMap<string, Terrain>,
  groundByKey: ReadonlyMap<string, GroundCell>,
): TerrainCell[] {
  const seen = new Set<string>();
  const cells: TerrainCell[] = [];
  for (const chunk of chunks) {
    const corners = [
      pixelToAxial({ x: chunk.x, y: chunk.y }, WORLD_SCALE),
      pixelToAxial({ x: chunk.x + chunk.span, y: chunk.y }, WORLD_SCALE),
      pixelToAxial({ x: chunk.x, y: chunk.y + chunk.span }, WORLD_SCALE),
      pixelToAxial(
        { x: chunk.x + chunk.span, y: chunk.y + chunk.span },
        WORLD_SCALE,
      ),
    ];
    const minQ = Math.min(...corners.map(({ q }) => q)) - 2;
    const maxQ = Math.max(...corners.map(({ q }) => q)) + 2;
    const minR = Math.min(...corners.map(({ r }) => r)) - 2;
    const maxR = Math.max(...corners.map(({ r }) => r)) + 2;
    for (let q = minQ; q <= maxQ; q += 1) {
      for (let r = minR; r <= maxR; r += 1) {
        const key = cellKey(q, r);
        if (seen.has(key)) continue;
        const world = axialToPixel({ q, r }, WORLD_SCALE, { x: 0, y: 0 });
        if (
          world.x < chunk.x ||
          world.x >= chunk.x + chunk.span ||
          world.y < chunk.y ||
          world.y >= chunk.y + chunk.span
        )
          continue;
        seen.add(key);
        const terrain = terrainByKey.get(key) ?? "lowland";
        const ground = groundByKey.get(key);
        cells.push({
          q,
          r,
          terrain,
          x: world.x / WORLD_SCALE,
          z: world.y / WORLD_SCALE,
          // Grading moves the walked surface, so everything that stands on the terrain — buildings,
          // fences, overlays, the ghost of a pending edit — follows from this one number.
          height:
            visualHeight(terrain) +
            (ground?.elevation ?? 0) * GRADE_STEP_HEIGHT,
          elevation: ground?.elevation ?? 0,
          surface: ground?.surface ?? 0,
        });
      }
    }
  }
  cells.sort((a, b) => a.r - b.r || a.q - b.q);
  return cells;
}

function frontierLines(
  cells: readonly TerrainCell[],
  materials: WorldMaterials,
): { mesh: LineSegments; geometry: BufferGeometry } {
  const generated = new Set(cells.map(({ q, r }) => cellKey(q, r)));
  const positions: number[] = [];
  for (const cell of cells) {
    for (const direction of ADJACENCY_DIRECTIONS) {
      if (generated.has(cellKey(cell.q + direction.q, cell.r + direction.r)))
        continue;
      const neighbour = axialToPixel(direction, 1, { x: 0, y: 0 });
      const distance = Math.hypot(neighbour.x, neighbour.y);
      const midpointX = cell.x + neighbour.x * 0.5;
      const midpointZ = cell.z + neighbour.y * 0.5;
      const tangentX = (-neighbour.y / distance) * 0.5;
      const tangentZ = (neighbour.x / distance) * 0.5;
      const y = cell.height + 0.04;
      positions.push(
        midpointX - tangentX,
        y,
        midpointZ - tangentZ,
        midpointX + tangentX,
        y,
        midpointZ + tangentZ,
      );
    }
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(positions, 3));
  const mesh = new LineSegments(geometry, materials.frontier);
  mesh.name = "survey-frontier";
  mesh.renderOrder = 30;
  return { mesh, geometry };
}
