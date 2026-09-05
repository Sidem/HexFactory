import { BoxGeometry, CylinderGeometry, type BufferGeometry } from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";

import { CORNER_START } from "../../core/directions";
import type { BuildingKind } from "../../core/types";

export interface TransportGeometrySet {
  readonly belt: BufferGeometry;
  readonly beltDetail: BufferGeometry;
  readonly pipe: BufferGeometry;
  readonly pipeDetail: BufferGeometry;
  readonly portal: BufferGeometry;
  readonly portalDetail: BufferGeometry;
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
    pipe: pipeBodyGeometry(),
    pipeDetail: pipeCouplingGeometry(),
    portal: portalFrameGeometry(),
    portalDetail: portalStripeGeometry(),
    bridge: bridgeAssembly(),
  };
}

/** Deck seams, edge girders and parapets make the crossing legible when empty. */
function bridgeAssembly(): BufferGeometry {
  const pieces: BufferGeometry[] = [new BoxGeometry(0.92, 0.13, 1.25)];
  for (const x of [-0.43, 0.43]) {
    pieces.push(new BoxGeometry(0.08, 0.16, 1.25).translate(x, -0.06, 0));
    pieces.push(new BoxGeometry(0.055, 0.055, 1.25).translate(x, 0.31, 0));
    for (const z of [-0.55, 0, 0.55])
      pieces.push(new BoxGeometry(0.055, 0.3, 0.055).translate(x, 0.17, z));
  }
  for (let i = 0; i < 7; i++)
    pieces.push(
      new BoxGeometry(0.78, 0.025, 0.13).translate(0, 0.075, (i - 3) * 0.17),
    );
  return mergeAndDispose(pieces);
}

/** A closed round conduit, clearly narrower and taller than the open belt deck beside it. */
function pipeBodyGeometry(): BufferGeometry {
  const body = new CylinderGeometry(0.13, 0.13, 0.92, 12);
  body.rotateZ(Math.PI / 2);
  return body;
}

/** Two oversized collars make segment boundaries and flow direction readable at world scale. */
function pipeCouplingGeometry(): BufferGeometry {
  const collars = [-0.34, 0.34].map((x) => {
    const collar = new CylinderGeometry(0.18, 0.18, 0.09, 12);
    collar.rotateZ(Math.PI / 2);
    collar.translate(x, 0, 0);
    return collar;
  });
  return mergeAndDispose(collars);
}

/** Guard walls and a header around an underpass mouth; the lane itself descends between them. */
function portalFrameGeometry(): BufferGeometry {
  const left = new BoxGeometry(0.48, 0.24, 0.08);
  const right = new BoxGeometry(0.48, 0.24, 0.08);
  const header = new BoxGeometry(0.12, 0.12, 0.58);
  left.translate(0.12, 0.12, -0.3);
  right.translate(0.12, 0.12, 0.3);
  header.translate(0.3, 0.28, 0);
  return mergeAndDispose([left, right, header]);
}

/** High-contrast cap on the tunnel mouth, echoing the caution panel in the supplied reference. */
function portalStripeGeometry(): BufferGeometry {
  const stripe = new BoxGeometry(0.135, 0.035, 0.42);
  stripe.translate(0.305, 0.35, 0);
  return stripe;
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
  halfExtent = 0.46,
  medium: "solid" | "fluid" = "solid",
): CurvedTransportGeometry {
  const start = {
    x: -Math.cos(turnAngle) * halfExtent,
    z: Math.sin(turnAngle) * halfExtent,
  };
  const end = { x: halfExtent, z: 0 };
  const frameParts: BufferGeometry[] = [];
  const segments = 10;
  for (let segment = 0; segment < segments; segment += 1) {
    const from = quadraticPoint(start, end, segment / segments);
    const to = quadraticPoint(start, end, (segment + 1) / segments);
    const dx = to.x - from.x;
    const dz = to.z - from.z;
    const length = Math.hypot(dx, dz);
    const part = medium === "fluid" ? pipeBodyGeometry() : beltFrameGeometry();
    part.scale((length + 0.025) / 0.92, 1, 1);
    part.rotateY(Math.atan2(-dz, dx));
    part.translate((from.x + to.x) / 2, 0, (from.z + to.z) / 2);
    frameParts.push(part);
  }

  const detailCount = medium === "fluid" ? 5 : 7;
  const detailParts = Array.from({ length: detailCount }, (_, index) => {
    const t = (index + 0.5) / detailCount;
    const point = quadraticPoint(start, end, t);
    const tangent = quadraticTangent(start, end, t);
    const detail =
      medium === "fluid"
        ? new CylinderGeometry(0.18, 0.18, 0.075, 12)
        : new BoxGeometry(0.075, 0.055, 0.34);
    if (medium === "fluid") detail.rotateZ(Math.PI / 2);
    detail.rotateY(Math.atan2(-tangent.z, tangent.x));
    detail.translate(point.x, medium === "fluid" ? 0 : 0.07, point.z);
    return detail;
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
