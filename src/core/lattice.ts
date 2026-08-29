import type { AxialCoordinate } from "@hexlife/embed/hex";

import type { BoundaryAnchor, BoundarySegment, WorldPoint } from "./types";

/**
 * The hex vertex lattice, in native's own integers.
 *
 * A boundary is a chord between two corners of one hex, and a rectangular selection is anchored on
 * two corners, so the host now has to name vertices rather than only hexes. Every function here is
 * a transcription of the matching one in `boundaries.rs`, using the same integer constants and the
 * same rounding: the vertex the host draws a marker on has to be the vertex native prices, or the
 * preview would quote one line and the commit would build another.
 *
 * `WORLD_SCALE` in the renderer is this same circumradius, but the trigonometric layout it feeds
 * `axialToPixel` is a rounded `sqrt(3)`. Over a hundred thousand hexes that rounding drifts by tens
 * of hexes, which is fine for drawing a scene and not fine for agreeing with native on which corner
 * was clicked — hence the integer arithmetic here.
 */
export const HEX_RADIUS = 1024;
const HEX_X = 1774;
const HEX_Y = 1536;

/**
 * The six corners of a hex as world offsets from its centre. Index 0 is due north, then clockwise,
 * so corners `k + 1` and `k + 2` are the ends of the hex edge in direction `k`.
 */
export const CORNERS: readonly (readonly [number, number])[] = [
  [0, -HEX_RADIUS],
  [HEX_X / 2, -HEX_RADIUS / 2],
  [HEX_X / 2, HEX_RADIUS / 2],
  [0, HEX_RADIUS],
  [-HEX_X / 2, HEX_RADIUS / 2],
  [-HEX_X / 2, -HEX_RADIUS / 2],
];

/** How each corner reads on a compass, for the labels and the spoken selection. */
export const CORNER_NAMES: readonly string[] = [
  "North",
  "Northeast",
  "Southeast",
  "South",
  "Southwest",
  "Northwest",
];

const DIRECTIONS: readonly (readonly [number, number])[] = [
  [1, 0],
  [0, 1],
  [-1, 1],
  [-1, 0],
  [0, -1],
  [1, -1],
];

/** Where a hex centre sits in world units. Native's `axial_world`. */
export function hexCenter(cell: AxialCoordinate): WorldPoint {
  return { x: cell.q * HEX_X + cell.r * (HEX_X / 2), y: cell.r * HEX_Y };
}

/** Where one corner of one hex sits in world units. Native's `corner_world`. */
export function cornerWorld(anchor: BoundaryAnchor): WorldPoint {
  const centre = hexCenter(anchor);
  const offset = CORNERS[((anchor.corner % 6) + 6) % 6]!;
  return { x: centre.x + offset[0]!, y: centre.y + offset[1]! };
}

/** The two corners a chord joins. Native's `chord_corners`. */
export function chordCorners(chord: number): [number, number] {
  if (chord < 6) return [(chord + 1) % 6, (chord + 2) % 6];
  if (chord < 12) return [chord - 6, (chord - 4) % 6];
  return [chord - 12, chord - 9];
}

/** Both ends of a boundary segment in world units. */
export function segmentEnds(
  segment: BoundarySegment,
): [WorldPoint, WorldPoint] {
  const [a, b] = chordCorners(segment.chord);
  return [
    cornerWorld({ q: segment.q, r: segment.r, corner: a }),
    cornerWorld({ q: segment.q, r: segment.r, corner: b }),
  ];
}

/** The three hexes meeting at a vertex. Native's `corner_hexes`. */
export function cornerHexes(anchor: BoundaryAnchor): AxialCoordinate[] {
  const k = ((anchor.corner % 6) + 6) % 6;
  const around: readonly (readonly [number, number])[] = [
    [0, 0],
    DIRECTIONS[(k + 5) % 6]!,
    DIRECTIONS[(k + 4) % 6]!,
  ];
  return around.map((step) => ({
    q: anchor.q + step[0]!,
    r: anchor.r + step[1]!,
  }));
}

