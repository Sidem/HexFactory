import type { GroundEdit } from "../core/types";

export type GroundBrushMode = "grade" | "surface" | "strip";
export interface BrushHex {
  q: number;
  r: number;
}

const DIRECTIONS: readonly BrushHex[] = [
  { q: 1, r: 0 },
  { q: 0, r: 1 },
  { q: -1, r: 1 },
  { q: -1, r: 0 },
  { q: 0, r: -1 },
  { q: 1, r: -1 },
];

export function brushDistance(a: BrushHex, b: BrushHex): number {
  const q = a.q - b.q;
  const r = a.r - b.r;
  return Math.max(Math.abs(q), Math.abs(r), Math.abs(q + r));
}

/**
 * How far one pointer event may have travelled and still be a drag. Beyond it the cursor jumped —
 * a zoom, a pan, a pointer that left the window and came back — and the hexes in between were never
 * crossed, so filling them would commit a stripe of ground the player never brushed.
 */
export const MAX_BRUSH_RUN = 12;

/** Fill pointer-event gaps with a deterministic shortest run of adjacent brush centres. */
export function brushLine(from: BrushHex, to: BrushHex): BrushHex[] {
  if (brushDistance(from, to) > MAX_BRUSH_RUN) return [{ ...from }, { ...to }];
  const line = [{ ...from }];
  let current = from;
  while (current.q !== to.q || current.r !== to.r) {
    current = DIRECTIONS.map(({ q, r }) => ({
      q: current.q + q,
      r: current.r + r,
    })).reduce((best, cell) =>
      brushDistance(cell, to) < brushDistance(best, to) ? cell : best,
    );
    line.push(current);
  }
  return line;
}

/** One immediate native stamp. The disc is centred under the pointer; grade keeps the stroke datum. */
export function groundBrushEdit(
  centre: BrushHex,
  datum: BrushHex,
  radius: number,
  mode: GroundBrushMode,
  definitionId: number,
  cover: boolean,
): GroundEdit {
  const boundedRadius = Math.max(0, Math.min(2, Math.round(radius)));
  return {
    q: centre.q,
    r: centre.r,
    to_q: centre.q + boundedRadius,
    to_r: centre.r,
    datum: mode === "grade" ? [datum.q, datum.r] : undefined,
    corner: 0,
    to_corner: 0,
    shape: boundedRadius === 0 ? "cell" : "disc",
    definition_id: definitionId,
    action: mode === "grade" ? "smooth" : mode === "surface" ? "pave" : "clear",
    cover,
    steps: 1,
    reference: "first",
  };
}
