import {
  Color,
  type MeshStandardMaterial,
  type WebGLProgramParametersWithUniforms,
} from "three";

import type { Terrain } from "../../core/types";
import { NOISE_GLSL } from "./noiseGlsl";

/**
 * Four procedural surface families cover the seven bands. A terrain differs from its family only
 * by palette and a few constants, so the shader count stays bounded: sand never gets its own
 * program, and adding a band never adds a material system.
 */
export type SurfaceFamily = "water" | "sand" | "meadow" | "rock";

export interface TerrainSurface {
  readonly family: SurfaceFamily;
  /** Trough colour of the band. */
  readonly low: string;
  /** Crest colour of the band. */
  readonly high: string;
  /** Foam and glint on water, quartz on sand, sunlit grass on meadow, fracture on rock. */
  readonly accent: string;
  /** The exposed flank below the cap: wet stone, sandstone, soil, bedrock. */
  readonly flank: string;
  /** World-space frequency. One hex is one metre, so 1 is roughly a per-hex feature. */
  readonly grain: number;
  /** How much accent the cap carries, 0..1. */
  readonly patch: number;
  /** Wave-normal strength. Water only. */
  readonly wave: number;
  /** Glint, foam, and mineral sparkle strength. */
  readonly sparkle: number;
}

/**
 * Presentation only, exactly like `TERRAIN_STYLE`. Nothing here reaches a save, a checksum, or the
 * wire, and no pattern is keyed on an axial coordinate: every function below samples world space,
 * so a stamped tile can never re-draw the hex lattice the prisms already carry.
 */
export const TERRAIN_SURFACE: Record<Terrain, TerrainSurface> = {
  deep_water: {
    family: "water",
    low: "#0a2b42",
    high: "#1f5b7d",
    accent: "#b9ecff",
    flank: "#14313d",
    grain: 0.8,
    patch: 0,
    wave: 1,
    sparkle: 1,
  },
  shallow_water: {
    family: "water",
    low: "#195a72",
    high: "#3d90a4",
    accent: "#d8f8ff",
    flank: "#2a5b62",
    grain: 1.15,
    patch: 0,
    wave: 0.7,
    sparkle: 0.85,
  },
  shore: {
    family: "sand",
    low: "#ab8f5b",
    high: "#e2cb92",
    accent: "#fff6da",
    flank: "#9c7f57",
    grain: 1.15,
    patch: 0,
    wave: 0,
    sparkle: 0.9,
  },
  lowland: {
    family: "meadow",
    low: "#2b5a38",
    high: "#5ea05c",
    accent: "#86b45e",
    flank: "#4a3a2a",
    grain: 1,
    patch: 0.32,
    wave: 0,
    sparkle: 0,
  },
  hills: {
    family: "meadow",
    low: "#456b45",
    high: "#77955f",
    accent: "#857a54",
    flank: "#4c4030",
    grain: 0.95,
    patch: 0.42,
    wave: 0,
    sparkle: 0,
  },
  highland: {
    family: "meadow",
    low: "#5c6a4d",
    high: "#8f9273",
    accent: "#7e7a62",
    flank: "#55503f",
    grain: 0.9,
    patch: 0.55,
    wave: 0,
    sparkle: 0,
  },
  cliff: {
    family: "rock",
    low: "#5b4d42",
    high: "#93836f",
    accent: "#40372f",
    flank: "#4f4339",
    grain: 0.85,
    patch: 0.45,
    wave: 0,
    sparkle: 0.6,
  },
};

const VERTEX_PARS = /* glsl */ `
attribute float frontierFade;
varying vec3 hfWorld;
varying vec3 hfNormal;
varying float hfFrontier;
`;

/**
 * The instanced prism's own world position. `worldpos_vertex` only defines one when shadows, fog,
 * or an envmap ask for it, so the surface takes its own rather than depending on another feature
 * staying switched on.
 */
const VERTEX_WORLD = /* glsl */ `
vec4 hfInstanced = vec4( transformed, 1.0 );
#ifdef USE_INSTANCING
	hfInstanced = instanceMatrix * hfInstanced;
#endif
hfWorld = ( modelMatrix * hfInstanced ).xyz;
hfFrontier = frontierFade;
`;

