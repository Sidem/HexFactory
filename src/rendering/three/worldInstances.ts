import {
  BoxGeometry,
  Color,
  ConeGeometry,
  CylinderGeometry,
  Group,
  IcosahedronGeometry,
  InstancedMesh,
  Matrix4,
  Mesh,
  OctahedronGeometry,
  Quaternion,
  RingGeometry,
  Vector3,
} from "three";
import { axialToPixel, pixelToAxial } from "@hexlife/embed/hex";

import type {
  BuildingDefinition,
  Definitions,
  EntitySnapshot,
  FactorySnapshot,
  ItemDefinition,
  ResourceSnapshot,
} from "../../core/types";
import { TRANSPORT_DIRECTIONS } from "../../core/directions";
import { MAX_UNDERPASS_SPAN } from "../../core/definitions";
import {
  beltLaneTravel,
  cargoTravel,
  stallMark,
  trimOf,
} from "../buildingLook";
import { BUILDING_COLORS } from "../FactoryRenderer";
import { WORLD_SCALE } from "../landmarks";
import {
  PartGeometryLibrary,
  collectMachineParts,
  machinePartMatrix,
  type MachinePartInstance,
} from "./machineMeshes";
import type { WorldMaterials } from "./materials";
import type { TerrainCell } from "./terrainMeshes";
import { heightAt, stableVariation } from "./terrainMeshes";
import {
  createCurvedTransportGeometry,
  createTransportGeometry,
  isTransportKind,
  transportScale,
  type CurvedTransportGeometry,
} from "./transportGeometry";
import { directionAngle } from "./directionAngle";

interface PartBucket {
  readonly mesh: InstancedMesh;
  readonly instances: MachinePartInstance[];
  readonly animated: boolean;
}

interface ResourcePartInstance {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly angle: number;
  readonly scaleX: number;
  readonly scaleY: number;
  readonly scaleZ: number;
  readonly color: string;
}

export interface PowerWireLink {
  readonly fromId: number;
  readonly toId: number;
}

/** The field repeats the inventory glyph vocabulary as unmistakable 3D silhouettes. */
export const FIELD_RESOURCE_SHAPES = Object.freeze({
  ore: "faceted-shards",
  lump: "boulder-cluster",
  grains: "low-mounds",
  crystal: "prismatic-spires",
  log: "trunk-and-canopy",
});

/** A person must read at world scale, not as another inventory token dropped on one hex. */
export const WAYFINDER_VISUAL_SCALE = 3.2;

export class WorldInstanceLayer {
  readonly group = new Group();
  readonly geometryLibrary = new PartGeometryLibrary();
  private readonly transportGeometry = createTransportGeometry();
  private readonly curvedTransportGeometry = new Map<
    string,
    CurvedTransportGeometry
  >();
  private readonly definitions: ReadonlyMap<number, BuildingDefinition>;
  private readonly items: ReadonlyMap<number, ItemDefinition>;
  private readonly geometry = {
    buildingFoot: new CylinderGeometry(0.72, 0.78, 0.18, 6),
    footprintDeck: new CylinderGeometry(0.94, 0.98, 1, 6),
    footprintLink: new BoxGeometry(1, 1, 1),
    belt: this.transportGeometry.belt,
    beltDetail: this.transportGeometry.beltDetail,
    pipe: this.transportGeometry.pipe,
    pipeDetail: this.transportGeometry.pipeDetail,
    portal: this.transportGeometry.portal,
    portalDetail: this.transportGeometry.portalDetail,
    bridge: this.transportGeometry.bridge,
    ore: new OctahedronGeometry(1, 0),
    lump: new IcosahedronGeometry(1, 0),
    grains: new ConeGeometry(1, 1, 8),
    crystal: new ConeGeometry(1, 1, 4),
    trunk: new CylinderGeometry(1, 1, 1, 7),
    canopy: new ConeGeometry(1, 1, 7),
    progress: new BoxGeometry(0.38, 0.08, 0.1),
    // Big enough to be the thing you watch. A belt with a speck on it reads as an empty belt, and
    // "is anything actually moving?" is the first question a factory has to answer at a glance.
    cargo: new IcosahedronGeometry(0.17, 0),
    status: new SphereGeometryCompat(0.09),
    plume: new IcosahedronGeometry(0.16, 1),
    scar: new CylinderGeometry(0.34, 0.38, 0.025, 6),
    outputIndicator: outputIndicatorGeometry(),
    wireSegment: new CylinderGeometry(1, 1, 1, 6),
  };
  private readonly ownedGeometries = Object.values(this.geometry);
  private staticGroup = new Group();
  private resourceGroup = new Group();
  private readonly dynamicGroup = new Group();
  private readonly playerGroup = new Group();
  private partBuckets: PartBucket[] = [];
  private structureKey = "";
  private resourcesIdentity: FactorySnapshot["resources"] | null = null;
  private snapshot: FactorySnapshot | null = null;
  private terrainByKey: ReadonlyMap<string, TerrainCell> = new Map();
  private statusMesh: InstancedMesh | null = null;
  private progressMesh: InstancedMesh | null = null;
  private cargoMesh: InstancedMesh | null = null;
  private groundItemMesh: InstancedMesh | null = null;
  private plumeMesh: InstancedMesh | null = null;
  private readonly playerBody: Mesh;
  private readonly playerShell: Mesh;
  private readonly playerHead: Mesh;
  private readonly playerFacing: Mesh;
  private readonly playerPack: Mesh;
  private readonly playerBeacon: Mesh;
  private readonly playerLeftLeg: Mesh;
  private readonly playerRightLeg: Mesh;
  private readonly playerLeftArm: Mesh;
  private readonly playerRightArm: Mesh;
  private readonly playerTool: Mesh;
  private readonly playerWork: Mesh;
  private readonly playerMeshes: readonly Mesh[];
  private readonly pointById = new Map<number, { x: number; z: number }>();
  private readonly groundById = new Map<number, number>();
  private readonly buildingById = new Map<number, EntitySnapshot>();
  private readonly scratchMatrix = new Matrix4();
  private readonly scratchPosition = new Vector3();
  private readonly scratchQuaternion = new Quaternion();
  private readonly scratchScale = new Vector3(1, 1, 1);
  private readonly scratchColor = new Color();
  private readonly scratchTrim = new Color();
  private playerDirty = true;
  private cargoTickAt = 0;
  private cargoTickMs = 250;

  constructor(
    definitions: Definitions,
    private readonly materials: WorldMaterials,
  ) {
    this.group.name = "world-instances";
    this.definitions = new Map(
      definitions.buildings.map((definition) => [definition.id, definition]),
    );
    this.items = new Map(definitions.items.map((item) => [item.id, item]));
    this.staticGroup.name = "static-factory";
    this.resourceGroup.name = "field-resources";
    this.dynamicGroup.name = "dynamic-factory-state";
    this.playerGroup.name = "player";
    this.playerGroup.scale.setScalar(WAYFINDER_VISUAL_SCALE);
    this.group.add(
      this.staticGroup,
      this.resourceGroup,
      this.dynamicGroup,
      this.playerGroup,
    );
    // The wayfinder casts no shadow. The shadow map is baked on demand rather than every
    // frame, so the one caster that moves continuously would leave its shadow standing a
    // stride behind it until the next bake caught up.
    this.playerBody = new Mesh(
      new CylinderGeometry(0.13, 0.16, 0.25, 7),
      materials.wayfinderHull,
    );
    this.playerBody.position.y = 0.31;
    this.playerShell = new Mesh(
      new CylinderGeometry(0.17, 0.14, 0.13, 7),
      materials.wayfinderShell,
    );
    this.playerShell.position.y = 0.43;
    this.playerHead = new Mesh(
      new IcosahedronGeometry(0.095, 1),
      materials.wayfinderShell,
    );
    this.playerHead.position.y = 0.535;
    this.playerFacing = new Mesh(
      new BoxGeometry(0.115, 0.045, 0.035),
      materials.wayfinderSignal,
    );
    this.playerFacing.position.set(0, 0.545, 0.083);
    this.playerPack = new Mesh(
      new BoxGeometry(0.17, 0.22, 0.1),
      materials.wayfinderBrass,
    );
    this.playerPack.position.set(0, 0.34, -0.13);
    this.playerBeacon = new Mesh(
      new OctahedronGeometry(0.035, 0),
      materials.wayfinderBrass,
    );
    this.playerBeacon.position.set(0, 0.65, -0.025);
    this.playerLeftLeg = new Mesh(
      new CylinderGeometry(0.04, 0.05, 0.2, 6),
      materials.wayfinderHull,
    );
    this.playerLeftLeg.position.set(-0.07, 0.105, 0);
    this.playerRightLeg = new Mesh(
      new CylinderGeometry(0.04, 0.05, 0.2, 6),
      materials.wayfinderHull,
    );
    this.playerRightLeg.position.set(0.07, 0.105, 0);
    this.playerLeftArm = new Mesh(
      new CylinderGeometry(0.03, 0.04, 0.21, 6),
      materials.wayfinderShell,
    );
    this.playerLeftArm.position.set(-0.17, 0.34, 0.015);
    this.playerLeftArm.rotation.z = -0.18;
    this.playerRightArm = new Mesh(
      new CylinderGeometry(0.03, 0.04, 0.21, 6),
      materials.wayfinderShell,
    );
    this.playerRightArm.position.set(0.17, 0.34, 0.015);
    this.playerRightArm.rotation.z = 0.18;
    this.playerTool = new Mesh(
      new CylinderGeometry(0.025, 0.04, 0.24, 6),
      materials.wayfinderBrass,
    );
    this.playerTool.position.set(0.22, 0.25, 0.075);
    this.playerTool.rotation.z = -0.5;
    this.playerWork = new Mesh(
      new RingGeometry(0.31, 0.36, 32),
      materials.overlaySelection,
    );
    this.playerWork.rotateX(-Math.PI / 2);
    this.playerWork.position.y = 0.025;
    this.playerMeshes = [
      this.playerBody,
      this.playerShell,
      this.playerHead,
      this.playerFacing,
      this.playerPack,
      this.playerBeacon,
      this.playerLeftLeg,
      this.playerRightLeg,
      this.playerLeftArm,
      this.playerRightArm,
      this.playerTool,
    ];
    this.playerGroup.add(...this.playerMeshes, this.playerWork);
  }

