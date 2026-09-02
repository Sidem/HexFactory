import {
  BufferGeometry,
  CylinderGeometry,
  Float32BufferAttribute,
  Group,
  InstancedMesh,
  LineSegments,
  Matrix4,
  Mesh,
  Quaternion,
  Raycaster,
  Vector3,
  type Material,
} from "three";
import { axialToPixel, pixelToAxial } from "@hexlife/embed/hex";

import { TRANSPORT_DIRECTIONS } from "../../core/directions";
import type {
  FactorySnapshot,
  SurfaceDefinition,
  Terrain,
  TerrainSnapshot,
  WorldPoint,
} from "../../core/types";
import { hexCorner } from "../hexDraw";
import { WORLD_SCALE } from "../landmarks";
import { CLIFF_THRESHOLD, HEIGHT_UNIT_HEIGHT } from "../sceneScale";
import {
  buildHeightfieldGeometry,
  pickHeightfieldRay,
  type GeometryBuckets,
  type HeightfieldSample,
  type HeightfieldSubstrate,
} from "./heightfieldTerrain";
import type { WorldMaterials } from "./materials";
import { TERRAIN_STYLE } from "./terrainStyle";

/**
 * How far the survey frontier skirt drops below the last published edge. Deep enough that no camera
 * angle sees under the world, shallow enough that it reads as an edge rather than as a cliff.
 */
export const WORLD_FLOOR = -0.34;
/** How thick a laid surface reads. Thin enough to be a skin, thick enough to catch the key light. */
const SURFACE_CAP_DEPTH = 0.05;
const ADJACENCY_DIRECTIONS = TRANSPORT_DIRECTIONS.slice(0, 6);
/** Exact circumradius for the public pointy-top axial projection. A smaller or rotated cell leaves
 * triangular holes where three cells meet; height may vary, but the logical plane is closed. */
export const HEX_RADIUS = 1;

/**
 * The band order the seven materials are declared in, used as the heightfield's look bucket.
 *
 * The band rather than the substrate decides which material a triangle draws with. Native's
 * substrate is what the ground is made of and travels per vertex for the shaders that want it; the
 * band is what it looks like, and it is the only one of the two that separates a river bed from the
 * hillside beside it when both are soil.
 */
const TERRAIN_LOOKS = Object.keys(TERRAIN_STYLE) as readonly Terrain[];
const TERRAIN_LOOK: ReadonlyMap<Terrain, number> = new Map(
  TERRAIN_LOOKS.map((terrain, index) => [terrain, index]),
);
/** Corner blending needs one neighbour; the frontier dissolve uses two rings. */
const SAMPLE_HALO = 2;

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
  /** What the ground is made of, as native generated it. */
  readonly substrate: HeightfieldSubstrate;
  /** Standing water depth in native's height unit. Zero on dry ground. */
  readonly waterDepth: number;
  /** Scene height of the water's own surface. Equal to {@link height} when the cell is dry. */
  readonly waterHeight: number;
  /** Native's flow class for the water standing here, 0 on still or dry ground. */
  readonly discharge: number;
}

export interface TerrainBuild {
  readonly group: Group;
  readonly cells: readonly TerrainCell[];
  readonly cellByKey: ReadonlyMap<string, TerrainCell>;
  readonly geometries: readonly BufferGeometry[];
  /** Scene height of the highest drawn ground. A pick ray meets nothing above it. */
  readonly ceiling: number;
  /** The one object a pointer ray is cast against. */
  readonly surface: Group;
}

interface TerrainChunkBuild {
  readonly surface: Group;
  readonly extras: Group;
  readonly grid: LineSegments;
  readonly geometries: readonly BufferGeometry[];
}

/** A scene-space ray, as the camera hands one over. */
export interface TerrainRay {
  readonly origin: Point3;
  readonly direction: Point3;
}

/** Where a ray met the drawn landform, and which cell owns the surface it met. */
export interface TerrainPick {
  readonly cell: TerrainCell;
  readonly x: number;
  readonly z: number;
  readonly height: number;
}

interface Point3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

