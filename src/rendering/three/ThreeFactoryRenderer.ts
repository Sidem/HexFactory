import {
  ACESFilmicToneMapping,
  AmbientLight,
  Color,
  DirectionalLight,
  HemisphereLight,
  PCFShadowMap,
  Scene,
  SRGBColorSpace,
  Vector3,
  WebGLRenderer,
} from "three";
import { rotateAxial, type AxialCoordinate } from "@hexlife/embed/hex";

import type {
  BuildingDefinition,
  Definitions,
  FactorySnapshot,
  LinePreviewCell,
  PlacementPreview,
  WorldPoint,
} from "../../core/types";
import type {
  FactoryRenderer,
  GraphicsProfile,
  ReachRadii,
  RendererDiagnostics,
} from "../FactoryRenderer";
import { WORLD_SCALE } from "../landmarks";
import { HexSceneCamera } from "./HexSceneCamera";
import { createWorldMaterials, type WorldMaterials } from "./materials";
import { SpatialOverlays, type SpatialOverlayState } from "./overlays";
import { QUALITY_SETTINGS } from "./quality";
import { buildTerrainMeshes, type TerrainBuild } from "./terrainMeshes";
import { WorldInstanceLayer } from "./worldInstances";

/** Three.js low-poly diorama over the unchanged native axial plane. */
export class ThreeFactoryRenderer implements FactoryRenderer {
  readonly camera = new HexSceneCamera();
  private readonly renderer: WebGLRenderer;
  private readonly scene = new Scene();
  private readonly materials: WorldMaterials;
  private readonly worldInstances: WorldInstanceLayer;
  private readonly overlays: SpatialOverlays;
  private readonly keyLight = new DirectionalLight("#ffe4b0", 2.6);
  private readonly fillLight = new HemisphereLight("#c9eef0", "#273b32", 1.6);
  private readonly ambient = new AmbientLight("#9bb7af", 0.46);
  private readonly systemReducedMotion = matchMedia(
    "(prefers-reduced-motion: reduce)",
  ).matches;
  private forcedReducedMotion = false;
  private snapshot: FactorySnapshot | null = null;
  private hover: AxialCoordinate | null = null;
  private selection: AxialCoordinate | null = null;
  private placement: PlacementPreview | null = null;
  private buildMode = false;
  private gridToggled = false;
  private buildFootprint: AxialCoordinate[] = [{ q: 0, r: 0 }];
  private buildOrientation = 0;
  private buildReach: ReachRadii | null = null;
  private gathering = false;
  private dragPath: LinePreviewCell[] = [];
  private terrain: TerrainBuild | null = null;
  private lastChunks: FactorySnapshot["chunks"] | null = null;
  private lastTerrain: FactorySnapshot["terrain"] | null = null;
  private layout = { width: 1, height: 1, left: 0, top: 0 };
  private layoutDirty = true;
  private needsDraw = true;
  private now = 0;
  private contextLost = false;
  private prepUs = 0;
  private profile: GraphicsProfile;
  private disposed = false;
  private overlaysDirty = true;
  private compiled = false;
  private shadowX = Number.POSITIVE_INFINITY;
  private shadowZ = Number.POSITIVE_INFINITY;
  private readonly lightOffset = new Vector3(-16, 28, 12);
  private readonly emptyTerrain = new Map<string, never>();
  private readonly frameTimesUs = new Float64Array(240);
  private frameTimeCursor = 0;
  private frameTimeCount = 0;

