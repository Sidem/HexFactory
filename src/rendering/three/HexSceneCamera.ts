import {
  OrthographicCamera,
  Plane,
  Raycaster,
  Vector2,
  Vector3,
  type Ray,
} from "three";
import { pixelToAxial, type AxialCoordinate } from "@hexlife/embed/hex";

import type { WorldPoint } from "../../core/types";
import { BASE_HEX_SIZE } from "../FactoryRenderer";
import { WORLD_SCALE } from "../landmarks";
import { RELIEF_SPAN } from "../sceneScale";

const MIN_ZOOM = 0.55;
/**
 * The close end of the range. Raised past the old 2.2 so one machine and the hexes it stands on
 * can be looked at properly. Nothing baked is scaled up by it: this route draws meshes, and the
 * flat renderer's sprite atlas keeps its own 2.2 clamp in `CanvasFactoryRenderer`.
 */
const MAX_ZOOM = 4;
const CAMERA_DISTANCE = 38;
const CAMERA_HEIGHT = 31;
/**
 * Orbit zero looks from the south-east toward north-west.
 *
 * It has to be a multiple of 30° or no stop ever squares up with the grid. The camera's right
 * vector across the ground is the heading turned a quarter turn, and the pointy-top hexes run their
 * edges along 30°, 90° and 150°; the old 45° start therefore left every one of the twelve stops
 * with the tiles askew. At a multiple of 60° an edge lies along the screen's horizontal, and the
 * half-steps between put one along the vertical, so every stop reads square.
 */
const BASE_ANGLE = Math.PI / 3;
/** Twelve stops: the six hex headings and the six half-steps between them. */
const ORBIT_STEPS = 12;
const ORBIT_STEP = (Math.PI * 2) / ORBIT_STEPS;
/**
 * One 30° sweep, eased. Half the duration the 60° step used, so the view turns at exactly the rate
 * it always did and a held key still crosses the circle in the same time.
 */
const ORBIT_STEP_MS = 230;
/** A sweep already carrying queued steps still lands inside a second. */
const ORBIT_MAX_MS = 1000;

/** The reach the flat world always had, and still the whole depth budget where relief is small. */
const NEAR_PLANE = 0.1;
const FAR_PLANE = 180;
/**
 * How much view depth one scene unit of height is worth to this camera.
 *
 * The rig looks down a fixed slope, so ground standing above the point being looked at sits nearer
 * along the view direction and ground below it sits further. Only the vertical component of that
 * slope matters, and it is the camera's height over its own distance from the target.
 */
const TARGET_DEPTH = Math.hypot(CAMERA_DISTANCE, CAMERA_HEIGHT);
const DEPTH_PER_HEIGHT = CAMERA_HEIGHT / TARGET_DEPTH;
/**
 * Where the distance haze begins and ends, in screenfuls of view depth beyond the target.
 *
 * The near end is just past the top edge of the view, so ground the player is working on is never
 * hazed however far up the screen it sits — a factory at the top of the frame is beside them, not
 * away from them. What the haze is for is the thing that is genuinely distant and still on screen,
 * which under this projection only relief can be: a peak stands high enough to be drawn while
 * being screenfuls further along the view than anything at the player's feet, and it is the fade
 * that says so. The far end is loose so the survey frontier dissolves rather than ending at a rim.
 */
const HAZE_START = 0.6;
const HAZE_END = 2;
/**
 * How much depth the world's full relief can move a point either side of the flat-world budget.
 *
 * The camera is looking at one height and the ground it is drawing may be anywhere in the world's
 * range, above as well as below, so both ends open by the same amount. Orthographic depth is
 * linear, so widening the range costs precision in proportion and nothing else — and a negative
 * near plane is ordinary here: it is what lets a summit standing over the camera still be drawn.
 */
const RELIEF_DEPTH = RELIEF_SPAN * DEPTH_PER_HEIGHT;
/**
 * How far back along its own direction a pick ray starts.
 *
 * An orthographic raycast begins at the camera's own position plane, so ground standing above the
 * camera would start behind the ray's origin and never be met — the summit the player is looking
 * straight at would be the one thing they could not click. Backing the origin off past the tallest
 * relief in the world fixes that and moves nothing else: sliding an origin along its own direction
 * does not change where the ray crosses a plane or a triangle.
 */
const PICK_BACKOFF = RELIEF_DEPTH + 1;

/** Fixed-tilt, twelve-orbit camera over the landform native published. */
export class HexSceneCamera {
  readonly camera = new OrthographicCamera(
    -1,
    1,
    1,
    -1,
    NEAR_PLANE - RELIEF_DEPTH,
    FAR_PLANE + RELIEF_DEPTH,
  );
  private readonly raycaster = new Raycaster();
  private readonly ground = new Plane(new Vector3(0, 1, 0), 0);
  private readonly target = new Vector3();
  private readonly projected = new Vector3();
  private width = 1;
  private height = 1;
  private zoom = 1;
  private orbit = 0;
  /** The heading actually drawn this frame; it chases `orbitTarget` through a sweep. */
  private orbitAngle = BASE_ANGLE;
  private orbitFrom = BASE_ANGLE;
  private orbitTarget = BASE_ANGLE;
  private orbitStarted = 0;
  /** Zero whenever the camera is settled, so it doubles as the "is a sweep running" flag. */
  private orbitDuration = 0;
  private following = true;