/**
 * Build the surveyed world as one continuous height mesh rather than as a prism per hex.
 *
 * Every cell native surveyed travels in the terrain group, so this reads that list and nothing else:
 * there is no chunk rectangle to scan and no omitted row to guess a band for. Ordinary neighbours
 * share averaged corners and become a slope; a step wider than the one the player can climb keeps
 * both heights and gets a vertical face. Water is its own surface at its own level.
 *
 * Height is native's published bed plus native's own earthwork, added exactly as native adds them,
 * converted once at this boundary by {@link HEIGHT_UNIT_HEIGHT}. Standing water is the same sum:
 * generated depth plus the published departure. The renderer interpolates between those samples and
 * never invents a second elevation oracle.
 */
export function buildTerrainMeshes(
  snapshot: FactorySnapshot,
  materials: WorldMaterials,
  surfaces: readonly SurfaceDefinition[] = [],
): TerrainBuild {
  const groundByKey = new Map(
    snapshot.ground.map((cell) => [cellKey(cell.q, cell.r), cell]),
  );
  const waterByKey = new Map(
    snapshot.water.map((cell) => [cellKey(cell.q, cell.r), cell]),
  );
  const cells = snapshot.terrain
    .map((tile) =>
      terrainCell(
        tile,
        groundByKey.get(cellKey(tile.q, tile.r)),
        waterByKey.get(cellKey(tile.q, tile.r)),
      ),
    )
    .sort((a, b) => a.r - b.r || a.q - b.q);

  const built = buildHeightfieldGeometry(cells.map(heightfieldSample), {
    cliffThreshold: CLIFF_THRESHOLD,
    frontierDepth: Math.max(0.01, -WORLD_FLOOR),
  });

  const group = new Group();
  group.name = "surveyed-terrain";
  // One object for the whole landform, so a pointer ray is cast against a single subtree and the
  // ground, its cliff faces and its frontier skirt all answer as the same surface.
  const surface = new Group();
  surface.name = "terrain-surface";
  group.add(surface);
  for (const [name, geometry, buckets] of [
    ["ground", built.ground, built.buckets.ground],
    ["cliffs", built.cliffs, built.buckets.cliffs],
    ["frontier-skirt", built.frontier, built.buckets.frontier],
  ] as const) {
    const mesh = bandMesh(name, geometry, buckets, materials);
    if (mesh) surface.add(mesh);
  }
  const water = bandMesh("water", built.water, built.buckets.water, materials);
  if (water) {
    // Water is drawn but never picked: a pointer over a ford names the bed it would wade across.
    water.renderOrder = 10;
    group.add(water);
  }

  const frontier = frontierLines(cells, materials);
  group.add(frontier.mesh);
  const caps = surfaceCaps(cells, surfaces, materials);
  if (caps) for (const mesh of caps.meshes) group.add(mesh);
  return {
    group,
    surface,
    cells,
    cellByKey: new Map(cells.map((cell) => [cellKey(cell.q, cell.r), cell])),
    geometries: [
      built.ground,
      built.water,
      built.cliffs,
      built.frontier,
      frontier.geometry,
      ...(caps ? [caps.geometry] : []),
    ],
    ceiling: cells.reduce(
      (highest, cell) => Math.max(highest, cell.height, cell.waterHeight),
      WORLD_FLOOR,
    ),
  };
}

/**
 * Incremental terrain presentation, partitioned by native generation chunk.
 *
 * The original Phase 8 heightfield was one mesh for every surveyed cell. That made slopes easy to
 * join, but it also meant discovering 64 cells rebuilt every cell ever seen and left the result as
 * one uncullable draw object. This cache keeps the exact same heightfield rules while giving each
 * native chunk its own meshes. A two-cell halo supplies the neighbours used by corner blending;
 * occupancy for the two-ring frontier fade is the whole surveyed world, so a chunk seam is not
 * mistaken for unexplored ground. The `includes` predicate makes only the centre chunk emit
 * triangles. Adding or changing one chunk therefore rebuilds at most its existing 3x3 neighbourhood,
 * and Three.js can frustum-cull every other chunk's meshes.
 */

export class TerrainMeshCache implements TerrainBuild {
  readonly group = new Group();
  readonly surface = new Group();
  readonly grid = new Group();
  readonly cellByKey = new Map<string, TerrainCell>();
  readonly cells: TerrainCell[] = [];
  ceiling = WORLD_FLOOR;

