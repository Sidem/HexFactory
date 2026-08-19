import type { PixelPoint } from "@hexlife/embed/hex";

/**
 * The part vocabulary — Stage D rule 6, the half of "a look is derived from the definition" that
 * Stage B left imperative.
 *
 * The names are machine anatomy rather than geometry on purpose. A part list is a description of a
 * machine, so it survives a change of renderer: "a vessel with two stacks and a four-blade rotor"
 * is something a mesh generator could consume, while "two rounded rects and a trapezoid" is a
 * sequence of canvas calls that only a canvas can read. The 2D walker below is one consumer of the
 * grammar, not the grammar itself.
 */
export type PartKind =
  | "vessel"
  | "chamber"
  | "stack"
  | "rotor"
  | "aperture"
  | "mast"
  | "band"
  | "mouth";

/**
 * How the work cycle moves a part. Motion is a property of the part, which is what keeps Stage C
 * inside the grammar instead of beside it: a rotor turns because it is a rotor with a `spin`
 * phase, not because a switch arm reached for `Math.cos`.
 */
export type PartPhase = "still" | "spin" | "pulse" | "rise" | "grind";

export interface ShapePart {
  readonly part: PartKind;
  /** Anchor, in hex sizes from the hex centre. */
  readonly x: number;
  readonly y: number;
  /** Half-extent, in hex sizes. */
  readonly scale: number;
  /** Static rotation, in radians. A stack at `PI` is a shaft driven into the ground. */
  readonly rotation?: number;
  /** Absent is `still`. */
  readonly phase?: PartPhase;
  /** Spokes for a rotor, rivets for a band. */
  readonly count?: number;
  /** Emissive colour, for an aperture. Every other part draws in the trim it is given. */
  readonly glow?: string;
}

const TAU = Math.PI * 2;

export function isStill(part: ShapePart): boolean {
  return part.phase === undefined || part.phase === "still";
}

/* ------------------------------------------------------------------ extents */

export interface PartExtent {
  top: number;
  bottom: number;
  left: number;
  right: number;
}

/**
 * The outline a part contributes, in hex sizes. Used to place a tier's stack above whatever the
 * base shape already reaches, and by the tests that check a tier is legible as a silhouette.
 */
export function partExtent(part: ShapePart): PartExtent {
  const { x, y, scale } = part;
  switch (part.part) {
    case "vessel":
      return {
        top: y - scale * 0.75,
        bottom: y + scale * 0.75,
        left: x - scale,
        right: x + scale,
      };
    case "chamber":
      return {
        top: y - scale,
        bottom: y + scale,
        left: x - scale,
        right: x + scale,
      };
    case "stack": {
      // A stack rises from its anchor along its own rotation. Only the upright case lifts the
      // profile; a shaft at PI reaches down instead, which is what an extractor's drill is.
      const reach = scale * 2;
      const angle = part.rotation ?? 0;
      const tipX = x + Math.sin(angle) * reach;
      const tipY = y - Math.cos(angle) * reach;
      return {
        top: Math.min(y, tipY) - scale * 0.2,
        bottom: Math.max(y, tipY) + scale * 0.2,
        left: Math.min(x, tipX) - scale * 0.7,
        right: Math.max(x, tipX) + scale * 0.7,
      };
    }
    case "rotor":
      return {
        top: y - scale,
        bottom: y + scale,
        left: x - scale,
        right: x + scale,
      };
    case "aperture":
      return {
        top: y - scale,
        bottom: y + scale,
        left: x - scale,
        right: x + scale,
      };
    case "mast":
      return {
        top: y - scale * 2,
        bottom: y + scale * 0.4,
        left: x - scale * 0.6,
        right: x + scale * 0.6,
      };
    case "band":
      return {
        top: y - scale * 0.14,
        bottom: y + scale * 0.14,
        left: x - scale,
        right: x + scale,
      };
    case "mouth":
      return {
        top: y - scale * 0.7,
        bottom: y + scale * 0.85,
        left: x - scale,
        right: x + scale,
      };
  }
}

/** The highest point the whole list reaches. Empty lists have no profile and return 0. */
export function profileTop(parts: readonly ShapePart[]): number {
  let top = 0;
  for (const part of parts) top = Math.min(top, partExtent(part).top);
  return top;
}

