import {
  HEX_DIRECTIONS,
  axialNeighbor,
  axialToPixel,
  pixelToAxial,
  rotateHexDirection,
} from "@hexlife/embed/hex";
import { describe, expect, it } from "vitest";

import directionFixture from "../fixtures/hex-directions.json";
import { encodeCommand } from "../src/core/commands";

describe("public hex host contract", () => {
  it("pins TypeScript and Rust to the published clockwise direction fixture", () => {
    expect(HEX_DIRECTIONS).toEqual(directionFixture);
    expect(axialNeighbor({ q: -2, r: 0 }, 1)).toEqual({ q: -2, r: 1 });
    expect(rotateHexDirection(5, 1)).toBe(0);
  });

  it("round-trips Canvas placement hit testing through @hexlife/embed/hex", () => {
    const origin = { x: 410, y: 330 };
    for (const coordinate of [
      { q: -4, r: 0 },
      { q: -2, r: 1 },
      { q: 2, r: 1 },
    ]) {
      expect(
        pixelToAxial(axialToPixel(coordinate, 35, origin), 35, origin),
      ).toEqual(coordinate);
    }
  });

  it("encodes bounded native commands without embedding simulation behavior", () => {
    expect(
      encodeCommand({
        type: "place",
        coordinate: { q: -3, r: 2 },
        definitionId: 2,
        orientation: 5,
      }),
    ).toEqual({ opcode: 0, args: [-3, 2, 2, 5, 0] });
    expect(encodeCommand({ type: "tick", count: 12 })).toEqual({
      opcode: 3,
      args: [12],
    });
    expect(() => encodeCommand({ type: "tick", count: 0 })).toThrow(
      /positive integer/,
    );
  });
});
