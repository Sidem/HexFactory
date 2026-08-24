import { BoxGeometry, type BufferGeometry } from "three";
import { mergeGeometries } from "three/addons/utils/BufferGeometryUtils.js";

import type { BuildingDefinition, BuildingKind } from "../../core/types";

export interface TransportGeometrySet {
  readonly belt: BufferGeometry;
  readonly beltDetail: BufferGeometry;
  readonly bridge: BufferGeometry;
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

export function transportScale(
  definition: BuildingDefinition,
): readonly [number, number, number] {
  return definition.kind === "belt" && definition.orientation_axis === "corner"
    ? [0.72, 1, 2.4]
    : [1, 1, 1];
}
