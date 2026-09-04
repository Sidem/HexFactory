import type { GroundAction, GroundEdit } from "../core/types";

export type GroundBrushMode = "grade" | "dig" | "mound" | "surface" | "strip";

/** Which native verb each brush mode commits. */
const ACTIONS: Readonly<Record<GroundBrushMode, GroundAction>> = {
  grade: "smooth",
  dig: "lower",
  mound: "raise",
  surface: "pave",
  strip: "clear",
};

/** The modes whose stamp moves earth by a depth the player chooses. */
export function movesEarth(mode: GroundBrushMode): boolean {
  return mode === "dig" || mode === "mound";
}

/** Modes that can resolve to cut or fill and therefore occupy the player's field-work clock. */
export function takesGroundwork(mode: GroundBrushMode): boolean {
  return mode === "grade" || movesEarth(mode);
}
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

/**
 * One immediate native stamp. The disc is centred under the pointer; grade keeps the stroke datum.
 *
 * `steps` is the chosen depth, and it only reaches native for the two modes that move earth by a
 * depth. Every other verb moves by one fixed thing — a surface is laid or it is not — so passing the
 * tray's number through to them would let a control they ignore look as though it changed something.
 */
export function groundBrushEdit(
  centre: BrushHex,
  datum: BrushHex,
  radius: number,
  mode: GroundBrushMode,
  definitionId: number,
  cover: boolean,
  steps = 1,
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
    action: ACTIONS[mode],
    cover,
    steps: movesEarth(mode) ? Math.max(1, Math.min(3, Math.round(steps))) : 1,
    reference: "first",
  };
}
