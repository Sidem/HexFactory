import {
  Color,
  MeshStandardMaterial,
  Vector2,
  type WebGLProgramParametersWithUniforms,
} from "three";

import { SURFACE_LOOK, UNKNOWN_SURFACE } from "../surfaceLook";
import { NOISE_GLSL } from "./noiseGlsl";
import { injectVertex } from "./terrainSurface";

/**
 * What a laid surface is made of, as a pattern rather than a colour.
 *
 * A paved yard used to be one flat tint per hex with a per-hex luminance jitter on top, and the
 * jitter was the lattice: every hex was a slightly different brightness, so a finished yard read as
 * a honeycomb of tiles rather than as ground. The fix is not a smaller jitter — it is to stop asking
 * the question per hex at all. Every pattern below is sampled from world-space metres, so it runs
 * straight across a hex boundary without knowing one is there, and a hundred paved hexes are one
 * continuous surface with courses and joints that line up across all of them.
 *
 * Presentation only, exactly like `SURFACE_LOOK`, which still owns the base colour the minimap and
 * the 2D renderer draw. Nothing here reaches a save, a checksum, or the wire.
 */
export type PavingPattern =
  | "earth"
  | "gravel"
  | "timber"
  | "brick"
  | "concrete"
  | "asphalt";

export interface PavingStyle {
  readonly pattern: PavingPattern;
  /** Trough colour. */
  readonly low: string;
  /** Crest colour. */
  readonly high: string;
  /** Mortar, expansion joint, or the gap between boards. Unused by the seamless patterns. */
  readonly joint: string;
  /** World-space frequency. One hex is one metre, so 1 keeps a course about a hex long. */
  readonly grain: number;
  /** Course size in metres: length along world x, width across world z. */
  readonly unit: readonly [number, number];
  /** Joint width as a fraction of a course. */
  readonly seam: number;
  readonly roughness: number;
}

/**
 * Colours are anchored on `SURFACE_LOOK` so the yard, the minimap, and the 2D renderer stay the same
 * material. `low` and `high` bracket that colour rather than replacing it.
 */
export const PAVING_STYLE: Record<string, PavingStyle> = {
  "compacted-earth": {
    pattern: "earth",
    low: "#6a563e",
    high: "#8b7354",
    joint: "#5a4934",
    grain: 1,
    unit: [1, 1],
    seam: 0,
    roughness: SURFACE_LOOK["compacted-earth"]?.roughness ?? 0.98,
  },
  "gravel-yard": {
    pattern: "gravel",
    low: "#7b796f",
    high: "#a5a397",
    joint: "#5f5e57",
    grain: 1,
    unit: [1, 1],
    seam: 0,
    roughness: SURFACE_LOOK["gravel-yard"]?.roughness ?? 0.94,
  },
  "timber-decking": {
    pattern: "timber",
    low: "#8a6136",
    high: "#c08f54",
    joint: "#3f2c18",
    grain: 1,
    unit: [2.4, 0.34],
    seam: 0.09,
    roughness: SURFACE_LOOK["timber-decking"]?.roughness ?? 0.82,
  },
  "brick-pavers": {
    pattern: "brick",
    low: "#8d4a38",
    high: "#b96b53",
    joint: "#6b6055",
    grain: 1,
    unit: [0.44, 0.22],
    seam: 0.14,
    roughness: SURFACE_LOOK["brick-pavers"]?.roughness ?? 0.8,
  },
  "concrete-slab": {
    pattern: "concrete",
    low: "#8d9393",
    high: "#adb3b2",
    joint: "#6c7272",
    grain: 1,
    unit: [3, 3],
    seam: 0.035,
    roughness: SURFACE_LOOK["concrete-slab"]?.roughness ?? 0.72,
  },
  "asphalt-road": {
    pattern: "asphalt",
    low: "#333d45",
    high: "#4d5a64",
    joint: "#2a323a",
    grain: 1,
    unit: [1, 1],
    seam: 0,
    roughness: SURFACE_LOOK["asphalt-road"]?.roughness ?? 0.91,
  },
};