const FRAGMENT_COMMON = /* glsl */ `
uniform float hfTime;
uniform float hfMotion;
uniform float hfFill;
uniform vec3 hfLow;
uniform vec3 hfHigh;
uniform vec3 hfAccent;
uniform vec3 hfFlank;
uniform float hfGrain;
uniform float hfPatch;
uniform float hfWave;
uniform float hfSparkle;
varying vec3 hfWorld;
varying vec3 hfNormal;
varying float hfFrontier;

vec3 hfAlbedo;
float hfRough;
vec3 hfGlow;
vec3 hfBend;
${NOISE_GLSL}
/** How much of this fragment is the flat cap rather than the prism's flank. */
float hfCap() {
	return smoothstep( 0.35, 0.85, hfNormal.y );
}

/** Stronger relief cue from the continuous surface normal; generation height is not retuned. */
void hfRelief() {
	// Vertex normals are unit inputs; interpolation can only shorten them, which slightly
	// strengthens the cue at a triangle edge and avoids a normalize + pow on every fragment.
	float slope = smoothstep( 0.015, 0.62, clamp( 1.0 - hfNormal.y, 0.0, 1.0 ) );
	hfAlbedo = mix( hfAlbedo, hfFlank, slope * 0.46 );
	hfAlbedo *= 1.0 - slope * 0.24;
}

/** Exposed earth under the cap, banded by world height so no two prisms wear the same strata. */
vec3 hfBank() {
	vec2 strata = vec2( ( hfWorld.x + hfWorld.z ) * 0.45 * hfGrain, hfWorld.y * 6.5 );
	float bed = hfValue( strata );
	float grit = hfValue( strata * 5.3 );
	return hfFlank * ( 0.84 + bed * 0.28 + grit * 0.12 );
}
`;

const WATER_SURFACE = /* glsl */ `
void hfSurface() {
	vec2 w = hfWorld.xz * hfGrain;
	float t = hfTime * hfMotion;
	float first = dot( w, vec2( 0.92, 0.39 ) ) * 2.1 + t * 1.05;
	float second = dot( w, vec2( -0.44, 0.90 ) ) * 3.1 - t * 1.42;
	float swell = hfFbm( w * 0.55 + vec2( t * 0.05, t * -0.035 ) );
	float height = sin( first ) * 0.5 + sin( second ) * 0.32 + ( swell - 0.5 ) * 0.9;
	vec3 cap = mix( hfLow, hfHigh, clamp( 0.5 + height * 0.55, 0.0, 1.0 ) );
	hfBend = vec3( 0.0 );
	hfGlow = vec3( 0.0 );
	#if HF_WATER_DETAIL > 0
		float foam = smoothstep( 0.62, 0.98, swell + sin( first ) * 0.16 );
		cap = mix( cap, hfAccent, foam * 0.45 * hfSparkle );
		vec2 slope = vec2( 0.92, 0.39 ) * 2.1 * cos( first ) * 0.05
			+ vec2( -0.44, 0.90 ) * 3.1 * cos( second ) * 0.028;
		hfBend = vec3( -slope.x, 0.0, -slope.y ) * hfWave;
		float crest = pow( clamp( height * 0.6 + 0.45, 0.0, 1.0 ), 6.0 );
		float glint = pow( hfValue( w * 7.5 + vec2( t * 0.42, t * -0.31 ) ), 14.0 );
		hfGlow = hfAccent * ( crest * 0.06 + glint * 0.7 ) * hfSparkle;
	#endif
	float cap01 = hfCap();
	hfAlbedo = mix( hfBank(), cap, cap01 );
	hfRough = mix( 0.92, 0.1, cap01 );
	hfGlow *= cap01;
	hfBend *= cap01;
}
`;

const SAND_SURFACE = /* glsl */ `
void hfSurface() {
	vec2 p = hfWorld.xz * hfGrain;
	float dune = hfFbm( p * 0.4 );
	float ripple = 0.5 + 0.5 * sin( p.x * 4.3 + p.y * 2.6 + dune * 12.0 );
	vec3 cap = mix( hfLow, hfHigh, clamp( dune * 0.85 + ripple * 0.3, 0.0, 1.0 ) );
	float quartz = 0.0;
	#if HF_DETAIL > 0
		quartz = smoothstep( 0.9, 1.0, hfHash12( floor( p * 34.0 ) ) ) * hfSparkle;
		cap += hfAccent * quartz * 0.5;
	#endif
	float cap01 = hfCap();
	hfAlbedo = mix( hfBank(), cap, cap01 );
	hfRough = mix( 1.0, 0.97 - quartz * 0.65, cap01 );
	hfGlow = vec3( 0.0 );
	hfBend = vec3( 0.0 );
}
`;

