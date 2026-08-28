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
import {
  partsFor,
  silhouetteOf,
  workCycle,
  type SilhouetteKey,
} from "../buildingLook";
import type { MachineMaterialRole, PartKind, ShapePart } from "../shapeGrammar";
import { directionAngle } from "./directionAngle";

const TAU = Math.PI * 2;
const PART_POSITION = new Vector3();
const PART_SCALE = new Vector3();
const PART_ROTATION = new Euler(0, 0, 0, "YXZ");
const PART_QUATERNION = new Quaternion();

/** Machines should read as factory equipment beside the narrow transport deck, not as another
 * belt-sized token. This presentation-only multiplier grows the generated grammar uniformly. */
export const MACHINE_VISUAL_SCALE = 1.38;

/**
 * How wide a machine's *body* is allowed to be against the height the hierarchy above gives it.
 *
 * Neighbouring hex centres are `√3` apart at world scale, so a body wider than `√3 / 2` overlaps
 * the machine next door — which is what every generated silhouette did, because the grammar's
 * girth and its height came off the same multiplier. Narrowing the bodies alone keeps the authored
 * height hierarchy intact while a machine stays inside the hex it was placed on. Wheels, masts and
 * vents are reach rather than bulk, so they keep their full scale; anchors move with the body they
 * are bolted to.
 */
export const MACHINE_BODY_GIRTH = 0.58;

/**
 * Silhouette scale is part of the authored visual hierarchy, not simulation size. Poles stay
 * narrow utility infrastructure, ordinary machines read above the Wayfinder's waist, and the wind
 * turbine owns the skyline. Multi-cell occupancy is still supplied exclusively by native state.
 */
export const MACHINE_SILHOUETTE_SCALE: Readonly<Record<SilhouetteKey, number>> =
  Object.freeze({
    extractor: 2.05,
    belt: 1,
    composer: 2.05,
    assembly: 2.05,
    "primitive-smelting": 1.7,
    "manual-workshop": 1.8,
    smelting: 2.1,
    firing: 2.05,
    cutting: 2,
    crushing: 2.05,
    refining: 2.15,
    "asphalt-mixing": 2,
    "oil-extraction": 2.1,
    container: 2,
    consumer: 1.55,
    hub: 1.45,
    pump: 1.95,
    pole: 0.9,
    generator: 2.05,
    burner: 2.1,
    wind: 3.1,
    hydro: 2.1,
    turbine: 2.1,
    boiler: 2.1,
    bridge: 1,
  });

