import {
  BoxGeometry,
  CylinderGeometry,
  Group,
  IcosahedronGeometry,
  Mesh,
  OctahedronGeometry,
  RingGeometry,
} from "three";
import { pixelToAxial } from "@hexlife/embed/hex";

import type { FactorySnapshot } from "../../core/types";
import { WORLD_SCALE } from "../landmarks";
import type { WorldMaterials } from "./materials";

/** A person must read at world scale, not as another inventory token dropped on one hex. */
export const WAYFINDER_VISUAL_SCALE = 3.2;

/**
 * The Wayfinder: the figure standing in the world, and the pose it holds.
 *
 * Its own module because it is the one thing in the diorama with a body — everything else is an
 * instanced mesh placed from a snapshot field, while this has limbs that have to agree with each
 * other frame by frame.
 */
export class PlayerRig {
  readonly group = new Group();
  private readonly body: Mesh;
  private readonly shell: Mesh;
  private readonly leftLeg: Mesh;
  private readonly rightLeg: Mesh;
  private readonly leftArm: Mesh;
  private readonly rightArm: Mesh;
  private readonly tool: Mesh;
  private readonly work: Mesh;
  private readonly meshes: readonly Mesh[];
  /** Where the player stood at the last snapshot, in hex units, or undefined before the first. */
  private lastX: number | undefined;
  private lastZ: number | undefined;
  /** Radians of stride wound forward by distance travelled, never by position or by clock. */
  private stridePhase = 0;
  /** How much of the walking pose is showing, 0 to 1, so the last step eases into idle. */
  private gait = 0;
  private movedAt = -Infinity;
  private poseAt = 0;

  constructor(materials: WorldMaterials) {
    this.group.name = "player";
    this.group.scale.setScalar(WAYFINDER_VISUAL_SCALE);
    // The wayfinder casts no shadow. The shadow map is baked on demand rather than every
    // frame, so the one caster that moves continuously would leave its shadow standing a
    // stride behind it until the next bake caught up.
    this.body = new Mesh(
      new CylinderGeometry(0.13, 0.16, 0.25, 7),
      materials.wayfinderHull,
    );
    this.body.position.y = 0.31;
    this.shell = new Mesh(
      new CylinderGeometry(0.17, 0.14, 0.13, 7),
      materials.wayfinderShell,
    );
    this.shell.position.y = 0.43;
    const head = new Mesh(
      new IcosahedronGeometry(0.095, 1),
      materials.wayfinderShell,
    );
    head.position.y = 0.535;
    const facing = new Mesh(
      new BoxGeometry(0.115, 0.045, 0.035),
      materials.wayfinderSignal,
    );
    facing.position.set(0, 0.545, 0.083);
    const pack = new Mesh(
      new BoxGeometry(0.17, 0.22, 0.1),
      materials.wayfinderBrass,
    );
    pack.position.set(0, 0.34, -0.13);
    const beacon = new Mesh(
      new OctahedronGeometry(0.035, 0),
      materials.wayfinderBrass,
    );
    beacon.position.set(0, 0.65, -0.025);
    this.leftLeg = new Mesh(
      new CylinderGeometry(0.04, 0.05, 0.2, 6),
      materials.wayfinderHull,
    );
    this.leftLeg.position.set(-0.07, 0.105, 0);
    this.rightLeg = new Mesh(
      new CylinderGeometry(0.04, 0.05, 0.2, 6),
      materials.wayfinderHull,
    );
    this.rightLeg.position.set(0.07, 0.105, 0);
    this.leftArm = new Mesh(
      new CylinderGeometry(0.03, 0.04, 0.21, 6),
      materials.wayfinderShell,
    );
    this.leftArm.position.set(-0.17, 0.34, 0.015);
    this.leftArm.rotation.z = -0.18;
    this.rightArm = new Mesh(
      new CylinderGeometry(0.03, 0.04, 0.21, 6),
      materials.wayfinderShell,
    );
    this.rightArm.position.set(0.17, 0.34, 0.015);
    this.rightArm.rotation.z = 0.18;
    this.tool = new Mesh(
      new CylinderGeometry(0.025, 0.04, 0.24, 6),
      materials.wayfinderBrass,
    );
    this.tool.position.set(0.22, 0.25, 0.075);
    this.tool.rotation.z = -0.5;
    this.work = new Mesh(
      new RingGeometry(0.31, 0.36, 32),
      materials.overlaySelection,
    );
    this.work.rotateX(-Math.PI / 2);
    this.work.position.y = 0.025;
    this.meshes = [
      this.body,
      this.shell,
      head,
      facing,
      pack,
      beacon,
      this.leftLeg,
      this.rightLeg,
      this.leftArm,
      this.rightArm,
      this.tool,
    ];
    this.group.add(...this.meshes, this.work);
  }