/** The widest half-span the whole list reaches. */
export function profileWidth(parts: readonly ShapePart[]): number {
  let width = 0;
  for (const part of parts) {
    const extent = partExtent(part);
    width = Math.max(width, Math.abs(extent.left), Math.abs(extent.right));
  }
  return width;
}

/**
 * The shape alone, with colour removed. `glow` is deliberately excluded, because the acceptance
 * this milestone was written against is that a tier reads as a different machine at normal zoom
 * *with colour removed* — a signature that included the tint could pass while the outline did not
 * move, which is exactly the defect v0.14 shipped.
 */
export function silhouetteSignature(parts: readonly ShapePart[]): string {
  return parts
    .map((part) =>
      [
        part.part,
        part.x.toFixed(3),
        part.y.toFixed(3),
        part.scale.toFixed(3),
        (part.rotation ?? 0).toFixed(3),
        part.count ?? 0,
        part.phase ?? "still",
      ].join(":"),
    )
    .join("|");
}

/* ---------------------------------------------------------------- modifiers */

/**
 * The named modifier set. A tier is a modifier on a part list, so an upgrade costs a row in
 * `TIER_LADDER` rather than a drawing.
 */
export type ModifierName =
  | "addStack"
  | "addRotorBlade"
  | "segmentVessel"
  | "platingBand"
  | "widenMouth"
  | "raiseMast";

type Modifier = (parts: readonly ShapePart[]) => ShapePart[];

/**
 * Adds a vent above everything the shape already reaches. This is the one modifier that is
 * unconditional — every other one needs a part of a particular kind to act on, and a tier step
 * that found no target would be a tier the map cannot show. Anchoring off `profileTop` is what
 * makes it lift the outline of any non-empty shape rather than only of the ones with a body.
 */
const addStack: Modifier = (parts) => [
  ...parts,
  { part: "stack", x: 0.17, y: profileTop(parts) + 0.06, scale: 0.085 },
];

const addRotorBlade: Modifier = (parts) =>
  parts.map((part) =>
    part.part === "rotor"
      ? { ...part, count: (part.count ?? 0) + 1, scale: part.scale * 1.06 }
      : { ...part },
  );

const segmentVessel: Modifier = (parts) => {
  const grown = parts.map((part) =>
    part.part === "vessel"
      ? { ...part, scale: part.scale * 1.08 }
      : { ...part },
  );
  const seams = grown
    .filter((part) => part.part === "vessel")
    .map(
      (vessel): ShapePart => ({
        part: "band",
        x: vessel.x,
        y: vessel.y,
        scale: vessel.scale * 0.92,
        count: 3,
      }),
    );
  return [...grown, ...seams];
};

const platingBand: Modifier = (parts) => [
  ...parts.map((part) => ({ ...part })),
  {
    part: "band",
    x: 0,
    y: 0.19,
    scale: Math.max(0.18, profileWidth(parts) * 0.86),
    count: 2,
  },
];

const widenMouth: Modifier = (parts) =>
  parts.map((part) =>
    part.part === "mouth" ? { ...part, scale: part.scale * 1.22 } : { ...part },
  );

/**
 * Grows what a shape already reaches upward, and puts a second one beside it. A structure that has
 * been built onto reads as taller and busier at the top, which is the one difference legible at
 * ordinary play zoom without changing the footprint the world is drawn on.
 */
const raiseMast: Modifier = (parts) => {
  const grown = parts.map((part) =>
    part.part === "mast"
      ? { ...part, scale: part.scale * 1.28, y: part.y - 0.03 }
      : { ...part },
  );
  const masts = grown.filter((part) => part.part === "mast");
  return [
    ...grown,
    ...masts.map(
      (mast): ShapePart => ({
        ...mast,
        x: mast.x + 0.15,
        y: mast.y + 0.05,
        scale: mast.scale * 0.7,
      }),
    ),
  ];
};

const MODIFIERS: Record<ModifierName, Modifier> = {
  addStack,
  addRotorBlade,
  segmentVessel,
  platingBand,
  widenMouth,
  raiseMast,
};

export interface TierStep {
  readonly name: string;
  /** What the step is meant to read as, so the contact sheet has something to be checked against. */
  readonly reads: string;
  readonly modifiers: readonly ModifierName[];
}

/**
 * One row per tier above the base. A definition at tier N wears steps 0..N-1, so the ladder is
 * cumulative and an upgrade always adds to what the player already recognises rather than
 * replacing it — the same "edits in place, never replaces" rule the `upgrade` command follows.
 */
