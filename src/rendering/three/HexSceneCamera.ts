import { OrthographicCamera, Plane, Raycaster, Vector2, Vector3 } from "three";
import { pixelToAxial, type AxialCoordinate } from "@hexlife/embed/hex";

import type { WorldPoint } from "../../core/types";
import { BASE_HEX_SIZE } from "../FactoryRenderer";
import { WORLD_SCALE } from "../landmarks";

const MIN_ZOOM = 0.55;
/**
 * The close end of the range. Raised past the old 2.2 so one machine and the hexes it stands on
 * can be looked at properly. Nothing baked is scaled up by it: this route draws meshes, and the
 * flat renderer's sprite atlas keeps its own 2.2 clamp in `CanvasFactoryRenderer`.
 */
const MAX_ZOOM = 4;
const CAMERA_DISTANCE = 38;
const CAMERA_HEIGHT = 31;
/** Orbit zero looks from the south-west toward north-east. */
const BASE_ANGLE = Math.PI / 4;
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

/** Fixed-tilt, twelve-orbit camera over the native logical plane. */
export class HexSceneCamera {
  readonly camera = new OrthographicCamera(-1, 1, 1, -1, 0.1, 180);
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

  resize(width: number, height: number): void {
    this.width = Math.max(1, width);
    this.height = Math.max(1, height);
    this.updateProjection(this.width, this.height);
  }

  follow(point: WorldPoint): void {
    if (!this.following) return;
    this.setTarget(point);
  }

  recenter(point: WorldPoint): void {
    this.following = true;
    this.setTarget(point);
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

  projectWorld(point: WorldPoint): { x: number; y: number } {
    return this.projectScene(point.x / WORLD_SCALE, 0, point.y / WORLD_SCALE);
  }

  worldOnScreen(point: WorldPoint, margin = 44): boolean {
    this.projected
      .set(point.x / WORLD_SCALE, 0, point.y / WORLD_SCALE)
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

  /** Ground intersection, deliberately ignoring terrain and machine meshes. */
  groundAt(screenX: number, screenY: number): Vector3 {
    const ndc = new Vector2(
      (screenX / this.width) * 2 - 1,
      1 - (screenY / this.height) * 2,
    );
    this.raycaster.setFromCamera(ndc, this.camera);
    return (
      this.raycaster.ray.intersectPlane(this.ground, new Vector3()) ??
      new Vector3()
    );
  }

  private setTarget(point: WorldPoint): void {
    this.target.set(point.x / WORLD_SCALE, 0, point.y / WORLD_SCALE);
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
      CAMERA_HEIGHT,
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
