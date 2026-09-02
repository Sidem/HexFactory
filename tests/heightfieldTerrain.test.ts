import {
  DoubleSide,
  Mesh,
  MeshBasicMaterial,
  PerspectiveCamera,
  Raycaster,
  Vector3,
} from "three";
import { describe, expect, it } from "vitest";

import {
  buildHeightfieldGeometry,
  heightfieldHeightAt,
  pickHeightfieldAt,
  pickHeightfieldRay,
  type HeightfieldSample,
} from "../src/rendering/three/heightfieldTerrain";

const dry = (
  q: number,
  r: number,
  height: number,
  substrate: HeightfieldSample["substrate"] = "meadow",
  look = 0,
): HeightfieldSample => ({
  q,
  r,
  height,
  substrate,
  waterDepth: 0,
  waterHeight: height,
  dischargeClass: 0,
  look,
  waterLook: look,
});

const triangles = (
  geometry: ReturnType<typeof buildHeightfieldGeometry>["ground"],
): number => (geometry.getIndex()?.count ?? 0) / 3;

describe("physical heightfield terrain", () => {
  it("blends ordinary neighbours into one deterministic continuous surface", () => {
    const samples = [dry(0, 0, 0), dry(1, 0, 0.2), dry(0, 1, 0.1)];
    const forward = buildHeightfieldGeometry(samples, {
      cliffThreshold: 0.25,
    });
    const reverse = buildHeightfieldGeometry([...samples].reverse(), {
      cliffThreshold: 0.25,
    });

    expect(triangles(forward.ground)).toBe(18);
    expect(triangles(forward.cliffs)).toBe(0);
    expect(forward.ground.getAttribute("position").array).toEqual(
      reverse.ground.getAttribute("position").array,
    );
    expect(forward.ground.getIndex()?.array).toEqual(
      reverse.ground.getIndex()?.array,
    );
    expect(heightfieldHeightAt(forward, 1, 0)).toBe(0.2);
    expect(heightfieldHeightAt(forward, 9, 9)).toBeUndefined();

    dispose(forward);
    dispose(reverse);

    // A sample the builder cannot make a surface out of is refused rather than drawn wrong.
    expect(() =>
      buildHeightfieldGeometry([dry(0, 0, 0), dry(0, 0, 1)], {
        cliffThreshold: 1,
      }),
    ).toThrow(/Duplicate/);
    expect(() =>
      buildHeightfieldGeometry([{ ...dry(0, 0, 0), waterDepth: Number.NaN }], {
        cliffThreshold: 1,
      }),
    ).toThrow(/Invalid/);

    // And the tapered end of a cliff closes through the ordinary-slope component: the shared
    // vertex has one height, not two.
    const built = buildHeightfieldGeometry(
      [dry(0, 0, 0), dry(1, 0, 0.2), dry(0, 1, 0.4)],
      { cliffThreshold: 0.25 },
    );
    const positions = built.ground.getAttribute("position");
    const atSharedVertex = new Set<number>();
    for (let index = 0; index < positions.count; index += 1) {
      if (
        Math.abs(positions.getX(index) - Math.cos(Math.PI / 6)) < 1e-6 &&
        Math.abs(positions.getZ(index) - 0.5) < 1e-6
      )
        atSharedVertex.add(positions.getY(index));
    }
    expect(atSharedVertex.size).toBe(1);
    expect([...atSharedVertex][0]).toBeCloseTo(0.2, 6);

    dispose(built);
  });

  it("keeps a real discontinuity vertical and drops only the surveyed frontier", () => {
    const built = buildHeightfieldGeometry(
      [dry(0, 0, 0, "soil"), dry(1, 0, 1, "rock")],
      { cliffThreshold: 0.25, frontierDepth: 2 },
    );

    expect(triangles(built.ground)).toBe(12);
    expect(triangles(built.cliffs)).toBe(2);
    // Two adjacent hexes expose ten outer edges, each one a two-triangle frontier skirt.
    expect(triangles(built.frontier)).toBe(20);
    const heights = [...built.frontier.getAttribute("position").array].filter(
      (_, index) => index % 3 === 1,
    );
    expect(Math.min(...heights)).toBe(-2);
    expect(Math.max(...heights)).toBe(1);

    dispose(built);
  });

  it("carries a dissolve weight that falls to nothing across the frontier", () => {
    const samples: HeightfieldSample[] = [];
    for (let q = -2; q <= 2; q += 1) {
      for (let r = -2; r <= 2; r += 1) {
        if (Math.max(Math.abs(q), Math.abs(r), Math.abs(q + r)) <= 2)
          samples.push(dry(q, r, 0));
      }
    }
    const built = buildHeightfieldGeometry(samples, { cliffThreshold: 1 });
    const weights = [
      ...built.ground.getAttribute("frontierFade").array,
    ] as number[];
    // The three ring values still reach the surface through the cell-centre vertices, and the
    // corners between them carry the means that turn those rings into a gradient rather than three
    // hex-shaped bands. The shader paints this weight now instead of dithering on it, so a step
    // between rings would be visible as a step.
    for (const expected of [0.42, 0.76, 1])
      expect(weights.some((value) => Math.abs(value - expected) < 1e-5)).toBe(
        true,
      );
    expect(weights.some((value) => value > 0.42 && value < 0.76 - 1e-5)).toBe(
      true,
    );
    // Nothing on the surface reaches the background, and nothing goes past it either: the disc
    // fades towards sky and the outermost corners are the closest it gets.
    expect(Math.min(...weights)).toBeGreaterThan(0);
    expect(Math.min(...weights)).toBeLessThan(0.42);
    expect(Math.max(...weights)).toBe(1);
    // The skirt behind the rim reaches the background exactly, so its unlit face never reads as a
    // dark band under the horizon.
    const skirt = [
      ...built.frontier.getAttribute("frontierFade").array,
    ] as number[];
    expect(Math.min(...skirt)).toBe(0);
    expect(Math.max(...skirt)).toBeLessThan(0.42);

    dispose(built);
  });

  it("builds water as its own native-level surface", () => {
    const wet: HeightfieldSample = {
      ...dry(0, 0, -0.5, "sand"),
      waterDepth: 2,
      waterHeight: 0.25,
      dischargeClass: 3,
    };
    const built = buildHeightfieldGeometry([wet, dry(1, 0, 0)], {
      cliffThreshold: 1,
    });

    expect(triangles(built.water)).toBe(6);
    expect([...built.water.getAttribute("discharge").array]).toEqual(
      expect.arrayContaining([3]),
    );
    const heights = [...built.water.getAttribute("position").array].filter(
      (_, index) => index % 3 === 1,
    );
    expect(new Set(heights)).toEqual(new Set([0.25]));

    dispose(built);
  });

  it("picks the visible height mesh rather than a fixed logical plane", () => {
    const built = buildHeightfieldGeometry([dry(0, 0, 2)], {
      cliffThreshold: 1,
    });
    const material = new MeshBasicMaterial({ side: DoubleSide });
    const mesh = new Mesh(built.ground, material);
    const raycaster = new Raycaster(
      new Vector3(0, 10, 0),
      new Vector3(0, -1, 0),
    );
    expect(pickHeightfieldRay(raycaster, mesh)).toMatchObject({
      axial: { q: 0, r: 0 },
      world: { x: 0, y: 0 },
      height: 2,
    });

    const camera = new PerspectiveCamera(50, 1, 0.1, 100);
    camera.position.set(0, 10, 0.01);
    camera.lookAt(0, 0, 0);
    camera.updateMatrixWorld(true);
    expect(
      pickHeightfieldAt(
        new Raycaster(),
        camera,
        mesh,
        { left: 100, top: 50, width: 400, height: 400 },
        300,
        250,
      ),
    ).toMatchObject({ axial: { q: 0, r: 0 }, height: 2 });

    material.dispose();
    dispose(built);
  });

  it("groups one surface into draw runs so several looks share a mesh", () => {
    const built = buildHeightfieldGeometry(
      [
        dry(0, 0, 0, "rock", 6),
        dry(1, 0, 0, "sand", 2),
        dry(0, 1, 0, "rock", 6),
        {
          ...dry(-1, 0, -1, "soil", 0),
          waterDepth: 4,
          waterHeight: 0,
        },
        { ...dry(1, -1, -1, "soil", 1), waterDepth: 1, waterHeight: 0 },
      ],
      { cliffThreshold: 4 },
    );

    // Sorted and deduplicated: the two look-6 cells share one run rather than opening a second,
    // which is the whole point of grouping instead of splitting the mesh into one per look.
    expect(built.buckets.ground).toEqual([0, 1, 2, 6]);
    expect(built.ground.groups.map((group) => group.count)).toEqual([
      18, 18, 18, 36,
    ]);
    expect(built.ground.groups.map((group) => group.start)).toEqual([
      0, 18, 36, 54,
    ]);
    // The two wet cells part company on their look even though both carry discharge class 0 and
    // would be one run if the per-vertex channel were what decided the grouping.
    expect(built.buckets.water).toEqual([0, 1]);
    expect(built.water.groups.map((group) => group.count)).toEqual([18, 18]);

    dispose(built);

    // A frame that surveys rebuilds the whole surveyed landform, so the ground run of the commonest
    // look keeps growing as the player walks. Ten thousand cells is 180,000 indices in one run,
    // well past what a spread can pass as call arguments.
    const samples: HeightfieldSample[] = [];
    for (let q = 0; q < 100; q += 1)
      for (let r = 0; r < 100; r += 1) samples.push(dry(q, r, 0));
    const walked = buildHeightfieldGeometry(samples, { cliffThreshold: 0.25 });

    expect(walked.buckets.ground).toEqual([0]);
    expect(triangles(walked.ground)).toBe(samples.length * 6);
    expect(walked.ground.groups.map((group) => group.count)).toEqual([
      samples.length * 18,
    ]);

    dispose(walked);
  });
});

function dispose(build: ReturnType<typeof buildHeightfieldGeometry>): void {
  build.ground.dispose();
  build.water.dispose();
  build.cliffs.dispose();
  build.frontier.dispose();
}