  setSnapshot(
    snapshot: FactorySnapshot,
    terrainByKey: ReadonlyMap<string, TerrainCell>,
    receivedAt = performance.now(),
  ): boolean {
    if (
      !this.snapshot ||
      snapshot.seed !== this.snapshot.seed ||
      snapshot.scenario !== this.snapshot.scenario ||
      snapshot.tick < this.snapshot.tick
    ) {
      this.cargoTickAt = receivedAt;
    } else if (snapshot.tick > this.snapshot.tick) {
      const tickDelta = snapshot.tick - this.snapshot.tick;
      const measured = (receivedAt - this.cargoTickAt) / tickDelta;
      if (measured >= 16 && measured <= 4_000) this.cargoTickMs = measured;
      this.cargoTickAt = receivedAt;
    }
    this.snapshot = snapshot;
    this.playerDirty = true;
    this.terrainByKey = terrainByKey;
    const nextStructure =
      snapshot.buildings
        .map((entity) =>
          [
            entity.id,
            entity.definition_id,
            entity.orientation,
            entity.q,
            entity.r,
            entity.next_id ?? 0,
            (entity.branch_ids ?? []).join(","),
            (entity.output_routes ?? [])
              .map(
                ({ item_id, q, r, direction, target_id }) =>
                  `${item_id}@${q},${r},${direction}>${target_id ?? 0}`,
              )
              .join(","),
          ].join(":"),
        )
        .join("|") + `@${snapshot.contract.stage}`;
    const structureChanged = nextStructure !== this.structureKey;
    if (structureChanged) {
      this.structureKey = nextStructure;
      this.rebuildStatic(snapshot);
    }
    if (snapshot.resources !== this.resourcesIdentity) {
      this.resourcesIdentity = snapshot.resources;
      this.rebuildResources(snapshot.resources);
    }
    const movingCargo = snapshot.buildings.reduce(
      (count, building) =>
        count + (building.cargo ? 1 : 0) + (building.lane?.length ?? 0),
      0,
    );
    this.ensureDynamicCapacity(
      Math.max(snapshot.buildings.length, movingCargo),
    );
    return structureChanged;
  }

  update(now: number, reducedMotion: boolean): void {
    const snapshot = this.snapshot;
    if (!snapshot) return;
    for (const bucket of this.partBuckets) {
      if (!bucket.animated) continue;
      for (let index = 0; index < bucket.instances.length; index += 1)
        bucket.mesh.setMatrixAt(
          index,
          machinePartMatrix(
            bucket.instances[index]!,
            now,
            reducedMotion,
            this.scratchMatrix,
          ),
        );
      bucket.mesh.instanceMatrix.needsUpdate = true;
    }
    this.updateDynamicBuildings(snapshot, now, reducedMotion);
    if (this.playerDirty) {
      this.playerDirty = false;
      this.updatePlayer(snapshot);
    }
  }

  dispose(): void {
    this.geometryLibrary.dispose();
    for (const geometry of this.ownedGeometries) geometry.dispose();
    for (const geometry of this.curvedTransportGeometry.values()) {
      geometry.frame.dispose();
      geometry.detail.dispose();
    }
    this.curvedTransportGeometry.clear();
    for (const mesh of this.playerMeshes) mesh.geometry.dispose();
    this.playerWork.geometry.dispose();
  }

  private rebuildStatic(snapshot: FactorySnapshot): void {
    this.group.remove(this.staticGroup);
    this.staticGroup = new Group();
    this.staticGroup.name = "static-factory";
    this.partBuckets = [];
    this.pointById.clear();
    this.groundById.clear();
    this.buildingById.clear();
    const matrix = new Matrix4();
    const quaternion = new Quaternion();
    const position = new Vector3();
    const scale = new Vector3(1, 1, 1);
    const color = new Color();

    const singleCellBuildings = snapshot.buildings.filter(
      (building) =>
        building.footprint.length <= 1 && !isTransportKind(building.kind),
    );
    for (const building of snapshot.buildings) {
      this.buildingById.set(building.id, building);
      const cells = building.footprint.length ? building.footprint : [building];
      const centers = cells.map((cell) =>
        axialToPixel(cell, 1, { x: 0, y: 0 }),
      );
      const center = centers.reduce(
        (sum, point) => ({ x: sum.x + point.x, z: sum.z + point.y }),
        { x: 0, z: 0 },
      );
      center.x /= centers.length;
      center.z /= centers.length;
      this.pointById.set(building.id, center);
      this.groundById.set(
        building.id,
        Math.max(...cells.map((cell) => this.groundHeight(cell.q, cell.r))),
      );
    }
    if (singleCellBuildings.length) {
      const baseMesh = new InstancedMesh(
        this.geometry.buildingFoot,
        this.materials.machineDark,
        singleCellBuildings.length,
      );
      baseMesh.name = "building-feet";
      baseMesh.castShadow = true;
      baseMesh.receiveShadow = true;
      for (const [index, building] of singleCellBuildings.entries()) {
        const center = this.pointById.get(building.id)!;
        const height = this.groundById.get(building.id)!;
        position.set(center.x, height + 0.09, center.z);
        scale.set(MACHINE_BASE_SCALE, 1, MACHINE_BASE_SCALE);
        matrix.compose(position, quaternion.identity(), scale);
        baseMesh.setMatrixAt(index, matrix);
        baseMesh.setColorAt(
          index,
          color
            .set(BUILDING_COLORS[building.kind])
            .lerp(this.scratchTrim.set("#101b20"), 0.3),
        );
      }
      markInstancesDirty(baseMesh);
      this.staticGroup.add(baseMesh);
    }
    this.addMultiCellFootprints(
      snapshot,
      matrix,
      position,
      quaternion,
      scale,
      color,
    );

    this.addTransportMeshes(
      snapshot,
      matrix,
      position,
      quaternion,
      scale,
      color,
    );
    this.addPartMeshes(snapshot);
    this.addOutputIndicators(snapshot);
    this.addPowerWires(snapshot);
    this.group.add(this.staticGroup);
  }