/** Native's `div_round`: nearest, ties away from zero, truncating like Rust's integer division. */
function divRound(num: number, den: number): number {
  const half = Math.trunc(den / 2);
  return num >= 0
    ? Math.trunc((num + half) / den)
    : -Math.trunc((-num + half) / den);
}

/** The hex holding a world point. Native's `world_to_axial`, cube rounding and all. */
export function worldToAxial(point: WorldPoint): AxialCoordinate {
  const den = HEX_X * HEX_Y;
  const q = Math.round(point.x) * HEX_Y - Math.round(point.y) * (HEX_X / 2);
  const r = Math.round(point.y) * HEX_X;
  const s = -q - r;
  const [rq, rr, rs] = [divRound(q, den), divRound(r, den), divRound(s, den)];
  const [dq, dr, ds] = [
    Math.abs(rq * den - q),
    Math.abs(rr * den - r),
    Math.abs(rs * den - s),
  ];
  if (dq >= dr && dq >= ds) return { q: -rr - rs, r: rr };
  if (dr >= ds) return { q: rq, r: -rq - rs };
  return { q: rq, r: rr };
}

/**
 * The lattice vertex nearest a world point.
 *
 * Searched over the hex the point falls in *and its six neighbours* rather than that hex alone.
 * Native searches only the containing hex because it is always handed an exact vertex; a pointer is
 * not, and a click just outside a hex should still snap to the corner it is visibly closest to.
 * Which of the three hexes meeting at a vertex ends up naming it makes no difference — native folds
 * the three spellings together before it prices anything.
 */
export function nearestVertex(point: WorldPoint): BoundaryAnchor {
  const home = worldToAxial(point);
  let best: BoundaryAnchor = { ...home, corner: 0 };
  let nearest = Infinity;
  const around: readonly (readonly [number, number])[] = [
    [0, 0],
    ...DIRECTIONS,
  ];
  for (const step of around) {
    for (let corner = 0; corner < 6; corner += 1) {
      const anchor = { q: home.q + step[0]!, r: home.r + step[1]!, corner };
      const at = cornerWorld(anchor);
      const distance = (at.x - point.x) ** 2 + (at.y - point.y) ** 2;
      if (distance < nearest) {
        nearest = distance;
        best = anchor;
      }
    }
  }
  return best;
}

/** Whether two anchors name the same point of the lattice, however they spell it. */
export function sameVertex(
  a: BoundaryAnchor | null,
  b: BoundaryAnchor | null,
): boolean {
  if (!a || !b) return a === b;
  const from = cornerWorld(a);
  const to = cornerWorld(b);
  return from.x === to.x && from.y === to.y;
}

/**
 * How a run between two vertices reads out loud: a bearing and a length, or the word for a heading
 * the lattice cannot draw straight.
 *
 * Twelve headings leave every vertex, one per thirty degrees, and native's chain is exactly straight
 * on all twelve. Off them it staircases, which is worth saying before the player commits to it.
 */
export function headingLabel(
  from: BoundaryAnchor,
  to: BoundaryAnchor,
): { bearing: string; exact: boolean } | null {
  const a = cornerWorld(from);
  const b = cornerWorld(to);
  const [dx, dy] = [b.x - a.x, b.y - a.y];
  if (dx === 0 && dy === 0) return null;
  const degrees = (Math.atan2(dy, dx) * 180) / Math.PI;
  const step = degrees / 30;
  const compass = [
    "east",
    "east-southeast",
    "south-southeast",
    "south",
    "south-southwest",
    "west-southwest",
    "west",
    "west-northwest",
    "north-northwest",
    "north",
    "north-northeast",
    "east-northeast",
  ];
  const index = ((Math.round(step) % 12) + 12) % 12;
  return {
    bearing: compass[index]!,
    exact: Math.abs(step - Math.round(step)) < 1e-6,
  };
}