const MEADOW_SURFACE = /* glsl */ `
void hfSurface() {
	vec2 p = hfWorld.xz * hfGrain;
	float meadow = hfFbm( p * 0.45 );
	float clump = hfFbm( p * 2.1 + 11.0 );
	vec3 cap = mix( hfLow, hfHigh, clamp( meadow * 1.25 - 0.12, 0.0, 1.0 ) );
	cap = mix( cap, hfAccent, smoothstep( 0.5, 0.92, clump ) * hfPatch );
	#if HF_DETAIL > 0
		cap *= 0.9 + hfValue( p * 13.0 ) * 0.22;
	#endif
	float cap01 = hfCap();
	hfAlbedo = mix( hfBank(), cap, cap01 );
	hfRough = 1.0 - 0.06 * cap01 * clump;
	hfGlow = vec3( 0.0 );
	hfBend = vec3( 0.0 );
}
`;

const ROCK_SURFACE = /* glsl */ `
void hfSurface() {
	vec2 p = hfWorld.xz * hfGrain;
	float bed = hfFbm( p * 0.7 );
	float shear = abs( hfFbm( p * 2.3 + 5.0 ) - 0.5 ) * 2.0;
	vec3 cap = mix( hfLow, hfHigh, clamp( bed * 1.2 - 0.1, 0.0, 1.0 ) );
	cap = mix( cap, hfAccent, ( 1.0 - smoothstep( 0.06, 0.3, shear ) ) * hfPatch );
	float mica = 0.0;
	#if HF_DETAIL > 0
		cap *= 0.88 + hfValue( p * 9.0 ) * 0.26;
		mica = smoothstep( 0.93, 1.0, hfHash12( floor( p * 26.0 ) ) ) * hfSparkle;
	#endif
	float cap01 = hfCap();
	hfAlbedo = mix( hfBank(), cap, cap01 );
	hfRough = mix( 1.0, 1.0 - mica * 0.55, cap01 );
	hfGlow = vec3( 0.0 );
	hfBend = vec3( 0.0 );
}
`;

const FAMILY_SURFACE: Record<SurfaceFamily, string> = {
  water: WATER_SURFACE,
  sand: SAND_SURFACE,
  meadow: MEADOW_SURFACE,
  rock: ROCK_SURFACE,
};

/** Fragment source a family contributes ahead of `main`. Exported so a test can read it. */
export function surfaceSource(family: SurfaceFamily): string {
  return `${FRAGMENT_COMMON}${FAMILY_SURFACE[family]}`;
}

interface SharedUniform {
  value: number;
}

interface SurfaceUniforms {
  readonly hfTime: SharedUniform;
  readonly hfMotion: SharedUniform;
  readonly hfFill: SharedUniform;
  readonly hfLow: { value: Color };
  readonly hfHigh: { value: Color };
  readonly hfAccent: { value: Color };
  readonly hfFlank: { value: Color };
  readonly hfGrain: SharedUniform;
  readonly hfPatch: SharedUniform;
  readonly hfWave: SharedUniform;
  readonly hfSparkle: SharedUniform;
}

/** Detail is capped at the octave counts the quality profiles actually ask for. */
const OCTAVES = [2, 3, 4] as const;

/**
 * The seven terrain materials, their shared clock, and the detail switch. One object owns them so
 * the renderer advances every band's water with a single call and cannot leave one behind.
 */
export class TerrainSurfaces {
  private readonly time: SharedUniform = { value: 0 };
  private readonly motion: SharedUniform = { value: 1 };
  private readonly attached: MeshStandardMaterial[] = [];
  private detail = 1;
  private waterDetail = 1;

  attach(material: MeshStandardMaterial, terrain: Terrain): void {
    const surface = TERRAIN_SURFACE[terrain];
    const uniforms: SurfaceUniforms = {
      hfTime: this.time,
      hfMotion: this.motion,
      hfFill: { value: material.emissiveIntensity },
      hfLow: { value: new Color(surface.low) },
      hfHigh: { value: new Color(surface.high) },
      hfAccent: { value: new Color(surface.accent) },
      hfFlank: { value: new Color(surface.flank) },
      hfGrain: { value: surface.grain },
      hfPatch: { value: surface.patch },
      hfWave: { value: surface.wave },
      hfSparkle: { value: surface.sparkle },
    };
    material.defines = {
      ...material.defines,
      HF_OCTAVES: OCTAVES[this.detail],
      HF_DETAIL: this.detail,
      HF_WATER_DETAIL: this.waterDetail,
    };
    material.onBeforeCompile = (
      parameters: WebGLProgramParametersWithUniforms,
    ) => {
      Object.assign(parameters.uniforms, uniforms);
      parameters.vertexShader = injectVertex(parameters.vertexShader);
      parameters.fragmentShader = injectFragment(
        parameters.fragmentShader,
        surface.family,
      );
    };
    // Three keys the program cache on `onBeforeCompile.toString()` unless a material says
    // otherwise. Seven closures that stringify identically would share one program, and every
    // band would come out wearing whichever palette compiled first.
    material.customProgramCacheKey = () => `hf-terrain-${terrain}`;
    material.needsUpdate = true;
    this.attached.push(material);
  }