  private readonly chunks = new Map<string, TerrainChunkBuild>();
  private readonly cellsByChunk = new Map<string, Map<string, TerrainCell>>();
  private readonly tileByKey = new Map<string, TerrainSnapshot>();
  private readonly cellIndexByKey = new Map<string, number>();
  private groundByKey = new Map<string, FactorySnapshot["ground"][number]>();
  private waterByKey = new Map<string, FactorySnapshot["water"][number]>();
  private activeChunks = new Set<string>();
  private chunkSize = 1;
  private lastChunks: FactorySnapshot["chunks"] | null = null;
  private lastTerrain: FactorySnapshot["terrain"] | null = null;
  private lastGround: FactorySnapshot["ground"] | null = null;
  private lastWater: FactorySnapshot["water"] | null = null;

  constructor(
    private readonly materials: WorldMaterials,
    private readonly surfaces: readonly SurfaceDefinition[] = [],
  ) {
    this.group.name = "surveyed-terrain";
    this.surface.name = "terrain-surface";
    this.grid.name = "construction-grid";
    this.grid.visible = false;
    this.group.add(this.surface, this.grid);
  }

  /** Geometry currently retained by Three.js; exposed for the renderer's disposal contract. */
  get geometries(): readonly BufferGeometry[] {
    return [...this.chunks.values()].flatMap((chunk) => chunk.geometries);
  }

  /**
   * Bring the cache to one native snapshot. Returns the number of chunk meshes rebuilt, which is
   * useful to pin the bounded exploration cost without timing-sensitive tests.
   */
  update(snapshot: FactorySnapshot): number {
    const nextSize = inferChunkSize(snapshot);
    if (this.needsReset(snapshot, nextSize)) {
      this.reset(snapshot, nextSize);
      return this.chunks.size;
    }

    const dirty = new Set<string>();
    const nextActive = new Set(
      snapshot.chunks.map(({ chunk_q, chunk_r }) => chunkKey(chunk_q, chunk_r)),
    );
    for (const key of nextActive)
      if (!this.activeChunks.has(key)) dirty.add(key);
    this.activeChunks = nextActive;

    if (snapshot.ground !== this.lastGround) {
      const next = keyedCells(snapshot.ground);
      addChangedKeys(this.groundByKey, next, dirty, this.chunkSize);
      this.groundByKey = next;
    }
    if (snapshot.water !== this.lastWater) {
      const next = keyedCells(snapshot.water);
      addChangedKeys(this.waterByKey, next, dirty, this.chunkSize);
      this.waterByKey = next;
    }

    if (snapshot.terrain !== this.lastTerrain) {
      const start = this.lastTerrain?.length ?? 0;
      for (const tile of snapshot.terrain.slice(start)) {
        this.tileByKey.set(cellKey(tile.q, tile.r), tile);
        dirty.add(ownerKey(tile.q, tile.r, this.chunkSize));
      }
    }

    // Ground and water marks name their owner chunk above. Refresh every cell in a dirty owner;
    // this is 64 cells in production and keeps removed sparse overrides correct as well.
    for (const key of dirty) this.refreshChunkCells(key);
    const rebuilt = this.rebuildAffected(dirty);
    this.remember(snapshot);
    return rebuilt;
  }

  dispose(): void {
    for (const key of [...this.chunks.keys()]) this.disposeChunk(key);
    this.cellByKey.clear();
    this.cells.length = 0;
    this.cellsByChunk.clear();
    this.tileByKey.clear();
    this.cellIndexByKey.clear();
  }

  private needsReset(snapshot: FactorySnapshot, nextSize: number): boolean {
    if (!this.lastTerrain || nextSize !== this.chunkSize) return true;
    if (this.lastChunks && snapshot.chunks.length < this.lastChunks.length)
      return true;
    if (snapshot.terrain === this.lastTerrain) return false;
    if (snapshot.terrain.length <= this.lastTerrain.length) return true;
    if (this.lastTerrain.length === 0) return false;
    // applyTerrainPatch preserves the old object references and appends newly surveyed chunks.
    // A load/reset replaces them, even when it happens to contain more cells than the old world.
    return (
      snapshot.terrain[0] !== this.lastTerrain[0] ||
      snapshot.terrain[this.lastTerrain.length - 1] !==
        this.lastTerrain[this.lastTerrain.length - 1]
    );
  }