  constructor() {
    this.updateProjection(1, 1);
  }

  get orbitIndex(): number {
    return this.orbit;
  }

  get isOrbiting(): boolean {
    return this.orbitDuration > 0;
  }

  get zoomLevel(): number {
    return this.zoom;
  }

  get isFollowing(): boolean {
    return this.following;
  }

  /**
   * The view depths where distance haze should start and finish at the current zoom.
   *
   * Distance under an orthographic camera is a screenful, not a metre count: the far edge of the
   * view is a fixed fraction of a screen beyond the point being looked at, whatever the zoom. A
   * haze given constant depths would swallow the world zoomed out and do nothing zoomed in, so
   * both ends are measured from the target and scaled by what is actually on screen.
   */
  get hazeRange(): { near: number; far: number } {
    const span = this.camera.top - this.camera.bottom;
    return {
      near: TARGET_DEPTH + HAZE_START * span,
      far: TARGET_DEPTH + HAZE_END * span,
    };
  }

  resize(width: number, height: number): void {
    this.width = Math.max(1, width);
    this.height = Math.max(1, height);
    this.updateProjection(this.width, this.height);
  }

  /**
   * Look at a world point standing at `height`.
   *
   * The height is the finished ground under the player, resolved through the terrain's one height
   * route. The whole rig moves with it rather than the target alone: a player who walks up a
   * hillside stays in the middle of the view instead of climbing towards the top of it, and the
   * plane that panning and zooming are anchored to rises with them.
   */
  follow(point: WorldPoint, height = 0): void {
    if (!this.following) return;
    this.setTarget(point, height);
  }

  recenter(point: WorldPoint, height = 0): void {
    this.following = true;
    this.setTarget(point, height);
  }

  /**
   * Turn one twelfth of a circle. The twelve-step index moves at once — it is what the rest of the
   * game reads — while the drawn heading eases across to it over the next few frames.
   */
  orbitBy(step: -1 | 1, animate = true): void {
    this.orbit = (this.orbit + step + ORBIT_STEPS) % ORBIT_STEPS;
    const target = this.orbitTarget + step * ORBIT_STEP;
    if (!animate) {
      this.settleOrbit(target);
      return;
    }
    // A step pressed mid-sweep extends the one already running instead of restarting it, so a held
    // key spins at a steady rate rather than stalling at each hand-off.
    this.orbitFrom = this.orbitAngle;
    this.orbitTarget = target;
    this.orbitStarted = performance.now();
    this.orbitDuration = Math.min(
      ORBIT_MAX_MS,
      (ORBIT_STEP_MS * Math.abs(target - this.orbitFrom)) / ORBIT_STEP,
    );
  }

  /**
   * Advance a running sweep to `now`, reporting whether the camera moved. The renderer drives this
   * once per frame: a dirty flag raised at key-down would only ever buy the first frame.
   */
  advanceOrbit(now: number): boolean {
    if (this.orbitDuration === 0) return false;
    const progress = (now - this.orbitStarted) / this.orbitDuration;
    if (progress >= 1) {
      this.settleOrbit(this.orbitTarget);
      return true;
    }
    const eased = 0.5 - Math.cos(Math.PI * Math.max(0, progress)) / 2;
    this.orbitAngle =
      this.orbitFrom + (this.orbitTarget - this.orbitFrom) * eased;
    this.updatePose();
    return true;
  }

  panBy(screenX: number, screenY: number): void {
    const center = { x: this.width / 2, y: this.height / 2 };
    const before = this.groundAt(center.x, center.y);
    const after = this.groundAt(center.x - screenX, center.y - screenY);
    this.target.add(after.sub(before));
    this.following = false;
    this.updatePose();
  }

  zoomAt(screenX: number, screenY: number, factor: number): void {
    if (this.following) {
      this.zoom = clamp(this.zoom * factor, MIN_ZOOM, MAX_ZOOM);
      this.updateProjection(this.width, this.height);
      return;
    }
    const before = this.groundAt(screenX, screenY);
    this.zoom = clamp(this.zoom * factor, MIN_ZOOM, MAX_ZOOM);
    this.updateProjection(this.width, this.height);
    const after = this.groundAt(screenX, screenY);
    this.target.add(before.sub(after));
    this.updatePose();
  }

  worldAt(screenX: number, screenY: number): WorldPoint {
    const point = this.groundAt(screenX, screenY);
    return {
      x: Math.round(point.x * WORLD_SCALE),
      y: Math.round(point.z * WORLD_SCALE),
    };
  }

  axialAt(screenX: number, screenY: number): AxialCoordinate {
    const point = this.groundAt(screenX, screenY);
    return pixelToAxial({ x: point.x, y: point.z }, 1, { x: 0, y: 0 });
  }