/** An unrecognised surface is most plausibly worked earth, exactly as in `surfaceLook`. */
export const UNKNOWN_PAVING: PavingStyle = {
  pattern: "earth",
  low: "#6a563e",
  high: "#8b7354",
  joint: "#5a4934",
  grain: 1,
  unit: [1, 1],
  seam: 0,
  roughness: UNKNOWN_SURFACE.roughness,
};

export function pavingStyle(key: string | undefined): PavingStyle {
  return (key && PAVING_STYLE[key]) || UNKNOWN_PAVING;
}

const FRAGMENT_COMMON = /* glsl */ `
uniform vec3 hfPaveLow;
uniform vec3 hfPaveHigh;
uniform vec3 hfPaveJoint;
uniform float hfPaveGrain;
uniform vec2 hfPaveUnit;
uniform float hfPaveSeam;
uniform float hfPaveRoughness;
varying vec3 hfWorld;
varying vec3 hfNormal;

vec3 hfPaveAlbedo;
float hfPaveRough;
${NOISE_GLSL}
/** How much of this fragment is the laid top rather than the thin lip around it. */
float hfPaveTop() {
	return smoothstep( 0.35, 0.85, hfNormal.y );
}

/**
 * The cut edge of the paving where it stands proud of the ground: the same material seen end-on,
 * darker and coarser. Banded by world height so a graded terrace does not repeat itself.
 */
vec3 hfPaveLip() {
	float bed = hfValue( vec2( ( hfWorld.x + hfWorld.z ) * 2.1, hfWorld.y * 9.0 ) );
	return hfPaveLow * ( 0.5 + bed * 0.22 );
}
`;

/** Worked ground: no courses at all, just a continuous mottle that crosses every boundary. */
const EARTH_PAVING = /* glsl */ `
void hfPave() {
	vec2 p = hfWorld.xz * hfPaveGrain;
	float bed = hfFbm( p * 0.55 );
	float scuff = hfFbm( p * 2.4 + 7.0 );
	vec3 cap = mix( hfPaveLow, hfPaveHigh, clamp( bed * 1.25 - 0.1, 0.0, 1.0 ) );
	cap = mix( cap, hfPaveJoint, smoothstep( 0.62, 0.95, scuff ) * 0.35 );
	#if HF_PAVE_DETAIL > 0
		cap *= 0.93 + hfValue( p * 15.0 ) * 0.15;
	#endif
	float top = hfPaveTop();
	hfPaveAlbedo = mix( hfPaveLip(), cap, top );
	hfPaveRough = hfPaveRoughness;
}
`;

/** Loose stone: pebbles on a bed, sized in world metres so a yard is one spread of them. */
const GRAVEL_PAVING = /* glsl */ `
void hfPave() {
	vec2 p = hfWorld.xz * hfPaveGrain;
	vec2 grid = p * 11.0;
	vec2 cell = floor( grid );
	vec2 f = fract( grid ) - 0.5;
	float stone = hfHash12( cell );
	vec2 nudge = ( vec2( stone, hfHash12( cell + 41.0 ) ) - 0.5 ) * 0.42;
	float pebble = 1.0 - smoothstep( 0.14, 0.46, length( f - nudge ) );
	float bed = hfFbm( p * 0.5 );
	vec3 cap = mix( hfPaveLow, hfPaveHigh, clamp( bed * 0.55 + stone * 0.6, 0.0, 1.0 ) );
	cap = mix( hfPaveJoint * 0.94, cap, 0.35 + pebble * 0.65 );
	#if HF_PAVE_DETAIL > 0
		cap *= 0.92 + hfValue( p * 26.0 ) * 0.18;
	#endif
	float top = hfPaveTop();
	hfPaveAlbedo = mix( hfPaveLip(), cap, top );
	hfPaveRough = hfPaveRoughness - pebble * 0.08 * top;
}
`;

/**
 * Boards running along world x, each row shifted along its own length. Nothing about the layout
 * knows where a hex begins, so a deck laid over twenty of them is twenty hexes of the same boards.
 */
