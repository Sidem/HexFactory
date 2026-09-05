import { describe, expect, it } from "vitest";
import { Box3 } from "three";

import type { EntitySnapshot } from "../src/core/types";
import {
  BUILDING_SHAPES,
  partsFor,
  type SilhouetteKey,
} from "../src/rendering/buildingLook";
import type { ShapePart } from "../src/rendering/shapeGrammar";
import {
  buildPartGeometry,
  MACHINE_PLATFORM_HEIGHT,
  MACHINE_SILHOUETTE_SCALE,
  machinePartMatrix,
  machineRestingLift,
  type MachinePartInstance,
} from "../src/rendering/three/machineMeshes";

/** A cell well off the water line, so a mistake cannot hide inside a zero ground height. */
const GROUND = 0.4;

describe("Machine placement", () => {
  /**
   * The grammar's `y` is a 2D anchor measured from the hex centre, so an authored shape straddles
   * it. Read as a height above the ground it sank every machine into the plinth it stands on. The
   * assembly moves as one — the anchors still set the parts' heights against each other — so what
   * is checked here is the one thing that has to be true of the whole: its lowest standing point
   * touches the platform top exactly, at some phase of the work cycle.
   */
  it("stands every silhouette on its platform rather than in it", () => {
    for (const key of Object.keys(BUILDING_SHAPES) as SilhouetteKey[]) {
      for (const tier of [0, 1, 2]) {
        for (const growth of [0, 2]) {
          // A lone hex, and a plot wide enough to stretch the bodies standing on it.
          for (const footprintScale of [1, 2.5]) {
            const parts = partsFor(key, tier, growth);
            const standing = parts.filter(isStanding);
            if (!standing.length) continue;
            const lowest = lowestPoint(key, parts, standing, footprintScale);
            expect(
              lowest,
              `${key}/${tier}/${growth}/${footprintScale}`,
            ).toBeCloseTo(GROUND + MACHINE_PLATFORM_HEIGHT, 6);
          }
        }
      }
    }
  });

  it("still drives a drill into the ground it is biting", () => {
    const parts = partsFor("extractor", 0, 0);
    const drill = parts.filter((part) => !isStanding(part));
    expect(drill).toHaveLength(1);
    expect(lowestPoint("extractor", parts, drill, 1)).toBeLessThan(GROUND);
  });

  it("leaves a silhouette with no standing parts where transport geometry put it", () => {
    expect(partsFor("belt", 0, 0)).toHaveLength(0);
    expect(machineRestingLift([], MACHINE_SILHOUETTE_SCALE.belt)).toBe(0);
  });
});

/** A part rotated past horizontal is reaching into the cell on purpose, not standing on it. */
function isStanding(part: ShapePart): boolean {
  return Math.cos(part.rotation ?? 0) >= 0;
}

/**
 * The lowest world point the given parts reach across a whole work cycle, placed exactly as the
 * renderer places them. `progress / progress_total` is how a building drives its own cycle, so the
 * sweep below covers the resting pose and every phase a machine passes through.
 */
function lowestPoint(
  key: SilhouetteKey,
  silhouette: readonly ShapePart[],
  measured: readonly ShapePart[],
  footprintScale: number,
): number {
  const baseLift = machineRestingLift(
    silhouette,
    MACHINE_SILHOUETTE_SCALE[key],
    footprintScale,
  );
  const box = new Box3();
  let lowest = Number.POSITIVE_INFINITY;
  for (let step = 0; step <= 8; step += 1) {
    for (const part of measured) {
      const instance: MachinePartInstance = {
        building: cycling(step),
        part,
        key: `${part.part}:${part.count ?? 0}`,
        animated: part.phase !== undefined && part.phase !== "still",
        color: "#ffffff",
        glow: part.glow ?? null,
        material: part.material ?? "structure",
        groundHeight: GROUND,
        footprintScale,
        visualScale: MACHINE_SILHOUETTE_SCALE[key],
        baseLift,
        x: 0,
        z: 0,
      };
      const geometry = buildPartGeometry(part.part, part.count ?? 0);
      geometry.computeBoundingBox();
      box
        .copy(geometry.boundingBox!)
        .applyMatrix4(machinePartMatrix(instance, 0, step === 0));
      geometry.dispose();
      lowest = Math.min(lowest, box.min.y);
    }
  }
  return lowest;
}

/** A machine one eighth of the way through each step of its own work cycle. */
function cycling(step: number): EntitySnapshot {
  return {
    id: 1,
    definition_id: 1,
    kind: "composer",
    q: 0,
    r: 0,
    // An off-axis heading, so a lift that leaked into the horizontal transform shows up.
    orientation: 2,
    scenario_owned: false,
    inventory: [],
    progress: step,
    progress_total: 8,
    status: "composing",
    footprint: [{ q: 0, r: 0 }],
  };
}
