import { BoxGeometry, type BufferGeometry } from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";

import { CORNER_START } from "../../core/directions";
import type { BuildingKind } from "../../core/types";

export interface TransportGeometrySet {
  readonly belt: BufferGeometry;
  readonly beltDetail: BufferGeometry;
  readonly bridge: BufferGeometry;
}

export interface CurvedTransportGeometry {
  readonly frame: BufferGeometry;
  readonly detail: BufferGeometry;
}

/** Shared deck geometry for the game scene, contact scene, and every definition using the kind. */
export function createTransportGeometry(): TransportGeometrySet {
  return {
    belt: beltFrameGeometry(),
    beltDetail: beltTreadGeometry(),
    bridge: new BoxGeometry(0.92, 0.13, 1.25),
  };
}

/** A conveyor reads as two raised rails around a recessed moving bed, even when it carries nothing. */
function beltFrameGeometry(): BufferGeometry {
  const deck = new BoxGeometry(0.92, 0.07, 0.4);
  const leftRail = new BoxGeometry(0.92, 0.13, 0.055);
  const rightRail = new BoxGeometry(0.92, 0.13, 0.055);
  leftRail.translate(0, 0.07, -0.22);
  rightRail.translate(0, 0.07, 0.22);
  return mergeAndDispose([deck, leftRail, rightRail]);
}

/** Raised transverse slats make the shared transport geometry read as a belt rather than a beam. */
function beltTreadGeometry(): BufferGeometry {
  const treads = [-0.35, -0.175, 0, 0.175, 0.35].map((x) => {
    const tread = new BoxGeometry(0.1, 0.055, 0.34);
    tread.translate(x, 0.07, 0);
    return tread;
  });
  return mergeAndDispose(treads);
}

/** A belt turn is the same rail-and-tread vocabulary sampled along one quadratic path. The local
 * path arrives from `turnAngle`, bends around the tile centre, and leaves along +X, allowing one
 * cached geometry per relative heading to be instanced at every matching junction. */
export function createCurvedTransportGeometry(
  turnAngle: number,
): CurvedTransportGeometry {
  const half = 0.46;
  const start = {
    x: -Math.cos(turnAngle) * half,
    z: Math.sin(turnAngle) * half,
  };
  const end = { x: half, z: 0 };
  const frameParts: BufferGeometry[] = [];
  const segments = 10;
  for (let segment = 0; segment < segments; segment += 1) {
    const from = quadraticPoint(start, end, segment / segments);
    const to = quadraticPoint(start, end, (segment + 1) / segments);
    const dx = to.x - from.x;
    const dz = to.z - from.z;
    const length = Math.hypot(dx, dz);
    const part = beltFrameGeometry();
    part.scale((length + 0.025) / 0.92, 1, 1);
    part.rotateY(Math.atan2(-dz, dx));
    part.translate((from.x + to.x) / 2, 0, (from.z + to.z) / 2);
    frameParts.push(part);
  }

  const detailParts = Array.from({ length: 7 }, (_, index) => {
    const t = (index + 0.5) / 7;
    const point = quadraticPoint(start, end, t);
    const tangent = quadraticTangent(start, end, t);
    const tread = new BoxGeometry(0.075, 0.055, 0.34);
    tread.rotateY(Math.atan2(-tangent.z, tangent.x));
    tread.translate(point.x, 0.07, point.z);
    return tread;
  });
  return {
    frame: mergeAndDispose(frameParts),
    detail: mergeAndDispose(detailParts),
  };
}

function quadraticPoint(
  start: { readonly x: number; readonly z: number },
  end: { readonly x: number; readonly z: number },
  t: number,
): { x: number; z: number } {
  const inverse = 1 - t;
  return {
    x: inverse * inverse * start.x + t * t * end.x,
    z: inverse * inverse * start.z + t * t * end.z,
  };
}

function quadraticTangent(
  start: { readonly x: number; readonly z: number },
  end: { readonly x: number; readonly z: number },
  t: number,
): { x: number; z: number } {
  return {
    x: -2 * (1 - t) * start.x + 2 * t * end.x,
    z: -2 * (1 - t) * start.z + 2 * t * end.z,
  };
}

function mergeAndDispose(parts: BufferGeometry[]): BufferGeometry {
  const merged = mergeGeometries(parts, false);
  for (const part of parts) part.dispose();
  if (!merged) throw new Error("Could not merge transport geometry");
  merged.computeVertexNormals();
  return merged;
}

export function isTransportKind(kind: BuildingKind): kind is "belt" | "bridge" {
  return kind === "belt" || kind === "bridge";
}

/**
 * How far a deck is stretched along the heading it was built at.
 *
 * A vertex heading covers the two-row period — `3·size` against an edge step's `√3·size` — so a
 * deck that reaches only an edge's length would leave a gap over the seam it exists to bridge.
 * The test is the entity's own heading, not its definition's axis: one belt definition now takes
 * all twelve, so the axis no longer tells you which period any particular belt spans. The 2D
 * renderer makes the same test at `CanvasFactoryRenderer.ts`.
 */
export function transportScale(
  kind: BuildingKind,
  orientation: number,
): readonly [number, number, number] {
  return kind === "belt" && orientation >= CORNER_START
    ? [2.4, 1, 0.72]
    : [1, 1, 1];
}
