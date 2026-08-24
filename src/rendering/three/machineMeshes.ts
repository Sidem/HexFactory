import {
  BoxGeometry,
  ConeGeometry,
  CylinderGeometry,
  Euler,
  Matrix4,
  Quaternion,
  SphereGeometry,
  TorusGeometry,
  Vector3,
} from "three";
import type { BufferGeometry } from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";
import { axialToPixel } from "@hexlife/embed/hex";

import type {
  BuildingDefinition,
  EntitySnapshot,
  FactorySnapshot,
} from "../../core/types";
import { TRANSPORT_DIRECTIONS } from "../../core/directions";
import { partsFor, silhouetteOf, workCycle } from "../buildingLook";
import type { PartKind, ShapePart } from "../shapeGrammar";

const TAU = Math.PI * 2;
const PART_POSITION = new Vector3();
const PART_SCALE = new Vector3();
const PART_ROTATION = new Euler(0, 0, 0, "YXZ");
const PART_QUATERNION = new Quaternion();
const ORIENTATION_ANGLES = TRANSPORT_DIRECTIONS.map((direction) => {
  const projected = axialToPixel(direction, 1, { x: 0, y: 0 });
  return Math.atan2(projected.x, projected.y);
});

export interface MachinePartInstance {
  readonly building: EntitySnapshot;
  readonly part: ShapePart;
  readonly key: string;
  readonly animated: boolean;
  readonly color: string;
  readonly glow: string | null;
  readonly groundHeight: number;
  readonly footprintScale: number;
  readonly x: number;
  readonly z: number;
}

export class PartGeometryLibrary {
  private readonly cache = new Map<string, BufferGeometry>();

  get(part: ShapePart): BufferGeometry {
    const key = geometryKey(part);
    let geometry = this.cache.get(key);
    if (!geometry) {
      geometry = buildPartGeometry(part.part, part.count ?? 0);
      geometry.computeVertexNormals();
      this.cache.set(key, geometry);
    }
    return geometry;
  }

  values(): readonly BufferGeometry[] {
    return [...this.cache.values()];
  }

  dispose(): void {
    for (const geometry of this.cache.values()) geometry.dispose();
    this.cache.clear();
  }
}

export function geometryKey(part: ShapePart): string {
  return `${part.part}:${part.part === "rotor" || part.part === "band" ? (part.count ?? 0) : 0}`;
}

/** Every grammar kind becomes reusable low-poly geometry; no definition id appears here. */
export function buildPartGeometry(
  kind: PartKind,
  count: number,
): BufferGeometry {
  switch (kind) {
    case "vessel":
      return new CylinderGeometry(0.88, 1, 1.35, 8, 1, false);
    case "chamber":
      return new BoxGeometry(1.75, 1.35, 1.55, 1, 1, 1);
    case "stack":
      return new CylinderGeometry(0.48, 0.72, 2, 6, 1, false);
    case "rotor":
      return rotorGeometry(Math.max(1, count || 3));
    case "aperture":
      return new SphereGeometry(1, 6, 4);
    case "mast":
      return mastGeometry();
    case "band":
      return bandGeometry(Math.max(0, count));
    case "mouth":
      return mouthGeometry();
  }
}

export function collectMachineParts(
  snapshot: FactorySnapshot,
  definitions: ReadonlyMap<number, BuildingDefinition>,
  groundHeight: (q: number, r: number) => number,
  buildingColors: Readonly<Record<EntitySnapshot["kind"], string>>,
): MachinePartInstance[] {
  const instances: MachinePartInstance[] = [];
  for (const building of snapshot.buildings) {
    const definition = definitions.get(building.definition_id);
    const key = silhouetteOf(
      building.kind,
      definition?.recipe_category,
      definition?.power_source,
    );
    const tier = definition?.tier ?? 0;
    const growth = building.kind === "hub" ? snapshot.contract.stage : 0;
    const cells = building.footprint.length ? building.footprint : [building];
    const centers = cells.map((cell) => axialToPixel(cell, 1, { x: 0, y: 0 }));
    const center = centers.reduce(
      (sum, point) => ({ x: sum.x + point.x, y: sum.y + point.y }),
      { x: 0, y: 0 },
    );
    center.x /= centers.length;
    center.y /= centers.length;
    const footprintScale = 1 + Math.min(2, cells.length - 1) * 0.35;
    const buildingGround = Math.max(
      ...cells.map((cell) => groundHeight(cell.q, cell.r)),
    );
    for (const part of partsFor(key, tier, growth)) {
      instances.push({
        building,
        part,
        key: geometryKey(part),
        animated: part.phase !== undefined && part.phase !== "still",
        color: buildingColors[building.kind],
        glow: part.glow ?? null,
        groundHeight: buildingGround,
        footprintScale,
        x: center.x,
        z: center.y,
      });
    }
  }
  return instances;
}