  constructor(
    private readonly canvas: HTMLCanvasElement,
    definitions: Definitions,
    profile: GraphicsProfile = "medium",
  ) {
    this.profile = profile;
    this.renderer = new WebGLRenderer({
      canvas,
      antialias: profile !== "low",
      alpha: false,
      depth: true,
      powerPreference: "high-performance",
      premultipliedAlpha: false,
    });
    this.renderer.outputColorSpace = SRGBColorSpace;
    this.renderer.toneMapping = ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.05;
    this.renderer.shadowMap.type = PCFShadowMap;
    this.renderer.shadowMap.autoUpdate = false;
    this.renderer.setClearColor(new Color("#142129"), 1);
    this.scene.background = new Color("#142129");
    this.scene.fog = null;
    this.materials = createWorldMaterials();
    this.worldInstances = new WorldInstanceLayer(definitions, this.materials);
    this.overlays = new SpatialOverlays(this.materials);
    this.scene.add(
      this.fillLight,
      this.ambient,
      this.keyLight,
      this.keyLight.target,
      this.worldInstances.group,
      this.overlays.group,
    );
    this.keyLight.position.set(-16, 28, 12);
    this.keyLight.target.position.set(0, 0, 0);
    this.keyLight.castShadow = true;
    this.keyLight.shadow.camera.near = 1;
    this.keyLight.shadow.camera.far = 90;
    this.keyLight.shadow.camera.left = -22;
    this.keyLight.shadow.camera.right = 22;
    this.keyLight.shadow.camera.top = 22;
    this.keyLight.shadow.camera.bottom = -22;
    this.applyProfile();
    canvas.dataset.rendererContext = "ready";
    canvas.addEventListener("webglcontextlost", this.onContextLost, false);
    canvas.addEventListener(
      "webglcontextrestored",
      this.onContextRestored,
      false,
    );
    canvas.addEventListener(
      "hexfactory:test-context-cycle",
      this.onTestContextCycle,
    );
    new ResizeObserver(() => {
      this.layoutDirty = true;
      this.markDirty();
    }).observe(canvas);
  }

  get motionReduced(): boolean {
    return this.systemReducedMotion || this.forcedReducedMotion;
  }

  setSnapshot(snapshot: FactorySnapshot): void {
    const started = performance.now();
    this.snapshot = snapshot;
    this.camera.follow(snapshot.player);
    if (
      snapshot.chunks !== this.lastChunks ||
      snapshot.terrain !== this.lastTerrain
    ) {
      this.lastChunks = snapshot.chunks;
      this.lastTerrain = snapshot.terrain;
      this.rebuildTerrain(snapshot);
    }
    const structureChanged = this.worldInstances.setSnapshot(
      snapshot,
      this.terrain?.cellByKey ?? this.emptyTerrain,
    );
    this.overlaysDirty = true;
    if (structureChanged) this.renderer.shadowMap.needsUpdate = true;
    if (!this.compiled) {
      this.renderer.compile(this.scene, this.camera.camera);
      this.compiled = true;
    }
    this.prepUs = smooth(this.prepUs, (performance.now() - started) * 1000);
    this.markDirty();
  }

  setHome(): void {}

  setReducedMotion(value: boolean): void {
    this.forcedReducedMotion = value;
    this.markDirty();
  }

  setHover(
    coordinate: AxialCoordinate | null,
    placement: PlacementPreview | null = null,
  ): void {
    this.hover = coordinate;
    this.placement = placement;
    this.overlaysDirty = true;
    this.markDirty();
  }

  setSelection(coordinate: AxialCoordinate | null): void {
    this.selection = coordinate;
    this.overlaysDirty = true;
    this.markDirty();
  }

  setBuildMode(active: boolean): void {
    this.buildMode = active;
    this.overlaysDirty = true;
    this.markDirty();
  }

  setDragPath(cells: LinePreviewCell[]): void {
    this.dragPath = cells;
    this.overlaysDirty = true;
    this.markDirty();
  }

  setBuildFootprint(footprint: AxialCoordinate[], orientation: number): void {
    this.buildOrientation = orientation;
    this.buildFootprint = footprint.map((cell) =>
      rotateAxial(cell, orientation, { q: 0, r: 0 }),
    );
    this.overlaysDirty = true;
    this.markDirty();
  }

  setBuildReach(definition: BuildingDefinition | null): void {
    this.buildReach = definition
      ? {
          extract: definition.extract_radius ?? null,
          supply: definition.supply_radius ?? null,
          link: definition.pole_reach ?? null,
        }
      : null;
    this.overlaysDirty = true;
    this.markDirty();
  }

