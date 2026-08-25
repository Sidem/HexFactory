import {
  Color,
  LineBasicMaterial,
  MeshBasicMaterial,
  MeshStandardMaterial,
} from "three";

import type { Terrain } from "../../core/types";
import { TERRAIN_STYLE } from "./terrainStyle";
import { TerrainSurfaces } from "./terrainSurface";

export interface WorldMaterials {
  readonly terrain: Record<Terrain, MeshStandardMaterial>;
  /** The procedural landform surfaces: one clock, one detail switch, seven materials. */
  readonly terrainSurfaces: TerrainSurfaces;
  readonly machine: MeshStandardMaterial;
  readonly machineDark: MeshStandardMaterial;
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
        vertexColors: true,
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
  const machine = new MeshStandardMaterial({
    color: 0xffffff,
    roughness: 0.7,
    metalness: 0.3,
    emissive: "#09110f",
    emissiveIntensity: 0.12,
    flatShading: true,
  });
  const machineDark = new MeshStandardMaterial({
    color: 0xffffff,
    roughness: 0.9,
    metalness: 0.15,
    emissive: "#050908",
    emissiveIntensity: 0.08,
    flatShading: true,
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
    machine,
    machineDark,
    resource,
    resourceCoal,
    resourceStone,
    resourceSand,
    emissive,
    overlayLegal,
    overlayIllegal,
    overlaySelection,
    grid,
    frontier,
    route,
    routeGoal,
  ];
  return {
    terrain,
    terrainSurfaces,
    machine,
    machineDark,
    resource,
    resourceCoal,
    resourceStone,
    resourceSand,
    emissive,
    overlayLegal,
    overlayIllegal,
    overlaySelection,
    grid,
    frontier,
    route,
    routeGoal,
    materials,
  };
}
