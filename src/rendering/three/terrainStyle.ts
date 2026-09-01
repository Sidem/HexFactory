import type { Terrain } from "../../core/types";

export interface TerrainStyle {
  readonly color: string;
  readonly roughness: number;
}

/**
 * One total, presentation-only band lookup, in the order the bands rise.
 *
 * `color` is the band's identity: the colour a legend, a fallback, or a screenshot description
 * means by "lowland". What the ground actually wears is the procedural band in `terrainSurface.ts`,
 * which straddles this colour rather than replacing it, and `tests/visualDepth.test.ts` pins the
 * two together so the surface cannot drift away from the band it is meant to be.
 *
 * There is no height here any more. A band used to carry a cosmetic elevation because the generator
 * published nothing else; native now publishes a real bed height per cell, and inventing a second
 * one would put the drawn ground somewhere the player cannot stand. The declaration order is load
 * bearing instead — `terrainMeshes.ts` uses each band's index as its draw-group look.
 */
export const TERRAIN_STYLE: Record<Terrain, TerrainStyle> = {
  deep_water: { color: "#123d59", roughness: 0.58 },
  shallow_water: { color: "#276b84", roughness: 0.64 },
  shore: { color: "#c4a56a", roughness: 0.94 },
  lowland: { color: "#3d6b45", roughness: 1 },
  hills: { color: "#58784f", roughness: 1 },
  highland: { color: "#757e62", roughness: 1 },
  cliff: { color: "#715d4e", roughness: 1 },
};
