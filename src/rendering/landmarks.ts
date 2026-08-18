import { axialDistance, axialToPixel, pixelToAxial } from "@hexlife/embed/hex";

import type { FactorySnapshot, WorldPoint } from "../core/types";

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