  private addMultiCellFootprints(
    snapshot: FactorySnapshot,
    matrix: Matrix4,
    position: Vector3,
    quaternion: Quaternion,
    scale: Vector3,
    color: Color,
  ): void {
    const buildings = snapshot.buildings.filter(
      (building) => building.footprint.length > 1,
    );
    const deckCount = buildings.reduce(
      (total, building) => total + building.footprint.length,
      0,
    );
    if (!deckCount) return;

    const decks = new InstancedMesh(
      this.geometry.footprintDeck,
      this.materials.machineDark,
      deckCount,
    );
    decks.name = "multi-cell-decks";
    decks.castShadow = true;
    decks.receiveShadow = true;
    let deckIndex = 0;
    for (const building of buildings) {
      const platformTop = (this.groundById.get(building.id) ?? 0.07) + 0.18;
      for (const cell of building.footprint) {
        const center = axialToPixel(cell, 1, { x: 0, y: 0 });
        const ground = this.groundHeight(cell.q, cell.r);
        const depth = Math.max(0.18, platformTop - ground);
        position.set(center.x, ground + depth / 2, center.y);
        scale.set(1, depth, 1);
        matrix.compose(position, quaternion.identity(), scale);
        decks.setMatrixAt(deckIndex, matrix);
        decks.setColorAt(
          deckIndex,
          color
            .set(BUILDING_COLORS[building.kind])
            .lerp(this.scratchTrim.set("#101b20"), 0.24),
        );
        deckIndex += 1;
      }
    }
    markInstancesDirty(decks);
    this.staticGroup.add(decks);

    const links = buildings.flatMap((building) =>
      adjacentFootprintPairs(building).map(([from, to]) => ({
        building,
        from,
        to,
      })),
    );
    if (!links.length) return;
    const linkMesh = new InstancedMesh(
      this.geometry.footprintLink,
      this.materials.machineDark,
      links.length,
    );
    linkMesh.name = "multi-cell-links";
    linkMesh.castShadow = true;
    linkMesh.receiveShadow = true;
    for (const [index, { building, from, to }] of links.entries()) {
      const a = axialToPixel(from, 1, { x: 0, y: 0 });
      const b = axialToPixel(to, 1, { x: 0, y: 0 });
      const dx = b.x - a.x;
      const dz = b.y - a.y;
      const length = Math.hypot(dx, dz);
      position.set(
        (a.x + b.x) / 2,
        (this.groundById.get(building.id) ?? 0.07) + 0.19,
        (a.y + b.y) / 2,
      );
      quaternion.setFromAxisAngle(new Vector3(0, 1, 0), -Math.atan2(dz, dx));
      scale.set(length * 0.76, 0.1, 0.9);
      matrix.compose(position, quaternion, scale);
      linkMesh.setMatrixAt(index, matrix);
      linkMesh.setColorAt(
        index,
        color
          .set(BUILDING_COLORS[building.kind])
          .lerp(this.scratchTrim.set("#dcefe6"), 0.08),
      );
    }
    markInstancesDirty(linkMesh);
    this.staticGroup.add(linkMesh);
  }

  private addTransportMeshes(
    snapshot: FactorySnapshot,
    matrix: Matrix4,
    position: Vector3,
    quaternion: Quaternion,
    scale: Vector3,
    color: Color,
  ): void {
    const transport = snapshot.buildings.filter(({ kind }) => kind === "belt");
    const mediumOf = (building: EntitySnapshot): "solid" | "fluid" =>
      this.definitions.get(building.definition_id)?.transport_medium === "fluid"
        ? "fluid"
        : "solid";
    const belts = transport.filter(
      (building) => mediumOf(building) === "solid",
    );
    const pipes = transport.filter(
      (building) => mediumOf(building) === "fluid",
    );
    const connected = connectedTransportLinks(
      snapshot.buildings,
      this.definitions,
    );
    const incomingTargets = new Set<number>();
    const straightInputTargets = new Set<number>();
    for (const link of connected) {
      if (link.to.kind !== "belt") continue;
      incomingTargets.add(link.to.id);
      if (isCollinearTransportTurn(this.transportTurn(link)))
        straightInputTargets.add(link.to.id);
    }
    const straightBelts = belts.filter(
      (building) =>
        !incomingTargets.has(building.id) ||
        straightInputTargets.has(building.id),
    );
    const straightPipes = pipes.filter(
      (building) =>
        !incomingTargets.has(building.id) ||
        straightInputTargets.has(building.id),
    );
    if (belts.length) {
      if (straightBelts.length) {
        const frame = new InstancedMesh(
          this.geometry.belt,
          this.materials.machine,
          straightBelts.length,
        );
        frame.name = "transport-rails";
        const treads = new InstancedMesh(
          this.geometry.beltDetail,
          this.materials.machineDark,
          straightBelts.length,
        );
        treads.name = "transport-treads";
        for (const [index, building] of straightBelts.entries()) {
          const center = axialToPixel(building, 1, { x: 0, y: 0 });
          const height = this.groundHeight(building.q, building.r) + 0.23;
          const angle = directionAngle(building.orientation);
          position.set(center.x, height, center.y);
          quaternion.setFromAxisAngle(new Vector3(0, 1, 0), angle);
          const [x, y, z] = transportScale(building.kind, building.orientation);
          scale.set(x, y, z);
          matrix.compose(position, quaternion, scale);
          frame.setMatrixAt(index, matrix);
          frame.setColorAt(index, color.set(BUILDING_COLORS.belt));
          treads.setMatrixAt(index, matrix);
          treads.setColorAt(index, color.set("#102b3a"));
        }
        markInstancesDirty(frame);
        markInstancesDirty(treads);
        frame.castShadow = true;
        treads.castShadow = true;
        this.staticGroup.add(frame, treads);
      }

      const solidLinks = connected.filter(({ medium }) => medium === "solid");
      const runs = solidLinks.flatMap((link) => this.transportDeckRuns(link));
      if (runs.length) {
        const links = new InstancedMesh(
          this.geometry.belt,
          this.materials.machine,
          runs.length,
        );
        links.name = "transport-connections";
        links.castShadow = true;
        const linkTreads = new InstancedMesh(
          this.geometry.beltDetail,
          this.materials.machineDark,
          runs.length,
        );
        linkTreads.name = "transport-connection-treads";
        linkTreads.castShadow = true;
        const linkDelta = new Vector3();
        for (const [index, { start, end }] of runs.entries()) {
          linkDelta.subVectors(end, start);
          const length = linkDelta.length();
          position.copy(start).add(end).multiplyScalar(0.5);
          quaternion.setFromUnitVectors(LOCAL_X, linkDelta.normalize());
          scale.set(length / 0.92, 1, 1);
          matrix.compose(position, quaternion, scale);
          links.setMatrixAt(index, matrix);
          links.setColorAt(index, color.set(BUILDING_COLORS.belt));
          linkTreads.setMatrixAt(index, matrix);
          linkTreads.setColorAt(index, color.set("#102b3a"));
        }
        markInstancesDirty(links);
        markInstancesDirty(linkTreads);
        this.staticGroup.add(links, linkTreads);
        this.addTransportCurves(solidLinks, "solid");
      }
    }
    if (pipes.length) {
      if (straightPipes.length) {
        const bodies = new InstancedMesh(
          this.geometry.pipe,
          this.materials.machine,
          straightPipes.length,
        );
        bodies.name = "fluid-pipes";
        bodies.castShadow = true;
        const couplings = new InstancedMesh(
          this.geometry.pipeDetail,
          this.materials.machine,
          straightPipes.length,
        );
        couplings.name = "fluid-pipe-couplings";
        for (const [index, building] of straightPipes.entries()) {
          const center = axialToPixel(building, 1, { x: 0, y: 0 });
          position.set(
            center.x,
            this.groundHeight(building.q, building.r) + PIPE_RIDE_HEIGHT,
            center.y,
          );
          quaternion.setFromAxisAngle(
            WORLD_UP,
            directionAngle(building.orientation),
          );
          const [x, y, z] = transportScale(building.kind, building.orientation);
          scale.set(x, y, z);
          matrix.compose(position, quaternion, scale);
          bodies.setMatrixAt(index, matrix);
          bodies.setColorAt(index, color.set("#2d8f91"));
          couplings.setMatrixAt(index, matrix);
          couplings.setColorAt(index, color.set("#b9ebe4"));
        }
        markInstancesDirty(bodies);
        markInstancesDirty(couplings);
        this.staticGroup.add(bodies, couplings);
      }

      const fluidLinks = connected.filter(({ medium }) => medium === "fluid");
      const fluidRuns = fluidLinks.flatMap((link) =>
        this.transportDeckRuns(link),
      );
      if (fluidRuns.length) {
        const bodies = new InstancedMesh(
          this.geometry.pipe,
          this.materials.machine,
          fluidRuns.length,
        );
        bodies.name = "fluid-pipe-connections";
        bodies.castShadow = true;
        const couplings = new InstancedMesh(
          this.geometry.pipeDetail,
          this.materials.machine,
          fluidRuns.length,
        );
        couplings.name = "fluid-pipe-connection-couplings";
        const delta = new Vector3();
        for (const [index, { start, end }] of fluidRuns.entries()) {
          delta.subVectors(end, start);
          const length = delta.length();
          position.copy(start).add(end).multiplyScalar(0.5);
          quaternion.setFromUnitVectors(LOCAL_X, delta.normalize());
          scale.set(length / 0.92, 1, 1);
          matrix.compose(position, quaternion, scale);
          bodies.setMatrixAt(index, matrix);
          bodies.setColorAt(index, color.set("#2d8f91"));
          couplings.setMatrixAt(index, matrix);
          couplings.setColorAt(index, color.set("#b9ebe4"));
        }
        markInstancesDirty(bodies);
        markInstancesDirty(couplings);
        this.staticGroup.add(bodies, couplings);
      }
      this.addTransportCurves(fluidLinks, "fluid");
    }

    const portals = transport.filter(
      (building) =>
        this.definitions.get(building.definition_id)?.underpass_span !==
        undefined,
    );
    if (portals.length) {
      const frames = new InstancedMesh(
        this.geometry.portal,
        this.materials.machineDark,
        portals.length,
      );
      frames.name = "underpass-portals";
      frames.castShadow = true;
      const stripes = new InstancedMesh(
        this.geometry.portalDetail,
        this.materials.machine,
        portals.length,
      );
      stripes.name = "underpass-caution-panels";
      for (const [index, building] of portals.entries()) {
        const center = axialToPixel(building, 1, { x: 0, y: 0 });
        position.set(
          center.x,
          this.groundHeight(building.q, building.r) + 0.12,
          center.y,
        );
        quaternion.setFromAxisAngle(
          WORLD_UP,
          directionAngle(building.orientation),
        );
        scale.set(1, 1, 1);
        matrix.compose(position, quaternion, scale);
        frames.setMatrixAt(index, matrix);
        frames.setColorAt(index, color.set("#59636a"));
        stripes.setMatrixAt(index, matrix);
        stripes.setColorAt(index, color.set("#f4c542"));
      }
      markInstancesDirty(frames);
      markInstancesDirty(stripes);
      this.staticGroup.add(frames, stripes);
    }

    const bridges = snapshot.buildings.filter(({ kind }) => kind === "bridge");
    if (bridges.length) {
      const mesh = new InstancedMesh(
        this.geometry.bridge,
        this.materials.machineDark,
        bridges.length,
      );
      mesh.name = "bridge-decks";
      for (const [index, building] of bridges.entries()) {
        const center = axialToPixel(building, 1, { x: 0, y: 0 });
        position.set(
          center.x,
          this.groundHeight(building.q, building.r) + 0.18,
          center.y,
        );
        quaternion.setFromAxisAngle(
          new Vector3(0, 1, 0),
          directionAngle(building.orientation),
        );
        scale.set(1, 1, 1);
        matrix.compose(position, quaternion, scale);
        mesh.setMatrixAt(index, matrix);
        mesh.setColorAt(index, color.set(BUILDING_COLORS.bridge));
      }
      mesh.instanceMatrix.needsUpdate = true;
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
      this.staticGroup.add(mesh);
    }
  }