export const TIER_LADDER: readonly TierStep[] = [
  {
    name: "reinforced",
    reads:
      "plated, vented, open wider, and standing taller than the machine it grew out of",
    modifiers: ["platingBand", "addStack", "widenMouth", "raiseMast"],
  },
  {
    name: "overbuilt",
    reads:
      "segmented body, a second vent, another blade on anything that turns, and a mast higher again",
    modifiers: ["segmentVessel", "addRotorBlade", "addStack", "raiseMast"],
  },
];

/**
 * One row per completed contract stage. The landing hub is the one building in the game the player
 * does not place, and a founding project that changed nothing on screen would be a number in a
 * panel: this is what makes finishing one visible from across the map.
 *
 * It is a ladder rather than a drawing for the same reason `TIER_LADDER` is. A later contract with
 * a third stage costs a row here, not an artist.
 */
export const HUB_LADDER: readonly TierStep[] = [
  {
    name: "certified",
    reads: "a wider seamed body, plated around the base, under a second mast",
    modifiers: ["segmentVessel", "platingBand", "raiseMast"],
  },
  {
    name: "foundry",
    reads: "segmented again and vented, standing well above what landed here",
    modifiers: ["segmentVessel", "platingBand", "addStack", "raiseMast"],
  },
];

/**
 * Applies a ladder's first `steps` rows. Steps past its end wear every row it has, so a building
 * that outgrows a documented set is visibly odd rather than silently identical to the step below.
 */
export function applyLadder(
  base: readonly ShapePart[],
  ladder: readonly TierStep[],
  steps: number,
): readonly ShapePart[] {
  if (steps <= 0 || base.length === 0) return base;
  let parts: readonly ShapePart[] = base;
  for (const step of ladder.slice(0, steps)) {
    for (const name of step.modifiers) {
      const modifier = MODIFIERS[name];
      parts = modifier(parts);
    }
  }
  return parts;
}

export function applyTier(
  base: readonly ShapePart[],
  tier: number,
): readonly ShapePart[] {
  return applyLadder(base, TIER_LADDER, tier);
}

/* ------------------------------------------------------------------- walker */

/**
 * One renderer for every part, and the only place a `PartKind` becomes canvas calls. A new
 * building adds a row to a shape table; it does not come back here.
 */
export function drawParts(
  ctx: CanvasRenderingContext2D,
  parts: readonly ShapePart[],
  center: PixelPoint,
  size: number,
  stroke: string,
  cycle: number,
): void {
  if (parts.length === 0) return;
  ctx.save();
  ctx.strokeStyle = stroke;
  ctx.fillStyle = stroke;
  ctx.lineWidth = Math.max(1.4, size * 0.06);
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  for (const part of parts) drawPart(ctx, part, center, size, stroke, cycle);
  ctx.restore();
}

