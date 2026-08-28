import {
  Color,
  LineBasicMaterial,
  MeshBasicMaterial,
  MeshStandardMaterial,
} from "three";
import type { WebGLProgramParametersWithUniforms } from "three";

import type { Terrain } from "../../core/types";
import type { MachineMaterialRole } from "../shapeGrammar";
import { PavingSurfaces } from "./pavingSurface";
import { TERRAIN_STYLE } from "./terrainStyle";
import { TerrainSurfaces } from "./terrainSurface";

export interface WorldMaterials {
  readonly terrain: Record<Terrain, MeshStandardMaterial>;
  /** The procedural landform surfaces: one clock, one detail switch, seven materials. */
  readonly terrainSurfaces: TerrainSurfaces;
  /** The procedural laid surfaces: one material per paving, each patterned in world space. */
  readonly paving: PavingSurfaces;
  readonly machine: MeshStandardMaterial;
  readonly machineCeramic: MeshStandardMaterial;
  readonly machineBrass: MeshStandardMaterial;
  readonly machineDark: MeshStandardMaterial;
  readonly wayfinderHull: MeshStandardMaterial;
  readonly wayfinderShell: MeshStandardMaterial;
  readonly wayfinderBrass: MeshStandardMaterial;
  readonly wayfinderSignal: MeshStandardMaterial;
  readonly smoke: MeshBasicMaterial;
  readonly resource: MeshStandardMaterial;
  /** Specular anthracite: coal has to glint, not sit as another mid-grey lump. */
  readonly resourceCoal: MeshStandardMaterial;
  /** Matte rock: high roughness, so stone reads as grey mass instead of metal. */
  readonly resourceStone: MeshStandardMaterial;
  /** Dry grit: no metal, almost no specular, so sand stays beige dunes. */
  readonly resourceSand: MeshStandardMaterial;
  readonly emissive: MeshStandardMaterial;
  readonly overlayLegal: MeshBasicMaterial;
  readonly overlayIllegal: MeshBasicMaterial;
  readonly overlaySelection: MeshBasicMaterial;
  /** Dedicated range colours: changing one never recolours selection or placement feedback. */
  readonly buildRange: MeshBasicMaterial;
  readonly poleSupplyRange: MeshBasicMaterial;
  readonly poleLinkRange: MeshBasicMaterial;
  readonly grid: LineBasicMaterial;
  readonly frontier: LineBasicMaterial;
  /** The ribbon along an autonomous walk, and the ring on the hex it ends at. */
  readonly route: LineBasicMaterial;
  readonly routeGoal: MeshBasicMaterial;
  readonly materials: readonly (
    | MeshStandardMaterial
    | MeshBasicMaterial
    | LineBasicMaterial
  )[];
}

