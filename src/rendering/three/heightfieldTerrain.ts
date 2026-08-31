import {
  BufferGeometry,
  Float32BufferAttribute,
  Vector2,
  type Camera,
  type Intersection,
  type Object3D,
  type Raycaster,
} from "three";
import {
  axialToPixel,
  pixelToAxial,
  type AxialCoordinate,
} from "@hexlife/embed/hex";

import type { WorldPoint } from "../../core/types";

const DIRECTIONS: readonly AxialCoordinate[] = [
  { q: 1, r: 0 },
  { q: 0, r: 1 },
  { q: -1, r: 1 },
  { q: -1, r: 0 },
  { q: 0, r: -1 },
  { q: 1, r: -1 },
];

const SUBSTRATE_CODE: Readonly<Record<HeightfieldSubstrate, number>> = {
  sand: 0,
  meadow: 1,
  soil: 2,
  rock: 3,
};

export type HeightfieldSubstrate = "sand" | "meadow" | "soil" | "rock";

/**
 * One native-published sample, converted from height quanta to scene height at the rendering
 * boundary. The renderer may interpolate these values into triangles; it never samples or invents
 * another elevation oracle.
 */
export interface HeightfieldSample extends AxialCoordinate {
  readonly height: number;
  readonly substrate: HeightfieldSubstrate;
  readonly waterDepth: number;
  readonly waterHeight: number;
  readonly dischargeClass: number;
}

export interface HeightfieldOptions {
  /** Height difference that becomes a vertical face instead of an ordinary blended slope. */
  readonly cliffThreshold: number;
  /** How far the survey frontier drops below its last published edge. */
  readonly frontierDepth?: number;
}

export interface HeightfieldGeometryBuild {
  readonly ground: BufferGeometry;
  readonly water: BufferGeometry;
  readonly cliffs: BufferGeometry;
  readonly frontier: BufferGeometry;
  readonly cells: readonly HeightfieldSample[];
  readonly cellByKey: ReadonlyMap<string, HeightfieldSample>;
}

export interface HeightfieldPick {
  readonly axial: AxialCoordinate;
  readonly world: WorldPoint;
  readonly height: number;
}

