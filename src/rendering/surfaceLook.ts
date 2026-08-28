/**
 * What a prepared hex looks like, by surface key. Presentation only: native owns which hex carries
 * which surface, and the walking speed that buys.
 *
 * Keyed by the definition's key rather than its id, so the catalogue can be renumbered without
 * silently repainting the world. An unknown key falls back to worked earth, which is what an
 * unrecognised surface most plausibly is.
 */
export interface SurfaceLook {
  readonly color: string;
  readonly roughness: number;
}

export const SURFACE_LOOK: Record<string, SurfaceLook> = {
  "compacted-earth": { color: "#7a6448", roughness: 0.98 },
  "gravel-yard": { color: "#8f8d82", roughness: 0.94 },
  "timber-decking": { color: "#a87a45", roughness: 0.82 },
  "brick-pavers": { color: "#a45a45", roughness: 0.8 },
  "concrete-slab": { color: "#9ba1a1", roughness: 0.72 },
  "asphalt-road": { color: "#414d56", roughness: 0.91 },
};

export const UNKNOWN_SURFACE: SurfaceLook = {
  color: "#7a6448",
  roughness: 0.95,
};

export function surfaceLook(key: string | undefined): SurfaceLook {
  return (key && SURFACE_LOOK[key]) || UNKNOWN_SURFACE;
}

/**
 * How far one grade step lifts a hex in the scene. Chosen against the natural landform range in
 * `terrainStyle.ts`: three steps either way is about the drop from cliff to water, so a cut yard
 * reads as excavation at the same glance that reads the terrain.
 */
export const GRADE_STEP_HEIGHT = 0.14;
