import { nearestBoundaryDirection } from "../src/ui/boundaries";
import { axialToPixel } from "@hexlife/embed/hex";
import { WORLD_SCALE } from "../src/rendering/landmarks";
import { describe, expect, it } from "vitest";

import {
  CORNER_START,
  TRANSPORT_DIRECTIONS,
  rotateAnyOrientation,
} from "../src/core/directions";

/**
 * The host's half of the rotation contract.
 *
 * `rotateAnyOrientation` is a second implementation of `OrientationAxis::next` and `::previous`,
 * because the pending building turns before native has ever heard of it. Two implementations of one
 * rule can only be compared against something neither of them owns, so both are checked against the
 * *world vectors* in the shared direction fixture rather than against each other's arithmetic: one
 * press is 30° clockwise, twelve presses are a circle, and reverse is the inverse press. The core's
 * `rotation_walks_every_heading_once_in_angular_order` makes the same three assertions in Rust.
 */
const TAU = Math.PI * 2;

/** Pointy-top axial at unit size: `x = √3·(q + r/2)`, `y = 1.5·r`, with `y` running south. */
function angle(orientation: number): number {
  const direction = TRANSPORT_DIRECTIONS[orientation]!;
  return Math.atan2(
    1.5 * direction.r,
    Math.sqrt(3) * (direction.q + direction.r / 2),
  );
}

describe("rotationMatchesNativeAngularOrder", () => {
  it("nudges a heading 30° clockwise per press and reaches all twelve", () => {
    const seen = [0];
    let orientation = 0;
    for (let press = 0; press < 11; press += 1) {
      const next = rotateAnyOrientation(orientation, 1);
      const turned = (angle(next) - angle(orientation) + TAU) % TAU;
      expect(turned, `press ${press + 1} turned ${turned} radians`).toBeCloseTo(
        TAU / 12,
        9,
      );
      orientation = next;
      seen.push(next);
    }
    expect([...seen].sort((a, b) => a - b)).toEqual([...Array(12).keys()]);
    expect(rotateAnyOrientation(orientation, 1)).toBe(0);
  });

  it("turns back the way it came", () => {
    // Reverse from due east is 30° short of it, which is a corner heading — the table's second
    // family — so an implementation that stepped indices could not produce it.
    expect(rotateAnyOrientation(0, -1)).toBe(CORNER_START + 1);
    for (let orientation = 0; orientation < 12; orientation += 1)
      expect(
        rotateAnyOrientation(rotateAnyOrientation(orientation, 1), -1),
      ).toBe(orientation);
  });
});

it("picks each shared boundary side on the logical axial plane", () => {
  const cell = { q: -7, r: 4 };
  const center = axialToPixel(cell, WORLD_SCALE);
  for (let direction = 0; direction < 6; direction += 1) {
    const angle = (direction * Math.PI) / 3;
    expect(
      nearestBoundaryDirection(cell, {
        x: center.x + Math.cos(angle) * 100,
        y: center.y + Math.sin(angle) * 100,
      }),
    ).toBe(direction);
  }
});
