import { axialToPixel, type PixelPoint } from "@hexlife/embed/hex";

import type {
  BuildingDefinition,
  BuildingKind,
  EntitySnapshot,
  PowerSource,
} from "../core/types";
import { CORNER_START, TRANSPORT_DIRECTIONS } from "../core/directions";
import { hexPath } from "./hexDraw";
import {
  applyLadder,
  applyTier,
  drawParts,
  HUB_LADDER,
  isStill,
  type ShapePart,
} from "./shapeGrammar";

export type SilhouetteKey =
  | BuildingKind
  | "assembly"
  | "smelting"
  | "firing"
  | "cutting"
  | "crushing"
  | PowerSource;

export interface BuildingTrim {
  stroke: string;
  width: number;
}

/**
 * Changing this constant regenerates every baked building shape, the same way
 * `TERRAIN_TILE_VERSION` regenerates the terrain set. The bakes are presentation, so the version
 * is not a save, definition, generator, or wire number.
 */
export const BUILDING_SHAPE_VERSION = 1;

/**
 * Look from the definition, not from a drawing per id. Composer-kind machines split on
 * `recipe_category`; generators split on `power_source`. A new tier must not need a new function.
 */
export function silhouetteOf(
  kind: BuildingKind,
  recipeCategory?: string | null,
  powerSource?: PowerSource,
): SilhouetteKey {
  if (kind === "composer") {
    if (recipeCategory === "smelting") return "smelting";
    if (recipeCategory === "firing") return "firing";
    if (recipeCategory === "cutting") return "cutting";
    if (recipeCategory === "crushing") return "crushing";
    return "assembly";
  }
  if (kind === "generator") return powerSource ?? "burner";
  return kind;
}

/**
 * Trim carries `tier` as colour and weight. It is no longer what makes a tier legible — the part
 * list is, through `TIER_LADDER` — so this is the accent on a machine that has already changed
 * shape rather than the whole of the upgrade.
 */
export function trimOf(tier = 0): BuildingTrim {
  if (tier <= 0) return { stroke: "#dce7ef", width: 1.4 };
  if (tier === 1) return { stroke: "#e2c15a", width: 1.85 };
  return { stroke: "#8fd4ff", width: 2.2 };
}

const FURNACE = "#ff9440";
const HEARTH = "#ff6030";
const STEAM = "#dfeaf2";
const WATER = "#7fd8ff";
const EXHAUST = "#b4dcff";

/**
 * The base part list per silhouette. This is the data row a new building costs, and it is the
 * whole of what used to be a two-hundred-line `switch`: the table is total over `SilhouetteKey`,
 * so a new key is a compile error here rather than a machine that silently draws nothing.
 *
 * `belt` is deliberately empty in the machine-part grammar. Shared transport geometry owns its
 * raised rails and transverse treads; the heading tick and cargo remain separate state cues.
 */
