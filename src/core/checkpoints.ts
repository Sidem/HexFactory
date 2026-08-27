/**
 * Run checkpoints — a host-side stopwatch that reads published snapshot facts and nothing else.
 *
 * This is presentation in the same sense the art generator is: every checkpoint is a predicate over
 * numbers native already publishes, none of it reaches the simulation, and deleting this file would
 * not move a checksum or a save. That separation is the point rather than a convenience. A timer
 * that could change what it measures would make every run it recorded unverifiable, which is the
 * one thing a speedrun clock must never be.
 *
 * Two clocks are recorded for every checkpoint because neither is sufficient alone. `tick` is
 * deterministic and survives a replay; `elapsedMs` says how long a person sat there. The simulation
 * rate is fixed, but keeping both catches missing wall-clock intervals such as a loaded save.
 */

import type { StorageLike } from "./saveSlots";
import { SIMULATION_TICKS_PER_SECOND } from "./frameClock";

/** Snapshot facts a checkpoint may ask about, flattened so predicates stay one-liners. */
export interface CheckpointContext {
  tick: number;
  /** How many contract stages the hub has finished. */
  contractStage: number;
  researchedCount: number;
  /** Carried quantity per item key. Absent keys are zero. */
  carried: Readonly<Record<string, number>>;
  buildings: readonly CheckpointBuilding[];
}

export interface CheckpointBuilding {
  /** Definition key, so a predicate names `composer` rather than the id it happens to have. */
  key: string;
  kind: string;
  status: string;
  /**
   * On a live network with electricity banked. A machine fills a buffer whenever its network can
   * afford it, so a positive bank is the difference between "built" and "plugged in" — which is
   * the distinction the extractor-before-power trap turns on.
   */
  powered: boolean;
}

export interface CheckpointDefinition {
  id: string;
  label: string;
  /** What this moment proves, shown in the panel and carried into the report. */
  note: string;
  /** An alternative opening route may finish without this milestone. */
  optional?: boolean;
  reached(context: CheckpointContext): boolean;
}

export interface CheckpointRecord {
  id: string;
  tick: number;
  /** Real milliseconds from the first frame of the run to this checkpoint. */
  elapsedMs: number;
}

/**
 * Why a run's wall clock cannot be compared against another's. Kept as reasons rather than one
 * boolean so a report can say which guarantee was lost instead of only that one was.
 * `speed-changed` remains readable for reports created before simulation speed became fixed.
 */
export type RunTaint = "speed-changed" | "loaded-save" | "creative";

export interface RunTimings {
  /** Epoch milliseconds, for naming the run rather than for timing it. */
  startedAt: number;
  startedTick: number;
  /** Fixed simulation rate, retained in the stored report format for older runs. */
  startedSpeed: number;
  taints: RunTaint[];
  records: CheckpointRecord[];
}

export const RUN_STORAGE_KEY = "hexfactory:run:v1";

/**
 * Opening milestones for both the primitive and industrial routes. Stable IDs preserve older
 * reports; optional industrial steps must not make a completed manual opening read unfinished.
 */
