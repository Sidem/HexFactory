import {
  Box3,
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
import {
  chamberAssembly,
  vesselAssembly,
  stackAssembly,
} from "./machineAnatomy";
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
    barreling: 2.05,
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

/**
 * How far the plinth a machine stands on rises above its cell's ground height. `buildingFoot` and
 * the multi-cell decks are built to reach exactly this far, so the number lives beside the
 * placement that has to land on it rather than being spelled again at each mesh.
 */
export const MACHINE_PLATFORM_HEIGHT = 0.18;

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
  /** Lift that seats this silhouette on its platform — see `machineRestingLift`. */
  readonly baseLift: number;
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
      return vesselAssembly();
    case "chamber":
      return chamberGeometry();
    case "stack":
      return stackAssembly();
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
  // Every building of one silhouette, tier and plot size rests at the same height, and a factory
  // holds many of each, so the bound is measured once per shape instead of once per machine.
  const lifts = new Map<string, number>();
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
    // How big a machine looks follows how far its plot actually reaches, not how many hexes it
    // happens to be made of. Counting cells and capping at three was readable while nothing stood
    // on more than a hex or two; against a nineteen-hex refinery it drew the same silhouette as a
    // pair. Reach is measured from the plot's own centre, so a compact ring and a long line of the
    // same count read differently, which is what the player is looking at. The coefficient is
    // tuned so a plot's silhouette grows to cover most of it while leaving the rim cells showing:
    // the service clearance stays visible, which is the thing the player reads a plot for.
    const reach = Math.max(
      ...centers.map((point) =>
        Math.hypot(point.x - center.x, point.y - center.y),
      ),
    );
    const footprintScale = 1 + reach * 0.9;
    const buildingGround = Math.max(
      ...cells.map((cell) => groundHeight(cell.q, cell.r)),
    );
    const parts = partsFor(key, tier, growth);
    const liftKey = `${key}|${tier}|${growth}|${footprintScale.toFixed(3)}`;
    let baseLift = lifts.get(liftKey);
    if (baseLift === undefined) {
      baseLift = machineRestingLift(
        parts,
        MACHINE_SILHOUETTE_SCALE[key],
        footprintScale,
      );
      lifts.set(liftKey, baseLift);
    }
    for (const part of parts) {
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
        baseLift,
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
  const { building } = instance;
  return composeMachinePart(
    instance.part,
    {
      visualScale: MACHINE_VISUAL_SCALE * instance.visualScale,
      footprintScale: instance.footprintScale,
      angle: directionAngle(building.orientation),
      cycle: workCycle(building, now, reducedMotion),
      x: instance.x,
      baseY:
        instance.groundHeight + MACHINE_PLATFORM_HEIGHT + instance.baseLift,
      z: instance.z,
    },
    target,
  );
}

/** Where a silhouette's parts are placed from, once the building around them is resolved. */
interface PartPlacement {
  /** `MACHINE_VISUAL_SCALE` times the silhouette's own scale. */
  visualScale: number;
  footprintScale: number;
  angle: number;
  cycle: number;
  x: number;
  /** World height the grammar's `y = 0` sits at: the platform top, plus the resting lift. */
  baseY: number;
  z: number;
}

function composeMachinePart(
  part: ShapePart,
  place: Readonly<PartPlacement>,
  target: Matrix4,
): Matrix4 {
  const { cycle, visualScale } = place;
  const phase = part.phase ?? "still";
  const localRotation = part.rotation ?? 0;
  const animatedRotation = phase === "spin" ? cycle * TAU : 0;
  const uprightRotor = part.part === "rotor" && part.upright === true;
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
  const lateralX = Math.cos(place.angle) * lateral;
  const lateralZ = -Math.sin(place.angle) * lateral;
  const axisLift =
    part.part === "stack"
      ? Math.cos(localRotation) * part.scale * 0.75
      : part.part === "mast"
        ? part.scale * 1.1
        : 0;
  PART_POSITION.set(
    place.x + lateralX,
    place.baseY + (-part.y * 1.25 + axisLift + rise) * visualScale,
    place.z + lateralZ,
  );
  partScale(part, pulse, grind, PART_SCALE);
  PART_SCALE.multiplyScalar(visualScale);
  PART_SCALE.x *= place.footprintScale;
  PART_SCALE.z *= place.footprintScale;
  PART_SCALE.y *= 1 + (place.footprintScale - 1) * 0.3;
  const buildingAngle = place.angle;
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

const REST_MATRIX = new Matrix4();
const REST_BOX = new Box3();
const REST_PLACEMENT: PartPlacement = {
  visualScale: 1,
  footprintScale: 1,
  angle: 0,
  cycle: 0,
  x: 0,
  baseY: 0,
  z: 0,
};
/** An upright rotor sweeps its own lowest point around, so its bound is sampled through a turn. */
const SPIN_SAMPLES = [0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875] as const;
const STILL_SAMPLE = [0] as const;
const PULSE_SAMPLES = [0, 1] as const;
const localBounds = new Map<string, Box3>();

/** The authored vertical reach of a part's geometry, measured once per geometry key. */
function partLocalBounds(part: ShapePart): Box3 {
  const key = geometryKey(part);
  let bounds = localBounds.get(key);
  if (!bounds) {
    const geometry = buildPartGeometry(part.part, part.count ?? 0);
    geometry.computeBoundingBox();
    bounds = geometry.boundingBox!.clone();
    geometry.dispose();
    localBounds.set(key, bounds);
  }
  return bounds;
}

/**
 * How far to raise a silhouette so it stands on its plinth instead of wading through it.
 *
 * The grammar's `y` is a 2D anchor measured from the hex centre, and a shape straddles it: a
 * smelter's vessel is authored half above the centre and half below, which is exactly right for
 * the flat stamp the canvas walker draws. Read as a height above the ground it buried the lower
 * third to three quarters of every machine in the deck it is bolted to. The anchor still sets the
 * parts' heights relative to each other — that authored hierarchy is what makes a tier legible —
 * so the whole assembly moves as one, by its own measured overhang.
 *
 * A part rotated past horizontal is reaching *into* the ground on purpose: an extractor's drill
 * and a pump's intake shaft are the machine biting the cell it works. Those are left out of the
 * bound, or the drill would be jacked up out of the hole it is boring.
 */
export function machineRestingLift(
  parts: readonly ShapePart[],
  silhouetteScale: number,
  footprintScale = 1,
): number {
  let lowest = Number.POSITIVE_INFINITY;
  REST_PLACEMENT.visualScale = MACHINE_VISUAL_SCALE * silhouetteScale;
  REST_PLACEMENT.footprintScale = footprintScale;
  for (const part of parts) {
    if (Math.cos(part.rotation ?? 0) < 0) continue;
    for (const cycle of restSamples(part)) {
      REST_PLACEMENT.cycle = cycle;
      composeMachinePart(part, REST_PLACEMENT, REST_MATRIX);
      REST_BOX.copy(partLocalBounds(part)).applyMatrix4(REST_MATRIX);
      lowest = Math.min(lowest, REST_BOX.min.y);
    }
  }
  // A silhouette with no standing parts — `belt`, whose deck is shared transport geometry — has
  // nothing to seat, so it keeps the platform height it already had.
  return Number.isFinite(lowest) ? -lowest : 0;
}

/** The work-cycle phases at which a part can reach its lowest. */
function restSamples(part: ShapePart): readonly number[] {
  if (part.phase === "spin" && part.upright === true) return SPIN_SAMPLES;
  // A pulse only ever swells, and a grind is a horizontal squeeze; a rise off a part that has not
  // been rotated under the deck travels upward, so rest is its floor.
  if (part.phase === "pulse") return PULSE_SAMPLES;
  return STILL_SAMPLE;
}

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

/** Box-framed process housing contrasts with round pressure vessels. */
function chamberGeometry(): BufferGeometry {
  return chamberAssembly();
}

function rotorGeometry(blades: number): BufferGeometry {
  const pieces: BufferGeometry[] = [
    new CylinderGeometry(0.18, 0.28, 0.46, 10),
    new CylinderGeometry(0.34, 0.34, 0.08, 10).translate(0, -0.15, 0),
  ];
  for (let blade = 0; blade < blades; blade += 1) {
    const angle = (blade * TAU) / blades;
    const geometry = new BoxGeometry(0.22, 0.18, 0.94);
    geometry.translate(0, 0, 0.5);
    geometry.rotateY(angle);
    pieces.push(geometry);
    const tip = new BoxGeometry(0.32, 0.12, 0.28);
    tip.translate(0, 0.04, 0.82);
    tip.rotateY(angle);
    pieces.push(tip);
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
  const pieces: BufferGeometry[] = [stem, arm];
  for (const x of [-0.46, 0.46]) {
    pieces.push(new CylinderGeometry(0.1, 0.1, 0.32, 8).translate(x, 0.72, 0));
    for (const y of [0.65, 0.76, 0.86])
      pieces.push(new CylinderGeometry(0.15, 0.15, 0.04, 8).translate(x, y, 0));
    pieces.push(
      new BoxGeometry(0.09, 0.8, 0.09)
        .rotateZ(x < 0 ? -0.6 : 0.6)
        .translate(x / 2, 0.25, 0),
    );
  }
  pieces.push(new CylinderGeometry(0.32, 0.36, 0.16, 8).translate(0, -1.02, 0));
  const merged = mergeGeometries(pieces, false);
  for (const piece of pieces) piece.dispose();
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
