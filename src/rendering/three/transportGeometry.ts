import { BoxGeometry, type BufferGeometry } from "three";

import type { BuildingDefinition, BuildingKind } from "../../core/types";

export interface TransportGeometrySet {
  readonly belt: BufferGeometry;
  readonly bridge: BufferGeometry;
}

/** Shared deck geometry for the game scene, contact scene, and every definition using the kind. */
export function createTransportGeometry(): TransportGeometrySet {
  return {
    belt: new BoxGeometry(0.82, 0.1, 0.3),
    bridge: new BoxGeometry(0.92, 0.13, 1.25),
  };
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