  private reset(snapshot: FactorySnapshot, chunkSize: number): void {
    for (const key of [...this.chunks.keys()]) this.disposeChunk(key);
    this.cellByKey.clear();
    this.cells.length = 0;
    this.cellsByChunk.clear();
    this.tileByKey.clear();
    this.cellIndexByKey.clear();
    this.ceiling = WORLD_FLOOR;
    this.chunkSize = chunkSize;
    this.activeChunks = new Set(
      snapshot.chunks.map(({ chunk_q, chunk_r }) => chunkKey(chunk_q, chunk_r)),
    );
    this.groundByKey = keyedCells(snapshot.ground);
    this.waterByKey = keyedCells(snapshot.water);
    for (const tile of snapshot.terrain)
      this.tileByKey.set(cellKey(tile.q, tile.r), tile);
    for (const key of this.activeChunks) this.refreshChunkCells(key);
    for (const key of this.activeChunks) this.rebuildChunk(key);
    this.remember(snapshot);
  }

  private remember(snapshot: FactorySnapshot): void {
    this.lastChunks = snapshot.chunks;
    this.lastTerrain = snapshot.terrain;
    this.lastGround = snapshot.ground;
    this.lastWater = snapshot.water;
  }

  private refreshChunkCells(key: string): void {
    if (!this.activeChunks.has(key)) return;
    const [chunkQ, chunkR] = parseChunkKey(key);
    let bucket = this.cellsByChunk.get(key);
    if (!bucket) {
      bucket = new Map();
      this.cellsByChunk.set(key, bucket);
    }
    const q0 = chunkQ * this.chunkSize;
    const r0 = chunkR * this.chunkSize;
    for (let r = r0; r < r0 + this.chunkSize; r += 1) {
      for (let q = q0; q < q0 + this.chunkSize; q += 1) {
        const keyAt = cellKey(q, r);
        const tile = this.tileByKey.get(keyAt);
        if (!tile) continue;
        const previous = this.cellByKey.get(keyAt);
        const cell = terrainCell(
          tile,
          this.groundByKey.get(keyAt),
          this.waterByKey.get(keyAt),
        );
        this.cellByKey.set(keyAt, cell);
        bucket.set(keyAt, cell);
        const index = this.cellIndexByKey.get(keyAt);
        if (index === undefined) {
          this.cellIndexByKey.set(keyAt, this.cells.length);
          this.cells.push(cell);
        } else {
          this.cells[index] = cell;
        }
        if (cell.height >= this.ceiling) this.ceiling = cell.height;
        else if (previous?.height === this.ceiling)
          this.ceiling = this.cells.reduce(
            (highest, entry) => Math.max(highest, entry.height),
            WORLD_FLOOR,
          );
      }
    }
  }

  private rebuildAffected(dirty: ReadonlySet<string>): number {
    const affected = new Set<string>();
    for (const key of dirty) {
      const [chunkQ, chunkR] = parseChunkKey(key);
      for (let dr = -1; dr <= 1; dr += 1)
        for (let dq = -1; dq <= 1; dq += 1) {
          const neighbour = chunkKey(chunkQ + dq, chunkR + dr);
          if (this.activeChunks.has(neighbour)) affected.add(neighbour);
        }
    }
    for (const key of affected) this.rebuildChunk(key);
    return affected.size;
  }

