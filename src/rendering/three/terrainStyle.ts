import type { Terrain } from "../../core/types";

export interface TerrainStyle {
  readonly height: number;
  readonly color: string;
  readonly roughness: number;
}

/** One total, presentation-only landform lookup. Scene x/z are the native plane; y never returns. */
export const TERRAIN_STYLE: Record<Terrain, TerrainStyle> = {
  deep_water: { height: -0.2, color: "#123d59", roughness: 0.58 },
  shallow_water: { height: -0.08, color: "#276b84", roughness: 0.64 },
  shore: { height: 0.02, color: "#c4a56a", roughness: 0.94 },
  lowland: { height: 0.07, color: "#315442", roughness: 1 },
  hills: { height: 0.2, color: "#536d55", roughness: 1 },
  highland: { height: 0.36, color: "#727b68", roughness: 1 },
  cliff: { height: 0.62, color: "#715d4e", roughness: 1 },
};

export function visualHeight(terrain: Terrain): number {
  return TERRAIN_STYLE[terrain].height;
}