  setGathering(active: boolean): void {
    if (this.gathering === active) return;
    this.gathering = active;
    this.overlaysDirty = true;
    this.markDirty();
  }

  toggleGrid(): boolean {
    this.gridToggled = !this.gridToggled;
    this.overlaysDirty = true;
    this.markDirty();
    return this.gridToggled;
  }

  pick(clientX: number, clientY: number): AxialCoordinate {
    this.syncLayout();
    return this.camera.axialAt(
      clientX - this.layout.left,
      clientY - this.layout.top,
    );
  }

  pickWorld(clientX: number, clientY: number): WorldPoint {
    this.syncLayout();
    return this.camera.worldAt(
      clientX - this.layout.left,
      clientY - this.layout.top,
    );
  }

  screenMovement(x: number, y: number): WorldPoint {
    this.syncLayout();
    return this.camera.screenMovement(x, y);
  }

  panBy(x: number, y: number): void {
    this.syncLayout();
    this.camera.panBy(x, y);
    this.markDirty();
  }

  zoomAt(clientX: number, clientY: number, factor: number): void {
    this.syncLayout();
    this.camera.zoomAt(
      clientX - this.layout.left,
      clientY - this.layout.top,
      factor,
    );
    this.markDirty();
  }

  orbitBy(step: -1 | 1): void {
    this.camera.orbitBy(step);
    this.markDirty();
  }

  recenter(): void {
    if (this.snapshot) this.camera.recenter(this.snapshot.player);
    this.markDirty();
  }

  renderFrame(now: number): void {
    this.now = now;
    if (this.needsDraw || !this.motionReduced) this.draw();
    this.needsDraw = false;
  }

  draw(): void {
    if (this.disposed || this.contextLost) return;
    const frameStarted = performance.now();
    this.syncLayout();
    const snapshot = this.snapshot;
    if (snapshot) {
      const started = performance.now();
      this.worldInstances.update(this.now, this.motionReduced);
      if (this.overlaysDirty) {
        this.overlays.update(
          snapshot,
          this.overlayState(),
          this.terrain?.cellByKey ?? this.emptyTerrain,
        );
        this.overlaysDirty = false;
      }
      const playerX = snapshot.player.x / WORLD_SCALE;
      const playerZ = snapshot.player.y / WORLD_SCALE;
      if (
        Math.abs(playerX - this.shadowX) >= 2 ||
        Math.abs(playerZ - this.shadowZ) >= 2
      ) {
        this.shadowX = playerX;
        this.shadowZ = playerZ;
        this.keyLight.target.position.set(playerX, 0, playerZ);
        this.keyLight.position
          .copy(this.keyLight.target.position)
          .add(this.lightOffset);
        this.renderer.shadowMap.needsUpdate = true;
      }
      this.prepUs = smooth(this.prepUs, (performance.now() - started) * 1000);
    }
    this.renderer.render(this.scene, this.camera.camera);
    this.frameTimesUs[this.frameTimeCursor] =
      (performance.now() - frameStarted) * 1000;
    this.frameTimeCursor =
      (this.frameTimeCursor + 1) % this.frameTimesUs.length;
    this.frameTimeCount = Math.min(
      this.frameTimeCount + 1,
      this.frameTimesUs.length,
    );
  }

  setGraphicsProfile(profile: GraphicsProfile): void {
    if (profile === this.profile) return;
    this.profile = profile;
    this.applyProfile();
    this.layoutDirty = true;
    this.markDirty();
  }

  getGraphicsProfile(): GraphicsProfile {
    return this.profile;
  }