export const BUILDING_SHAPES: Record<SilhouetteKey, readonly ShapePart[]> = {
  extractor: [
    { part: "vessel", x: 0, y: -0.06, scale: 0.19 },
    {
      part: "stack",
      x: 0,
      y: 0.06,
      scale: 0.11,
      rotation: Math.PI,
      phase: "rise",
    },
  ],
  belt: [],
  composer: [
    { part: "chamber", x: 0, y: 0, scale: 0.2 },
    { part: "rotor", x: 0, y: 0, scale: 0.12, count: 1, phase: "spin" },
  ],
  assembly: [
    { part: "chamber", x: 0, y: 0, scale: 0.2 },
    { part: "rotor", x: 0, y: 0, scale: 0.12, count: 1, phase: "spin" },
  ],
  smelting: [
    { part: "vessel", x: 0, y: 0.05, scale: 0.2 },
    {
      part: "aperture",
      x: 0,
      y: 0.06,
      scale: 0.19,
      phase: "pulse",
      glow: FURNACE,
    },
    { part: "stack", x: 0, y: -0.13, scale: 0.085 },
  ],
  firing: [
    { part: "vessel", x: 0, y: 0.08, scale: 0.26 },
    {
      part: "aperture",
      x: 0,
      y: 0.1,
      scale: 0.13,
      phase: "pulse",
      glow: HEARTH,
    },
  ],
  cutting: [
    { part: "rotor", x: 0, y: -0.02, scale: 0.21, count: 1, phase: "spin" },
    { part: "band", x: 0, y: 0.24, scale: 0.2, count: 2 },
  ],
  crushing: [{ part: "mouth", x: 0, y: 0, scale: 0.26, phase: "grind" }],
  container: [
    { part: "vessel", x: 0, y: 0, scale: 0.22 },
    { part: "band", x: 0, y: -0.04, scale: 0.2, count: 2 },
  ],
  consumer: [{ part: "mouth", x: 0, y: 0, scale: 0.2, rotation: -Math.PI / 2 }],
  hub: [
    { part: "vessel", x: 0, y: 0.02, scale: 0.3 },
    { part: "mast", x: 0, y: -0.14, scale: 0.14 },
    { part: "band", x: 0, y: 0.16, scale: 0.26, count: 3 },
  ],
  pump: [
    { part: "vessel", x: 0, y: -0.05, scale: 0.16 },
    { part: "stack", x: 0, y: 0.08, scale: 0.1, rotation: Math.PI },
    { part: "aperture", x: 0, y: 0.2, scale: 0.06, phase: "rise", glow: WATER },
  ],
  pole: [{ part: "mast", x: 0, y: 0.1, scale: 0.68 }],
  generator: [{ part: "chamber", x: 0, y: 0, scale: 0.18 }],
  burner: [
    { part: "vessel", x: 0, y: 0.08, scale: 0.18 },
    { part: "stack", x: 0, y: -0.05, scale: 0.09 },
    {
      part: "aperture",
      x: 0,
      y: -0.2,
      scale: 0.075,
      phase: "pulse",
      glow: FURNACE,
    },
  ],
  wind: [
    { part: "mast", x: 0, y: 0.14, scale: 0.19 },
    { part: "rotor", x: 0, y: -0.08, scale: 0.27, count: 3, phase: "spin" },
  ],
  hydro: [{ part: "rotor", x: 0, y: 0, scale: 0.22, count: 4, phase: "spin" }],
  turbine: [
    { part: "rotor", x: 0, y: 0.02, scale: 0.19, count: 6, phase: "spin" },
    { part: "stack", x: 0, y: -0.16, scale: 0.085 },
    {
      part: "aperture",
      x: 0,
      y: -0.3,
      scale: 0.05,
      phase: "rise",
      glow: EXHAUST,
    },
  ],
  boiler: [
    { part: "vessel", x: 0, y: 0.02, scale: 0.24 },
    {
      part: "aperture",
      x: -0.08,
      y: -0.24,
      scale: 0.055,
      phase: "rise",
      glow: STEAM,
    },
    {
      part: "aperture",
      x: 0.1,
      y: -0.3,
      scale: 0.042,
      phase: "rise",
      glow: STEAM,
    },
  ],
  bridge: [
    { part: "band", x: 0, y: -0.18, scale: 0.3, count: 4 },
    { part: "band", x: 0, y: 0, scale: 0.32, count: 4 },
    { part: "band", x: 0, y: 0.18, scale: 0.3, count: 4 },
  ],
};

/**
 * The player, in the same vocabulary the machines use, so the world reads as one visual system.
 * Drawn in two passes because the reading is a fill inside a dark hull: the awareness ring first,
 * then the body. Scales are in player radii rather than hex sizes — the walker takes whatever
 * unit its caller works in.
 */
export const PLAYER_RING: readonly ShapePart[] = [
  { part: "rotor", x: 0, y: 0, scale: 1, count: 0 },
];
export const PLAYER_BODY: readonly ShapePart[] = [
  { part: "aperture", x: 0, y: 0, scale: 0.62, glow: "#f4f7f2" },
  { part: "rotor", x: 0, y: 0, scale: 0.62, count: 0 },
];