  /** Seconds since the renderer started, wrapped so a long session keeps float precision. */
  setTime(seconds: number): void {
    this.time.value = seconds % 3600;
  }

  /** Reduced motion holds the sea still rather than slowing it. */
  setMotion(moving: boolean): void {
    this.motion.value = moving ? 1 : 0;
  }

  setDetail(detail: number, waterDetail: number): void {
    const next = clampDetail(detail);
    const nextWater = clampDetail(waterDetail);
    if (next === this.detail && nextWater === this.waterDetail) return;
    this.detail = next;
    this.waterDetail = nextWater;
    for (const material of this.attached) {
      material.defines = {
        ...material.defines,
        HF_OCTAVES: OCTAVES[next],
        HF_DETAIL: next,
        HF_WATER_DETAIL: nextWater,
      };
      material.needsUpdate = true;
    }
  }
}

function clampDetail(value: number): number {
  return Math.min(OCTAVES.length - 1, Math.max(0, Math.round(value)));
}

export function injectVertex(source: string): string {
  return source
    .replace("#include <common>", `#include <common>\n${VERTEX_PARS}`)
    .replace(
      "#include <beginnormal_vertex>",
      "#include <beginnormal_vertex>\nhfNormal = objectNormal;",
    )
    .replace(
      "#include <begin_vertex>",
      `#include <begin_vertex>\n${VERTEX_WORLD}`,
    );
}

export function injectFragment(source: string, family: SurfaceFamily): string {
  return (
    source
      .replace(
        "#include <common>",
        `#include <common>\n${surfaceSource(family)}`,
      )
      // The instance colour is a luminance jitter, so the pattern keeps its own hue.
      .replace(
        "#include <color_fragment>",
        "#include <color_fragment>\nhfSurface();\nhfRelief();\ndiffuseColor.rgb *= hfAlbedo;",
      )
      .replace(
        "#include <roughnessmap_fragment>",
        "#include <roughnessmap_fragment>\nroughnessFactor = clamp( hfRough, 0.04, 1.0 );",
      )
      .replace(
        "#include <normal_fragment_maps>",
        "#include <normal_fragment_maps>\nnormal = normalize( normal + mat3( viewMatrix ) * hfBend );",
      )
      // The flat emissive fill would otherwise wash every pattern back to the band's base colour.
      .replace(
        "#include <emissivemap_fragment>",
        "#include <emissivemap_fragment>\ntotalEmissiveRadiance = hfAlbedo * hfFill + hfGlow;",
      )
      // The frontier dissolves into the sky by colour rather than by coverage.
      //
      // It used to discard on a screen-space hash. That kept the surface opaque for draw ordering
      // and shadow baking, but it punched the background through the last two survey rings, and the
      // haze meant to hide those holes never reached them: the near end of the haze sits 0.6 of a
      // screenful beyond the target, which under this projection is where the top edge of the view
      // already is, so ground at the frontier is not hazed at any zoom. Measured on 2026-09-02 —
      // the holes stood at 293–340 of a possible 765 against the ground around them at every zoom
      // the frontier was on screen, byte-identical on Low, Medium and High.
      //
      // Mixing toward `fogColor` after the fog chunk leaves every fragment opaque and
      // depth-writing, so draw order and the shadow pass are untouched — and the colour pass now
      // agrees with the shadow pass instead of contradicting it. The mix rides after fog rather
      // than before tone mapping so it lands in the same space the background was cleared in, which
      // is what makes a fully dissolved fragment indistinguishable from sky rather than merely
      // close to it.
      .replace(
        "#include <fog_fragment>",
        "#include <fog_fragment>\ngl_FragColor.rgb = mix( fogColor, gl_FragColor.rgb, clamp( hfFrontier, 0.0, 1.0 ) );",
      )
  );
}
