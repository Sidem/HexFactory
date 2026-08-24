import type { GraphicsProfile } from "../FactoryRenderer";

export interface QualitySettings {
  readonly pixelRatioCap: number;
  readonly shadows: boolean;
  readonly shadowMapSize: number;
  readonly terrainDetail: number;
  readonly waterDetail: number;
  readonly secondaryMotion: boolean;
}

export const QUALITY_SETTINGS: Record<GraphicsProfile, QualitySettings> = {
  low: {
    pixelRatioCap: 1,
    shadows: false,
    shadowMapSize: 0,
    terrainDetail: 0,
    waterDetail: 0,
    secondaryMotion: false,
  },
  medium: {
    pixelRatioCap: 1.25,
    shadows: true,
    shadowMapSize: 1024,
    terrainDetail: 1,
    waterDetail: 1,
    secondaryMotion: true,
  },
  high: {
    pixelRatioCap: 1.5,
    shadows: true,
    shadowMapSize: 2048,
    terrainDetail: 2,
    waterDetail: 2,
    secondaryMotion: true,
  },
};

export const GRAPHICS_STORAGE_KEY = "hexfactory:graphics:v1";

export function parseGraphicsProfile(value: unknown): GraphicsProfile | null {
  return value === "low" || value === "medium" || value === "high"
    ? value
    : null;
}

export function defaultGraphicsProfile(): GraphicsProfile {
  const cores = navigator.hardwareConcurrency || 4;
  return cores <= 4 ? "low" : cores <= 8 ? "medium" : "high";
}
