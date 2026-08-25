import {
  ACESFilmicToneMapping,
  AmbientLight,
  BoxGeometry,
  Color,
  CylinderGeometry,
  DirectionalLight,
  Group,
  HemisphereLight,
  Mesh,
  MeshStandardMaterial,
  OrthographicCamera,
  Scene,
  SphereGeometry,
  SRGBColorSpace,
  WebGLRenderer,
} from "three";

import type { BuildingDefinition, EntitySnapshot } from "../../core/types";
import { partsFor, silhouetteOf, stallMark } from "../buildingLook";
import { BUILDING_COLORS } from "../FactoryRenderer";
import {
  MACHINE_SILHOUETTE_SCALE,
  machinePartMatrix,
  PartGeometryLibrary,
  type MachinePartInstance,
} from "./machineMeshes";
import {
  createTransportGeometry,
  isTransportKind,
  transportScale,
} from "./transportGeometry";

const ORBITS = 6;

/**
 * One offscreen Three.js scene for the entire contact sheet. Destination cells are ordinary 2D
 * canvases copied from this one retained WebGL context; a definition, tier, status, or orbit never
 * owns a renderer or a hand-authored model.
 */
export class ContactSheetRenderer {
  readonly orbitCount = ORBITS;
  private readonly renderer: WebGLRenderer;
  private readonly scene = new Scene();
  private readonly camera = new OrthographicCamera(
    -1.45,
    1.45,
    1.45,
    -1.45,
    0.1,
    20,
  );
  private readonly model = new Group();
  private readonly geometries = new PartGeometryLibrary();
  private readonly transportGeometry = createTransportGeometry();
  private readonly footGeometry = new CylinderGeometry(0.72, 0.78, 0.16, 6);
  private readonly statusGeometry = new SphereGeometry(0.1, 5, 3);
  private readonly progressGeometry = new BoxGeometry(0.5, 0.07, 0.08);
  private readonly materials = new Map<string, MeshStandardMaterial>();
  private readonly ratio: number;

  constructor(
    private readonly source: HTMLCanvasElement,
    readonly cellSize: number,
  ) {
    this.ratio = Math.min(window.devicePixelRatio || 1, 1.5);
    source.width = Math.floor(cellSize * this.ratio);
    source.height = Math.floor(cellSize * this.ratio);
    source.style.width = `${cellSize}px`;
    source.style.height = `${cellSize}px`;
    this.renderer = new WebGLRenderer({
      canvas: source,
      alpha: false,
      antialias: true,
      preserveDrawingBuffer: true,
      premultipliedAlpha: false,
    });
    this.renderer.setPixelRatio(this.ratio);
    this.renderer.setSize(cellSize, cellSize, false);
    this.renderer.outputColorSpace = SRGBColorSpace;
    this.renderer.toneMapping = ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.05;
    this.renderer.setClearColor("#0a1513", 1);
    this.scene.background = new Color("#0a1513");
    const key = new DirectionalLight("#ffe4b0", 2.6);
    key.position.set(-3, 6, 4);
    this.scene.add(
      new HemisphereLight("#d7eff0", "#26342f", 1.6),
      new AmbientLight("#9bb7af", 0.46),
      key,
      this.model,
    );
  }

  paintStrip(
    target: HTMLCanvasElement,
    definition: BuildingDefinition,
    tier: number,
    status: string,
    cycle: number,
    colour: boolean,
    now: number,
  ): void {
    const width = this.cellSize * ORBITS;
    target.width = Math.floor(width * this.ratio);
    target.height = Math.floor(this.cellSize * this.ratio);
    target.style.width = `${width}px`;
    target.style.height = `${this.cellSize}px`;
    const context = target.getContext("2d");
    if (!context) return;
    context.setTransform(1, 0, 0, 1, 0, 0);
    context.clearRect(0, 0, target.width, target.height);
    for (let orbit = 0; orbit < ORBITS; orbit += 1) {
      this.buildModel(definition, tier, status, cycle, colour, now);
      this.placeCamera(orbit);
      this.renderer.render(this.scene, this.camera);
      const sourceWidth = this.source.width;
      const sourceHeight = this.source.height;
      context.drawImage(
        this.source,
        0,
        0,
        sourceWidth,
        sourceHeight,
        orbit * this.cellSize * this.ratio,
        0,
        this.cellSize * this.ratio,
        this.cellSize * this.ratio,
      );
    }
  }

  dispose(): void {
    this.geometries.dispose();
    this.footGeometry.dispose();
    this.statusGeometry.dispose();
    this.progressGeometry.dispose();
    this.transportGeometry.belt.dispose();
    this.transportGeometry.beltDetail.dispose();
    this.transportGeometry.bridge.dispose();
    for (const material of this.materials.values()) material.dispose();
    this.renderer.dispose();
  }