  private rebuildChunk(key: string): void {
    this.disposeChunk(key);
    const owned = this.cellsByChunk.get(key);
    if (!owned?.size) return;
    const [chunkQ, chunkR] = parseChunkKey(key);
    const samples = haloSamples(chunkQ, chunkR, this.chunkSize, this.cellByKey);
    const ownedKeys = new Set(owned.keys());
    const built = buildHeightfieldGeometry(samples.map(heightfieldSample), {
      cliffThreshold: CLIFF_THRESHOLD,
      frontierDepth: Math.max(0.01, -WORLD_FLOOR),
      includes: (sample) => ownedKeys.has(cellKey(sample.q, sample.r)),
      occupied: (q, r) => this.cellByKey.has(cellKey(q, r)),
    });
    const surface = new Group();
    surface.name = `terrain-chunk-surface-${key}`;
    for (const [name, geometry, buckets] of [
      ["ground", built.ground, built.buckets.ground],
      ["cliffs", built.cliffs, built.buckets.cliffs],
      ["frontier-skirt", built.frontier, built.buckets.frontier],
    ] as const) {
      const mesh = bandMesh(
        `${name}-${key}`,
        geometry,
        buckets,
        this.materials,
      );
      if (mesh) surface.add(mesh);
    }
    const extras = new Group();
    extras.name = `terrain-chunk-${key}`;
    const water = bandMesh(
      `water-${key}`,
      built.water,
      built.buckets.water,
      this.materials,
    );
    if (water) {
      water.renderOrder = 10;
      extras.add(water);
    }
    const frontier = frontierLines(
      [...owned.values()],
      this.materials,
      this.cellByKey,
    );
    extras.add(frontier.mesh);
    const caps = surfaceCaps(
      [...owned.values()],
      this.surfaces,
      this.materials,
    );
    if (caps) for (const mesh of caps.meshes) extras.add(mesh);
    const gridGeometry = constructionGridGeometry([...owned.values()]);
    const grid = new LineSegments(gridGeometry, this.materials.grid);
    grid.name = `construction-grid-${key}`;
    grid.renderOrder = 35;
    this.surface.add(surface);
    this.group.add(extras);
    this.grid.add(grid);
    this.chunks.set(key, {
      surface,
      extras,
      grid,
      geometries: [
        built.ground,
        built.water,
        built.cliffs,
        built.frontier,
        frontier.geometry,
        gridGeometry,
        ...(caps ? [caps.geometry] : []),
      ],
    });
  }

  private disposeChunk(key: string): void {
    const chunk = this.chunks.get(key);
    if (!chunk) return;
    this.surface.remove(chunk.surface);
    this.group.remove(chunk.extras);
    this.grid.remove(chunk.grid);
    for (const geometry of chunk.geometries) geometry.dispose();
    this.chunks.delete(key);
  }
}

/** One published tile joined with whatever earthwork and water the player has moved on top of it. */
function terrainCell(
  tile: TerrainSnapshot,
  ground:
    | {
        readonly elevation: number;
        readonly erosion?: number;
        readonly surface: number;
      }
    | undefined,
  water: { readonly departure: number } | undefined,
): TerrainCell {
  const world = axialToPixel(tile, WORLD_SCALE, { x: 0, y: 0 });
  const elevation = (ground?.elevation ?? 0) + (ground?.erosion ?? 0);
  // Generated bed and paid-for earthwork are the same unit and native sums them, so the host does
  // too. Everything that stands on the terrain follows from this one number.
  const height = (tile.height + elevation) * HEIGHT_UNIT_HEIGHT;
  const waterDepth = Math.max(0, tile.water_depth + (water?.departure ?? 0));
  return {
    q: tile.q,
    r: tile.r,
    terrain: tile.terrain,
    x: world.x / WORLD_SCALE,
    z: world.y / WORLD_SCALE,
    height,
    elevation,
    surface: ground?.surface ?? 0,
    substrate: tile.substrate,
    waterDepth,
    waterHeight: height + waterDepth * HEIGHT_UNIT_HEIGHT,
    discharge: tile.discharge,
  };
}

function heightfieldSample(cell: TerrainCell): HeightfieldSample {
  const look = TERRAIN_LOOK.get(cell.terrain) ?? 0;
  return {
    q: cell.q,
    r: cell.r,
    height: cell.height,
    substrate: cell.substrate,
    waterDepth: cell.waterDepth,
    waterHeight: cell.waterHeight,
    dischargeClass: cell.discharge,
    look,
    waterLook: look,
  };
}

/**
 * One mesh per surface, drawn in as many passes as it spans bands. Splitting by band would have made
 * the landform several meshes, and a continuous surface has to be one: shared corner vertices are
 * the whole reason neighbouring cells read as a slope rather than as tiles.
 */
