import { Euler, Quaternion, Vector3 } from "three";
import type { Matrix4 } from "three";
import { axialToPixel } from "@hexlife/embed/hex";
import { TRANSPORT_DIRECTIONS } from "../../core/directions";
import type { EntitySnapshot } from "../../core/types";
import type { MachinePartInstance } from "./machineMeshes";
import { directionAngle } from "./directionAngle";
import { elementaryScale } from "./elementaryModels";

const UP = new Vector3(0, 1, 0);
const position = new Vector3();
const size = new Vector3();
const rotation = new Quaternion();
const euler = new Euler();
const yaw = new Quaternion();
const axis = new Vector3();
const TURN = Math.PI * 2;
const clamp = (value: number): number => Math.max(0, Math.min(1, value));
const smooth = (value: number): number => {
  const t = clamp(value);
  return t * t * (3 - 2 * t);
};

/** Shortest signed turn, settled at half a circle so opposite cells always sweep the same way. */
function turnTo(from: number, to: number): number {
  const shifted = (((to - from) % TURN) + TURN) % TURN;
  return shifted > Math.PI ? shifted - TURN : shifted;
}

/** Carry the hand around the mast. A straight drag passes through it and flips the arm over. */
function swing(
  from: Vector3,
  to: Vector3,
  t: number,
  x: number,
  z: number,
): Vector3 {
  const fromRadius = Math.hypot(from.x - x, from.z - z);
  const toRadius = Math.hypot(to.x - x, to.z - z);
  // An extractor working its own hex sits on the mast, where a bearing means nothing. Borrowing the
  // far end's bearing reaches straight in and out instead of spiralling around for no reason.
  const fromAngle =
    fromRadius > 0.01 ? Math.atan2(from.x - x, from.z - z) : undefined;
  const toAngle = toRadius > 0.01 ? Math.atan2(to.x - x, to.z - z) : undefined;
  const start = fromAngle ?? toAngle ?? 0;
  const angle = start + turnTo(start, toAngle ?? start) * t;
  const radius = fromRadius + (toRadius - fromRadius) * t;
  return new Vector3(
    x + Math.sin(angle) * radius,
    from.y + (to.y - from.y) * t,
    z + Math.cos(angle) * radius,
  );
}

export function machineWorking(building: EntitySnapshot): boolean {
  return (
    building.status === "extracting" ||
    building.status === "composing" ||
    building.status === "generating" ||
    building.status === "pumping"
  );
}

/** Interpolate two published progress values, never run a recipe ahead of native. */
export function elementaryCycle(
  instance: MachinePartInstance,
  now: number,
  reducedMotion: boolean,
): number {
  const b = instance.building;
  const value = b.progress_total > 0 ? clamp(b.progress / b.progress_total) : 0;
  const previous = instance.previousBuilding;
  if (
    reducedMotion ||
    !machineWorking(b) ||
    !previous ||
    !machineWorking(previous)
  )
    return value;
  const before =
    previous.progress_total > 0
      ? clamp(previous.progress / previous.progress_total)
      : 0;
  const elapsed = clamp(
    (now - (instance.receivedAt ?? now)) /
      Math.max(1, instance.tickDuration ?? 100),
  );
  const end =
    value < before && b.progress_total === previous.progress_total ? 1 : value;
  return before + (end - before) * elapsed;
}

export interface ExtractorPose {
  shoulder: Vector3;
  elbow: Vector3;
  wrist: Vector3;
  source: Vector3;
  output: Vector3;
  carrying: boolean;
  itemId?: number;
}

