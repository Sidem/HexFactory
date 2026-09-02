import type { Mesh } from "three";
import { describe, expect, it } from "vitest";

import type { FactorySnapshot } from "../src/core/types";
import { WORLD_SCALE } from "../src/rendering/landmarks";
import { createWorldMaterials } from "../src/rendering/three/materials";
import {
  PlayerRig,
  WAYFINDER_VISUAL_SCALE,
} from "../src/rendering/three/playerRig";

/** Only the player is read, so the rest of a snapshot is not this rig's business to invent. */
const at = (x: number, y: number): FactorySnapshot =>
  ({
    player: {
      x,
      y,
      facing_x: 0,
      facing_y: 1,
      action_cooldown: 0,
      action_cooldown_total: 15,
    },
  }) as unknown as FactorySnapshot;

const flat = (): number => 0;

/** The legs, by the order the rig adds them: six body parts, then left and right. */
function legs(rig: PlayerRig): { left: Mesh; right: Mesh } {
  const children = rig.group.children as Mesh[];
  return { left: children[6]!, right: children[7]! };
}

describe("the Wayfinder rig", () => {
  it("stands at human scale rather than at one hex", () => {
    const materials = createWorldMaterials();
    const rig = new PlayerRig(materials);

    expect(WAYFINDER_VISUAL_SCALE).toBeGreaterThanOrEqual(3);
    expect(rig.group.name).toBe("player");
    expect(rig.group.scale.x).toBe(WAYFINDER_VISUAL_SCALE);

    rig.dispose();
    for (const material of materials.materials) material.dispose();
  });

  it("winds the stride by distance walked, whatever the heading", () => {
    const materials = createWorldMaterials();
    const rig = new PlayerRig(materials);
    const { left, right } = legs(rig);

    // Standing still is a pose, not a paused animation: no snapshot moves, so nothing swings.
    for (let frame = 0; frame < 12; frame += 1)
      rig.update(at(0, 0), frame * 16, frame % 2 === 0, 200, flat);
    expect(left.rotation.x).toBe(0);
    expect(Math.abs(right.rotation.x)).toBe(0);

    // Walk due east, along which the old position-derived phase `sin((x + y) * 8)` still advanced,
    // and then due north-west, along which `x + y` barely changes and it very nearly froze. Both
    // cover the same ground, so both must wind the phase by the same amount.
    const walk = (dx: number, dy: number): number[] => {
      const angles: number[] = [];
      let x = 0;
      let y = 0;
      for (let frame = 0; frame < 60; frame += 1) {
        x += dx * WORLD_SCALE * 0.05;
        y += dy * WORLD_SCALE * 0.05;
        rig.update(at(x, y), 10_000 + frame * 16, true, 200, flat);
        angles.push(left.rotation.x);
      }
      return angles;
    };
    const east = walk(1, 0);
    const acrossTheOldAxis = walk(-Math.SQRT1_2, Math.SQRT1_2);
    const swing = (angles: number[]): number =>
      Math.max(...angles) - Math.min(...angles);
    // Very nearly the full 0.96 the amplitude allows; the shortfall is the gait easing in.
    expect(swing(east)).toBeGreaterThan(0.85);
    expect(swing(acrossTheOldAxis)).toBeCloseTo(swing(east), 1);

    // A swing, not a strobe: the pose steps far enough to read as motion and never so far that
    // consecutive frames land on unrelated angles. The legs stay exactly opposed throughout.
    const steps = east
      .slice(1)
      .map((angle, index) => Math.abs(angle - east[index]!));
    expect(Math.max(...steps)).toBeLessThan(0.2);
    expect(left.rotation.x).toBeCloseTo(-right.rotation.x, 10);

    // A load or a respawn is a jump rather than a stride, so it restarts the phase instead of
    // winding a hundred hexes of it forward.
    rig.update(at(400 * WORLD_SCALE, 0), 20_000, true, 200, flat);
    rig.update(at(400 * WORLD_SCALE, 0), 20_016, true, 200, flat);
    expect(left.rotation.x).toBeCloseTo(0, 10);

    // And the last step eases out rather than snapping: still walking one frame, settled the next.
    const walking = Math.abs(east.at(-1)!);
    expect(walking).toBeGreaterThan(0);
    for (let frame = 0; frame < 40; frame += 1)
      rig.update(
        at(400 * WORLD_SCALE, 0),
        30_000 + frame * 16,
        false,
        200,
        flat,
      );
    expect(left.rotation.x).toBe(0);

    rig.dispose();
    for (const material of materials.materials) material.dispose();
  });
});