function bandMesh(
  name: string,
  geometry: BufferGeometry,
  buckets: GeometryBuckets,
  materials: WorldMaterials,
): Mesh | null {
  if (buckets.length === 0) return null;
  const used: Material[] = buckets.map(
    (look) => materials.terrain[TERRAIN_LOOKS[look] ?? "lowland"],
  );
  const mesh = new Mesh(geometry, used);
  mesh.name = `terrain-${name}`;
  mesh.receiveShadow = true;
  return mesh;
}

/**
 * Name the cell whose drawn surface a ray meets first.
 *
 * The old picker intersected the flat logical plane, which is only where a cell stands when the
 * landform is flat. This camera looks down at about forty degrees, so ground standing a cliff and
 * three graded steps above its neighbour draws more than a hex away from the plane point beneath it:
 * the player clicked the top of the rise and native was handed the cell in front of it.
 *
 * The landform is now real geometry, so the ray is cast against it instead of marched down a field
 * of columns, and the point it meets is resolved back to the axial cell that owns it. A cliff face
 * answers as well as a top, because the face of a rise belongs to the rise. Nothing here becomes
 * simulation truth: native still decides what the named cell allows.
 */
export function pickTerrainCell(
  build: TerrainBuild,
  ray: TerrainRay,
): TerrainPick | null {
  const raycaster = new Raycaster(
    new Vector3(ray.origin.x, ray.origin.y, ray.origin.z),
    new Vector3(ray.direction.x, ray.direction.y, ray.direction.z).normalize(),
  );
  const hit = pickHeightfieldRay(raycaster, build.surface);
  if (!hit) return null;
  const cell =
    build.cellByKey.get(cellKey(hit.axial.q, hit.axial.r)) ??
    // A frontier skirt hangs off the outside of the last published edge, so the point where a ray
    // meets it can resolve to the unsurveyed cell beyond. The edge belongs to the cell it hangs
    // from, and that cell is the nearest published one.
    nearestCell(build, hit.world.x, hit.world.y);
  return cell
    ? { cell, x: hit.world.x, z: hit.world.y, height: hit.height }
    : null;
}