/** Work target and port are native answers. Only the path connecting them is presentation. */
export function extractorPose(
  instance: MachinePartInstance,
  cycle: number,
): ExtractorPose {
  const b = instance.building;
  const [horizontal, vertical] = elementaryScale(instance.footprintScale);
  const angle = directionAngle(b.orientation);
  const shoulder = new Vector3(-0.32 * horizontal, 0.9 * vertical, 0)
    .applyAxisAngle(UP, angle)
    .add(new Vector3(instance.x, instance.groundHeight + 0.18, instance.z));
  // Finish the return from a depleted cell using the preceding native target.
  const completed =
    instance.previousBuilding &&
    b.progress < instance.previousBuilding.progress &&
    cycle > 0.5;
  const source =
    (completed
      ? instance.previousBuilding?.extraction_source
      : b.extraction_source) ?? b.extraction_source;
  const route =
    b.extraction_output ??
    b.output_routes?.find((port) => port.item_id === source?.item_id) ??
    b.output_routes?.[0];
  const cell = route ?? b;
  const point = axialToPixel(cell, 1, { x: 0, y: 0 });
  const heading =
    TRANSPORT_DIRECTIONS[route?.direction ?? b.orientation] ??
    TRANSPORT_DIRECTIONS[0]!;
  const direction = axialToPixel(heading, 1, { x: 0, y: 0 });
  const length = Math.hypot(direction.x, direction.y);
  const output = new Vector3(
    point.x + (direction.x / length) * 0.79,
    (instance.groundAt?.(cell.q, cell.r) ?? instance.groundHeight) + 0.4,
    point.y + (direction.y / length) * 0.79,
  );
  const depotPoint = source ? axialToPixel(source, 1, { x: 0, y: 0 }) : point;
  const depot = source
    ? new Vector3(
        depotPoint.x,
        (instance.groundAt?.(source.q, source.r) ?? instance.groundHeight) +
          0.27,
        depotPoint.y,
      )
    : output.clone();
  const returning = cycle >= 0.55;
  const t = returning ? smooth((cycle - 0.55) / 0.45) : smooth(cycle / 0.4);
  const wrist = returning
    ? swing(depot, output, t, instance.x, instance.z)
    : swing(output, depot, t, instance.x, instance.z);
  wrist.y += 0.24 + Math.sin(t * Math.PI) * 0.5;
  const delta = wrist.clone().sub(shoulder);
  const distance = delta.length();
  const reach = Math.max(1.65, distance / 2 + 0.08);
  const flat = Math.hypot(delta.x, delta.z);
  const perpendicular =
    flat > 0.001
      ? new Vector3(
          (-delta.y * delta.x) / flat,
          flat,
          (-delta.y * delta.z) / flat,
        ).normalize()
      : new Vector3(1, 0, 0);
  const elbow = shoulder
    .clone()
    .lerp(wrist, 0.5)
    .addScaledVector(
      perpendicular,
      Math.sqrt(Math.max(0, reach * reach - (distance * distance) / 4)),
    );
  return {
    shoulder,
    elbow,
    wrist,
    source: depot,
    output,
    carrying: !!source && cycle >= 0.5 && cycle < 0.995,
    itemId: source?.item_id,
  };
}

export function elementaryPartMatrix(
  instance: MachinePartInstance,
  now: number,
  reducedMotion: boolean,
  target: Matrix4,
): Matrix4 {
  const model = instance.part.model!;
  const [horizontal, vertical] = elementaryScale(instance.footprintScale);
  const angle = directionAngle(instance.building.orientation);
  const cycle = elementaryCycle(instance, now, reducedMotion);
  position
    .fromArray(model.at)
    .multiply(new Vector3(horizontal, vertical, horizontal));
  size
    .fromArray(model.size)
    .multiply(new Vector3(horizontal, vertical, horizontal));
  rotation.setFromEuler(euler.set(...(model.turn ?? [0, 0, 0])));
  const working = machineWorking(instance.building);
  switch (model.motion) {
    case "wheel":
      if (working && !reducedMotion)
        rotation.multiply(yaw.setFromAxisAngle(UP, (now / 950) * Math.PI * 2));
      break;
    case "heat":
      if (!working) size.setScalar(0);
      break;
    case "tool-left":
    case "tool-right": {
      const squeeze = Math.sin(cycle * Math.PI) * 0.2 * horizontal;
      position.z += model.motion === "tool-left" ? squeeze : -squeeze;
      break;
    }
    case "stock": {
      const stock = instance.building.inventory.reduce(
        (sum, item) => sum + item.quantity,
        0,
      );
      const amount = clamp(stock / Math.max(1, instance.capacity ?? 1));
      size.y *= amount;
      position.y += size.y / 2;
      if (!stock) size.setScalar(0);
      break;
    }
    case "upper-arm":
    case "forearm":
    case "claw":
    case "load":
    case "outlet":
    case "stored-output": {
      const pose = extractorPose(instance, cycle);
      rotation.identity();
      if (model.motion === "upper-arm" || model.motion === "forearm") {
        const start = model.motion === "upper-arm" ? pose.shoulder : pose.elbow;
        const end = model.motion === "upper-arm" ? pose.elbow : pose.wrist;
        position.copy(start).lerp(end, 0.5);
        axis.copy(end).sub(start);
        size.y = axis.length();
        rotation.setFromUnitVectors(UP, axis.normalize());
      } else if (
        model.motion === "outlet" ||
        model.motion === "stored-output"
      ) {
        position.copy(pose.output);
        position.y += model.motion === "outlet" ? -0.16 : 0;
        if (
          model.motion === "stored-output" &&
          !instance.building.output_inventory?.some((item) => item.quantity > 0)
        )
          size.setScalar(0);
        rotation.setFromAxisAngle(UP, angle);
      } else {
        position.copy(pose.wrist);
        // Face out along the arm so the hand turns with the swing.
        rotation.setFromAxisAngle(
          UP,
          Math.atan2(pose.wrist.x - instance.x, pose.wrist.z - instance.z),
        );
        if (model.motion === "claw") size.x *= pose.carrying ? 0.72 : 1;
        else {
          position.y -= 0.18;
          if (!pose.carrying || reducedMotion) size.setScalar(0);
        }
      }
      return target.compose(position, rotation, size);
    }
  }
  yaw.setFromAxisAngle(UP, angle);
  position
    .applyQuaternion(yaw)
    .add(new Vector3(instance.x, instance.groundHeight + 0.18, instance.z));
  rotation.premultiply(yaw);
  return target.compose(position, rotation, size);
}