export function machinePartMatrix(
  instance: MachinePartInstance,
  now: number,
  reducedMotion: boolean,
  target = new Matrix4(),
): Matrix4 {
  const { building, part } = instance;
  const cycle = workCycle(building, now, reducedMotion);
  const buildingAngle = ORIENTATION_ANGLES[building.orientation] ?? 0;
  const phase = part.phase ?? "still";
  const localRotation = part.rotation ?? 0;
  const animatedRotation = phase === "spin" ? cycle * TAU : 0;
  const pulse = phase === "pulse" ? 1 + cycle * 0.13 : 1;
  const grind = phase === "grind" ? 0.78 + Math.sin(cycle * Math.PI) * 0.22 : 1;
  const rise = phase === "rise" ? cycle * part.scale * 1.7 : 0;
  const lateralX = Math.cos(buildingAngle) * part.x * 1.45;
  const lateralZ = -Math.sin(buildingAngle) * part.x * 1.45;
  const axisLift =
    part.part === "stack" ? Math.cos(localRotation) * part.scale * 0.75 : 0;
  PART_POSITION.set(
    instance.x + lateralX,
    instance.groundHeight + 0.2 - part.y * 1.25 + axisLift + rise,
    instance.z + lateralZ,
  );
  partScale(part, pulse, grind, PART_SCALE);
  PART_SCALE.x *= instance.footprintScale;
  PART_SCALE.z *= instance.footprintScale;
  PART_SCALE.y *= 1 + (instance.footprintScale - 1) * 0.3;
  PART_ROTATION.set(
    part.part === "rotor" || part.part === "band" ? 0 : localRotation,
    buildingAngle + animatedRotation,
    part.part === "stack" ? localRotation : 0,
  );
  PART_QUATERNION.setFromEuler(PART_ROTATION);
  return target.compose(PART_POSITION, PART_QUATERNION, PART_SCALE);
}

function partScale(
  part: ShapePart,
  pulse: number,
  grind: number,
  target: Vector3,
): Vector3 {
  const scale = part.scale;
  switch (part.part) {
    case "vessel":
      return target.set(scale * 1.35, scale * 1.25, scale * 1.35);
    case "chamber":
      return target.set(scale * 1.22, scale * 1.2, scale * 1.22);
    case "stack":
      return target.set(scale, scale, scale);
    case "rotor":
      return target.set(scale, scale * 0.62, scale);
    case "aperture":
      return target.set(scale * pulse, scale * pulse, scale * pulse);
    case "mast":
      return target.set(scale, scale, scale);
    case "band":
      return target.set(scale, scale * 0.72, scale);
    case "mouth":
      return target.set(scale * grind, scale, scale);
  }
}

function rotorGeometry(blades: number): BufferGeometry {
  const pieces: BufferGeometry[] = [new CylinderGeometry(0.22, 0.22, 0.34, 8)];
  for (let blade = 0; blade < blades; blade += 1) {
    const angle = (blade * TAU) / blades;
    const geometry = new BoxGeometry(0.22, 0.18, 0.94);
    geometry.translate(0, 0, 0.5);
    geometry.rotateY(angle);
    pieces.push(geometry);
  }
  const merged = mergeGeometries(pieces, false);
  for (const piece of pieces) piece.dispose();
  if (!merged) throw new Error("Could not merge rotor geometry");
  return merged;
}

function mastGeometry(): BufferGeometry {
  const stem = new CylinderGeometry(0.16, 0.22, 2.2, 6);
  const arm = new BoxGeometry(1.2, 0.16, 0.16);
  arm.translate(0, 0.55, 0);
  const merged = mergeGeometries([stem, arm], false);
  stem.dispose();
  arm.dispose();
  if (!merged) throw new Error("Could not merge mast geometry");
  return merged;
}

function bandGeometry(rivets: number): BufferGeometry {
  const pieces: BufferGeometry[] = [new TorusGeometry(0.78, 0.12, 4, 8)];
  pieces[0]!.rotateX(Math.PI / 2);
  for (let rivet = 0; rivet < rivets; rivet += 1) {
    const angle = (rivet * TAU) / Math.max(1, rivets);
    const stud = new SphereGeometry(0.13, 4, 3);
    stud.translate(Math.cos(angle) * 0.78, 0.08, Math.sin(angle) * 0.78);
    pieces.push(stud);
  }
  const merged = mergeGeometries(pieces, false);
  for (const piece of pieces) piece.dispose();
  if (!merged) throw new Error("Could not merge band geometry");
  return merged;
}

function mouthGeometry(): BufferGeometry {
  const left = new ConeGeometry(0.72, 1.5, 4);
  left.rotateZ(-Math.PI / 2.8);
  left.translate(-0.35, 0, 0);
  const right = new ConeGeometry(0.72, 1.5, 4);
  right.rotateZ(Math.PI / 2.8);
  right.translate(0.35, 0, 0);
  const merged = mergeGeometries([left, right], false);
  left.dispose();
  right.dispose();
  if (!merged) throw new Error("Could not merge mouth geometry");
  return merged;
}
