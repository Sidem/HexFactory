import { describe, expect, it } from "vitest";

import {
  FRAME_BUDGET_US,
  RENDER_VIEWPORT,
  TIER_COLUMNS,
  deltaMergeIsIntact,
  frameShare,
  hostHasRender,
  mergeBrowserReport,
  probeClockResolutionUs,
  tierRow,
  timeMeanUs,
} from "../src/bench/report";
import type {
  BenchEnvironment,
  HostTierResult,
  NativeReport,
  NativeTierResult,
} from "../src/bench/report";

const nativeTier: NativeTierResult = {
  key: "small",
  lines: 16,
  entities: 192,
  tiles: 1216,
  chunks: 12,
  measured_ticks: 200,
  tick_us: 6,
  ticks_per_second: 166_666,
  snapshot_us: 74.7,
  checksum_us: 45,
  frame_us: 122.8,
  frames_per_second: 8143,
  delta_bytes: 19_745,
  delta_json_bytes: 61_284,
  full_compile_us: 17.8,
  incremental_recompile_us: 57.8,
  edit_us: 67.1,
  checksum: 1_693_021_923,
  delivered: 224,
};

const hostTier: HostTierResult = {
  key: "small",
  frames: 60,
  round_trip_us: 400,
  apply_us: 100,
  host_frame_us: 500,
  checksum: 1_693_021_923,
  applied_entities: 192,
};

const environment: BenchEnvironment = {
  user_agent: "test-agent",
  hardware_concurrency: 16,
  cross_origin_isolated: false,
  main_clock_resolution_us: 100,
  worker_clock_resolution_us: 100,
  recorded: "2026-08-17T00:00:00.000Z",
};

const nativeReport: NativeReport = {
  schema: 3,
  crate_version: "0.8.0",
  profile: "release",
  platform: "wasm32",
  tiers: [nativeTier],
};

describe("browser capacity report", () => {
  it("keeps the crate's own account of what it measured, paired by key", () => {
    const report = mergeBrowserReport(nativeReport, [hostTier], environment);
    expect(report.platform).toBe("wasm32");
    expect(report.schema).toBe(3);
    expect(report.crate_version).toBe("0.8.0");
    expect(report.environment).toEqual(environment);
    expect(report.tiers[0]?.host).toEqual(hostTier);
    // One cell per column, and the tier names itself.
    const row = tierRow(report.tiers[0]!);
    expect(row).toHaveLength(TIER_COLUMNS.length);
    expect(row[0]).toBe("small");

    // Host results are matched to their tier by key: a report that listed them positionally would
    // quote the medium tier's cost against the small tier's work.
    const paired = mergeBrowserReport(
      nativeReport,
      [{ ...hostTier, key: "medium" }, hostTier],
      environment,
    );
    expect(paired.tiers[0]?.host?.key).toBe("small");
  });

  it("leaves an unmeasured host and an unmeasured renderer absent, not free", () => {
    const unmeasured = mergeBrowserReport(nativeReport, [], environment);
    expect(unmeasured.tiers[0]?.host).toBeNull();
    // An unmeasured host cost must read as absent, never as zero cost.
    expect(tierRow(unmeasured.tiers[0]!).slice(-9)).toEqual([
      "—",
      "—",
      "—",
      "—",
      "—",
      "—",
      "—",
      "—",
      "—",
    ]);

    const report = mergeBrowserReport(nativeReport, [hostTier], environment);
    expect(hostHasRender(report.tiers[0]!.host)).toBe(false);
    const row = tierRow(report.tiers[0]!);
    expect(row.slice(-13, -2)).toEqual([
      "—",
      "—",
      "—",
      "—",
      "—",
      "—",
      "—",
      "—",
      "—",
      "—",
      "—",
    ]);
    expect(row.at(-2)).toBe("3.0%");
    expect(row.at(-1)).toBe("—");
  });

  it("states a complete browser frame, and its share of 60 Hz, once the renderer is timed", () => {
    expect(frameShare(FRAME_BUDGET_US)).toBe("100.0%");
    expect(frameShare(FRAME_BUDGET_US / 2)).toBe("50.0%");
    expect(frameShare(500)).toBe("3.0%");

    const rendered: HostTierResult = {
      ...hostTier,
      render_world_us: 800,
      render_minimap_us: 200,
      render_us: 1_000,
      render_samples: 20,
      browser_frame_us: 1_500,
    };
    const report = mergeBrowserReport(nativeReport, [rendered], environment);
    expect(hostHasRender(report.tiers[0]!.host)).toBe(true);
    const row = tierRow(report.tiers[0]!);
    expect(row).toHaveLength(TIER_COLUMNS.length);
    expect(row.slice(-13)).toEqual([
      "800.0",
      "200.0",
      "1,000.0",
      "—",
      "—",
      "—",
      "—",
      "—",
      "—",
      "—",
      "1,500.0",
      "3.0%",
      "9.0%",
    ]);
  });

  it("fails the merge check when an applied snapshot lost entities", () => {
    const intact = mergeBrowserReport(nativeReport, [hostTier], environment);
    expect(deltaMergeIsIntact(intact)).toBe(true);
    const lost = mergeBrowserReport(
      nativeReport,
      [{ ...hostTier, applied_entities: 191 }],
      environment,
    );
    expect(deltaMergeIsIntact(lost)).toBe(false);
    // A tier the host never measured cannot fail the check it never ran.
    expect(
      deltaMergeIsIntact(mergeBrowserReport(nativeReport, [], environment)),
    ).toBe(true);
  });
});

describe("timed sample budget", () => {
  it("repeats work until the minimum duration has elapsed, but never fewer than once", () => {
    let reading = 0;
    let calls = 0;
    const now = (): number => reading;
    const timed = timeMeanUs(
      () => {
        calls += 1;
        reading += 5;
      },
      now,
      20,
    );
    expect(timed.samples).toBe(4);
    expect(timed.elapsedMs).toBe(20);
    expect(timed.meanUs).toBe(5_000);
    expect(calls).toBe(4);

    // One sample that already exceeds the budget is the whole measurement.
    reading = 0;
    const slow = timeMeanUs(
      () => {
        reading += 50;
      },
      now,
      20,
    );
    expect(slow.samples).toBe(1);
    expect(slow.meanUs).toBe(50_000);
  });
});

describe("pinned renderer viewport", () => {
  it("is the playtest world size and the shipped minimap, on the bench page's own canvases", async () => {
    expect(RENDER_VIEWPORT).toEqual({
      width: 1440,
      height: 900,
      minimap: 178,
    });

    const { readFileSync } = await import("node:fs");
    const page = readFileSync(
      new URL("../bench.html", import.meta.url),
      "utf8",
    );
    expect(page).toContain('id="bench-world"');
    expect(page).toContain('id="bench-minimap"');
    expect(page).toContain("width: 1440px");
    expect(page).toContain("height: 900px");
    expect(page).toContain("width: 178px");
    expect(page).toContain("height: 178px");
  });
});

describe("clock resolution probe", () => {
  it("reports the smallest step a clock actually moves by, quantized or fine", () => {
    // A clamped browser clock: readings advance only in whole 0.1 ms steps.
    let reading = 0;
    const clamped = (): number => {
      reading += 1;
      return Math.floor(reading / 3) * 0.1;
    };
    expect(probeClockResolutionUs(clamped, 4)).toBeCloseTo(100, 6);

    reading = 0;
    const fine = (): number => {
      reading += 1;
      return reading * 0.000_005;
    };
    expect(probeClockResolutionUs(fine, 4)).toBeCloseTo(0.005, 6);
  });
});
