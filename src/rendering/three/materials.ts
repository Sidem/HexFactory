import {
  Color,
  LineBasicMaterial,
  MeshBasicMaterial,
  MeshStandardMaterial,
} from "three";

import type { Terrain } from "../../core/types";
import { TERRAIN_STYLE } from "./terrainStyle";

export interface WorldMaterials {
  readonly terrain: Record<Terrain, MeshStandardMaterial>;
  readonly machine: MeshStandardMaterial;
  readonly machineDark: MeshStandardMaterial;
  readonly resource: MeshBasicMaterial;
  readonly emissive: MeshStandardMaterial;
  readonly overlayLegal: MeshBasicMaterial;
  readonly overlayIllegal: MeshBasicMaterial;
  readonly overlaySelection: MeshBasicMaterial;
  readonly grid: LineBasicMaterial;
  readonly frontier: LineBasicMaterial;
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
  const resource = new MeshBasicMaterial({
    color: 0xffffff,
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
  const materials = [
    ...Object.values(terrain),
    machine,
    machineDark,
    resource,
    emissive,
    overlayLegal,
    overlayIllegal,
    overlaySelection,
    grid,
    frontier,
  ];
  return {
    terrain,
    machine,
    machineDark,
    resource,
    emissive,
    overlayLegal,
    overlayIllegal,
    overlaySelection,
    grid,
    frontier,
    materials,
  };
}
