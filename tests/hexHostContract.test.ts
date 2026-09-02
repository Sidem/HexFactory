import {
  HEX_DIRECTIONS,
  axialNeighbor,
  axialToPixel,
  pixelToAxial,
  rotateHexDirection,
} from "@hexlife/embed/hex";
import { describe, expect, it } from "vitest";

import directionFixture from "../fixtures/hex-directions.json";
import { HexCamera } from "../src/rendering/CanvasFactoryRenderer";

describe("public hex host contract", () => {
  it("pins TypeScript and Rust to the clockwise twelve-heading fixture", () => {
    expect(HEX_DIRECTIONS.map(({ q, r }) => ({ q, r }))).toEqual(
      directionFixture.slice(0, 6).map(({ q, r }) => ({ q, r })),
    );
    expect(directionFixture).toHaveLength(12);
    for (let index = 6; index < 12; index += 1) {
      const current = directionFixture[index]!;
      const next = directionFixture[index === 11 ? 6 : index + 1]!;
      expect({ q: -current.r, r: current.q + current.r }).toEqual({
        q: next.q,
        r: next.r,
      });
    }
    expect(axialNeighbor({ q: -2, r: 0 }, 1)).toEqual({ q: -2, r: 1 });
    expect(rotateHexDirection(5, 1)).toBe(0);
  });

  it("round-trips base and pan/zoom camera picking through @hexlife/embed/hex", () => {
    const origin = { x: 410, y: 330 };
    expect(
      pixelToAxial(axialToPixel({ q: -4, r: 2 }, 35, origin), 35, origin),
    ).toEqual({ q: -4, r: 2 });
    const camera = new HexCamera();
    camera.recenter({ x: 3550, y: -3072 });
    const coordinate = { q: -4, r: 5 };
    const screen = camera.project(coordinate, 900, 650);
    expect(camera.pick(screen, 900, 650)).toEqual(coordinate);
    camera.panBy(73, -42);
    camera.zoomAt(1.6, { x: 320, y: 240 }, 900, 650);
    const moved = camera.project(coordinate, 900, 650);
    expect(camera.pick(moved, 900, 650)).toEqual(coordinate);

    const followed = new HexCamera();
    const player = { x: 3550, y: -3072 };
    followed.recenter(player);
    expect(followed.following).toBe(true);
    followed.zoomAt(1.6, { x: 80, y: 40 }, 900, 650);
    expect(followed.following).toBe(true);
    expect(followed.pan).toEqual({ x: 0, y: 0 });
    expect(followed.center).toEqual(player);
    followed.follow({ x: 4000, y: 0 });
    expect(followed.center).toEqual({ x: 4000, y: 0 });
  });
});
