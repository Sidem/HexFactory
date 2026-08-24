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
import { cargoTravel, stallMark, trimOf } from "../buildingLook";
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
import { cellKey, stableVariation } from "./terrainMeshes";
import { createTransportGeometry, transportScale } from "./transportGeometry";
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

interface TransportTreads {
  readonly mesh: InstancedMesh;
  readonly buildings: readonly EntitySnapshot[];
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

export class WorldInstanceLayer {
  readonly group = new Group();
  readonly geometryLibrary = new PartGeometryLibrary();
  private readonly transportGeometry = createTransportGeometry();
  private readonly definitions: ReadonlyMap<number, BuildingDefinition>;
  private readonly items: ReadonlyMap<number, ItemDefinition>;
  private readonly geometry = {
    buildingFoot: new CylinderGeometry(0.72, 0.78, 0.18, 6),
    footprintDeck: new CylinderGeometry(0.94, 0.98, 1, 6),
    footprintLink: new BoxGeometry(1, 1, 1),
    belt: this.transportGeometry.belt,
    beltDetail: this.transportGeometry.beltDetail,
    bridge: this.transportGeometry.bridge,
    ore: new OctahedronGeometry(1, 0),
    lump: new IcosahedronGeometry(1, 0),
    grains: new ConeGeometry(1, 1, 8),
    crystal: new ConeGeometry(1, 1, 4),
    trunk: new CylinderGeometry(1, 1, 1, 7),
    canopy: new ConeGeometry(1, 1, 7),
    progress: new BoxGeometry(0.38, 0.08, 0.1),
    cargo: new IcosahedronGeometry(0.09, 0),
    status: new SphereGeometryCompat(0.09),
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
  private transportTreads: TransportTreads | null = null;
  private structureKey = "";
  private resourcesIdentity: FactorySnapshot["resources"] | null = null;
  private snapshot: FactorySnapshot | null = null;
  private terrainByKey: ReadonlyMap<string, TerrainCell> = new Map();
  private statusMesh: InstancedMesh | null = null;
  private progressMesh: InstancedMesh | null = null;
  private cargoMesh: InstancedMesh | null = null;
  private readonly playerBody: Mesh;
  private readonly playerFacing: Mesh;
  private readonly playerWork: Mesh;
  private readonly pointById = new Map<number, { x: number; z: number }>();
  private readonly groundById = new Map<number, number>();
  private readonly scratchMatrix = new Matrix4();
  private readonly scratchPosition = new Vector3();
  private readonly scratchQuaternion = new Quaternion();
  private readonly scratchScale = new Vector3(1, 1, 1);
  private readonly scratchColor = new Color();
  private readonly scratchTrim = new Color();
  private playerDirty = true;

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
    this.group.add(
      this.staticGroup,
      this.resourceGroup,
      this.dynamicGroup,
      this.playerGroup,
    );
    this.playerBody = new Mesh(
      new CylinderGeometry(0.18, 0.24, 0.48, 7),
      materials.machine,
    );
    this.playerBody.castShadow = true;
    this.playerBody.material = materials.machine;
    this.playerFacing = new Mesh(
      new ConeGeometry(0.09, 0.36, 5),
      materials.emissive,
    );
    this.playerFacing.rotateX(Math.PI / 2);
    this.playerWork = new Mesh(
      new RingGeometry(0.31, 0.36, 32),
      materials.overlaySelection,
    );
    this.playerWork.rotateX(-Math.PI / 2);
    this.playerGroup.add(this.playerBody, this.playerFacing, this.playerWork);
  }

  setSnapshot(
    snapshot: FactorySnapshot,
    terrainByKey: ReadonlyMap<string, TerrainCell>,
  ): boolean {
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
    this.ensureDynamicCapacity(snapshot.buildings.length);
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
    this.updateTransportTreads(now, reducedMotion);
    this.updateDynamicBuildings(snapshot, now, reducedMotion);
    if (this.playerDirty) {
      this.playerDirty = false;
      this.updatePlayer(snapshot);
    }
  }

  dispose(): void {
    this.geometryLibrary.dispose();
    for (const geometry of this.ownedGeometries) geometry.dispose();
    this.playerBody.geometry.dispose();
    this.playerFacing.geometry.dispose();
    this.playerWork.geometry.dispose();
  }