/** The published cell whose centre is closest to a scene point. Used only to own a frontier edge. */
function nearestCell(
  build: TerrainBuild,
  x: number,
  z: number,
): TerrainCell | null {
  let best: TerrainCell | null = null;
  let bestDistance = Number.POSITIVE_INFINITY;
  for (const cell of build.cells) {
    const distance = (cell.x - x) ** 2 + (cell.z - z) ** 2;
    if (distance < bestDistance) {
      bestDistance = distance;
      best = cell;
    }
  }
  // Beyond a cell and a half there is nothing this skirt could have hung from, so this was fog.
  return best && bestDistance <= 2.25 ? best : null;
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

/**
 * The height of unsurveyed ground.
 *
 * Nothing is drawn out there, so this only decides where a thing standing over fog is put. Zero is
 * the logical plane, which is where the flat renderer, the pick plane and the camera all still
 * assume the world sits when they have been told nothing else.
 */
export const FOG_HEIGHT = 0;

/**
 * The one route from a hex to the height of the ground drawn on it.
 *
 * Buildings, boundaries, overlays, cargo, the camera and the player all come through here, so
 * nothing floats over a cut or buries itself in a slope by resolving the ground its own way. It is
 * the cell's own published height, not an interpolation: a machine stands on its hex, not on the
 * blended corner the triangle happens to draw beneath its centre.
 *
 * Each caller used to carry its own copy of this lookup and its own fallback, and they had drifted
 * apart — a fixed 0.07 in three places, a re-derived grade in a fourth. With real relief that is
 * how a wall ends up buried in the hillside its gate stands on.
 */
export function heightAt(
  cellByKey: ReadonlyMap<string, TerrainCell>,
  q: number,
  r: number,
): number {
  return cellByKey.get(cellKey(q, r))?.height ?? FOG_HEIGHT;
}

/** {@link heightAt} for a native world point, for the layers that hold one rather than a hex. */
export function heightAtWorld(
  cellByKey: ReadonlyMap<string, TerrainCell>,
  point: WorldPoint,
): number {
  const { q, r } = pixelToAxial(
    { x: point.x / WORLD_SCALE, y: point.y / WORLD_SCALE },
    1,
    { x: 0, y: 0 },
  );
  return heightAt(cellByKey, q, r);
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

function frontierLines(
  cells: readonly TerrainCell[],
  materials: WorldMaterials,
  knownCells: ReadonlyMap<string, TerrainCell> = new Map(
    cells.map((cell) => [cellKey(cell.q, cell.r), cell]),
  ),
): { mesh: LineSegments; geometry: BufferGeometry } {
  const positions: number[] = [];
  for (const cell of cells) {
    for (const direction of ADJACENCY_DIRECTIONS) {
      if (knownCells.has(cellKey(cell.q + direction.q, cell.r + direction.r)))
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
  if (positions.length > 0) geometry.computeBoundingSphere();
  const mesh = new LineSegments(geometry, materials.frontier);
  mesh.name = "survey-frontier";
  mesh.renderOrder = 30;
  return { mesh, geometry };
}

function constructionGridGeometry(
  cells: readonly TerrainCell[],
): BufferGeometry {
  const positions: number[] = [];
  for (const cell of cells) {
    for (let corner = 0; corner < 6; corner += 1) {
      const first = hexCorner({ x: cell.x, y: cell.z }, HEX_RADIUS, corner);
      const second = hexCorner(
        { x: cell.x, y: cell.z },
        HEX_RADIUS,
        corner + 1,
      );
      const height = cell.height + 0.018;
      positions.push(first.x, height, first.y, second.x, height, second.y);
    }
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute("position", new Float32BufferAttribute(positions, 3));
  if (positions.length > 0) geometry.computeBoundingSphere();
  return geometry;
}

function inferChunkSize(snapshot: FactorySnapshot): number {
  if (snapshot.chunks.length === 0) return 1;
  const cellsPerChunk = snapshot.terrain.length / snapshot.chunks.length;
  const size = Math.sqrt(cellsPerChunk);
  // Production snapshots contain every cell of every chunk, so this is exact. Keeping a bounded
  // fallback makes deliberately tiny renderer fixtures useful without inventing a second native
  // chunk-size field on the wire.
  return Number.isInteger(size) && size > 0
    ? size
    : Math.max(1, Math.round(size));
}

function keyedCells<T extends { readonly q: number; readonly r: number }>(
  cells: readonly T[],
): Map<string, T> {
  return new Map(cells.map((cell) => [cellKey(cell.q, cell.r), cell]));
}

function addChangedKeys<T extends { readonly q: number; readonly r: number }>(
  previous: ReadonlyMap<string, T>,
  next: ReadonlyMap<string, T>,
  dirty: Set<string>,
  chunkSize: number,
): void {
  for (const [key, cell] of previous) {
    if (next.get(key) !== cell) dirty.add(ownerKey(cell.q, cell.r, chunkSize));
  }
  for (const [key, cell] of next) {
    if (previous.get(key) !== cell)
      dirty.add(ownerKey(cell.q, cell.r, chunkSize));
  }
}

function haloSamples(
  chunkQ: number,
  chunkR: number,
  chunkSize: number,
  cellByKey: ReadonlyMap<string, TerrainCell>,
): TerrainCell[] {
  const q0 = chunkQ * chunkSize - SAMPLE_HALO;
  const r0 = chunkR * chunkSize - SAMPLE_HALO;
  const q1 = (chunkQ + 1) * chunkSize + SAMPLE_HALO;
  const r1 = (chunkR + 1) * chunkSize + SAMPLE_HALO;
  const samples: TerrainCell[] = [];
  for (let r = r0; r < r1; r += 1) {
    for (let q = q0; q < q1; q += 1) {
      const cell = cellByKey.get(cellKey(q, r));
      if (cell) samples.push(cell);
    }
  }
  return samples;
}

function ownerKey(q: number, r: number, chunkSize: number): string {
  return chunkKey(Math.floor(q / chunkSize), Math.floor(r / chunkSize));
}

function chunkKey(q: number, r: number): string {
  return `${q},${r}`;
}

function parseChunkKey(key: string): [number, number] {
  const comma = key.indexOf(",");
  return [Number(key.slice(0, comma)), Number(key.slice(comma + 1))];
}
