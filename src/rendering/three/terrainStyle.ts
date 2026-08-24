import type { Terrain } from "../../core/types";

export interface TerrainStyle {
  readonly height: number;
  readonly color: string;
  readonly roughness: number;
}

/**
 * One total, presentation-only landform lookup. Scene x/z are the native plane; y never returns.
 *
 * `color` is the band's identity: the colour a legend, a fallback, or a screenshot description
 * means by "lowland". What the prism actually wears is the procedural band in `terrainSurface.ts`,
 * which straddles this colour rather than replacing it, and `tests/visualDepth.test.ts` pins the
 * two together so the surface cannot drift away from the band it is meant to be.
 */
export const TERRAIN_STYLE: Record<Terrain, TerrainStyle> = {
  deep_water: { height: -0.2, color: "#123d59", roughness: 0.58 },
  shallow_water: { height: -0.08, color: "#276b84", roughness: 0.64 },
  shore: { height: 0.02, color: "#c4a56a", roughness: 0.94 },
  lowland: { height: 0.07, color: "#3d6b45", roughness: 1 },
  hills: { height: 0.2, color: "#58784f", roughness: 1 },
  highland: { height: 0.36, color: "#757e62", roughness: 1 },
  cliff: { height: 0.62, color: "#715d4e", roughness: 1 },
};

export function visualHeight(terrain: Terrain): number {
  return TERRAIN_STYLE[terrain].height;
}
