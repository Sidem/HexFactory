import { timeMeanUs } from "./report";

/** Isolated CPU spans. Neither submission nor their sum measures GPU/presentation time. */
export function measureRenderPhases(
  phases: {
    setWorldSnapshot(): void;
    drawWorld(): void;
    setMinimapSnapshot(): void;
    drawMinimap(): void;
  },
  nowMs: () => number = () => performance.now(),
  minMs = 20,
): {
  render_measurement: "snapshot-and-draw-v1";
  snapshot_setup_us: number;
  snapshot_world_us: number;
  snapshot_minimap_us: number;
  render_world_us: number;
  render_minimap_us: number;
  render_us: number;
  render_samples: number;
  preparation_submission_us: number;
} {
  // First application can compile shaders and build terrain. Keep it out of steady timings,
  // but retain its cost. Repeated setters below explicitly measure the same snapshot.
  const started = nowMs();
  phases.setWorldSnapshot();
  phases.setMinimapSnapshot();
  const snapshotSetupUs = (nowMs() - started) * 1000;
  phases.drawWorld();
  phases.drawMinimap();
  const worldSnapshot = timeMeanUs(phases.setWorldSnapshot, nowMs, minMs);
  const minimapSnapshot = timeMeanUs(phases.setMinimapSnapshot, nowMs, minMs);
  const world = timeMeanUs(phases.drawWorld, nowMs, minMs);
  const minimap = timeMeanUs(phases.drawMinimap, nowMs, minMs);
  return {
    render_measurement: "snapshot-and-draw-v1",
    snapshot_setup_us: snapshotSetupUs,
    snapshot_world_us: worldSnapshot.meanUs,
    snapshot_minimap_us: minimapSnapshot.meanUs,
    render_world_us: world.meanUs,
    render_minimap_us: minimap.meanUs,
    render_us: world.meanUs + minimap.meanUs,
    render_samples: world.samples,
    preparation_submission_us:
      worldSnapshot.meanUs +
      minimapSnapshot.meanUs +
      world.meanUs +
      minimap.meanUs,
  };
}