export const OPENING_CHECKPOINTS: readonly CheckpointDefinition[] = [
  {
    id: "first-iron",
    label: "First iron ore",
    note: "The hand alone. Iron is 45 steps, so this is 1.5 s of held action plus the walk out.",
    reached: (context) => (context.carried.ore ?? 0) > 0,
  },
  {
    id: "first-research",
    optional: true,
    label: "First technology researched",
    note: "Field Logistics costs 3 insight and gates both extraction and power.",
    reached: (context) => context.researchedCount > 0,
  },
  {
    id: "first-extraction",
    optional: true,
    label: "First extractor producing",
    note: "Needs 12 insight across three technologies, a burner, and coal in it. An extractor with no network banks nothing and browns out.",
    // `extracting` is only published while progress is above zero, so an extractor emits one idle
    // frame per cadence and a snapshot could land on it. `output blocked` is the same proof read
    // from the other side: the machine is holding something it made. Either answers the question,
    // and between them there is no frame where a working extractor reads as an unbuilt one.
    reached: (context) =>
      context.buildings.some(
        (building) =>
          building.kind === "extractor" &&
          (building.status === "extracting" ||
            building.status === "output blocked"),
      ),
  },
  {
    id: "first-crystal",
    optional: true,
    label: "First signal crystal",
    note: "No hand rate exists for it. This is an expedition plus power delivered to wherever the seam is.",
    reached: (context) => (context.carried.crystal ?? 0) > 0,
  },
  {
    id: "composer-live",
    optional: true,
    label: "Composer built and powered",
    note: "Costs 2 iron plate, a gear and a frame, and 8 more insight to unlock. Crystal is no longer on the way here.",
    reached: (context) =>
      context.buildings.some(
        (building) => building.key === "composer" && building.powered,
      ),
  },
  {
    id: "first-workshop",
    label: "Manual workshop built",
    optional: true,
    note: "Four wood and two stone; no insight or power. Materials and walking count in the real run clock.",
    reached: (context) =>
      context.buildings.some((building) => building.key === "manual-workshop"),
  },
  {
    id: "first-timber",
    label: "First timber carried",
    optional: true,
    note: "One wood makes two timber; a manual batch takes 24 factory ticks of attended work.",
    reached: (context) => (context.carried.timber ?? 0) > 0,
  },
  {
    id: "first-plate",
    label: "First iron plate carried",
    optional: true,
    note: "The primitive furnace uses ordinary fuel without electricity; this records withdrawal as well as production.",
    reached: (context) => (context.carried["iron-plate"] ?? 0) > 0,
  },
  {
    id: "stage-one",
    label: "Stage 1 complete — three components delivered",
    note: "Three manual batches need six iron ore and 96 factory ticks of player work. Research and a crystal expedition are no longer required for this stage.",
    reached: (context) => context.contractStage >= 1,
  },
];

export function startRun(startedAt: number, startedTick: number): RunTimings {
  return {
    startedAt,
    startedTick,
    startedSpeed: SIMULATION_TICKS_PER_SECOND,
    taints: [],
    records: [],
  };
}

/** Add a taint once. Repeating a reason tells a reader nothing the first mention did not. */
export function taintRun(run: RunTimings, taint: RunTaint): RunTimings {
  if (run.taints.includes(taint)) return run;
  return { ...run, taints: [...run.taints, taint] };
}

/**
 * Latch every checkpoint whose predicate is newly true.
 *
 * Latching is the whole contract: a run that reached `first-iron` and then spent the ore has still
 * reached it, so a recorded id is never re-evaluated and never withdrawn. That also makes the pass
 * cheap enough to run on every snapshot, since a finished run tests nothing.
 */
export function recordCheckpoints(
  run: RunTimings,
  context: CheckpointContext,
  elapsedMs: number,
  checkpoints: readonly CheckpointDefinition[] = OPENING_CHECKPOINTS,
): { run: RunTimings; reached: CheckpointRecord[] } {
  const already = new Set(run.records.map((record) => record.id));
  const reached: CheckpointRecord[] = [];
  for (const checkpoint of checkpoints) {
    if (already.has(checkpoint.id)) continue;
    if (!checkpoint.reached(context)) continue;
    reached.push({
      id: checkpoint.id,
      tick: context.tick,
      elapsedMs: Math.max(0, Math.round(elapsedMs)),
    });
  }
  if (reached.length === 0) return { run, reached };
  return { run: { ...run, records: [...run.records, ...reached] }, reached };
}

export function isRunComplete(
  run: RunTimings,
  checkpoints: readonly CheckpointDefinition[] = OPENING_CHECKPOINTS,
): boolean {
  const reached = new Set(run.records.map((record) => record.id));
  return checkpoints.every(
    (checkpoint) => checkpoint.optional || reached.has(checkpoint.id),
  );
}

/** `mm:ss.d`, which is the resolution a human stopwatch was ever going to deliver anyway. */
export function formatElapsed(elapsedMs: number): string {
  const total = Math.max(0, elapsedMs) / 1000;
  const minutes = Math.floor(total / 60);
  const seconds = total - minutes * 60;
  return `${minutes}:${seconds.toFixed(1).padStart(4, "0")}`;
}

/**
 * The report, as plain text meant to be pasted somewhere else. It leads with the conditions so a
 * loaded save or creative run cannot be mistaken for a clean timed opening.
 */