export interface ViewportRect {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

interface Point3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

/**
 * Build a continuous top surface, a separate water surface, and only the vertical faces the
 * sampled ground actually needs. Ordinary neighbours share averaged corner vertices, so they
 * become one slope instead of two isolated prisms. A discontinuity keeps each side's own corner
 * height and receives a skirt; the unsurveyed frontier receives the same treatment downward.
 */
export function buildHeightfieldGeometry(
  samples: readonly HeightfieldSample[],
  options: HeightfieldOptions,
): HeightfieldGeometryBuild {
  if (!Number.isFinite(options.cliffThreshold) || options.cliffThreshold < 0)
    throw new Error(
      "Heightfield cliff threshold must be finite and non-negative",
    );
  const frontierDepth = options.frontierDepth ?? 1;
  if (!Number.isFinite(frontierDepth) || frontierDepth <= 0)
    throw new Error("Heightfield frontier depth must be finite and positive");

  const cells = [...samples].sort((a, b) => a.r - b.r || a.q - b.q);
  const cellByKey = new Map<string, HeightfieldSample>();
  for (const sample of cells) {
    if (!validSample(sample))
      throw new Error(`Invalid heightfield sample at ${sample.q},${sample.r}`);
    const key = cellKey(sample.q, sample.r);
    if (cellByKey.has(key))
      throw new Error(
        `Duplicate heightfield sample at ${sample.q},${sample.r}`,
      );
    cellByKey.set(key, sample);
  }

  const ground = new GeometryWriter("substrate");
  const water = new GeometryWriter("discharge");
  const cliffs = new GeometryWriter("substrate");
  const frontier = new GeometryWriter("substrate");

  for (const cell of cells) {
    const substrate = SUBSTRATE_CODE[cell.substrate];
    const centre = cellCentre(cell, cell.height);
    for (let corner = 0; corner < DIRECTIONS.length; corner += 1) {
      const first = groundCorner(
        cell,
        corner,
        cellByKey,
        options.cliffThreshold,
      );
      const second = groundCorner(
        cell,
        corner + 1,
        cellByKey,
        options.cliffThreshold,
      );
      // Winding is counter-clockwise when viewed from above, so generated normals face +Y.
      ground.triangle(centre, second, first, substrate);

      if (cell.waterDepth > 0) {
        const waterCentre = cellCentre(cell, cell.waterHeight);
        const waterFirst = waterCorner(cell, corner, cellByKey);
        const waterSecond = waterCorner(cell, corner + 1, cellByKey);
        water.triangle(
          waterCentre,
          waterSecond,
          waterFirst,
          cell.dischargeClass,
        );
      }
    }

    for (let edge = 0; edge < DIRECTIONS.length; edge += 1) {
      const direction = DIRECTIONS[edge]!;
      const neighbour = cellByKey.get(
        cellKey(cell.q + direction.q, cell.r + direction.r),
      );
      const first = groundCorner(cell, edge, cellByKey, options.cliffThreshold);
      const second = groundCorner(
        cell,
        edge + 1,
        cellByKey,
        options.cliffThreshold,
      );
      if (!neighbour) {
        frontier.quad(
          first,
          second,
          { ...second, y: second.y - frontierDepth },
          { ...first, y: first.y - frontierDepth },
          substrate,
        );
        continue;
      }
      if (
        compareCells(cell, neighbour) >= 0 ||
        Math.abs(cell.height - neighbour.height) <= options.cliffThreshold
      )
        continue;

      // The opposite edge runs in the other direction. Reuse this edge's x/z positions while
      // taking the neighbour's independently resolved heights, which closes the face at both ends.
      const opposite = (edge + 3) % DIRECTIONS.length;
      const neighbourSecond = groundCorner(
        neighbour,
        opposite,
        cellByKey,
        options.cliffThreshold,
      );
      const neighbourFirst = groundCorner(
        neighbour,
        opposite + 1,
        cellByKey,
        options.cliffThreshold,
      );
      cliffs.quad(
        first,
        second,
        { ...second, y: neighbourSecond.y },
        { ...first, y: neighbourFirst.y },
        substrate,
      );
    }
  }

  return {
    ground: ground.finish(),
    water: water.finish(),
    cliffs: cliffs.finish(),
    frontier: frontier.finish(),
    cells,
    cellByKey,
  };
}

/** Native's exact cell-centre answer used by buildings, overlays and the player. */
export function heightfieldHeightAt(
  build: HeightfieldGeometryBuild,
  q: number,
  r: number,
): number | undefined {
  return build.cellByKey.get(cellKey(q, r))?.height;
}

/**
 * Resolve a ray against the visible native-derived surface, then translate its x/z point back to
 * the logical axial cell. Native still decides legality; this only replaces the obsolete flat
 * plane used to decide what the player pointed at.
 */
export function pickHeightfieldRay(
  raycaster: Raycaster,
  surface: Object3D,
): HeightfieldPick | null {
  const hit = raycaster.intersectObject(surface, true)[0];
  return hit ? pickFromIntersection(hit) : null;
}

/** Browser-coordinate wrapper around {@link pickHeightfieldRay}. */
export function pickHeightfieldAt(
  raycaster: Raycaster,
  camera: Camera,
  surface: Object3D,
  viewport: ViewportRect,
  clientX: number,
  clientY: number,
): HeightfieldPick | null {
  if (viewport.width <= 0 || viewport.height <= 0) return null;
  const pointer = new Vector2(
    ((clientX - viewport.left) / viewport.width) * 2 - 1,
    -((clientY - viewport.top) / viewport.height) * 2 + 1,
  );
  raycaster.setFromCamera(pointer, camera);
  return pickHeightfieldRay(raycaster, surface);
}

function pickFromIntersection(hit: Intersection<Object3D>): HeightfieldPick {
  const axial = pixelToAxial({ x: hit.point.x, y: hit.point.z }, 1);
  return {
    axial,
    world: { x: hit.point.x, y: hit.point.z },
    height: hit.point.y,
  };
}

function validSample(sample: HeightfieldSample): boolean {
  return (
    Number.isInteger(sample.q) &&
    Number.isInteger(sample.r) &&
    Number.isFinite(sample.height) &&
    Number.isInteger(sample.waterDepth) &&
    sample.waterDepth >= 0 &&
    Number.isFinite(sample.waterHeight) &&
    (sample.waterDepth === 0 || sample.waterHeight >= sample.height) &&
    Number.isInteger(sample.dischargeClass) &&
    sample.dischargeClass >= 0 &&
    sample.dischargeClass <= 255 &&
    Object.hasOwn(SUBSTRATE_CODE, sample.substrate)
  );
}

function compareCells(
  left: HeightfieldSample,
  right: HeightfieldSample,
): number {
  return left.r - right.r || left.q - right.q;
}

function cellKey(q: number, r: number): string {
  return `${q},${r}`;
}

function cellCentre(cell: HeightfieldSample, height: number): Point3 {
  const point = axialToPixel(cell, 1, { x: 0, y: 0 });
  return { x: point.x, y: height, z: point.y };
}

function groundCorner(
  cell: HeightfieldSample,
  rawCorner: number,
  cellByKey: ReadonlyMap<string, HeightfieldSample>,
  cliffThreshold: number,
): Point3 {
  const corner = modulo(rawCorner, DIRECTIONS.length);
  const previous = DIRECTIONS[modulo(corner - 1, DIRECTIONS.length)]!;
  const next = DIRECTIONS[corner]!;
  const candidates = [cell];
  for (const direction of [previous, next]) {
    const neighbour = cellByKey.get(
      cellKey(cell.q + direction.q, cell.r + direction.r),
    );
    if (neighbour) candidates.push(neighbour);
  }
  // The three cells touching a vertex form a tiny adjacency graph. Use the whole ordinary-slope
  // component containing this cell, not only neighbours directly within its threshold: A can join
  // B and B can join C at the end of an A/C cliff. Transitive closure gives all three callers the
  // same endpoint, so the sloped edges close and the vertical face naturally tapers to that point.
  const connected = new Set([0]);
  let changed = true;
  while (changed) {
    changed = false;
    for (let candidate = 1; candidate < candidates.length; candidate += 1) {
      if (connected.has(candidate)) continue;
      if (
        [...connected].some(
          (index) =>
            Math.abs(
              candidates[index]!.height - candidates[candidate]!.height,
            ) <= cliffThreshold,
        )
      ) {
        connected.add(candidate);
        changed = true;
      }
    }
  }
  return cornerPoint(
    cell,
    corner,
    mean([...connected].map((index) => candidates[index]!.height)),
  );
}

function waterCorner(
  cell: HeightfieldSample,
  rawCorner: number,
  cellByKey: ReadonlyMap<string, HeightfieldSample>,
): Point3 {
  const corner = modulo(rawCorner, DIRECTIONS.length);
  const previous = DIRECTIONS[modulo(corner - 1, DIRECTIONS.length)]!;
  const next = DIRECTIONS[corner]!;
  const heights = [cell.waterHeight];
  for (const direction of [previous, next]) {
    const neighbour = cellByKey.get(
      cellKey(cell.q + direction.q, cell.r + direction.r),
    );
    if (neighbour && neighbour.waterDepth > 0)
      heights.push(neighbour.waterHeight);
  }
  return cornerPoint(cell, corner, mean(heights));
}

function cornerPoint(
  cell: HeightfieldSample,
  corner: number,
  height: number,
): Point3 {
  const centre = cellCentre(cell, height);
  const angle = -Math.PI / 6 + corner * (Math.PI / 3);
  return {
    x: centre.x + Math.cos(angle),
    y: height,
    z: centre.z + Math.sin(angle),
  };
}

function mean(values: readonly number[]): number {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function modulo(value: number, divisor: number): number {
  return ((value % divisor) + divisor) % divisor;
}

class GeometryWriter {
  readonly #positions: number[] = [];
  readonly #channels: number[] = [];
  readonly #indices: number[] = [];
  readonly #vertices = new Map<string, number>();

  constructor(private readonly channelName: string) {}

  triangle(
    first: Point3,
    second: Point3,
    third: Point3,
    channel: number,
  ): void {
    this.#indices.push(
      this.vertex(first, channel),
      this.vertex(second, channel),
      this.vertex(third, channel),
    );
  }

  quad(
    first: Point3,
    second: Point3,
    third: Point3,
    fourth: Point3,
    channel: number,
  ): void {
    this.triangle(first, second, third, channel);
    this.triangle(first, third, fourth, channel);
  }

  finish(): BufferGeometry {
    const geometry = new BufferGeometry();
    geometry.setAttribute(
      "position",
      new Float32BufferAttribute(this.#positions, 3),
    );
    geometry.setAttribute(
      this.channelName,
      new Float32BufferAttribute(this.#channels, 1),
    );
    geometry.setIndex(this.#indices);
    if (this.#indices.length > 0) {
      geometry.computeVertexNormals();
      geometry.computeBoundingBox();
      geometry.computeBoundingSphere();
    }
    return geometry;
  }

  private vertex(point: Point3, channel: number): number {
    const key = `${fixed(point.x)},${fixed(point.y)},${fixed(point.z)},${channel}`;
    const existing = this.#vertices.get(key);
    if (existing !== undefined) return existing;
    const index = this.#positions.length / 3;
    this.#vertices.set(key, index);
    this.#positions.push(point.x, point.y, point.z);
    this.#channels.push(channel);
    return index;
  }
}

function fixed(value: number): string {
  return value.toFixed(8);
}