  private buildModel(
    definition: BuildingDefinition,
    tier: number,
    status: string,
    cycle: number,
    colour: boolean,
    now: number,
  ): void {
    this.model.clear();
    const baseColour = colour ? BUILDING_COLORS[definition.kind] : "#747b79";
    const building = fakeEntity(definition, status, cycle);
    const foot = new Mesh(this.footGeometry, this.material("foot", "#26312e"));
    foot.position.y = 0.08;
    this.model.add(foot);
    const key = silhouetteOf(
      definition.kind,
      definition.recipe_category,
      definition.power_source,
    );
    for (const part of partsFor(
      key,
      tier,
      definition.kind === "hub" ? tier : 0,
    )) {
      const instance: MachinePartInstance = {
        building,
        part,
        key: `${part.part}:${part.count ?? 0}`,
        animated: part.phase !== undefined && part.phase !== "still",
        color: baseColour,
        glow: part.glow ?? null,
        material: part.material ?? "structure",
        groundHeight: 0,
        footprintScale: 1,
        visualScale: MACHINE_SILHOUETTE_SCALE[key],
        x: 0,
        z: 0,
      };
      const partColour = colour
        ? contactPartColour(baseColour, part.material ?? "structure")
        : baseColour;
      const material = part.glow
        ? this.material(`glow:${part.glow}`, part.glow, true)
        : this.material(`body:${partColour}`, partColour);
      const mesh = new Mesh(this.geometries.get(part), material);
      mesh.matrixAutoUpdate = false;
      mesh.matrix.copy(machinePartMatrix(instance, now, false));
      this.model.add(mesh);
    }
    if (isTransportKind(definition.kind)) {
      const geometry =
        definition.kind === "bridge"
          ? this.transportGeometry.bridge
          : this.transportGeometry.belt;
      const deck = new Mesh(
        geometry,
        this.material(`body:${baseColour}`, baseColour),
      );
      // The portrait poses every building at the same heading the fake entity above carries, so a
      // belt sits for its catalogue photograph at an edge step rather than the two-row reach.
      const [x, y, z] = transportScale(building.kind, building.orientation);
      deck.position.y = definition.kind === "bridge" ? 0.25 : 0.22;
      deck.scale.set(x, y, z);
      this.model.add(deck);
      if (definition.kind === "belt") {
        const treads = new Mesh(
          this.transportGeometry.beltDetail,
          this.material("belt-treads", colour ? "#102b3a" : "#343a38"),
        );
        treads.position.y = 0.22;
        treads.scale.set(x, y, z);
        this.model.add(treads);
      }
    }
    const mark = stallMark(status);
    if (mark) {
      const bead = new Mesh(
        this.statusGeometry,
        this.material(`status:${mark}`, mark, true),
      );
      bead.position.set(0.52, 0.75, 0);
      this.model.add(bead);
    }
    if (cycle > 0) {
      const progress = new Mesh(
        this.progressGeometry,
        this.material("progress", "#7fe0c0", true),
      );
      progress.position.set(-0.25 + cycle * 0.25, 0.68, -0.48);
      progress.scale.x = Math.max(0.06, cycle);
      this.model.add(progress);
    }
  }

  private placeCamera(orbit: number): void {
    const angle = (orbit * Math.PI) / 3;
    this.camera.position.set(Math.sin(angle) * 4.4, 3.7, Math.cos(angle) * 4.4);
    this.camera.lookAt(0, 0.72, 0);
    this.camera.updateMatrixWorld();
  }

  private material(
    key: string,
    colour: string,
    emissive = false,
  ): MeshStandardMaterial {
    let material = this.materials.get(key);
    if (!material) {
      material = new MeshStandardMaterial({
        color: colour,
        emissive: emissive ? colour : "#202826",
        emissiveIntensity: emissive ? 0.45 : 0.2,
        flatShading: true,
        metalness: emissive ? 0.08 : 0.24,
        roughness: emissive ? 0.4 : 0.74,
      });
      this.materials.set(key, material);
    }
    return material;
  }
}

function contactPartColour(
  base: string,
  role: MachinePartInstance["material"],
): string {
  const color = new Color(base);
  if (role === "ceramic") color.lerp(new Color("#d9d1b8"), 0.68);
  else if (role === "brass") color.lerp(new Color("#bf8948"), 0.78);
  else if (role === "dark") color.lerp(new Color("#142126"), 0.74);
  return `#${color.getHexString()}`;
}

function fakeEntity(
  definition: BuildingDefinition,
  status: string,
  cycle: number,
): EntitySnapshot {
  return {
    id: definition.id,
    q: 0,
    r: 0,
    definition_id: definition.id,
    kind: definition.kind,
    orientation: 0,
    scenario_owned: false,
    inventory: [],
    progress: Math.round(cycle * 1000),
    progress_total: 1000,
    status,
    footprint: [{ q: 0, r: 0 }],
  };
}