  /**
   * The player's pose, driven by how far they have actually walked.
   *
   * Both halves used to ask the wrong question. The gait was gated on `walk_path` — the pathfinding
   * route, not motion — so a player on the keys was not "walking" by that test and both legs held
   * at zero. And the stride phase was a function of *position*, `sin((x + y) * 8)`, so a heading
   * along which `x + y` barely changes nearly froze the cycle, every waypoint turn moved the phase
   * discontinuously, and arrival snapped it to zero. Displacement answers both: distance travelled
   * advances the phase monotonically whatever the heading, and having moved recently is what
   * "walking" means, whether the step came from a key or from a route.
   *
   * Measured on a click-to-move route on 2026-09-02: fourteen waypoint turns, and the leg angle
   * sweeps through every one of them without a break, at most 0.179 radians between poses.
   *
   * `measured` is whether this frame carries a new snapshot. Displacement is read only on those;
   * the blend runs every frame so the last step eases into idle at the display's cadence rather
   * than the simulation's. `idleAfter` is how long a stillness has to last to count as a stop — a
   * snapshot reporting no displacement is the ordinary case mid-step, not evidence of one.
   */
  update(
    snapshot: FactorySnapshot,
    now: number,
    measured: boolean,
    idleAfter: number,
    groundHeight: (q: number, r: number) => number,
  ): void {
    const player = snapshot.player;
    const axial = pixelToAxial(player, WORLD_SCALE);
    const x = player.x / WORLD_SCALE;
    const z = player.y / WORLD_SCALE;
    this.group.position.set(x, groundHeight(axial.q, axial.r) + 0.02, z);
    this.group.rotation.y = Math.atan2(player.facing_x, player.facing_y);
    if (measured) {
      const stepped = Math.hypot(x - (this.lastX ?? x), z - (this.lastZ ?? z));
      this.lastX = x;
      this.lastZ = z;
      // A load, a scenario restart or a respawn moves the player further in one snapshot than any
      // walk can, and that is a jump rather than a stride to wind the phase forward by.
      if (stepped > STRIDE_TELEPORT_HEXES) {
        this.stridePhase = 0;
      } else if (stepped > STRIDE_STILL_HEXES) {
        this.stridePhase =
          (this.stridePhase + stepped * STRIDE_RADIANS_PER_HEX) % (Math.PI * 2);
        this.movedAt = now;
      }
    }
    const target = now - this.movedAt < idleAfter ? 1 : 0;
    const elapsed = this.poseAt > 0 ? now - this.poseAt : GAIT_EASE_MS;
    this.poseAt = now;
    this.gait +=
      (target - this.gait) * Math.min(1, Math.max(0, elapsed) / GAIT_EASE_MS);
    if (this.gait < 0.002) {
      this.gait = 0;
      this.stridePhase = 0;
    }
    const gait = this.gait;
    const stride = Math.sin(this.stridePhase) * 0.48 * gait;
    this.body.rotation.x = -0.06 * gait;
    this.shell.rotation.x = -0.04 * gait;
    this.leftLeg.rotation.x = stride;
    this.rightLeg.rotation.x = -stride;
    this.leftArm.rotation.x = -stride * 0.72;
    this.rightArm.rotation.x = stride * 0.72;
    const total = player.action_cooldown_total;
    const done = total > 0 ? 1 - player.action_cooldown / total : 0;
    this.tool.visible = player.action_cooldown > 0;
    this.tool.rotation.z = -0.5 - done * 0.75;
    this.work.visible = player.action_cooldown > 0;
    this.work.scale.setScalar(Math.max(0.05, done));
  }

  dispose(): void {
    for (const mesh of this.meshes) mesh.geometry.dispose();
    this.work.geometry.dispose();
  }
}

/**
 * Stride cadence, in radians of leg swing per hex crossed.
 *
 * The pace the old position-derived phase ran at is not a pace to reproduce. Measured on the walk
 * on 2026-09-02: a snapshot moves the player 0.161 of a hex and about thirty of them land a second,
 * so at 8 radians a hex the legs ran a little over six full cycles a second with 1.29 radians of
 * phase between consecutive poses — under five poses a cycle. That is past the rate at which a
 * swing reads as a swing at all; the legs strobed between unrelated angles, which is a fair part of
 * why walking looked like it had no gait even on the headings where the old phase did advance.
 *
 * 1.4 puts 0.225 radians between poses — some twenty-eight a cycle — and the walk at 0.92 to 1.07
 * cycles a second, which is about a person's walking cadence. The run (25 m/s against the walk's
 * 15) comes out 1.67 times that, because it is distance that winds the phase and the cadence
 * therefore still rises with speed.
 */
const STRIDE_RADIANS_PER_HEX = 1.4;
/** Below this much movement in one snapshot the player is standing, not shuffling. */
const STRIDE_STILL_HEXES = 1e-4;
/** Above this the player was moved rather than walked, so the phase restarts instead of winding. */
const STRIDE_TELEPORT_HEXES = 1.5;
/** How long the walking pose takes to blend fully in or out. */
const GAIT_EASE_MS = 130;
