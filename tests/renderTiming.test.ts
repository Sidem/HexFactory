import { expect, it } from "vitest";
import { measureRenderPhases } from "../src/bench/renderTiming";

it("measures both snapshot setters, separates initial setup, and retains draw-only metrics", () => {
  let clock = 0;
  let worldSets = 0;
  const calls: string[] = [];
  const measured = measureRenderPhases(
    {
      setWorldSnapshot() {
        calls.push("world snapshot");
        clock += worldSets++ === 0 ? 50 : 2;
      },
      setMinimapSnapshot() {
        calls.push("minimap snapshot");
        clock += 3;
      },
      drawWorld() {
        calls.push("world draw");
        clock += 4;
      },
      drawMinimap() {
        calls.push("minimap draw");
        clock += 1;
      },
    },
    () => clock,
    10,
  );
  expect(calls.slice(0, 4)).toEqual([
    "world snapshot",
    "minimap snapshot",
    "world draw",
    "minimap draw",
  ]);
  expect(measured).toEqual({
    render_measurement: "snapshot-and-draw-v1",
    snapshot_setup_us: 53_000,
    snapshot_world_us: 2_000,
    snapshot_minimap_us: 3_000,
    render_world_us: 4_000,
    render_minimap_us: 1_000,
    render_us: 5_000,
    render_samples: 3,
    preparation_submission_us: 10_000,
  });
});