const TIMBER_PAVING = /* glsl */ `
void hfPave() {
	vec2 p = hfWorld.xz * hfPaveGrain;
	float row = floor( p.y / hfPaveUnit.y );
	float across = fract( p.y / hfPaveUnit.y );
	float along = p.x / hfPaveUnit.x + hfHash12( vec2( row, 3.0 ) ) * 5.0;
	float board = floor( along );
	float end = fract( along );
	float tone = hfHash12( vec2( board, row ) );
	float grain = hfFbm( vec2( p.x * 2.6, ( p.y + tone * 0.4 ) * 30.0 ) );
	vec3 cap = mix( hfPaveLow, hfPaveHigh, clamp( tone * 0.55 + grain * 0.7 - 0.1, 0.0, 1.0 ) );
	#if HF_PAVE_DETAIL > 0
		float knot = smoothstep( 0.86, 1.0, hfValue( vec2( along * 3.0, row * 5.0 ) ) );
		cap = mix( cap, hfPaveJoint, knot * 0.4 );
	#endif
	float seam = min( hfEdge( across, hfPaveSeam ), hfEdge( end, hfPaveSeam * 0.22 ) );
	cap = mix( hfPaveJoint, cap, seam );
	float top = hfPaveTop();
	hfPaveAlbedo = mix( hfPaveLip(), cap, top );
	hfPaveRough = mix( hfPaveRoughness + 0.1, hfPaveRoughness, seam );
}
`;

/** Running bond: every other course offset by half a paver, mortar between. */
const BRICK_PAVING = /* glsl */ `
void hfPave() {
	vec2 p = hfWorld.xz * hfPaveGrain;
	float course = floor( p.y / hfPaveUnit.y );
	float across = fract( p.y / hfPaveUnit.y );
	float along = p.x / hfPaveUnit.x + mod( course, 2.0 ) * 0.5;
	float paver = floor( along );
	float end = fract( along );
	float tone = hfHash12( vec2( paver, course ) );
	vec3 cap = mix( hfPaveLow, hfPaveHigh, clamp( tone * 1.1 - 0.05, 0.0, 1.0 ) );
	#if HF_PAVE_DETAIL > 0
		cap *= 0.93 + hfValue( p * 24.0 ) * 0.14;
	#endif
	float seam = min( hfEdge( across, hfPaveSeam ), hfEdge( end, hfPaveSeam * 0.5 ) );
	cap = mix( hfPaveJoint, cap, seam );
	float top = hfPaveTop();
	hfPaveAlbedo = mix( hfPaveLip(), cap, top );
	hfPaveRough = mix( hfPaveRoughness + 0.14, hfPaveRoughness, seam );
}
`;

/** Poured slabs with expansion joints every few metres, and aggregate under the float finish. */
const CONCRETE_PAVING = /* glsl */ `
void hfPave() {
	vec2 p = hfWorld.xz * hfPaveGrain;
	vec2 slab = p / hfPaveUnit;
	vec2 f = fract( slab );
	float pour = hfHash12( floor( slab ) );
	vec3 cap = mix( hfPaveLow, hfPaveHigh, clamp( hfFbm( p * 0.7 ) * 0.9 + pour * 0.25, 0.0, 1.0 ) );
	#if HF_PAVE_DETAIL > 0
		cap *= 0.95 + hfValue( p * 32.0 ) * 0.11;
	#endif
	float joint = min( hfEdge( f.x, hfPaveSeam ), hfEdge( f.y, hfPaveSeam ) );
	cap = mix( hfPaveJoint, cap, joint );
	float top = hfPaveTop();
	hfPaveAlbedo = mix( hfPaveLip(), cap, top );
	hfPaveRough = mix( hfPaveRoughness + 0.12, hfPaveRoughness, joint );
}
`;

/** Rolled bitumen: no courses, wide patches from separate laying passes, fine grit throughout. */
const ASPHALT_PAVING = /* glsl */ `
void hfPave() {
	vec2 p = hfWorld.xz * hfPaveGrain;
	float pass = hfFbm( p * 0.22 );
	float coarse = hfValue( p * 8.0 );
	vec3 cap = mix( hfPaveLow, hfPaveHigh, clamp( pass * 0.85 + coarse * 0.3, 0.0, 1.0 ) );
	#if HF_PAVE_DETAIL > 0
		cap *= 0.9 + hfValue( p * 34.0 ) * 0.2;
	#endif
	float top = hfPaveTop();
	hfPaveAlbedo = mix( hfPaveLip(), cap, top );
	hfPaveRough = hfPaveRoughness;
}
`;