  private transportLinkInset(building: EntitySnapshot): number {
    if (building.kind !== "belt") return 0.68;
    return 0.46 * transportScale(building.kind, building.orientation)[0];
  }

  /**
   * The straight decks one compiled link is drawn as, face to face.
   *
   * An ordinary link is a single run, and that run is the whole of "clicking together": a
   * splitter's flank is drawn with the same deck as the belt it faces, so the three read as one
   * junction. An underpass is three runs — down, across, and up — because a level deck at belt
   * height would read as joining the line it exists to clear rather than passing beneath it. The
   * dived section follows each end's own ground, the same way the line above it follows terrain.
   */
  private transportDeckRuns(
    link: TransportLink,
  ): { start: Vector3; end: Vector3 }[] {
    const fromPoint = axialToPixel(link.fromCell, 1, { x: 0, y: 0 });
    const a = { x: fromPoint.x, z: fromPoint.y };
    const b = this.pointById.get(link.to.id)!;
    const direction = new Vector3(b.x - a.x, 0, b.z - a.z).normalize();
    const fromGround = this.groundHeight(link.fromCell.q, link.fromCell.r);
    const toGround = this.groundById.get(link.to.id) ?? 0.07;
    const fromInset = this.transportLinkInset(link.from);
    const toInset = this.transportLinkInset(link.to);
    const rideHeight = link.medium === "fluid" ? PIPE_RIDE_HEIGHT : DECK_HEIGHT;
    const start = new Vector3(
      a.x + direction.x * fromInset,
      fromGround + rideHeight,
      a.z + direction.z * fromInset,
    );
    const end = new Vector3(
      b.x - direction.x * toInset,
      toGround + rideHeight,
      b.z - direction.z * toInset,
    );
    if (link.steps === 1) return [{ start, end }];
    // A ramp no longer than a third of the run, so even the shortest crossing still dives rather
    // than stepping down and straight back up.
    const ramp = Math.min(0.5, start.distanceTo(end) / 3);
    const descended = start
      .clone()
      .addScaledVector(direction, ramp)
      .setY(fromGround + UNDERPASS_HEIGHT);
    const climbing = end
      .clone()
      .addScaledVector(direction, -ramp)
      .setY(toGround + UNDERPASS_HEIGHT);
    return [
      { start, end: descended },
      { start: descended, end: climbing },
      { start: climbing, end },
    ];
  }

  private transportTurn(link: TransportLink): number {
    const fromPoint = axialToPixel(link.fromCell, 1, { x: 0, y: 0 });
    const fromCenter = { x: fromPoint.x, z: fromPoint.y };
    const toCenter = this.pointById.get(link.to.id)!;
    const incomingAngle = Math.atan2(
      -(toCenter.z - fromCenter.z),
      toCenter.x - fromCenter.x,
    );
    return normalizeAngle(incomingAngle - directionAngle(link.to.orientation));
  }

  /** Every incoming branch gets its own centre curve into the target belt. That keeps a merge
   * legible without asking presentation to choose one predecessor as the "real" lane. */
  private addTransportCurves(
    links: readonly TransportLink[],
    medium: "solid" | "fluid",
  ): void {
    const buckets = new Map<
      string,
      {
        turn: number;
        halfExtent: number;
        buildings: EntitySnapshot[];
      }
    >();
    for (const link of links) {
      const { to } = link;
      if (to.kind !== "belt") continue;
      const turn = this.transportTurn(link);
      if (isCollinearTransportTurn(turn)) continue;
      const halfExtent = this.transportLinkInset(to);
      const roundedTurn = Math.round(turn * 1_000_000) / 1_000_000;
      const key = `${medium}:${roundedTurn}:${halfExtent}`;
      const bucket = buckets.get(key);
      if (bucket) bucket.buildings.push(to);
      else buckets.set(key, { turn: roundedTurn, halfExtent, buildings: [to] });
    }
    if (!buckets.size) return;

    const group = new Group();
    group.name = medium === "fluid" ? "fluid-pipe-curves" : "transport-curves";
    for (const [key, { turn, halfExtent, buildings }] of buckets) {
      let geometry = this.curvedTransportGeometry.get(key);
      if (!geometry) {
        geometry = createCurvedTransportGeometry(turn, halfExtent, medium);
        this.curvedTransportGeometry.set(key, geometry);
      }
      const frame = new InstancedMesh(
        geometry.frame,
        this.materials.machine,
        buildings.length,
      );
      frame.name =
        medium === "fluid"
          ? "fluid-pipe-curve-bodies"
          : "transport-curve-rails";
      frame.castShadow = true;
      const treads = new InstancedMesh(
        geometry.detail,
        this.materials.machineDark,
        buildings.length,
      );
      treads.name =
        medium === "fluid"
          ? "fluid-pipe-curve-couplings"
          : "transport-curve-treads";
      treads.castShadow = true;
      for (const [index, building] of buildings.entries()) {
        const center = this.pointById.get(building.id)!;
        this.scratchMatrix.compose(
          this.scratchPosition.set(
            center.x,
            (this.groundById.get(building.id) ?? 0.07) +
              (medium === "fluid" ? PIPE_RIDE_HEIGHT : DECK_HEIGHT),
            center.z,
          ),
          this.scratchQuaternion.setFromAxisAngle(
            WORLD_UP,
            directionAngle(building.orientation),
          ),
          this.scratchScale.set(1, 1, 1),
        );
        frame.setMatrixAt(index, this.scratchMatrix);
        frame.setColorAt(
          index,
          this.scratchColor.set(
            medium === "fluid" ? "#2d8f91" : BUILDING_COLORS.belt,
          ),
        );
        treads.setMatrixAt(index, this.scratchMatrix);
        treads.setColorAt(
          index,
          this.scratchColor.set(medium === "fluid" ? "#b9ebe4" : "#102b3a"),
        );
      }
      markInstancesDirty(frame);
      markInstancesDirty(treads);
      group.add(frame, treads);
    }
    this.staticGroup.add(group);
  }

  private addOutputIndicators(snapshot: FactorySnapshot): void {
    const indicators = snapshot.buildings.flatMap((building) => {
      if (!hasDirectionalOutput(building.kind)) return [];
      const routes = building.output_routes ?? [];
      if (routes.length === 0)
        return [
          {
            building,
            q: building.q,
            r: building.r,
            direction: building.orientation,
            color: "#ffd166",
          },
        ];
      const grouped = new Map<
        string,
        { q: number; r: number; direction: number; itemIds: number[] }
      >();
      for (const route of routes) {
        const key = `${route.q},${route.r},${route.direction}`;
        const existing = grouped.get(key);
        if (existing) existing.itemIds.push(route.item_id);
        else
          grouped.set(key, {
            q: route.q,
            r: route.r,
            direction: route.direction,
            itemIds: [route.item_id],
          });
      }
      return Array.from(grouped.values()).map((route) => ({
        building,
        q: route.q,
        r: route.r,
        direction: route.direction,
        color:
          route.itemIds.length === 1
            ? (this.items.get(route.itemIds[0]!)?.color ?? "#ffd166")
            : "#ffd166",
      }));
    });
    if (!indicators.length) return;
    const mesh = new InstancedMesh(
      this.geometry.outputIndicator,
      this.materials.emissive,
      indicators.length,
    );
    mesh.name = "building-output-indicators";
    for (const [index, indicator] of indicators.entries()) {
      const center = axialToPixel({ q: indicator.q, r: indicator.r }, 1, {
        x: 0,
        y: 0,
      });
      const angle = directionAngle(indicator.direction);
      this.scratchPosition.set(
        center.x + Math.cos(angle) * 0.68,
        this.groundHeight(indicator.q, indicator.r) + 0.53,
        center.y - Math.sin(angle) * 0.68,
      );
      this.scratchQuaternion.setFromAxisAngle(WORLD_UP, angle);
      this.scratchMatrix.compose(
        this.scratchPosition,
        this.scratchQuaternion,
        this.scratchScale.set(1, 1, 1),
      );
      mesh.setMatrixAt(index, this.scratchMatrix);
      mesh.setColorAt(index, this.scratchColor.set(indicator.color));
    }
    markInstancesDirty(mesh);
    this.staticGroup.add(mesh);
  }

