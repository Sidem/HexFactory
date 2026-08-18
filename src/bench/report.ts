/**
 * Pure report assembly for the browser capacity harness.
 *
 * The native crate owns the measurement itself — the browser runs the same ladder through the same
 * Rust code, only with `performance.now` as its clock. This module owns the parts the host has to
 * contribute: the worker round trip the game actually pays per frame, the environment the numbers
 * were produced in, and the table the page draws.
 */

export interface TierSpecSummary {
  key: string;
  lines: number;
  entities: number;
  frames: number;
}

/** One tier as measured inside wasm. Mirrors the crate's `TierResult`. */
export interface NativeTierResult {
  key: string;
  lines: number;
  entities: number;
  tiles: number;
  chunks: number;
  measured_ticks: number;
  tick_us: number;
  ticks_per_second: number;
  snapshot_us: number;
  checksum_us: number;
  frame_us: number;
  frames_per_second: number;
  /** The binary wire payload the game ships, per frame. */
  delta_bytes: number;
  /** What the same frames would have cost as JSON, so the encoding's saving is measured here. */
  delta_json_bytes: number;
  full_compile_us: number;
  incremental_recompile_us: number;
  edit_us: number;
  checksum: number;
  delivered: number;
}

/** The crate's `Report`. */
export interface NativeReport {
  schema: number;
  crate_version: string;
  profile: string;
  platform: string;
  tiers: NativeTierResult[];
}

/**
 * What one frame costs the host, measured on the main thread around the ordinary worker RPC.
 *
 * `round_trip_us` covers everything the native `frame_us` cannot see: posting the bounded command
 * batch, the worker's own `JSON.parse`, the structured clone of the delta, and both scheduling
 * hops. `apply_us` is the main thread merging that delta into its cached snapshot.
 */
export interface HostTierResult {
  key: string;
  frames: number;
  round_trip_us: number;
  apply_us: number;
  /** `round_trip_us + apply_us`: one simulated frame, end to end, excluding rendering. */
  host_frame_us: number;
  /** The native checksum the last measured frame carried, recorded rather than asserted. */
  checksum: number;
  /** Buildings in the host's cached snapshot after the run, through the per-entity patch path. */
  applied_entities: number;
}

export interface BenchEnvironment {
  user_agent: string;
  hardware_concurrency: number;
  /** Cross-origin isolation lifts the browser's `performance.now` clamp, so a record must say. */
  cross_origin_isolated: boolean;
  main_clock_resolution_us: number;
  worker_clock_resolution_us: number;
  recorded: string;
}

export type MergedTierResult = NativeTierResult & {
  host: HostTierResult | null;
};

export interface BrowserReport {
  schema: number;
  crate_version: string;
  profile: string;
  platform: string;
  environment: BenchEnvironment;
  tiers: MergedTierResult[];
}

/** A 60 Hz frame in microseconds, the budget every verdict below is measured against. */
export const FRAME_BUDGET_US = 16_667;

/**
 * The smallest non-zero step the supplied clock reports, in microseconds.
 *
 * Browsers quantize `performance.now`, so a phase timed for less than a few steps is mostly
 * measuring the clock. Recording the observed step lets a reader tell which numbers those are.
 */
export function probeClockResolutionUs(
  nowMs: () => number,
  samples = 32,
): number {
  let smallest = Number.POSITIVE_INFINITY;
  for (let sample = 0; sample < samples; sample += 1) {
    const start = nowMs();
    let next = nowMs();
    // Spin until the clock reports a different value; that difference is one step.
    while (next === start) next = nowMs();
    smallest = Math.min(smallest, (next - start) * 1000);
  }
  return Number.isFinite(smallest) ? smallest : 0;
}

export function mergeBrowserReport(
  native: NativeReport,
  host: HostTierResult[],
  environment: BenchEnvironment,
): BrowserReport {
  const byKey = new Map(host.map((tier) => [tier.key, tier]));
  return {
    schema: native.schema,
    crate_version: native.crate_version,
    profile: native.profile,
    platform: native.platform,
    environment,
    tiers: native.tiers.map((tier) => ({
      ...tier,
      host: byKey.get(tier.key) ?? null,
    })),
  };
}

export const TIER_COLUMNS = [
  "tier",
  "lines",
  "entities",
  "tick µs",
  "snapshot µs",
  "checksum µs",
  "frame µs",
  "delta bytes",
  "json bytes",
  "compile µs",
  "recompile µs",
  "edit µs",
  "round trip µs",
  "apply µs",
  "host frame µs",
  "60 Hz share",
] as const;

export function tierRow(tier: MergedTierResult): string[] {
  return [
    tier.key,
    integer(tier.lines),
    integer(tier.entities),
    micros(tier.tick_us),
    micros(tier.snapshot_us),
    micros(tier.checksum_us),
    micros(tier.frame_us),
    integer(Math.round(tier.delta_bytes)),
    integer(Math.round(tier.delta_json_bytes)),
    micros(tier.full_compile_us),
    micros(tier.incremental_recompile_us),
    micros(tier.edit_us),
    tier.host ? micros(tier.host.round_trip_us) : "—",
    tier.host ? micros(tier.host.apply_us) : "—",
    tier.host ? micros(tier.host.host_frame_us) : "—",
    tier.host ? frameShare(tier.host.host_frame_us) : "—",
  ];
}

export function frameShare(microseconds: number): string {
  return `${((microseconds / FRAME_BUDGET_US) * 100).toFixed(1)}%`;
}

function micros(value: number): string {
  return value.toLocaleString("en-US", {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  });
}

function integer(value: number): string {
  return value.toLocaleString("en-US");
}

/**
 * The measured round trip is only a measurement of the real boundary if the delta it carries
 * actually rebuilds the blueprint on the main thread. Every tier's applied snapshot must end with
 * the entity count the tier was built with, or the host was timing a patch it never merged.
 */
export function deltaMergeIsIntact(report: BrowserReport): boolean {
  return report.tiers.every(
    (tier) =>
      tier.host === null || tier.host.applied_entities === tier.entities,
  );
}