/**
 * The shape a definition draws at: its base list, wearing every tier step it has earned, and then
 * every contract stage the hub has finished.
 *
 * Growth is only ever non-zero for the landing hub, and it is the same kind of thing a tier is —
 * a modifier on a part list — so it goes through the same walker rather than a second one.
 */
export function partsFor(
  key: SilhouetteKey,
  tier = 0,
  growth = 0,
): readonly ShapePart[] {
  return applyLadder(applyTier(BUILDING_SHAPES[key], tier), HUB_LADDER, growth);
}

/* --------------------------------------------------------------- baked stills */

/**
 * Baked at a hex size larger than the camera can reach (`BASE_HEX_SIZE` 22 x max zoom 2.2 x a
 * 2x display is 96.8 device pixels), so a stamp is always scaled down and never up.
 */
const BAKE_HEX = 128;
const BAKE_HALF = 118;

const bakes = new Map<string, HTMLCanvasElement>();

/**
 * The still parts of a shape, drawn once into an offscreen canvas — ART.md rule 3, applied to
 * buildings. What is left to draw per entity per frame is only the parts that actually move, so
 * the grammar's indirection is paid at startup instead of at 60 Hz.
 */
/** Offscreen stamp of the still parts. The WebGL atlas uploads these; Canvas 2D draws them too. */
export function buildingStamp(
  key: SilhouetteKey,
  tier: number,
  growth: number,
): HTMLCanvasElement {
  return bakedStills(key, tier, growth);
}

function bakedStills(
  key: SilhouetteKey,
  tier: number,
  growth: number,
): HTMLCanvasElement {
  const cacheKey = `${BUILDING_SHAPE_VERSION}|${key}|${tier}|${growth}`;
  const cached = bakes.get(cacheKey);
  if (cached) return cached;
  const canvas = document.createElement("canvas");
  canvas.width = BAKE_HALF * 2;
  canvas.height = BAKE_HALF * 2;
  const ctx = canvas.getContext("2d");
  if (ctx) {
    const still = partsFor(key, tier, growth).filter(isStill);
    drawParts(
      ctx,
      still,
      { x: BAKE_HALF, y: BAKE_HALF },
      BAKE_HEX,
      trimOf(tier).stroke,
      0,
    );
  }
  bakes.set(cacheKey, canvas);
  return canvas;
}

export interface BuildingLookInput {
  building: EntitySnapshot;
  definition?: BuildingDefinition;
  center: PixelPoint;
  size: number;
  color: string;
  now: number;
  reducedMotion: boolean;
  /** The definition's tier. Absent is the base tier, which is what every v0.13 building was. */
  tier?: number;
  /** Completed contract stages, for the landing hub. Absent is the hub as it was landed. */
  growth?: number;
}

export function drawBuildingLook(
  ctx: CanvasRenderingContext2D,
  input: BuildingLookInput,
): void {
  const { building, definition, center, size, color, now, reducedMotion } =
    input;
  const tier = input.tier ?? 0;
  const growth = input.growth ?? 0;
  const trim = trimOf(tier);
  const key = silhouetteOf(
    building.kind,
    definition?.recipe_category,
    definition?.power_source,
  );
  const cycle = workCycle(building, now, reducedMotion);
  ctx.save();
  if (building.status === "no power" || building.status === "brownout")
    ctx.globalAlpha = 0.72;
  hexPath(ctx, center, size * 0.8);
  ctx.fillStyle = color;
  ctx.fill();
  ctx.strokeStyle = trim.stroke;
  ctx.lineWidth = trim.width;
  ctx.stroke();
  drawShape(ctx, key, center, size, tier, cycle, growth);
  ctx.restore();
}

/**
 * Stamp the stills, then walk only what moves. Both halves come from the same part list, so a
 * part cannot appear in one and not the other.
 */
