import type { AxialCoordinate } from "@hexlife/embed/hex";

import type {
  BoundaryAnchor,
  BoundaryPreview,
  BuildingDefinition,
  ChunkSnapshot,
  EntitySnapshot,
  FactorySnapshot,
  GroundPreview,
  LinePreviewCell,
  PlacementPreview,
  WorldPoint,
} from "../core/types";

export const BASE_HEX_SIZE = 22;
export const MAX_DEVICE_PIXEL_RATIO = 1.5;

/** One material family per native kind; definitions vary by generated anatomy, never an id case. */
export const BUILDING_COLORS: Record<EntitySnapshot["kind"], string> = {
  extractor: "#e05f3f",
  belt: "#2196c7",
  composer: "#956fe8",
  container: "#c98b31",
  consumer: "#35a77d",
  hub: "#e6b83f",
  pump: "#27a6ce",
  pole: "#e6cf68",
  generator: "#ed9f1a",
  boiler: "#d86932",
  bridge: "#aa8151",
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
  /** True while the view is still easing toward a requested orbit, so the scene moves on its own. */
  readonly cameraSettling: boolean;
  setSnapshot(snapshot: FactorySnapshot): void;
  setBoundaryPreview(preview: BoundaryPreview | null): void;
  /**
   * The lattice vertices a selection is pinned to. Separate from the preview because the first
   * click of a two-vertex selection has no run to price yet and still has to show where it landed.
   */
  setBoundaryAnchors(anchors: readonly BoundaryAnchor[]): void;
  setGroundPreview(preview: GroundPreview | null): void;
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
  tiltBy(step: -1 | 1): void;
  recenter(): void;
  renderFrame(now: number): void;
  draw(): void;
  setGraphicsProfile(profile: GraphicsProfile): void;
  getGraphicsProfile(): GraphicsProfile;
  getDiagnostics(): RendererDiagnostics;
  dispose(): void;
}