export interface MachinePartInstance {
  readonly building: EntitySnapshot;
  readonly part: ShapePart;
  readonly key: string;
  readonly animated: boolean;
  readonly color: string;
  readonly glow: string | null;
  readonly material: MachineMaterialRole;
  readonly groundHeight: number;
  readonly footprintScale: number;
  readonly visualScale: number;
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
      // Six sides, not eight: a machine standing on a hex reads as part of the grid it occupies,
      // and an octagonal drum on a hexagonal pad never quite lined up with anything around it.
      return new CylinderGeometry(0.88, 1, 1.35, 6, 1, false);
    case "chamber":
      return chamberGeometry();
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
      definition?.recipe_category ?? definition?.source_category,
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
    const footprintScale = 1 + Math.min(2, cells.length - 1) * 0.18;
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
        material: part.material ?? "structure",
        groundHeight: buildingGround,
        footprintScale,
        visualScale: MACHINE_SILHOUETTE_SCALE[key],
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
  const buildingAngle = directionAngle(building.orientation);
  const phase = part.phase ?? "still";
  const localRotation = part.rotation ?? 0;
  const animatedRotation = phase === "spin" ? cycle * TAU : 0;
  const uprightRotor = part.part === "rotor" && part.upright === true;
  const visualScale = MACHINE_VISUAL_SCALE * instance.visualScale;
  const pulse = phase === "pulse" ? 1 + cycle * 0.13 : 1;
  const grind = phase === "grind" ? 0.78 + Math.sin(cycle * Math.PI) * 0.22 : 1;
  // `rise` travels along the part's own axis, the same rule the 2D walker follows: an upright vent
  // puffs upward and a shaft driven to `PI` plunges. Without the sign an extractor's drill rose out
  // of the ground it is supposed to be biting into.
  const rise =
    phase === "rise" ? cycle * part.scale * 1.7 * Math.cos(localRotation) : 0;
  // Anchors ride the body they are bolted to, so a survey lamp set against a vessel's flank stays
  // against it once the body is narrowed rather than floating off into the next hex.
  const lateral = part.x * 1.45 * visualScale * MACHINE_BODY_GIRTH;
  const lateralX = Math.cos(buildingAngle) * lateral;
  const lateralZ = -Math.sin(buildingAngle) * lateral;
  const axisLift =
    part.part === "stack"
      ? Math.cos(localRotation) * part.scale * 0.75
      : part.part === "mast"
        ? part.scale * 1.1
        : 0;
  PART_POSITION.set(
    instance.x + lateralX,
    instance.groundHeight +
      0.2 +
      (-part.y * 1.25 + axisLift + rise) * visualScale,
    instance.z + lateralZ,
  );
  partScale(part, pulse, grind, PART_SCALE);
  PART_SCALE.multiplyScalar(visualScale);
  PART_SCALE.x *= instance.footprintScale;
  PART_SCALE.z *= instance.footprintScale;
  PART_SCALE.y *= 1 + (instance.footprintScale - 1) * 0.3;
  if (uprightRotor) {
    // Rotor geometry is authored in the XZ plane around local Y. Tilt that disc upright, yaw its
    // normal with the building, then spin around the rotor's own local Y axis. Euler Z rotation
    // made the old turbine tumble like a ceiling fan seen edge-on.
    PART_QUATERNION.setFromAxisAngle(WORLD_Y, buildingAngle)
      .multiply(ROTOR_UPRIGHT)
      .multiply(PART_SPIN.setFromAxisAngle(WORLD_Y, animatedRotation));
  } else {
    PART_ROTATION.set(
      part.part === "rotor" || part.part === "band" ? 0 : localRotation,
      buildingAngle + animatedRotation,
      part.part === "stack" ? localRotation : 0,
    );
    PART_QUATERNION.setFromEuler(PART_ROTATION);
  }
  return target.compose(PART_POSITION, PART_QUATERNION, PART_SCALE);
}

const WORLD_Y = new Vector3(0, 1, 0);
const ROTOR_UPRIGHT = new Quaternion().setFromAxisAngle(
  new Vector3(1, 0, 0),
  Math.PI / 2,
);
const PART_SPIN = new Quaternion();

function partScale(
  part: ShapePart,
  pulse: number,
  grind: number,
  target: Vector3,
): Vector3 {
  const scale = part.scale;
  const girth = scale * MACHINE_BODY_GIRTH;
  switch (part.part) {
    case "vessel":
      return target.set(girth * 1.35, scale * 1.25, girth * 1.35);
    case "chamber":
      return target.set(girth * 1.22, scale * 1.2, girth * 1.22);
    case "stack":
      return target.set(scale, scale, scale);
    case "rotor":
      return target.set(scale, scale * 0.62, scale);
    case "aperture":
      return target.set(scale * pulse, scale * pulse, scale * pulse);
    case "mast":
      return target.set(scale, scale, scale);
    case "band":
      // Part scale names the vessel it embraces. The torus geometry's authored radius is smaller
      // than a vessel's, so the old 1:1 transform buried every brass ring inside the body.
      return target.set(girth * 1.6, scale * 0.72, girth * 1.6);
    case "mouth":
      return target.set(girth * grind, scale, girth);
  }
}

/**
 * The chamber is the vessel's opposite number and shares its hexagonal footprint: straight sides
 * rather than a taper, and turned half a face so the two still read as different machines standing
 * on the same grid.
 */
function chamberGeometry(): BufferGeometry {
  const body = new CylinderGeometry(0.94, 1, 1.45, 6, 1, false);
  body.rotateY(Math.PI / 6);
  return body;
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