function drawShape(
  ctx: CanvasRenderingContext2D,
  key: SilhouetteKey,
  center: PixelPoint,
  size: number,
  tier: number,
  cycle: number,
  growth: number,
): void {
  const parts = partsFor(key, tier, growth);
  if (parts.length === 0) return;
  const dim = BAKE_HALF * 2 * (size / BAKE_HEX);
  ctx.drawImage(
    bakedStills(key, tier, growth),
    center.x - dim / 2,
    center.y - dim / 2,
    dim,
    dim,
  );
  drawParts(
    ctx,
    parts.filter((part) => !isStill(part)),
    center,
    size,
    trimOf(tier).stroke,
    cycle,
  );
}

/**
 * What a stalled machine is stalled *by*, as a colour.
 *
 * A machine that is working and a machine that has been starved for ten minutes drew identically:
 * both were a shape with a stamp on it, and the only way to tell them apart was to click one. The
 * status string was already published and already exact, so this is a table over it rather than a
 * new rule — and it splits by cause, because "feed me" and "I am blocked downstream" are different
 * problems and a player fixes them in different places.
 *
 * `no power` and `brownout` are deliberately absent: they already read as a dimmed machine, and a
 * second mark for the same cause would be noise.
 *
 * `switched off` is the one mark that is not a problem. It is here for the same reason as the rest —
 * a stopped machine that drew like a running one is what the table exists to fix — and it takes the
 * grey the other "nothing is wrong, there is simply nothing to do" causes take, so a field of
 * deliberately-idle machines does not read as a factory on fire.
 */
export const STALL_MARKS: Record<string, string> = {
  "waiting for inputs": "#f5d572",
  "out of fuel": "#ff9440",
  "no boiler": "#ff9440",
  "output blocked": "#ff6b5e",
  "deposit depleted": "#9aa7a2",
  "no water in reach": "#9aa7a2",
  "switched off": "#9aa7a2",
};

export function stallMark(status: string): string | undefined {
  return STALL_MARKS[status];
}

export function cargoTravel(
  elapsed: number,
  duration: number,
  reducedMotion: boolean,
  blocked: boolean,
): number {
  if (blocked || reducedMotion) return 1;
  return Math.max(0, Math.min(1, elapsed / Math.max(1, duration)));
}

export function workCycle(
  building: EntitySnapshot,
  now: number,
  reducedMotion: boolean,
): number {
  if (building.progress_total > 0 && building.progress > 0)
    return building.progress / building.progress_total;
  if (reducedMotion) return 0;
  if (
    building.status === "extracting" ||
    building.status === "generating" ||
    building.status === "pumping"
  )
    return (now / 700 + building.id * 0.13) % 1;
  return 0;
}

/** The first orientation index off the six-edge table. Matches `NORTH` in the core. */
export const NORTH = CORNER_START;
/**
 * A two-row step spans `3 · size` of world distance against `√3 · size` for a unit step, so a
 * vertical heading drawn at the edge scale would reach √3 times as far. The tip is a heading
 * indicator, not a measurement, so it is shortened to read the same length as the other six.
 */
const VERTICAL_TIP_SCALE = 1 / Math.sqrt(3);

/**
 * Where a building's heading points, in pixels.
 *
 * Every routing vector comes from the cross-language fixture. The package owns the conversion to
 * pixels; the fixture owns the six edge and six corner indices native sends.
 */
export function facingTip(
  center: PixelPoint,
  size: number,
  orientation: number,
  reach = 0.39,
): PixelPoint {
  const direction =
    TRANSPORT_DIRECTIONS[orientation] ?? TRANSPORT_DIRECTIONS[0]!;
  return axialToPixel(
    direction,
    size * reach * (orientation >= NORTH ? VERTICAL_TIP_SCALE : 1),
    center,
  );
}

/**
 * The far end of a riser's span: the centre of the hex two rows away that it actually reaches.
 * Drawn as the gantry, so the seam it crosses reads as a short bridge over the crack between two
 * hexes rather than as a tile that is half of something.
 */
export function spanEnd(
  center: PixelPoint,
  size: number,
  orientation: number,
): PixelPoint {
  return axialToPixel(
    TRANSPORT_DIRECTIONS[orientation] ?? TRANSPORT_DIRECTIONS[NORTH]!,
    size,
    center,
  );
}
