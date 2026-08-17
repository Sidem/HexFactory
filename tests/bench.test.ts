import { describe, expect, it } from "vitest";

import {
  FRAME_BUDGET_US,
  TIER_COLUMNS,
  deltaMergeIsIntact,
  frameShare,
  mergeBrowserReport,
  probeClockResolutionUs,
  tierRow,
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
  it("keeps the crate's own account of what it measured", () => {
    const report = mergeBrowserReport(nativeReport, [hostTier], environment);
    expect(report.platform).toBe("wasm32");
    expect(report.schema).toBe(3);
    expect(report.crate_version).toBe("0.8.0");
    expect(report.environment).toEqual(environment);
    expect(report.tiers[0]?.host).toEqual(hostTier);
  });

  it("leaves a tier without a host measurement explicitly unmeasured", () => {
    const report = mergeBrowserReport(nativeReport, [], environment);
    expect(report.tiers[0]?.host).toBeNull();
    // An unmeasured host cost must read as absent, never as zero cost.
    const row = tierRow(report.tiers[0]!);
    expect(row.slice(-4)).toEqual(["—", "—", "—", "—"]);
  });

  it("pairs host results with their tier by key, not by position", () => {
    const other: HostTierResult = { ...hostTier, key: "medium" };
    const report = mergeBrowserReport(
      { ...nativeReport, tiers: [nativeTier] },
      [other, hostTier],
      environment,
    );
    expect(report.tiers[0]?.host?.key).toBe("small");
  });

  it("renders one cell per column", () => {
    const report = mergeBrowserReport(nativeReport, [hostTier], environment);
    expect(tierRow(report.tiers[0]!)).toHaveLength(TIER_COLUMNS.length);
    expect(tierRow(report.tiers[0]!)[0]).toBe("small");
  });

  it("states the host frame cost as a share of a 60 Hz frame", () => {
    expect(frameShare(FRAME_BUDGET_US)).toBe("100.0%");
    expect(frameShare(FRAME_BUDGET_US / 2)).toBe("50.0%");
    expect(frameShare(500)).toBe("3.0%");
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

describe("clock resolution probe", () => {
  it("reports the smallest step a quantized clock actually moves by", () => {
    // A clamped browser clock: readings advance only in whole 0.1 ms steps.
    let reading = 0;
    const clamped = (): number => {
      reading += 1;
      return Math.floor(reading / 3) * 0.1;
    };
    expect(probeClockResolutionUs(clamped, 4)).toBeCloseTo(100, 6);
  });

  it("reports a fine-grained clock as fine-grained", () => {
    let reading = 0;
    const fine = (): number => {
      reading += 1;
      return reading * 0.000_005;
    };
    expect(probeClockResolutionUs(fine, 4)).toBeCloseTo(0.005, 6);
  });
});