  getDiagnostics(): RendererDiagnostics {
    const info = this.renderer.info;
    return {
      name: "Three.js Visual Depth",
      profile: this.profile,
      drawCalls: info.render.calls,
      triangles: info.render.triangles,
      geometries: info.memory.geometries,
      textures: info.memory.textures,
      cpuPreparationUs: this.prepUs,
      contextLost: this.contextLost,
      pixelRatio: this.renderer.getPixelRatio(),
      frameP95Us: percentile95(this.frameTimesUs, this.frameTimeCount),
      frameSamples: this.frameTimeCount,
    };
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.canvas.removeEventListener("webglcontextlost", this.onContextLost);
    this.canvas.removeEventListener(
      "webglcontextrestored",
      this.onContextRestored,
    );
    this.canvas.removeEventListener(
      "hexfactory:test-context-cycle",
      this.onTestContextCycle,
    );
    this.disposeTerrain();
    this.worldInstances.dispose();
    this.overlays.dispose();
    for (const material of this.materials.materials) material.dispose();
    this.renderer.dispose();
  }

  private rebuildTerrain(snapshot: FactorySnapshot): void {
    this.disposeTerrain();
    this.terrain = buildTerrainMeshes(snapshot, this.materials);
    this.scene.add(this.terrain.group);
    this.overlays.setTerrain(this.terrain.cells);
  }

  private disposeTerrain(): void {
    if (!this.terrain) return;
    this.scene.remove(this.terrain.group);
    for (const geometry of this.terrain.geometries) geometry.dispose();
    this.terrain = null;
  }

  private overlayState(): SpatialOverlayState {
    return {
      hover: this.hover,
      selection: this.selection,
      placement: this.placement,
      dragPath: this.dragPath,
      buildMode: this.buildMode,
      gridToggled: this.gridToggled,
      buildFootprint: this.buildFootprint,
      buildOrientation: this.buildOrientation,
      buildReach: this.buildReach,
      gathering: this.gathering,
    };
  }

  private syncLayout(): void {
    if (!this.layoutDirty) return;
    const rect = this.canvas.getBoundingClientRect();
    const width = Math.max(1, this.canvas.clientWidth);
    const height = Math.max(1, this.canvas.clientHeight);
    this.layout = { width, height, left: rect.left, top: rect.top };
    this.camera.resize(width, height);
    const cap = QUALITY_SETTINGS[this.profile].pixelRatioCap;
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, cap));
    this.renderer.setSize(width, height, false);
    this.layoutDirty = false;
  }

  private applyProfile(): void {
    const settings = QUALITY_SETTINGS[this.profile];
    this.renderer.shadowMap.enabled = settings.shadows;
    this.keyLight.castShadow = settings.shadows;
    if (settings.shadows) {
      this.keyLight.shadow.mapSize.set(
        settings.shadowMapSize,
        settings.shadowMapSize,
      );
      this.keyLight.shadow.map?.dispose();
      this.keyLight.shadow.map = null;
      this.renderer.shadowMap.needsUpdate = true;
    }
  }

  private markDirty(): void {
    this.needsDraw = true;
  }

  private readonly onContextLost = (event: Event): void => {
    event.preventDefault();
    this.contextLost = true;
    this.canvas.dataset.rendererContext = "lost";
    this.needsDraw = false;
  };

  private readonly onContextRestored = (): void => {
    // Three restores programs and buffers for the retained scene. Recompile the bounded material
    // set immediately so the first interactive frame does not pay every shader at once.
    this.contextLost = false;
    this.canvas.dataset.rendererContext = "restored";
    this.renderer.compile(this.scene, this.camera.camera);
    this.compiled = true;
    this.layoutDirty = true;
    this.markDirty();
  };

  private readonly onTestContextCycle = (): void => {
    if (!import.meta.env.DEV) return;
    this.renderer.forceContextLoss();
    window.setTimeout(() => this.renderer.forceContextRestore(), 250);
  };
}

function smooth(previous: number, next: number): number {
  return previous === 0 ? next : previous * 0.86 + next * 0.14;
}

function percentile95(samples: Float64Array, count: number): number {
  if (count === 0) return 0;
  const ordered = Array.from(samples.subarray(0, count)).sort((a, b) => a - b);
  return ordered[Math.ceil(ordered.length * 0.95) - 1] ?? 0;
}