export function formatRunReport(
  run: RunTimings,
  checkpoints: readonly CheckpointDefinition[] = OPENING_CHECKPOINTS,
): string {
  const lines: string[] = ["HexFactory run report"];
  lines.push(`started: ${new Date(run.startedAt).toISOString()}`);
  lines.push(`sim speed: ${run.startedSpeed} tps`);
  if (run.taints.length > 0)
    lines.push(`NOT COMPARABLE: ${run.taints.map(taintReason).join("; ")}`);
  lines.push("");
  const byId = new Map(run.records.map((record) => [record.id, record]));
  for (const checkpoint of checkpoints) {
    const record = byId.get(checkpoint.id);
    lines.push(
      record
        ? `${formatElapsed(record.elapsedMs).padStart(7)}  tick ${String(record.tick).padStart(6)}  ${checkpoint.label}`
        : `${"--:--".padStart(7)}  ${"".padStart(11)}  ${checkpoint.label} (not reached)`,
    );
  }
  const splits = splitDurations(run, checkpoints);
  if (splits.length > 0) {
    lines.push("");
    lines.push("splits:");
    for (const split of splits)
      lines.push(
        `  ${formatElapsed(split.elapsedMs).padStart(7)}  ${split.label}`,
      );
  }
  return lines.join("\n");
}

function taintReason(taint: RunTaint): string {
  switch (taint) {
    case "speed-changed":
      return "simulation speed changed mid-run";
    case "loaded-save":
      return "a save was loaded, so the clock missed part of the run";
    case "creative":
      return "creative mode was used, so nothing here had to be earned";
  }
}

/**
 * Read a run back, defensively. Storage is the one input here that another build wrote, so a shape
 * that does not parse is discarded rather than repaired: a run with half its records missing would
 * report times that never happened, and no run at all is the honest answer to a corrupt one.
 */
export function readRun(storage: StorageLike): RunTimings | null {
  const raw = storage.getItem(RUN_STORAGE_KEY);
  if (!raw) return null;
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return null;
    const run = parsed as Partial<RunTimings>;
    if (
      typeof run.startedAt !== "number" ||
      typeof run.startedTick !== "number" ||
      typeof run.startedSpeed !== "number" ||
      !Array.isArray(run.records) ||
      !Array.isArray(run.taints)
    )
      return null;
    const records = run.records.filter(
      (record): record is CheckpointRecord =>
        !!record &&
        typeof record.id === "string" &&
        typeof record.tick === "number" &&
        typeof record.elapsedMs === "number",
    );
    return {
      startedAt: run.startedAt,
      startedTick: run.startedTick,
      startedSpeed: run.startedSpeed,
      taints: run.taints.filter(
        (taint): taint is RunTaint =>
          taint === "speed-changed" ||
          taint === "loaded-save" ||
          taint === "creative",
      ),
      records,
    };
  } catch {
    return null;
  }
}

export function writeRun(storage: StorageLike, run: RunTimings): void {
  storage.setItem(RUN_STORAGE_KEY, JSON.stringify(run));
}

export function clearRun(storage: StorageLike): void {
  storage.removeItem(RUN_STORAGE_KEY);
}

/**
 * Time spent between one checkpoint and the next. The cumulative column answers "how long is the
 * opening"; this one answers "which leg is the problem", and they are rarely the same question.
 */
export function splitDurations(
  run: RunTimings,
  checkpoints: readonly CheckpointDefinition[] = OPENING_CHECKPOINTS,
): { label: string; elapsedMs: number }[] {
  const byId = new Map(run.records.map((record) => [record.id, record]));
  const splits: { label: string; elapsedMs: number }[] = [];
  let previous = 0;
  let previousLabel = "start";
  const chronological = checkpoints
    .filter((checkpoint) => byId.has(checkpoint.id))
    .sort((a, b) => byId.get(a.id)!.elapsedMs - byId.get(b.id)!.elapsedMs);
  for (const checkpoint of chronological) {
    const record = byId.get(checkpoint.id);
    if (!record) break;
    splits.push({
      label: `${previousLabel} -> ${checkpoint.label}`,
      elapsedMs: record.elapsedMs - previous,
    });
    previous = record.elapsedMs;
    previousLabel = checkpoint.label;
  }
  return splits;
}