const PATTERN_SOURCE: Record<PavingPattern, string> = {
  earth: EARTH_PAVING,
  gravel: GRAVEL_PAVING,
  timber: TIMBER_PAVING,
  brick: BRICK_PAVING,
  concrete: CONCRETE_PAVING,
  asphalt: ASPHALT_PAVING,
};

/** Fragment source a pattern contributes ahead of `main`. Exported so a test can read it. */
export function pavingSource(pattern: PavingPattern): string {
  return `${FRAGMENT_COMMON}${PATTERN_SOURCE[pattern]}`;
}

export function injectPavingFragment(
  source: string,
  pattern: PavingPattern,
): string {
  return source
    .replace("#include <common>", `#include <common>\n${pavingSource(pattern)}`)
    .replace(
      "#include <color_fragment>",
      "#include <color_fragment>\nhfPave();\ndiffuseColor.rgb *= hfPaveAlbedo;",
    )
    .replace(
      "#include <roughnessmap_fragment>",
      "#include <roughnessmap_fragment>\nroughnessFactor = clamp( hfPaveRough, 0.04, 1.0 );",
    );
}

/** Detail is capped at the octave counts the quality profiles actually ask for. */
const OCTAVES = [2, 3, 4] as const;

/**
 * One material per laid surface, their palettes, and the detail switch.
 *
 * Built eagerly for every key in the catalogue rather than on demand, so disposal is a list rather
 * than a cache that has to be walked at the right moment. Six unused materials cost six objects and
 * no GPU program at all — Three compiles one only when something is actually drawn with it.
 */
export class PavingSurfaces {
  private readonly byKey = new Map<string, MeshStandardMaterial>();
  private readonly fallback: MeshStandardMaterial;
  private detail = 1;

  constructor() {
    for (const key of Object.keys(PAVING_STYLE))
      this.byKey.set(key, this.build(key, pavingStyle(key)));
    this.fallback = this.build("unknown", UNKNOWN_PAVING);
  }

  /** The material for a surface key, or worked earth for one this build does not recognise. */
  material(key: string | undefined): MeshStandardMaterial {
    return (key && this.byKey.get(key)) || this.fallback;
  }

  all(): MeshStandardMaterial[] {
    return [...this.byKey.values(), this.fallback];
  }

  setDetail(detail: number): void {
    const next = Math.min(OCTAVES.length - 1, Math.max(0, Math.round(detail)));
    if (next === this.detail) return;
    this.detail = next;
    for (const material of this.all()) {
      material.defines = {
        ...material.defines,
        HF_OCTAVES: OCTAVES[next],
        HF_PAVE_DETAIL: next,
      };
      material.needsUpdate = true;
    }
  }

  private build(key: string, style: PavingStyle): MeshStandardMaterial {
    const material = new MeshStandardMaterial({
      color: 0xffffff,
      roughness: style.roughness,
      metalness: 0.02,
      flatShading: true,
    });
    const uniforms = {
      hfPaveLow: { value: new Color(style.low) },
      hfPaveHigh: { value: new Color(style.high) },
      hfPaveJoint: { value: new Color(style.joint) },
      hfPaveGrain: { value: style.grain },
      hfPaveUnit: { value: new Vector2(style.unit[0], style.unit[1]) },
      hfPaveSeam: { value: style.seam },
      hfPaveRoughness: { value: style.roughness },
    };
    material.defines = {
      ...material.defines,
      HF_OCTAVES: OCTAVES[this.detail],
      HF_PAVE_DETAIL: this.detail,
    };
    material.onBeforeCompile = (
      parameters: WebGLProgramParametersWithUniforms,
    ) => {
      Object.assign(parameters.uniforms, uniforms);
      parameters.vertexShader = injectVertex(parameters.vertexShader);
      parameters.fragmentShader = injectPavingFragment(
        parameters.fragmentShader,
        style.pattern,
      );
    };
    // Three keys the program cache on `onBeforeCompile.toString()` unless a material says
    // otherwise. Seven closures that stringify identically would share one program, and every
    // surface would come out wearing whichever palette compiled first.
    material.customProgramCacheKey = () => `hf-paving-${key}`;
    material.needsUpdate = true;
    return material;
  }
}