  private addPowerWires(snapshot: FactorySnapshot): void {
    const links = powerWireLinks(snapshot.buildings, this.definitions);
    if (!links.length) return;
    const segmentsPerWire = 7;
    const mesh = new InstancedMesh(
      this.geometry.wireSegment,
      this.materials.machineDark,
      links.length * segmentsPerWire,
    );
    mesh.name = "pole-wires";
    mesh.castShadow = true;
    const byId = new Map(
      snapshot.buildings.map((building) => [building.id, building]),
    );
    const start = new Vector3();
    const end = new Vector3();
    const a = new Vector3();
    const b = new Vector3();
    const delta = new Vector3();
    let index = 0;
    for (const link of links) {
      const from = byId.get(link.fromId)!;
      const to = byId.get(link.toId)!;
      const fromCenter = this.pointById.get(from.id)!;
      const toCenter = this.pointById.get(to.id)!;
      start.set(
        fromCenter.x,
        (this.groundById.get(from.id) ?? 0.07) +
          poleWireHeight(from, this.definitions),
        fromCenter.z,
      );
      end.set(
        toCenter.x,
        (this.groundById.get(to.id) ?? 0.07) +
          (to.kind === "pole" ? poleWireHeight(to, this.definitions) : 0.72),
        toCenter.z,
      );
      const span = start.distanceTo(end);
      const sag = Math.min(0.62, 0.12 + span * 0.055);
      for (let segment = 0; segment < segmentsPerWire; segment += 1) {
        wirePoint(start, end, segment / segmentsPerWire, sag, a);
        wirePoint(start, end, (segment + 1) / segmentsPerWire, sag, b);
        delta.subVectors(b, a);
        const length = delta.length();
        this.scratchPosition.copy(a).add(b).multiplyScalar(0.5);
        this.scratchQuaternion.setFromUnitVectors(WORLD_UP, delta.normalize());
        this.scratchMatrix.compose(
          this.scratchPosition,
          this.scratchQuaternion,
          this.scratchScale.set(0.022, length, 0.022),
        );
        mesh.setMatrixAt(index, this.scratchMatrix);
        mesh.setColorAt(index, this.scratchColor.set("#91a79f"));
        index += 1;
      }
    }
    markInstancesDirty(mesh);
    this.staticGroup.add(mesh);
  }

  private addPartMeshes(snapshot: FactorySnapshot): void {
    const parts = collectMachineParts(
      snapshot,
      this.definitions,
      (q, r) => this.groundHeight(q, r),
      BUILDING_COLORS,
    );
    const buckets = new Map<string, MachinePartInstance[]>();
    for (const instance of parts) {
      const key = `${instance.key}:${instance.glow ? "glow" : instance.material}:${instance.animated ? "animated" : "static"}`;
      const bucket = buckets.get(key);
      if (bucket) bucket.push(instance);
      else buckets.set(key, [instance]);
    }
    const matrix = new Matrix4();
    const color = new Color();
    for (const [key, instances] of buckets) {
      const first = instances[0]!;
      const mesh = new InstancedMesh(
        this.geometryLibrary.get(first.part),
        first.glow
          ? this.materials.emissive
          : machineMaterialFor(this.materials, first.material),
        instances.length,
      );
      mesh.name = `machine-part-${key}`;
      mesh.castShadow = true;
      for (const [index, instance] of instances.entries()) {
        mesh.setMatrixAt(index, machinePartMatrix(instance, 0, true, matrix));
        const tier =
          this.definitions.get(instance.building.definition_id)?.tier ?? 0;
        mesh.setColorAt(
          index,
          instance.glow
            ? color.set(instance.glow)
            : machinePartColor(color, this.scratchTrim, instance, tier),
        );
      }
      mesh.instanceMatrix.needsUpdate = true;
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
      this.staticGroup.add(mesh);
      this.partBuckets.push({
        mesh,
        instances,
        animated: first.animated,
      });
    }
  }