export function createWorldMaterials(): WorldMaterials {
  const terrain = Object.fromEntries(
    Object.entries(TERRAIN_STYLE).map(([key, style]) => [
      key,
      new MeshStandardMaterial({
        color: 0xffffff,
        emissive: style.color,
        emissiveIntensity: 0.32,
        roughness: style.roughness,
        metalness: key.includes("water") ? 0.08 : 0,
        flatShading: true,
      }),
    ]),
  ) as Record<Terrain, MeshStandardMaterial>;
  const terrainSurfaces = new TerrainSurfaces();
  for (const [key, material] of Object.entries(terrain))
    terrainSurfaces.attach(material, key as Terrain);
  const paving = new PavingSurfaces();
  const machine = machineMaterial("#ffffff", "structure", 0.7, 0.3);
  const machineCeramic = machineMaterial("#ffffff", "ceramic", 0.86, 0.04);
  const machineBrass = machineMaterial("#ffffff", "brass", 0.42, 0.62);
  const machineDark = machineMaterial("#ffffff", "dark", 0.9, 0.15);
  const wayfinderHull = machineMaterial("#14262a", "dark", 0.82, 0.18);
  const wayfinderShell = machineMaterial("#d9d1b8", "ceramic", 0.8, 0.05);
  const wayfinderBrass = machineMaterial("#bf8948", "brass", 0.4, 0.64);
  const wayfinderSignal = new MeshStandardMaterial({
    color: "#f4ead0",
    roughness: 0.32,
    metalness: 0.02,
    emissive: "#7fe0c0",
    emissiveIntensity: 0.72,
    flatShading: true,
  });
  const smoke = new MeshBasicMaterial({
    color: "#ffffff",
    transparent: true,
    opacity: 0.32,
    depthWrite: false,
  });
  const resource = new MeshStandardMaterial({
    color: 0xffffff,
    roughness: 0.62,
    metalness: 0.22,
    emissive: "#09110f",
    emissiveIntensity: 0.06,
    flatShading: true,
  });
  const resourceCoal = new MeshStandardMaterial({
    color: 0xffffff,
    roughness: 0.18,
    metalness: 0.74,
    emissive: "#141a24",
    emissiveIntensity: 0.16,
    flatShading: true,
  });
  const resourceStone = new MeshStandardMaterial({
    color: 0xffffff,
    roughness: 0.88,
    metalness: 0.08,
    emissive: "#000000",
    emissiveIntensity: 0,
    flatShading: true,
  });
  const resourceSand = new MeshStandardMaterial({
    color: 0xffffff,
    roughness: 0.98,
    metalness: 0,
    emissive: "#000000",
    emissiveIntensity: 0,
    flatShading: true,
  });
  const emissive = new MeshStandardMaterial({
    color: 0xffffff,
    roughness: 0.36,
    metalness: 0.08,
    emissive: new Color("#ffffff"),
    emissiveIntensity: 0.45,
    flatShading: true,
  });
  const overlayLegal = new MeshBasicMaterial({
    color: "#76e0aa",
    transparent: true,
    opacity: 0.58,
    depthTest: false,
  });
  const overlayIllegal = new MeshBasicMaterial({
    color: "#ff6b5e",
    transparent: true,
    opacity: 0.62,
    depthTest: false,
  });
  const overlaySelection = new MeshBasicMaterial({
    color: "#f6c85f",
    transparent: true,
    opacity: 0.72,
    depthTest: false,
  });
  const buildRange = new MeshBasicMaterial({
    color: "#8fd4ff",
    transparent: true,
    opacity: 0.68,
    depthTest: false,
  });
  const poleSupplyRange = new MeshBasicMaterial({
    color: "#f6c85f",
    transparent: true,
    opacity: 0.62,
    depthTest: false,
  });
  const poleLinkRange = new MeshBasicMaterial({
    color: "#ff7669",
    transparent: true,
    opacity: 0.54,
    depthTest: false,
  });
  const grid = new LineBasicMaterial({
    color: "#80b7a8",
    transparent: true,
    opacity: 0.22,
    depthTest: false,
  });
  const frontier = new LineBasicMaterial({
    color: "#7fe0c0",
    transparent: true,
    opacity: 0.68,
    depthTest: false,
  });
  // A walk in progress is the player's own standing order, so the route reads as a decision rather
  // than a warning: it borrows the selection's warmth without competing with the legality colours a
  // build overlay is saying something with. `depthTest: false` keeps it readable across a ridge,
  // which is exactly the case a drawn route is worth having.
  const route = new LineBasicMaterial({
    color: "#ffd479",
    transparent: true,
    opacity: 0.85,
    depthTest: false,
  });
  const routeGoal = new MeshBasicMaterial({
    color: "#ffd479",
    transparent: true,
    opacity: 0.66,
    depthTest: false,
  });
  const materials = [
    ...Object.values(terrain),
    ...paving.all(),
    machine,
    machineCeramic,
    machineBrass,
    machineDark,
    wayfinderHull,
    wayfinderShell,
    wayfinderBrass,
    wayfinderSignal,
    smoke,
    resource,
    resourceCoal,
    resourceStone,
    resourceSand,
    emissive,
    overlayLegal,
    overlayIllegal,
    overlaySelection,
    buildRange,
    poleSupplyRange,
    poleLinkRange,
    grid,
    frontier,
    route,
    routeGoal,
  ];
  return {
    terrain,
    terrainSurfaces,
    paving,
    machine,
    machineCeramic,
    machineBrass,
    machineDark,
    wayfinderHull,
    wayfinderShell,
    wayfinderBrass,
    wayfinderSignal,
    smoke,
    resource,
    resourceCoal,
    resourceStone,
    resourceSand,
    emissive,
    overlayLegal,
    overlayIllegal,
    overlaySelection,
    buildRange,
    poleSupplyRange,
    poleLinkRange,
    grid,
    frontier,
    route,
    routeGoal,
    materials,
  };
}

