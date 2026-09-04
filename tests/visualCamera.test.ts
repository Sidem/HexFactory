import { axialToPixel } from "@hexlife/embed/hex";
import { describe, expect, it } from "vitest";

import { WORLD_SCALE } from "../src/rendering/landmarks";
import { HexSceneCamera } from "../src/rendering/three/HexSceneCamera";

const DEFAULT_CAMERA_TILT = Math.atan2(31, 38);

function heading(camera: HexSceneCamera): number {
  return Math.atan2(camera.camera.position.z, camera.camera.position.x);
}

function elevation(camera: HexSceneCamera): number {
  const { x, y, z } = camera.camera.position;
  return Math.atan2(y, Math.hypot(x, z));
}

function turnedBy(camera: HexSceneCamera, from: number): number {
  const delta = heading(camera) - from;
  return delta - Math.round(delta / (Math.PI * 2)) * Math.PI * 2;
}

function settle(camera: HexSceneCamera): void {
  camera.advanceOrbit(performance.now() + 10_000);
}

describe("Visual Depth camera", () => {
  it("round-trips native world and axial points at every orbit and zoom extreme", () => {
    const coordinate = { q: 7, r: -4 };
    const world = axialToPixel(coordinate, WORLD_SCALE, { x: 0, y: 0 });
    for (let orbit = 0; orbit < 12; orbit += 1) {
      for (const factor of [0.01, 100]) {
        const camera = new HexSceneCamera();
        camera.resize(1440, 900);
        camera.recenter(world);
        for (let step = 0; step < orbit; step += 1) camera.orbitBy(1);
        settle(camera);
        camera.zoomAt(720, 450, factor);
        const screen = camera.projectWorld(world);
        const roundTrip = camera.worldAt(screen.x, screen.y);
        expect(roundTrip.x).toBeCloseTo(world.x, 0);
        expect(roundTrip.y).toBeCloseTo(world.y, 0);
        expect(camera.axialAt(screen.x, screen.y)).toEqual(coordinate);
      }
    }
  });

  it("zooms in past the old ceiling and still clamps at both ends", () => {
    const camera = new HexSceneCamera();
    camera.resize(1440, 900);
    camera.zoomAt(720, 450, 100);
    expect(camera.zoomLevel).toBeCloseTo(4, 6);
    camera.zoomAt(720, 450, 0.0001);
    expect(camera.zoomLevel).toBeCloseTo(0.55, 6);
  });

  it("tilts over a fixed-radius dome, stays pointed at its target, and clamps safely", () => {
    const camera = new HexSceneCamera();
    camera.resize(1440, 900);
    const start = elevation(camera);
    const radius = camera.camera.position.length();

    camera.tiltBy(1, false);
    expect(elevation(camera) - start).toBeCloseTo((Math.PI * 5) / 180, 6);
    expect(camera.camera.position.length()).toBeCloseTo(radius, 8);

    for (let step = 0; step < 20; step += 1) camera.tiltBy(1, false);
    expect(elevation(camera)).toBeCloseTo((Math.PI * 70) / 180, 6);
    for (let step = 0; step < 20; step += 1) camera.tiltBy(-1, false);
    expect(elevation(camera)).toBeCloseTo((Math.PI * 20) / 180, 6);

    const target = camera.projectWorld({ x: 0, y: 0 });
    expect(target.x).toBeCloseTo(720, 8);
    expect(target.y).toBeCloseTo(450, 8);
  });

  it("eases a tilt step and snaps it under reduced motion", () => {
    const camera = new HexSceneCamera();
    const start = elevation(camera);
    camera.tiltBy(1);
    expect(camera.isTilting).toBe(true);
    expect(camera.advance(performance.now() + 10_000)).toBe(true);
    expect(camera.isTilting).toBe(false);
    expect(elevation(camera) - start).toBeCloseTo((Math.PI * 5) / 180, 6);

    camera.tiltBy(-1, false);
    expect(camera.isTilting).toBe(false);
    expect(elevation(camera)).toBeCloseTo(start, 6);
  });

  it("follows mouse-look continuously without entering a stepped sweep", () => {
    const camera = new HexSceneCamera();
    const startHeading = heading(camera);
    const startElevation = elevation(camera);

    camera.lookBy(8, -8);

    expect(turnedBy(camera, startHeading)).toBeCloseTo(Math.PI / 24, 6);
    expect(elevation(camera) - startElevation).toBeCloseTo(Math.PI / 144, 6);
    expect(camera.isOrbiting).toBe(false);
    expect(camera.isTilting).toBe(false);
  });

  it("sends arrow keys to the next stop after a free mouse angle", () => {
    const right = new HexSceneCamera();
    right.lookBy(8, -8);
    const rightStart = heading(right);
    right.orbitBy(1, false);
    expect(turnedBy(right, rightStart)).toBeCloseTo(Math.PI / 8, 6);
    expect(right.orbitIndex).toBe(1);
    right.tiltBy(1, false);
    expect(elevation(right)).toBeCloseTo(DEFAULT_CAMERA_TILT + Math.PI / 36, 6);

    const left = new HexSceneCamera();
    left.lookBy(8, 8);
    const leftStart = heading(left);
    left.orbitBy(-1, false);
    expect(turnedBy(left, leftStart)).toBeCloseTo(-Math.PI / 24, 6);
    expect(left.orbitIndex).toBe(0);
    left.tiltBy(-1, false);
    expect(elevation(left)).toBeCloseTo(DEFAULT_CAMERA_TILT - Math.PI / 36, 6);
  });

  it("closes the full circle in twelve thirty-degree stops, and the index wraps both ways", () => {
    const wrapping = new HexSceneCamera();
    for (let step = 0; step < 24; step += 1) wrapping.orbitBy(1);
    expect(wrapping.orbitIndex).toBe(0);
    wrapping.orbitBy(-1);
    expect(wrapping.orbitIndex).toBe(11);

    const camera = new HexSceneCamera();
    camera.resize(1440, 900);
    const start = heading(camera);
    let previous = start;
    for (let step = 1; step <= 12; step += 1) {
      camera.orbitBy(1, false);
      expect(camera.orbitIndex).toBe(step % 12);
      expect(turnedBy(camera, previous)).toBeCloseTo(Math.PI / 6, 6);
      previous = heading(camera);
    }
    expect(turnedBy(camera, start)).toBeCloseTo(0, 6);
  });

  it("squares the hex edges up with the screen at every orbit stop", () => {
    const corners = Array.from({ length: 6 }, (_, index) => {
      const theta = (index * Math.PI) / 3;
      return { x: Math.sin(theta), z: Math.cos(theta) };
    });
    const camera = new HexSceneCamera();
    camera.resize(1440, 900);
    for (let orbit = 0; orbit < 12; orbit += 1) {
      const screen = corners.map((corner) =>
        camera.projectScene(corner.x, 0, corner.z),
      );
      const edges = screen.map((from, index) => {
        const to = screen[(index + 1) % 6]!;
        return { x: Math.abs(to.x - from.x), y: Math.abs(to.y - from.y) };
      });
      const flat = edges.filter((edge) => edge.y < 1e-6).length;
      const upright = edges.filter((edge) => edge.x < 1e-6).length;
      expect(orbit % 2 === 0 ? flat : upright, `orbit ${orbit}`).toBe(2);
      camera.orbitBy(1, false);
    }
  });

  it("inverse-projects WASD through every camera orbit", () => {
    const directions = [
      { x: 0, y: -1 },
      { x: -1, y: 0 },
      { x: 0, y: 1 },
      { x: 1, y: 0 },
    ];
    const camera = new HexSceneCamera();
    camera.resize(1440, 900);
    for (let orbit = 0; orbit < 12; orbit += 1) {
      const center = camera.projectScene(0, 0, 0);
      for (const direction of directions) {
        const world = camera.screenMovement(direction.x, direction.y);
        const projected = camera.projectScene(world.x, 0, world.y);
        const screenX = projected.x - center.x;
        const screenY = projected.y - center.y;
        expect(screenX * direction.y - screenY * direction.x).toBeCloseTo(0, 8);
        expect(screenX * direction.x + screenY * direction.y).toBeGreaterThan(
          0,
        );
      }
      camera.orbitBy(1);
      settle(camera);
    }
  });

  it("eases each thirty-degree step, absorbs a press mid-sweep, and snaps under reduced motion", () => {
    const fresh = (): HexSceneCamera => {
      const camera = new HexSceneCamera();
      camera.resize(1440, 900);
      return camera;
    };

    const landing = fresh();
    const landingStart = heading(landing);
    landing.orbitBy(1);
    expect(landing.isOrbiting).toBe(true);
    settle(landing);
    expect(landing.isOrbiting).toBe(false);
    expect(turnedBy(landing, landingStart)).toBeCloseTo(Math.PI / 6, 6);
    expect(landing.orbitIndex).toBe(1);

    const easing = fresh();
    const easingStart = heading(easing);
    const began = performance.now();
    easing.orbitBy(1);
    expect(easing.advanceOrbit(began + 115)).toBe(true);
    const partway = turnedBy(easing, easingStart);
    expect(partway).toBeGreaterThan(0.05);
    expect(partway).toBeLessThan(Math.PI / 6 - 0.05);
    easing.advanceOrbit(began + 1000);
    expect(easing.isOrbiting).toBe(false);
    expect(turnedBy(easing, easingStart)).toBeCloseTo(Math.PI / 6, 6);

    const pressed = fresh();
    const pressedStart = heading(pressed);
    const pressedAt = performance.now();
    pressed.orbitBy(1);
    pressed.advanceOrbit(pressedAt + 100);
    pressed.orbitBy(1);
    expect(pressed.orbitIndex).toBe(2);
    expect(pressed.isOrbiting).toBe(true);
    pressed.advanceOrbit(pressedAt + 1000);
    expect(pressed.isOrbiting).toBe(false);
    expect(turnedBy(pressed, pressedStart)).toBeCloseTo(Math.PI / 3, 6);

    const snapped = fresh();
    const snappedStart = heading(snapped);
    snapped.orbitBy(1, false);
    expect(snapped.isOrbiting).toBe(false);
    expect(snapped.advanceOrbit(performance.now() + 1000)).toBe(false);
    expect(turnedBy(snapped, snappedStart)).toBeCloseTo(Math.PI / 6, 6);

    const walking = fresh();
    walking.orbitBy(1);
    walking.advanceOrbit(performance.now() + 100);
    const drawn = heading(walking);
    const during = walking.screenMovement(0, -1);
    expect(heading(walking)).toBeCloseTo(drawn, 12);
    settle(walking);
    const after = walking.screenMovement(0, -1);
    expect(during.x).toBeCloseTo(after.x, 6);
    expect(during.y).toBeCloseTo(after.y, 6);
  });
});