  private rebuildResources(resources: readonly ResourceSnapshot[]): void {
    this.group.remove(this.resourceGroup);
    this.resourceGroup = new Group();
    this.resourceGroup.name = "field-resources";
    const ore: ResourcePartInstance[] = [];
    const lumps: ResourcePartInstance[] = [];
    const coalLumps: ResourcePartInstance[] = [];
    const stoneLumps: ResourcePartInstance[] = [];
    const grains: ResourcePartInstance[] = [];
    const sandMounds: ResourcePartInstance[] = [];
    const crystals: ResourcePartInstance[] = [];
    const trunks: ResourcePartInstance[] = [];
    const canopies: ResourcePartInstance[] = [];

    for (const resource of resources) {
      if (resource.quantity <= 0) continue;
      const item = this.items.get(resource.item_id);
      if (!item) continue;
      const ground = this.groundHeight(resource.q, resource.r);
      const x = resource.x / WORLD_SCALE;
      const z = resource.y / WORLD_SCALE;
      const angle = stableVariation(resource.q, resource.r) * Math.PI * 2;
      const fraction = resource.initial_quantity
        ? resource.quantity / resource.initial_quantity
        : 1;
      const abundance = 0.78 + Math.sqrt(Math.max(0, fraction)) * 0.22;
      const fieldColor = fieldVisualColor(item.color);
      const add = (
        target: ResourcePartInstance[],
        offsetX: number,
        offsetZ: number,
        scaleX: number,
        scaleY: number,
        scaleZ: number,
        centerY: number,
        color = fieldColor,
      ): void => {
        const cos = Math.cos(angle);
        const sin = Math.sin(angle);
        target.push({
          x: x + offsetX * cos - offsetZ * sin,
          y: ground + centerY,
          z: z + offsetX * sin + offsetZ * cos,
          angle,
          scaleX: scaleX * abundance,
          scaleY: scaleY * abundance,
          scaleZ: scaleZ * abundance,
          color,
        });
      };

      if (item.regrowth_ticks) {
        for (let unit = 0; unit < resource.quantity; unit += 1) {
          const treeAngle =
            stableVariation(resource.q * 7 + unit, resource.r * 11 - unit) *
            Math.PI *
            2;
          const radius = unit === 0 ? 0 : 0.19 + (unit % 2) * 0.08;
          const offsetX = Math.cos(treeAngle) * radius;
          const offsetZ = Math.sin(treeAngle) * radius;
          const trunkHeight = 0.34 + (unit % 3) * 0.035;
          add(
            trunks,
            offsetX,
            offsetZ,
            0.055,
            trunkHeight,
            0.055,
            trunkHeight / 2,
            fieldColor,
          );
          const canopyHeight = 0.42 + (unit % 2) * 0.05;
          add(
            canopies,
            offsetX,
            offsetZ,
            0.24,
            canopyHeight,
            0.24,
            trunkHeight + canopyHeight / 2,
            "#89bd62",
          );
        }
        continue;
      }

      switch (item.icon) {
        case "crystal":
          add(crystals, -0.18, 0.04, 0.12, 0.55, 0.12, 0.275);
          add(crystals, 0.13, -0.1, 0.16, 0.78, 0.16, 0.39);
          add(crystals, 0.18, 0.2, 0.1, 0.44, 0.1, 0.22);
          break;
        case "grains":
          if (item.key === "sand") {
            add(
              sandMounds,
              -0.19,
              -0.08,
              0.25,
              0.13,
              0.22,
              0.065,
              fieldShade(fieldColor, -0.08),
            );
            add(sandMounds, 0.17, -0.03, 0.28, 0.16, 0.24, 0.08);
            add(
              sandMounds,
              0.02,
              0.21,
              0.22,
              0.11,
              0.2,
              0.055,
              fieldShade(fieldColor, 0.07),
            );
            break;
          }
          add(grains, -0.19, -0.08, 0.25, 0.13, 0.22, 0.065);
          add(grains, 0.17, -0.03, 0.28, 0.16, 0.24, 0.08);
          add(grains, 0.02, 0.21, 0.22, 0.11, 0.2, 0.055);
          break;
        case "lump":
          if (item.key === "coal") {
            add(
              coalLumps,
              -0.18,
              -0.06,
              0.24,
              0.18,
              0.21,
              0.18,
              fieldShade(fieldColor, -0.04),
            );
            add(coalLumps, 0.16, -0.04, 0.29, 0.22, 0.25, 0.22);
            add(
              coalLumps,
              0.03,
              0.22,
              0.2,
              0.15,
              0.18,
              0.15,
              fieldShade(fieldColor, 0.1),
            );
            break;
          }
          if (item.key === "stone") {
            add(
              stoneLumps,
              -0.18,
              -0.06,
              0.24,
              0.18,
              0.21,
              0.18,
              fieldShade(fieldColor, -0.14),
            );
            add(stoneLumps, 0.16, -0.04, 0.29, 0.22, 0.25, 0.22);
            add(
              stoneLumps,
              0.03,
              0.22,
              0.2,
              0.15,
              0.18,
              0.15,
              fieldShade(fieldColor, 0.12),
            );
            break;
          }
          add(lumps, -0.18, -0.06, 0.24, 0.18, 0.21, 0.18);
          add(lumps, 0.16, -0.04, 0.29, 0.22, 0.25, 0.22);
          add(lumps, 0.03, 0.22, 0.2, 0.15, 0.18, 0.15);
          break;
        default:
          add(ore, -0.19, -0.08, 0.15, 0.34, 0.15, 0.34);
          add(ore, 0.16, -0.03, 0.2, 0.43, 0.2, 0.43);
          add(ore, 0.02, 0.2, 0.13, 0.29, 0.13, 0.29);
      }
    }

    this.addResourceParts("ore-field-shards", this.geometry.ore, ore);
    this.addResourceParts("lump-field-clusters", this.geometry.lump, lumps);
    this.addResourceParts(
      "coal-field-clusters",
      this.geometry.lump,
      coalLumps,
      this.materials.resourceCoal,
    );
    this.addResourceParts(
      "stone-field-clusters",
      this.geometry.lump,
      stoneLumps,
      this.materials.resourceStone,
    );
    this.addResourceParts("grain-field-mounds", this.geometry.grains, grains);
    this.addResourceParts(
      "sand-field-mounds",
      this.geometry.grains,
      sandMounds,
      this.materials.resourceSand,
    );
    this.addResourceParts(
      "signal-crystal-spires",
      this.geometry.crystal,
      crystals,
      this.materials.emissive,
    );
    this.addResourceParts("forest-trunks", this.geometry.trunk, trunks);
    this.addResourceParts("forest-canopies", this.geometry.canopy, canopies);

    const scars = resources.filter(
      (resource) =>
        !this.items.get(resource.item_id)?.regrowth_ticks &&
        resource.initial_quantity > 0 &&
        resource.quantity === 0,
    );
    if (scars.length) {
      const mesh = new InstancedMesh(
        this.geometry.scar,
        this.materials.machineDark,
        scars.length,
      );
      mesh.name = "depleted-field-scars";
      const matrix = new Matrix4();
      const color = new Color("#241f1a");
      for (const [index, resource] of scars.entries()) {
        const angle = stableVariation(resource.q, resource.r) * Math.PI;
        matrix.compose(
          new Vector3(
            resource.x / WORLD_SCALE,
            this.groundHeight(resource.q, resource.r) + 0.018,
            resource.y / WORLD_SCALE,
          ),
          new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), angle),
          new Vector3(1, 1, 0.72),
        );
        mesh.setMatrixAt(index, matrix);
        mesh.setColorAt(index, color);
      }
      mesh.instanceMatrix.needsUpdate = true;
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
      this.resourceGroup.add(mesh);
    }
    this.group.add(this.resourceGroup);
  }

  private addResourceParts(
    name: string,
    geometry:
      | OctahedronGeometry
      | IcosahedronGeometry
      | ConeGeometry
      | CylinderGeometry,
    instances: readonly ResourcePartInstance[],
    material:
      | WorldMaterials["resource"]
      | WorldMaterials["resourceCoal"]
      | WorldMaterials["resourceStone"]
      | WorldMaterials["resourceSand"]
      | WorldMaterials["emissive"] = this.materials.resource,
  ): void {
    if (!instances.length) return;
    const mesh = new InstancedMesh(geometry, material, instances.length);
    mesh.name = name;
    mesh.castShadow = true;
    mesh.receiveShadow = true;
    const matrix = new Matrix4();
    const quaternion = new Quaternion();
    const color = new Color();
    for (const [index, instance] of instances.entries()) {
      matrix.compose(
        new Vector3(instance.x, instance.y, instance.z),
        quaternion.setFromAxisAngle(new Vector3(0, 1, 0), instance.angle),
        new Vector3(instance.scaleX, instance.scaleY, instance.scaleZ),
      );
      mesh.setMatrixAt(index, matrix);
      mesh.setColorAt(index, color.set(instance.color));
    }
    markInstancesDirty(mesh);
    this.resourceGroup.add(mesh);
  }

  private ensureDynamicCapacity(capacity: number): void {
    if (this.statusMesh && this.statusMesh.instanceMatrix.count >= capacity)
      return;
    this.dynamicGroup.clear();
    const max = Math.max(1, capacity);
    this.statusMesh = new InstancedMesh(
      this.geometry.status,
      this.materials.emissive,
      max,
    );
    this.statusMesh.name = "machine-status";
    this.progressMesh = new InstancedMesh(
      this.geometry.progress,
      this.materials.emissive,
      max,
    );
    this.progressMesh.name = "machine-progress";
    this.cargoMesh = new InstancedMesh(
      this.geometry.cargo,
      this.materials.resource,
      max,
    );
    this.cargoMesh.name = "moving-cargo";
    this.groundItemMesh = new InstancedMesh(
      this.geometry.cargo,
      this.materials.resource,
      Math.max(256, max),
    );
    this.groundItemMesh.name = "ground-items";
    this.plumeMesh = new InstancedMesh(
      this.geometry.plume,
      this.materials.smoke,
      max * 3,
    );
    this.plumeMesh.name = "machine-plumes";
    this.plumeMesh.frustumCulled = false;
    this.dynamicGroup.add(
      this.statusMesh,
      this.progressMesh,
      this.cargoMesh,
      this.groundItemMesh,
      this.plumeMesh,
    );
  }

  private updateDynamicBuildings(
    snapshot: FactorySnapshot,
    now: number,
    reducedMotion: boolean,
  ): void {
    if (
      !this.statusMesh ||
      !this.progressMesh ||
      !this.cargoMesh ||
      !this.groundItemMesh ||
      !this.plumeMesh
    )
      return;
    const matrix = this.scratchMatrix;
    const color = this.scratchColor;
    const position = this.scratchPosition;
    const quaternion = this.scratchQuaternion.identity();
    const scale = this.scratchScale.set(1, 1, 1);
    let statuses = 0;
    let progresses = 0;
    let cargos = 0;
    let plumes = 0;
    for (const building of snapshot.buildings) {
      const center = this.pointById.get(building.id);
      if (!center) continue;
      const height =
        this.groundById.get(building.id) ??
        this.groundHeight(building.q, building.r);
      scale.set(1, 1, 1);
      const mark = stallMark(building.status);
      if (mark) {
        matrix.compose(
          position.set(center.x + 0.38, height + 0.62, center.z),
          quaternion,
          scale,
        );
        this.statusMesh.setMatrixAt(statuses, matrix);
        this.statusMesh.setColorAt(statuses, color.set(mark));
        statuses += 1;
      }
      if (building.progress_total > 0 && building.progress > 0) {
        const amount = Math.max(
          0.04,
          building.progress / building.progress_total,
        );
        matrix.compose(
          position.set(
            center.x - 0.26 + amount * 0.19,
            height + 0.52,
            center.z - 0.34,
          ),
          quaternion,
          scale.set(amount, 1, 1),
        );
        this.progressMesh.setMatrixAt(progresses, matrix);
        this.progressMesh.setColorAt(progresses, color.set("#7fe0c0"));
        progresses += 1;
      }
      const lane = building.lane ?? [];
      if (building.cargo || lane.length > 0) {
        const target = building.next_id
          ? this.pointById.get(building.next_id)
          : undefined;
        const targetBuilding = building.next_id
          ? this.buildingById.get(building.next_id)
          : undefined;
        let tx = target?.x;
        let tz = target?.z;
        let targetHeight = targetBuilding
          ? (this.groundById.get(targetBuilding.id) ?? height) +
            CARGO_RIDE_HEIGHT
          : height + CARGO_RIDE_HEIGHT;
        if (tx === undefined || tz === undefined) {
          const angle = directionAngle(building.orientation);
          tx = center.x + Math.cos(angle) * 0.78;
          tz = center.z - Math.sin(angle) * 0.78;
        } else if (targetBuilding?.kind !== "belt") {
          const dx = tx - center.x;
          const dz = tz - center.z;
          const distance = Math.hypot(dx, dz);
          const fraction = distance > 0 ? Math.max(0, 1 - 0.68 / distance) : 0;
          tx = center.x + dx * fraction;
          tz = center.z + dz * fraction;
          targetHeight =
            height +
            CARGO_RIDE_HEIGHT +
            (targetHeight - height - CARGO_RIDE_HEIGHT) * fraction;
        }
        const place = (itemId: number, travel: number): void => {
          if (cargos >= this.cargoMesh!.instanceMatrix.count) return;
          matrix.compose(
            position.set(
              center.x + (tx! - center.x) * travel,
              height +
                CARGO_RIDE_HEIGHT +
                (targetHeight - height - CARGO_RIDE_HEIGHT) * travel,
              center.z + (tz! - center.z) * travel,
            ),
            quaternion,
            scale.set(1, 1, 1),
          );
          this.cargoMesh!.setMatrixAt(cargos, matrix);
          this.cargoMesh!.setColorAt(
            cargos,
            color.set(this.items.get(itemId)?.color ?? "#ffffff"),
          );
          cargos += 1;
        };
        const transit = snapshot.belt_transit_ticks ?? 27;
        for (const item of lane) {
          place(
            item.cargo.item_id,
            beltLaneTravel(
              item.entered,
              snapshot.tick,
              transit,
              now - this.cargoTickAt,
              this.cargoTickMs,
              reducedMotion,
            ),
          );
        }
        if (building.cargo) {
          place(
            building.cargo.item_id,
            building.kind === "belt"
              ? 1
              : cargoTravel(
                  now - this.cargoTickAt,
                  this.cargoTickMs,
                  reducedMotion,
                  building.status === "output blocked",
                ),
          );
        }
      }
      const definition = this.definitions.get(building.definition_id);
      const plume = plumeFor(building, definition);
      if (plume) {
        const puffCount = reducedMotion ? 1 : 3;
        for (let puff = 0; puff < puffCount; puff += 1) {
          const cycle = reducedMotion
            ? 0.42
            : positiveFraction(now / 2_200 + building.id * 0.137 + puff / 3);
          const drift = building.id * 1.71 + cycle * Math.PI * 1.4;
          const spread = 0.035 + cycle * 0.075;
          const size = 0.5 + cycle * 0.9;
          matrix.compose(
            position.set(
              center.x + Math.sin(drift) * spread,
              height + plumeOriginHeight(building, definition) + cycle * 0.82,
              center.z + Math.cos(drift) * spread,
            ),
            quaternion,
            scale.set(size, size * 0.82, size),
          );
          this.plumeMesh.setMatrixAt(plumes, matrix);
          this.plumeMesh.setColorAt(
            plumes,
            color
              .set(plume === "steam" ? "#dceff0" : "#78817d")
              .lerp(this.scratchTrim.set("#edf3ed"), cycle * 0.34),
          );
          plumes += 1;
        }
      }
    }
    this.statusMesh.count = statuses;
    this.progressMesh.count = progresses;
    this.cargoMesh.count = cargos;
    this.plumeMesh.count = plumes;
    markInstancesDirty(this.statusMesh);
    markInstancesDirty(this.progressMesh);
    markInstancesDirty(this.cargoMesh);
    markInstancesDirty(this.plumeMesh);

    let groundItems = 0;
    const groundCapacity = this.groundItemMesh.instanceMatrix.count;
    for (const item of snapshot.ground_items ?? []) {
      if (groundItems >= groundCapacity) break;
      const point = axialToPixel(item, 1, { x: 0, y: 0 });
      const height = this.groundHeight(item.q, item.r);
      const remainingTicks =
        item.despawn_tick > snapshot.tick
          ? item.despawn_tick - snapshot.tick
          : 0;

      // Flash/blink during the last 100 ticks (10 seconds)
      if (remainingTicks < 100 && Math.floor(now / 150) % 2 === 0) {
        continue;
      }

      const bob = reducedMotion
        ? 0.32
        : 0.32 + Math.sin(now / 350 + item.id * 1.3) * 0.07;
      const angle = reducedMotion
        ? 0
        : (now / 1200 + item.id * 0.8) % (Math.PI * 2);
      quaternion.setFromAxisAngle(WORLD_UP, angle);
      const scaleVal = item.quantity > 1 ? 2.2 : 1.8;
      scale.set(scaleVal, scaleVal, scaleVal);

      matrix.compose(
        position.set(point.x, height + bob, point.y),
        quaternion,
        scale,
      );
      this.groundItemMesh.setMatrixAt(groundItems, matrix);
      this.groundItemMesh.setColorAt(
        groundItems,
        color.set(this.items.get(item.item_id)?.color ?? "#ffffff"),
      );
      groundItems += 1;
    }
    this.groundItemMesh.count = groundItems;
    markInstancesDirty(this.groundItemMesh);
  }

  private updatePlayer(snapshot: FactorySnapshot): void {
    const player = snapshot.player;
    const axial = pixelToAxial(player, WORLD_SCALE);
    const height = this.groundHeight(axial.q, axial.r);
    const x = player.x / WORLD_SCALE;
    const z = player.y / WORLD_SCALE;
    this.playerGroup.position.set(x, height + 0.02, z);
    const facing = Math.atan2(player.facing_x, player.facing_y);
    this.playerGroup.rotation.y = facing;
    const walking = player.walk_path.length > 0;
    const stride = walking
      ? Math.sin(((player.x + player.y) / WORLD_SCALE) * 8) * 0.48
      : 0;
    this.playerBody.rotation.x = walking ? -0.06 : 0;
    this.playerShell.rotation.x = walking ? -0.04 : 0;
    this.playerLeftLeg.rotation.x = stride;
    this.playerRightLeg.rotation.x = -stride;
    this.playerLeftArm.rotation.x = -stride * 0.72;
    this.playerRightArm.rotation.x = stride * 0.72;
    const total = player.action_cooldown_total;
    const done = total > 0 ? 1 - player.action_cooldown / total : 0;
    this.playerTool.visible = player.action_cooldown > 0;
    this.playerTool.rotation.z = -0.5 - done * 0.75;
    this.playerWork.visible = player.action_cooldown > 0;
    this.playerWork.scale.setScalar(Math.max(0.05, done));
  }

  private groundHeight(q: number, r: number): number {
    return heightAt(this.terrainByKey, q, r);
  }
}