interface MachineSurfaceSpec {
  readonly frequency: number;
  readonly contrast: number;
  readonly wear: number;
}

const MACHINE_SURFACES: Record<MachineMaterialRole, MachineSurfaceSpec> = {
  structure: { frequency: 17, contrast: 0.055, wear: 0.045 },
  ceramic: { frequency: 24, contrast: 0.035, wear: 0.025 },
  brass: { frequency: 31, contrast: 0.07, wear: 0.065 },
  dark: { frequency: 13, contrast: 0.045, wear: 0.025 },
};

/** Object-space procedural grain: generated geometry needs no UV atlas or per-definition texture. */
function machineMaterial(
  color: string,
  role: MachineMaterialRole,
  roughness: number,
  metalness: number,
): MeshStandardMaterial {
  const material = new MeshStandardMaterial({
    color,
    roughness,
    metalness,
    emissive: role === "dark" ? "#050908" : "#09110f",
    emissiveIntensity: role === "dark" ? 0.08 : 0.12,
    flatShading: true,
  });
  const surface = MACHINE_SURFACES[role];
  material.onBeforeCompile = (
    parameters: WebGLProgramParametersWithUniforms,
  ) => {
    parameters.vertexShader = injectMachineVertex(parameters.vertexShader);
    parameters.fragmentShader = injectMachineFragment(
      parameters.fragmentShader,
      surface,
    );
  };
  material.customProgramCacheKey = () => `hf-machine-${role}`;
  material.needsUpdate = true;
  return material;
}

function injectMachineVertex(source: string): string {
  return source
    .replace(
      "#include <common>",
      "#include <common>\nvarying vec3 hfMachineLocal;\nvarying vec3 hfMachineNormal;",
    )
    .replace(
      "#include <beginnormal_vertex>",
      "#include <beginnormal_vertex>\nhfMachineNormal = objectNormal;",
    )
    .replace(
      "#include <begin_vertex>",
      "#include <begin_vertex>\nhfMachineLocal = position;",
    );
}

function injectMachineFragment(
  source: string,
  surface: MachineSurfaceSpec,
): string {
  const frequency = surface.frequency.toFixed(1);
  const wear = surface.wear.toFixed(3);
  return source
    .replace(
      "#include <common>",
      `#include <common>
varying vec3 hfMachineLocal;
varying vec3 hfMachineNormal;
float hfMachineHash(vec3 p) {
  return fract(sin(dot(p, vec3(12.9898, 78.233, 37.719))) * 43758.5453);
}`,
    )
    .replace(
      "#include <color_fragment>",
      `#include <color_fragment>
float hfMachineGrain = hfMachineHash(floor(hfMachineLocal * ${frequency}));
float hfMachineBand = 0.5 + 0.5 * sin((hfMachineLocal.y + hfMachineLocal.x * 0.22) * ${frequency});
float hfMachineTexture = mix(hfMachineGrain, hfMachineBand, 0.36);
diffuseColor.rgb *= mix(${(1 - surface.contrast).toFixed(3)}, ${(1 + surface.contrast).toFixed(3)}, hfMachineTexture);
float hfMachineWear = pow(1.0 - abs(normalize(hfMachineNormal).y), 3.0) * ${wear};
diffuseColor.rgb = mix(diffuseColor.rgb, vec3(0.94, 0.89, 0.76), hfMachineWear);`,
    );
}
