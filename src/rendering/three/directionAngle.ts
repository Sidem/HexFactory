import { axialToPixel } from "@hexlife/embed/hex";

import { TRANSPORT_DIRECTIONS } from "../../core/directions";

/** Three.js Y rotation that points geometry authored along local +X at a native heading. */
export function directionAngle(orientation: number): number {
  const direction =
    TRANSPORT_DIRECTIONS[orientation] ?? TRANSPORT_DIRECTIONS[0]!;
  const point = axialToPixel(direction, 1, { x: 0, y: 0 });
  return Math.atan2(-point.y, point.x);
}