function drawPart(
  ctx: CanvasRenderingContext2D,
  part: ShapePart,
  center: PixelPoint,
  size: number,
  stroke: string,
  cycle: number,
): void {
  const phase = part.phase ?? "still";
  const angle = (part.rotation ?? 0) + (phase === "spin" ? cycle * TAU : 0);
  // `rise` travels along the part's own axis, so an upright stack puffs upward and a shaft
  // rotated to PI plunges. One rule, both readings.
  const drift = phase === "rise" ? cycle * part.scale * 1.5 : 0;
  const x = center.x + (part.x + Math.sin(angle) * drift) * size;
  const y = center.y + (part.y - Math.cos(angle) * drift) * size;
  const scale = part.scale * size;

  switch (part.part) {
    case "vessel": {
      strokeRoundedRect(ctx, x, y, scale, scale * 0.75, scale * 0.42, angle);
      break;
    }
    case "chamber": {
      ctx.save();
      ctx.translate(x, y);
      ctx.rotate(angle);
      ctx.strokeRect(-scale, -scale, scale * 2, scale * 2);
      ctx.restore();
      break;
    }
    case "stack": {
      // A tapered chimney: wider at the base than at the lip, so it reads as a vent rather than
      // as a post even at one hex on screen.
      ctx.save();
      ctx.translate(x, y);
      ctx.rotate(angle);
      ctx.beginPath();
      ctx.moveTo(-scale * 0.7, 0);
      ctx.lineTo(-scale * 0.5, -scale * 2);
      ctx.lineTo(scale * 0.5, -scale * 2);
      ctx.lineTo(scale * 0.7, 0);
      ctx.stroke();
      ctx.restore();
      break;
    }
    case "rotor": {
      ctx.beginPath();
      ctx.arc(x, y, scale, 0, TAU);
      ctx.stroke();
      const spokes = part.count ?? 0;
      for (let spoke = 0; spoke < spokes; spoke += 1) {
        const spokeAngle = angle + (spoke * TAU) / spokes;
        ctx.beginPath();
        ctx.moveTo(x, y);
        ctx.lineTo(
          x + Math.cos(spokeAngle) * scale,
          y + Math.sin(spokeAngle) * scale,
        );
        ctx.stroke();
      }
      break;
    }
    case "aperture": {
      // A pulsing opening brightens with the work; a rising one is a puff, so it thins as it
      // climbs; a still one is simply an opening and is solid.
      const lit =
        phase === "pulse"
          ? 0.18 + cycle * 0.45
          : phase === "rise"
            ? 0.8 - cycle * 0.5
            : 1;
      const radius = scale * (phase === "pulse" ? 1 + cycle * 0.12 : 1);
      ctx.save();
      ctx.globalAlpha = ctx.globalAlpha * lit;
      ctx.fillStyle = part.glow ?? stroke;
      ctx.beginPath();
      ctx.arc(x, y, radius, 0, TAU);
      ctx.fill();
      ctx.restore();
      break;
    }
    case "mast": {
      ctx.save();
      ctx.translate(x, y);
      ctx.rotate(angle);
      ctx.beginPath();
      ctx.moveTo(0, scale * 0.4);
      ctx.lineTo(0, -scale * 2);
      ctx.moveTo(-scale * 0.6, -scale * 1.35);
      ctx.lineTo(scale * 0.6, -scale * 1.35);
      ctx.stroke();
      ctx.restore();
      break;
    }
    case "band": {
      ctx.save();
      ctx.translate(x, y);
      ctx.rotate(angle);
      ctx.beginPath();
      ctx.moveTo(-scale, 0);
      ctx.lineTo(scale, 0);
      ctx.stroke();
      const rivets = part.count ?? 0;
      for (let rivet = 0; rivet < rivets; rivet += 1) {
        const across = rivets === 1 ? 0 : (rivet / (rivets - 1)) * 2 - 1;
        ctx.beginPath();
        ctx.arc(across * scale * 0.82, 0, Math.max(0.8, scale * 0.12), 0, TAU);
        ctx.fill();
      }
      ctx.restore();
      break;
    }
    case "mouth": {
      // Two converging jaws. `grind` closes the gap and opens it again, which is what a crusher
      // does and what `widenMouth` makes bigger on a tier.
      const gap =
        phase === "grind" ? 0.3 + Math.sin(cycle * Math.PI) * 0.26 : 0.3;
      ctx.save();
      ctx.translate(x, y);
      ctx.rotate(angle);
      ctx.beginPath();
      ctx.moveTo(-scale, -scale * 0.65);
      ctx.lineTo(-scale * gap, scale * 0.8);
      ctx.moveTo(scale, -scale * 0.65);
      ctx.lineTo(scale * gap, scale * 0.8);
      ctx.stroke();
      ctx.restore();
      break;
    }
  }
}

function strokeRoundedRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  halfWidth: number,
  halfHeight: number,
  radius: number,
  angle: number,
): void {
  const r = Math.min(radius, halfWidth, halfHeight);
  ctx.save();
  ctx.translate(x, y);
  ctx.rotate(angle);
  ctx.beginPath();
  ctx.moveTo(-halfWidth + r, -halfHeight);
  ctx.lineTo(halfWidth - r, -halfHeight);
  ctx.quadraticCurveTo(halfWidth, -halfHeight, halfWidth, -halfHeight + r);
  ctx.lineTo(halfWidth, halfHeight - r);
  ctx.quadraticCurveTo(halfWidth, halfHeight, halfWidth - r, halfHeight);
  ctx.lineTo(-halfWidth + r, halfHeight);
  ctx.quadraticCurveTo(-halfWidth, halfHeight, -halfWidth, halfHeight - r);
  ctx.lineTo(-halfWidth, -halfHeight + r);
  ctx.quadraticCurveTo(-halfWidth, -halfHeight, -halfWidth + r, -halfHeight);
  ctx.closePath();
  ctx.stroke();
  ctx.restore();
}
