import {
  axialDistance,
  axialToPixel,
  pixelToAxial,
  type AxialCoordinate,
} from "@hexlife/embed/hex";

import type { BuildingKind, FactorySnapshot, WorldPoint } from "../core/types";

/**
 * World units per hex circumradius — the scale native lays its lattice out on, and the only number
 * the host needs in order to turn an axial coordinate into the world position native would give it.
 */
export const WORLD_SCALE = 1024;

/** Where the landing hub stands, or `null` in a world that has none. */
export function findLandingHub(snapshot: FactorySnapshot): WorldPoint | null {
  const hub = snapshot.buildings.find(({ kind }) => kind === "hub");
  return hub ? axialToPixel(hub, WORLD_SCALE, { x: 0, y: 0 }) : null;
}

/**
 * What a belt, a pole, and a bridge have in common: they carry for something else. A player who
 * walks up to a smelter with a belt running past it has walked up to the smelter, so the two are
 * ranked rather than treated as equal claims on the same step.
 */
const CARRIES_FOR_SOMETHING_ELSE: ReadonlySet<BuildingKind> =
  new Set<BuildingKind>(["belt", "pole", "bridge"]);

/**
 * The building the player is standing at or beside, as the footprint cell nearest them — the cell a
 * click would have picked — or `null` when they stand clear of everything.
 *
 * Nearest wins, then a machine over the infrastructure serving it, then the lower entity ID. All
 * three are needed: a hex has six neighbours and a factory floor puts several of them in reach at
 * once, and a rule that left any of those ties open would let the selection flicker between two
 * buildings while the player stands still.
 */
export function buildingBeside(
  snapshot: FactorySnapshot,
): AxialCoordinate | null {
  const standing = pixelToAxial(snapshot.player, WORLD_SCALE);
  let best: { cell: AxialCoordinate; rank: number; id: number } | null = null;
  for (const building of snapshot.buildings) {
    const carries = CARRIES_FOR_SOMETHING_ELSE.has(building.kind) ? 1 : 0;
    for (const cell of building.footprint) {
      const distance = axialDistance(standing, cell);
      if (distance > 1) continue;
      // Distance is 0 or 1 here, so doubling it leaves the carried flag as the tie-break below it.
      const rank = distance * 2 + carries;
      if (
        best === null ||
        rank < best.rank ||
        (rank === best.rank && building.id < best.id)
      )
        best = { cell, rank, id: building.id };
    }
  }
  return best?.cell ?? null;
}

export interface HomeBearing {
  /** Unit vector from the player toward home, in world space. */
  x: number;
  y: number;
  /** How far, in whole hex steps — the distance the world is actually measured in. */
  hexes: number;
  /** Which of the six directions home lies in, for naming it in the same words the game uses. */
  direction: number;
}

/**
 * Which way home is and how far, or `null` when the player is standing on it.
 *
 * The distance is the hex distance rather than a world-unit magnitude, because "42 hex" is a number
 * the player can act on and "74,508" is not. Screen space is the world scaled by a positive factor
 * with no rotation, so the same unit vector aims the marker on the map and the marker on the edge of
 * the view.
 */
export function homeBearing(
  from: WorldPoint,
  home: WorldPoint,
): HomeBearing | null {
  const dx = home.x - from.x;
  const dy = home.y - from.y;
  const length = Math.hypot(dx, dy);
  if (length === 0) return null;
  const degrees = (Math.atan2(dy, dx) * 180) / Math.PI;
  return {
    x: dx / length,
    y: dy / length,
    hexes: axialDistance(
      pixelToAxial(from, WORLD_SCALE),
      pixelToAxial(home, WORLD_SCALE),
    ),
    direction: Math.round((((degrees % 360) + 360) % 360) / 60) % 6,
  };
}