  /** Inverse-project a screen direction so WASD remains up/left/down/right after every orbit. */
  screenMovement(screenX: number, screenY: number): WorldPoint {
    if (screenX === 0 && screenY === 0) return { x: 0, y: 0 };
    // Movement answers to where the sweep lands, not to the frame it happens to be passing through.
    // A held direction is re-read once per turn, so sampling mid-sweep would leave the player
    // walking the old heading until they let go of the key.
    const drawn = this.orbitAngle;
    if (this.orbitDuration > 0) this.poseAt(this.orbitTarget);
    const center = this.groundAt(this.width / 2, this.height / 2);
    const moved = this.groundAt(
      this.width / 2 + screenX * 100,
      this.height / 2 + screenY * 100,
    );
    if (this.orbitDuration > 0) this.poseAt(drawn);
    const x = moved.x - center.x;
    const y = moved.z - center.z;
    const length = Math.hypot(x, y);
    return length === 0 ? { x: 0, y: 0 } : { x: x / length, y: y / length };
  }

  /**
   * Where a world point lands on the canvas. `height` is the drawn ground under it — a marker over
   * a summit is not on the screen pixel the logical plane beneath it projects to.
   */
  projectWorld(point: WorldPoint, height = 0): { x: number; y: number } {
    return this.projectScene(
      point.x / WORLD_SCALE,
      height,
      point.y / WORLD_SCALE,
    );
  }

  worldOnScreen(point: WorldPoint, margin = 44, height = 0): boolean {
    this.projected
      .set(point.x / WORLD_SCALE, height, point.y / WORLD_SCALE)
      .project(this.camera);
    const x = (this.projected.x * 0.5 + 0.5) * this.width;
    const y = (-this.projected.y * 0.5 + 0.5) * this.height;
    return (
      x >= margin &&
      y >= margin &&
      x <= this.width - margin &&
      y <= this.height - margin
    );
  }

  projectScene(x: number, y: number, z: number): { x: number; y: number } {
    const projected = new Vector3(x, y, z).project(this.camera);
    return {
      x: (projected.x * 0.5 + 0.5) * this.width,
      y: (-projected.y * 0.5 + 0.5) * this.height,
    };
  }

  /**
   * The picking ray for a canvas point, in scene space. The returned ray belongs to the camera's
   * own raycaster, so a caller reads it before asking for the next one.
   */
  rayAt(screenX: number, screenY: number): Ray {
    const ndc = new Vector2(
      (screenX / this.width) * 2 - 1,
      1 - (screenY / this.height) * 2,
    );
    this.raycaster.setFromCamera(ndc, this.camera);
    this.raycaster.ray.origin.addScaledVector(
      this.raycaster.ray.direction,
      -PICK_BACKOFF,
    );
    return this.raycaster.ray;
  }

  /** Ground intersection, deliberately ignoring terrain and machine meshes. */
  groundAt(screenX: number, screenY: number): Vector3 {
    return (
      this.rayAt(screenX, screenY).intersectPlane(this.ground, new Vector3()) ??
      new Vector3()
    );
  }

  private setTarget(point: WorldPoint, height: number): void {
    this.target.set(point.x / WORLD_SCALE, height, point.y / WORLD_SCALE);
    // A plane with an upward normal passes through `y = -constant`. Panning and zooming stay
    // anchored to the height being looked at, so dragging across a hillside does not slide the
    // world out from under the pointer the way a fixed sea-level plane would.
    this.ground.constant = -height;
    this.updatePose();
  }

  private updateProjection(width: number, height: number): void {
    const viewHeight = height / (BASE_HEX_SIZE * this.zoom);
    const viewWidth = viewHeight * (width / height);
    this.camera.left = -viewWidth / 2;
    this.camera.right = viewWidth / 2;
    this.camera.top = viewHeight / 2;
    this.camera.bottom = -viewHeight / 2;
    this.camera.updateProjectionMatrix();
    this.updatePose();
  }

  /** End the sweep on `target`, wrapped so a long session cannot drift the angle out of precision. */
  private settleOrbit(target: number): void {
    const wrapped = target - Math.floor(target / (Math.PI * 2)) * Math.PI * 2;
    this.orbitDuration = 0;
    this.orbitFrom = wrapped;
    this.orbitTarget = wrapped;
    this.poseAt(wrapped);
  }

  private poseAt(angle: number): void {
    this.orbitAngle = angle;
    this.updatePose();
  }

  private updatePose(): void {
    // The orbit is presentation only: the native six/twelve heading indices are unchanged by it,
    // and a half-step between two hex headings moves nothing but where the scene is looked at from.
    const angle = this.orbitAngle;
    this.camera.position.set(
      this.target.x + Math.cos(angle) * CAMERA_DISTANCE,
      // Over the target rather than over sea level, so the tilt the whole scene is composed at is
      // the same on a summit as on a valley floor.
      this.target.y + CAMERA_HEIGHT,
      this.target.z + Math.sin(angle) * CAMERA_DISTANCE,
    );
    this.camera.up.set(0, 1, 0);
    this.camera.lookAt(this.target);
    this.camera.updateMatrixWorld(true);
  }
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}