const WORLD_UP = new Vector3(0, 1, 0);
const LOCAL_X = new Vector3(1, 0, 0);
const MACHINE_BASE_SCALE = 1.12;
/** Where a transport deck rides above the ground beneath it. */
const DECK_HEIGHT = 0.23;
/** Pipe bodies ride slightly above an open belt deck, matching their larger circular section. */
const PIPE_RIDE_HEIGHT = 0.29;
/** Where a carried item rides: sitting on the treads of the deck below it, not floating over them. */
const CARGO_RIDE_HEIGHT = 0.46;
/**
 * Where an underpass's crossing run rides instead: low enough that its rails pass under the
 * underside of the deck above, high enough that the deck stays clear of the ground.
 */
const UNDERPASS_HEIGHT = 0.04;

/** One compiled transport edge, with how many hexes it spans — see {@link connectedTransportLinks}. */
interface TransportLink {
  readonly from: EntitySnapshot;
  readonly fromCell: { q: number; r: number };
  readonly to: EntitySnapshot;
  readonly steps: number;
  readonly medium: "solid" | "fluid";
}

function normalizeAngle(angle: number): number {
  return Math.atan2(Math.sin(angle), Math.cos(angle));
}

/** A reversed heading shares the same undirected deck axis, so it needs no degenerate U-curve. */
function isCollinearTransportTurn(turn: number): boolean {
  return Math.abs(Math.sin(turn)) < 0.01;
}

function outputIndicatorGeometry(): ConeGeometry {
  const geometry = new ConeGeometry(0.11, 0.32, 4);
  geometry.rotateZ(-Math.PI / 2);
  return geometry;
}

function hasDirectionalOutput(kind: EntitySnapshot["kind"]): boolean {
  return (
    kind === "belt" ||
    kind === "extractor" ||
    kind === "composer" ||
    kind === "container" ||
    kind === "pump"
  );
}

/**
 * Every compiled transport edge a deck should be drawn along.
 *
 * A junction is only legible if its extra outputs are drawn like its first one, so a splitter's
 * flanks come from `branch_ids` and get the same deck as the belt it faces — that is what makes
 * one click together with its neighbours instead of reading as three unrelated belts. An
 * underpass's edge lands several hexes ahead of it rather than against its face, so the test is
 * "one straight run apart" rather than "one step along the facing"; `steps` is what the caller
 * needs to know to dive that run under the line it clears.
 */
