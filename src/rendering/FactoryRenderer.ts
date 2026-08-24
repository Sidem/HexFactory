import type { AxialCoordinate } from "@hexlife/embed/hex";

import type {
  BuildingDefinition,
  ChunkSnapshot,
  EntitySnapshot,
  FactorySnapshot,
  LinePreviewCell,
  PlacementPreview,
  WorldPoint,
} from "../core/types";

export const BASE_HEX_SIZE = 22;
export const MAX_DEVICE_PIXEL_RATIO = 1.5;

/** One material family per native kind; definitions vary by generated anatomy, never an id case. */
export const BUILDING_COLORS: Record<EntitySnapshot["kind"], string> = {
  extractor: "#b75e45",
  belt: "#415b78",
  composer: "#765bae",
  container: "#a07c3e",
  consumer: "#3c806a",
  hub: "#d1a945",
  pump: "#2f7d9c",
  pole: "#c8b56b",
  generator: "#d4a017",
  boiler: "#a85c32",
  bridge: "#8f7655",
};

/** Native chunk bounds are the whole surveyed-world predicate; omitted terrain inside is lowland. */
export function isSurveyed(
  chunks: readonly ChunkSnapshot[],
  point: WorldPoint,
): boolean {
  return chunks.some(
    (chunk) =>
      point.x >= chunk.x &&
      point.x < chunk.x + chunk.span &&
      point.y >= chunk.y &&
      point.y < chunk.y + chunk.span,
  );
}

export type GraphicsProfile = "low" | "medium" | "high";

export interface ReachRadii {
  readonly extract: number | null;
  readonly supply: number | null;
  readonly link: number | null;
}

export interface RendererDiagnostics {
  readonly name: string;
  readonly profile: GraphicsProfile;
  readonly drawCalls: number;
  readonly triangles: number;
  readonly geometries: number;
  readonly textures: number;
  readonly cpuPreparationUs: number;
  readonly contextLost: boolean;
  readonly pixelRatio: number;
  readonly frameP95Us: number;
  readonly frameSamples: number;
}

/**
 * The host-facing world-view contract. Rendering remains replaceable presentation: callers name
 * snapshots, native-resolved previews, and screen gestures, never scene nodes or matrices.
 */
export interface FactoryRenderer {
  readonly motionReduced: boolean;
  setSnapshot(snapshot: FactorySnapshot): void;
  setHome(point: WorldPoint | null): void;
  setReducedMotion(value: boolean): void;
  setHover(
    coordinate: AxialCoordinate | null,
    placement?: PlacementPreview | null,
  ): void;
  setSelection(coordinate: AxialCoordinate | null): void;
  setBuildMode(active: boolean): void;
  setDragPath(cells: LinePreviewCell[]): void;
  setBuildFootprint(footprint: AxialCoordinate[], orientation: number): void;
  setBuildReach(definition: BuildingDefinition | null): void;
  setGathering(active: boolean): void;
  toggleGrid(): boolean;
  pick(clientX: number, clientY: number): AxialCoordinate;
  pickWorld(clientX: number, clientY: number): WorldPoint;
  /** Convert an on-screen direction into a unit direction on the native world plane. */
  screenMovement(x: number, y: number): WorldPoint;
  panBy(x: number, y: number): void;
  zoomAt(clientX: number, clientY: number, factor: number): void;
  orbitBy(step: -1 | 1): void;
  recenter(): void;
  renderFrame(now: number): void;
  draw(): void;
  setGraphicsProfile(profile: GraphicsProfile): void;
  getGraphicsProfile(): GraphicsProfile;
  getDiagnostics(): RendererDiagnostics;
  dispose(): void;
}