  private rebuildStatic(snapshot: FactorySnapshot): void {
    this.group.remove(this.staticGroup);
    this.staticGroup = new Group();
    this.staticGroup.name = "static-factory";
    this.partBuckets = [];
    this.transportTreads = null;
    this.pointById.clear();
    this.groundById.clear();
    const matrix = new Matrix4();
    const quaternion = new Quaternion();
    const position = new Vector3();
    const scale = new Vector3(1, 1, 1);
    const color = new Color();

    const singleCellBuildings = snapshot.buildings.filter(
      (building) => building.footprint.length <= 1,
    );
    for (const building of snapshot.buildings) {
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
        scale.set(1, 1, 1);
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
    const belts = snapshot.buildings.filter(({ kind }) => kind === "belt");
    if (belts.length) {
      const frame = new InstancedMesh(
        this.geometry.belt,
        this.materials.machine,
        belts.length,
      );
      frame.name = "transport-rails";
      const treads = new InstancedMesh(
        this.geometry.beltDetail,
        this.materials.machineDark,
        belts.length,
      );
      treads.name = "transport-treads";
      for (const [index, building] of belts.entries()) {
        const center = axialToPixel(building, 1, { x: 0, y: 0 });
        const height = this.groundHeight(building.q, building.r) + 0.23;
        const angle = directionAngle(building.orientation);
        position.set(center.x, height, center.y);
        quaternion.setFromAxisAngle(new Vector3(0, 1, 0), angle);
        const definition = this.definitions.get(building.definition_id);
        const [x, y, z] = definition
          ? transportScale(definition)
          : ([1, 1, 1] as const);
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
      this.transportTreads = { mesh: treads, buildings: belts };
      this.staticGroup.add(frame, treads);

      const connected = connectedBeltLinks(snapshot.buildings);
      if (connected.length) {
        const links = new InstancedMesh(
          this.geometry.belt,
          this.materials.machine,
          connected.length,
        );
        links.name = "transport-connections";
        links.castShadow = true;
        for (const [index, { from, to }] of connected.entries()) {
          const a = this.pointById.get(from.id)!;
          const b = this.pointById.get(to.id)!;
          const dx = b.x - a.x;
          const dz = b.z - a.z;
          const length = Math.hypot(dx, dz);
          position.set(
            (a.x + b.x) / 2,
            Math.max(
              this.groundById.get(from.id) ?? 0.07,
              this.groundById.get(to.id) ?? 0.07,
            ) + 0.23,
            (a.z + b.z) / 2,
          );
          quaternion.setFromAxisAngle(WORLD_UP, Math.atan2(-dz, dx));
          scale.set(length / 0.92, 1, 1);
          matrix.compose(position, quaternion, scale);
          links.setMatrixAt(index, matrix);
          links.setColorAt(index, color.set(BUILDING_COLORS.belt));
        }
        markInstancesDirty(links);
        this.staticGroup.add(links);
      }
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

  private updateTransportTreads(now: number, reducedMotion: boolean): void {
    const bucket = this.transportTreads;
    if (!bucket) return;
    for (const [index, building] of bucket.buildings.entries()) {
      const center = this.pointById.get(building.id);
      if (!center) continue;
      const angle = directionAngle(building.orientation);
      const definition = this.definitions.get(building.definition_id);
      const [x, y, z] = definition
        ? transportScale(definition)
        : ([1, 1, 1] as const);
      const target = building.next_id
        ? this.pointById.get(building.next_id)
        : undefined;
      const directionDistance = target
        ? Math.hypot(target.x - center.x, target.z - center.z)
        : x * 0.92;
      const treadSpacing = 0.175 * x;
      const phase = reducedMotion
        ? 0
        : ((cargoTravel(now, false, building.id) * directionDistance) %
            treadSpacing) -
          treadSpacing / 2;
      this.scratchPosition.set(
        center.x + Math.cos(angle) * phase,
        (this.groundById.get(building.id) ?? 0.07) + 0.23,
        center.z - Math.sin(angle) * phase,
      );
      this.scratchQuaternion.setFromAxisAngle(WORLD_UP, angle);
      this.scratchScale.set(x, y, z);
      this.scratchMatrix.compose(
        this.scratchPosition,
        this.scratchQuaternion,
        this.scratchScale,
      );
      bucket.mesh.setMatrixAt(index, this.scratchMatrix);
    }
    bucket.mesh.instanceMatrix.needsUpdate = true;
  }

  private addOutputIndicators(snapshot: FactorySnapshot): void {
    const buildings = snapshot.buildings.filter((building) =>
      hasDirectionalOutput(building.kind),
    );
    if (!buildings.length) return;
    const mesh = new InstancedMesh(
      this.geometry.outputIndicator,
      this.materials.emissive,
      buildings.length,
    );
    mesh.name = "building-output-indicators";
    for (const [index, building] of buildings.entries()) {
      const center = this.pointById.get(building.id)!;
      const angle = directionAngle(building.orientation);
      const footprintReach = building.footprint.length > 1 ? 0.9 : 0.56;
      this.scratchPosition.set(
        center.x + Math.cos(angle) * footprintReach,
        (this.groundById.get(building.id) ?? 0.07) + 0.53,
        center.z - Math.sin(angle) * footprintReach,
      );
      this.scratchQuaternion.setFromAxisAngle(WORLD_UP, angle);
      this.scratchMatrix.compose(
        this.scratchPosition,
        this.scratchQuaternion,
        this.scratchScale.set(1, 1, 1),
      );
      mesh.setMatrixAt(index, this.scratchMatrix);
      mesh.setColorAt(index, this.scratchColor.set("#ffd166"));
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
      const key = `${instance.key}:${instance.glow ? "glow" : "solid"}:${instance.animated ? "animated" : "static"}`;
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
        first.glow ? this.materials.emissive : this.materials.machine,
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
            : color
                .set(instance.color)
                .lerp(
                  this.scratchTrim.set(
                    tier > 0 ? trimOf(tier).stroke : "#dcefe6",
                  ),
                  tier > 0 ? 0.2 : 0.04,
                ),
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
    const grains: ResourcePartInstance[] = [];
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
          add(grains, -0.19, -0.08, 0.25, 0.13, 0.22, 0.065);
          add(grains, 0.17, -0.03, 0.28, 0.16, 0.24, 0.08);
          add(grains, 0.02, 0.21, 0.22, 0.11, 0.2, 0.055);
          break;
        case "lump":
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
    this.addResourceParts("grain-field-mounds", this.geometry.grains, grains);
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
    material: WorldMaterials["resource"] | WorldMaterials["emissive"] = this
      .materials.resource,
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
    this.dynamicGroup.add(this.statusMesh, this.progressMesh, this.cargoMesh);
  }

  private updateDynamicBuildings(
    snapshot: FactorySnapshot,
    now: number,
    reducedMotion: boolean,
  ): void {
    if (!this.statusMesh || !this.progressMesh || !this.cargoMesh) return;
    const matrix = this.scratchMatrix;
    const color = this.scratchColor;
    const position = this.scratchPosition;
    const quaternion = this.scratchQuaternion.identity();
    const scale = this.scratchScale.set(1, 1, 1);
    let statuses = 0;
    let progresses = 0;
    let cargos = 0;
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
      if (building.cargo) {
        const target = building.next_id
          ? this.pointById.get(building.next_id)
          : undefined;
        const travel = cargoTravel(now, reducedMotion, building.id);
        const tx = target?.x ?? center.x;
        const tz = target?.z ?? center.z;
        matrix.compose(
          position.set(
            center.x + (tx - center.x) * travel,
            height + 0.42,
            center.z + (tz - center.z) * travel,
          ),
          quaternion,
          scale.set(1, 1, 1),
        );
        this.cargoMesh.setMatrixAt(cargos, matrix);
        this.cargoMesh.setColorAt(
          cargos,
          color.set(this.items.get(building.cargo.item_id)?.color ?? "#ffffff"),
        );
        cargos += 1;
      }
    }
    this.statusMesh.count = statuses;
    this.progressMesh.count = progresses;
    this.cargoMesh.count = cargos;
    markInstancesDirty(this.statusMesh);
    markInstancesDirty(this.progressMesh);
    markInstancesDirty(this.cargoMesh);
  }

  private updatePlayer(snapshot: FactorySnapshot): void {
    const player = snapshot.player;
    const axial = pixelToAxial(player, WORLD_SCALE);
    const height = this.groundHeight(axial.q, axial.r);
    const x = player.x / WORLD_SCALE;
    const z = player.y / WORLD_SCALE;
    this.playerGroup.position.set(x, height + 0.26, z);
    this.playerBody.position.set(0, 0, 0);
    this.playerBody.scale.set(1, 1, 1);
    const facing = Math.atan2(player.facing_x, player.facing_y);
    this.playerFacing.position.set(
      Math.sin(facing) * 0.32,
      0.12,
      Math.cos(facing) * 0.32,
    );
    this.playerFacing.rotation.set(Math.PI / 2, 0, -facing);
    const total = player.action_cooldown_total;
    const done = total > 0 ? 1 - player.action_cooldown / total : 0;
    this.playerWork.visible = player.action_cooldown > 0;
    this.playerWork.scale.setScalar(Math.max(0.05, done));
  }

  private groundHeight(q: number, r: number): number {
    return this.terrainByKey.get(cellKey(q, r))?.height ?? 0.07;
  }
}

const WORLD_UP = new Vector3(0, 1, 0);

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

function connectedBeltLinks(
  buildings: readonly EntitySnapshot[],
): { from: EntitySnapshot; to: EntitySnapshot }[] {
  const byId = new Map(buildings.map((building) => [building.id, building]));
  return buildings.flatMap((from) => {
    if (from.kind !== "belt" || !from.next_id) return [];
    const to = byId.get(from.next_id);
    if (!to || to.kind !== "belt") return [];
    const direction = TRANSPORT_DIRECTIONS[from.orientation];
    if (!direction) return [];
    if (to.q - from.q !== direction.q || to.r - from.r !== direction.r)
      return [];
    return [{ from, to }];
  });
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

/** Preserve each item's hue while ensuring even coal and stone stay legible against dark terrain. */
export function fieldVisualColor(source: string): string {
  const color = new Color(source);
  const hsl = { h: 0, s: 0, l: 0 };
  color.getHSL(hsl);
  color.setHSL(
    hsl.h,
    Math.min(1, Math.max(0.18, hsl.s * 1.2)),
    Math.min(0.72, Math.max(0.46, hsl.l * 1.08)),
  );
  return `#${color.getHexString()}`;
}

/** Small low-poly status bead without importing another geometry family into the bucket model. */
class SphereGeometryCompat extends IcosahedronGeometry {
  constructor(radius: number) {
    super(radius, 1);
  }
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