function connectedTransportLinks(
  buildings: readonly EntitySnapshot[],
  definitions: ReadonlyMap<number, BuildingDefinition>,
): TransportLink[] {
  const byId = new Map(buildings.map((building) => [building.id, building]));
  return buildings.flatMap((from) => {
    const routed = (from.output_routes ?? []).flatMap((route) =>
      route.target_id
        ? [{ id: route.target_id, fromCell: { q: route.q, r: route.r } }]
        : [],
    );
    const links =
      routed.length > 0
        ? routed
        : [from.next_id, ...(from.branch_ids ?? [])].flatMap((id) =>
            id ? [{ id, fromCell: { q: from.q, r: from.r } }] : [],
          );
    const seen = new Set<string>();
    return links.flatMap(({ id, fromCell }) => {
      if (!id) return [];
      const to = byId.get(id);
      if (!to || (from.kind !== "belt" && to.kind !== "belt")) return [];
      const fromMedium =
        from.kind === "belt" &&
        definitions.get(from.definition_id)?.transport_medium === "fluid"
          ? "fluid"
          : "solid";
      const toMedium =
        to.kind === "belt" &&
        definitions.get(to.definition_id)?.transport_medium === "fluid"
          ? "fluid"
          : "solid";
      // Two transport families may cross, but never draw a joined seam into one another.
      if (from.kind === "belt" && to.kind === "belt" && fromMedium !== toMedium)
        return [];
      const medium = from.kind === "belt" ? fromMedium : toMedium;
      const key = `${fromCell.q},${fromCell.r}>${id}`;
      if (seen.has(key)) return [];
      seen.add(key);
      const steps = transportRun(fromCell, to);
      return steps === null ? [] : [{ from, fromCell, to, steps, medium }];
    });
  });
}

/**
 * How many hexes separate two linked cells along one heading, or null when no single heading
 * joins them. Only an underpass ever answers more than one, and never more than its span.
 */
function transportRun(
  from: { q: number; r: number },
  to: EntitySnapshot,
): number | null {
  const dq = to.q - from.q;
  const dr = to.r - from.r;
  for (const direction of TRANSPORT_DIRECTIONS) {
    for (let steps = 1; steps <= MAX_UNDERPASS_SPAN; steps += 1)
      if (dq === direction.q * steps && dr === direction.r * steps)
        return steps;
  }
  return null;
}

/** Exact display links for the native pole rules: poles link by pole reach and machines attach to
 * every pole whose own supply radius covers their nearest footprint cells. */
export function powerWireLinks(
  buildings: readonly EntitySnapshot[],
  definitions: ReadonlyMap<number, BuildingDefinition>,
): PowerWireLink[] {
  const poles = buildings.filter(({ kind }) => kind === "pole");
  const machines = buildings.filter((building) => {
    const definition = definitions.get(building.definition_id);
    return (
      building.kind !== "pole" &&
      ((definition?.power_draw ?? 0) > 0 || (definition?.power_output ?? 0) > 0)
    );
  });
  const links: PowerWireLink[] = [];
  for (let left = 0; left < poles.length; left += 1) {
    for (let right = left + 1; right < poles.length; right += 1) {
      const a = poles[left]!;
      const b = poles[right]!;
      const reach = Math.max(
        definitions.get(a.definition_id)?.pole_reach ?? 0,
        definitions.get(b.definition_id)?.pole_reach ?? 0,
      );
      if (footprintDistance(a, b) <= reach)
        links.push({ fromId: a.id, toId: b.id });
    }
  }
  for (const pole of poles) {
    const radius = definitions.get(pole.definition_id)?.supply_radius ?? 0;
    for (const machine of machines) {
      if (footprintDistance(pole, machine) <= radius)
        links.push({ fromId: pole.id, toId: machine.id });
    }
  }
  return links;
}

function footprintDistance(a: EntitySnapshot, b: EntitySnapshot): number {
  const left = a.footprint.length ? a.footprint : [a];
  const right = b.footprint.length ? b.footprint : [b];
  let best = Number.POSITIVE_INFINITY;
  for (const from of left) {
    for (const to of right) {
      const dq = to.q - from.q;
      const dr = to.r - from.r;
      best = Math.min(
        best,
        (Math.abs(dq) + Math.abs(dr) + Math.abs(dq + dr)) / 2,
      );
    }
  }
  return best;
}

function poleWireHeight(
  pole: EntitySnapshot,
  definitions: ReadonlyMap<number, BuildingDefinition>,
): number {
  return 1.7 + (definitions.get(pole.definition_id)?.tier ?? 0) * 0.08;
}

function wirePoint(
  start: Vector3,
  end: Vector3,
  t: number,
  sag: number,
  target: Vector3,
): Vector3 {
  return target
    .lerpVectors(start, end, t)
    .addScaledVector(WORLD_UP, -4 * sag * t * (1 - t));
}

/**
 * Field colour is the item's colour. A near-void is lifted just enough to read as a lump against
 * dark highland; everything else keeps the hue, saturation, and lightness the catalogue already
 * chose. The old floor at 0.46 turned coal, stone, and sand into one mid-grey.
 */
export function fieldVisualColor(source: string): string {
  const color = new Color(source);
  const hsl = { h: 0, s: 0, l: 0 };
  color.getHSL(hsl);
  color.setHSL(hsl.h, hsl.s, hsl.l < 0.12 ? 0.16 : hsl.l);
  return `#${color.getHexString()}`;
}

/** One boulder or mound a step darker or lighter, so a cluster is a pile rather than a clone. */
export function fieldShade(source: string, delta: number): string {
  const color = new Color(source);
  const hsl = { h: 0, s: 0, l: 0 };
  color.getHSL(hsl);
  color.setHSL(hsl.h, hsl.s, Math.min(0.88, Math.max(0.08, hsl.l + delta)));
  return `#${color.getHexString()}`;
}

/** Small low-poly status bead without importing another geometry family into the bucket model. */
class SphereGeometryCompat extends IcosahedronGeometry {
  constructor(radius: number) {
    super(radius, 1);
  }
}

function machineMaterialFor(
  materials: WorldMaterials,
  role: MachinePartInstance["material"],
) {
  switch (role) {
    case "ceramic":
      return materials.machineCeramic;
    case "brass":
      return materials.machineBrass;
    case "dark":
      return materials.machineDark;
    case "structure":
      return materials.machine;
  }
}

function machinePartColor(
  target: Color,
  scratch: Color,
  instance: MachinePartInstance,
  tier: number,
): Color {
  target.set(instance.color);
  switch (instance.material) {
    case "ceramic":
      return target.lerp(scratch.set("#d9d1b8"), 0.68);
    case "brass":
      return target.lerp(scratch.set("#bf8948"), 0.78);
    case "dark":
      return target.lerp(scratch.set("#142126"), 0.74);
    case "structure":
      return target.lerp(
        scratch.set(tier > 0 ? trimOf(tier).stroke : "#dcefe6"),
        tier > 0 ? 0.2 : 0.04,
      );
  }
}

type PlumeKind = "smoke" | "steam";

/** Published status decides whether a chimney is live; the effect never guesses simulation work. */
export function plumeFor(
  building: EntitySnapshot,
  definition?: BuildingDefinition,
): PlumeKind | null {
  if (building.status === "generating") {
    if (building.kind === "boiler" || definition?.power_source === "turbine")
      return "steam";
    if (definition?.power_source === "burner") return "smoke";
  }
  if (
    building.status === "composing" &&
    (definition?.recipe_category === "smelting" ||
      definition?.recipe_category === "firing")
  )
    return "smoke";
  return null;
}

/** Effect sockets follow the authored silhouette scale so smoke leaves a chimney, not its deck. */
function plumeOriginHeight(
  building: EntitySnapshot,
  definition?: BuildingDefinition,
): number {
  if (building.kind === "boiler") return 1.9;
  if (definition?.power_source === "turbine") return 1.85;
  if (definition?.power_source === "burner") return 1.65;
  if (definition?.recipe_category === "smelting") return 1.75;
  if (definition?.recipe_category === "firing") return 1.55;
  return 1.35;
}

function positiveFraction(value: number): number {
  return value - Math.floor(value);
}

function markInstancesDirty(mesh: InstancedMesh): void {
  mesh.instanceMatrix.needsUpdate = true;
  if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
}

function adjacentFootprintPairs(
  building: EntitySnapshot,
): Array<
  [EntitySnapshot["footprint"][number], EntitySnapshot["footprint"][number]]
> {
  const pairs: Array<
    [EntitySnapshot["footprint"][number], EntitySnapshot["footprint"][number]]
  > = [];
  for (let left = 0; left < building.footprint.length; left += 1) {
    for (let right = left + 1; right < building.footprint.length; right += 1) {
      const a = building.footprint[left]!;
      const b = building.footprint[right]!;
      const dq = b.q - a.q;
      const dr = b.r - a.r;
      if ((Math.abs(dq) + Math.abs(dr) + Math.abs(dq + dr)) / 2 === 1)
        pairs.push([a, b]);
    }
  }
  return pairs;
}
