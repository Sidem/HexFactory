import { OrthographicCamera, Plane, Raycaster, Vector2, Vector3 } from "three";
import { pixelToAxial, type AxialCoordinate } from "@hexlife/embed/hex";

import type { WorldPoint } from "../../core/types";
import { BASE_HEX_SIZE } from "../FactoryRenderer";
import { WORLD_SCALE } from "../landmarks";

const MIN_ZOOM = 0.55;
const MAX_ZOOM = 2.2;
const CAMERA_DISTANCE = 38;
const CAMERA_HEIGHT = 31;

/** Fixed-tilt, six-orbit camera over the native logical plane. */
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
  private following = true;

  constructor() {
    this.updateProjection(1, 1);
  }

  get orbitIndex(): number {
    return this.orbit;
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

  orbitBy(step: -1 | 1): void {
    this.orbit = (this.orbit + step + 6) % 6;
    this.updatePose();
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
    const center = this.groundAt(this.width / 2, this.height / 2);
    const moved = this.groundAt(
      this.width / 2 + screenX * 100,
      this.height / 2 + screenY * 100,
    );
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

  private updatePose(): void {
    // Orbit zero looks from the south-west toward north-east. Sixty-degree steps keep the native
    // six/twelve heading indices intact; only the view moves.
    const angle = Math.PI / 4 + (this.orbit * Math.PI) / 3;
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
