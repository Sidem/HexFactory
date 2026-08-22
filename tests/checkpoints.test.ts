import { describe, expect, it } from "vitest";

import {
  formatElapsed,
  formatRunReport,
  isRunComplete,
  OPENING_CHECKPOINTS,
  readRun,
  recordCheckpoints,
  RUN_STORAGE_KEY,
  splitDurations,
  startRun,
  taintRun,
  writeRun,
  type CheckpointContext,
} from "../src/core/checkpoints";
import type { StorageLike } from "../src/core/saveSlots";

function context(
  overrides: Partial<CheckpointContext> = {},
): CheckpointContext {
  return {
    tick: 0,
    contractStage: 0,
    researchedCount: 0,
    carried: {},
    buildings: [],
    ...overrides,
  };
}

function memoryStorage(): StorageLike {
  const entries = new Map<string, string>();
  return {
    get length() {
      return entries.size;
    },
    getItem: (key) => entries.get(key) ?? null,
    setItem: (key, value) => void entries.set(key, value),
    removeItem: (key) => void entries.delete(key),
    key: (index) => [...entries.keys()][index] ?? null,
  };
}

describe("opening checkpoints", () => {
  it("latches the first iron and keeps it after the ore is spent", () => {
    let run = startRun(0, 0, 10);
    const gathered = recordCheckpoints(
      run,
      context({ tick: 40, carried: { ore: 1 } }),
      1_500,
    );
    run = gathered.run;
    expect(gathered.reached.map(({ id }) => id)).toEqual(["first-iron"]);

    // Spending the ore must not withdraw a checkpoint the run genuinely reached.
    const spent = recordCheckpoints(
      run,
      context({ tick: 90, carried: {} }),
      4_000,
    );
    expect(spent.reached).toEqual([]);
    expect(spent.run.records).toHaveLength(1);
    expect(spent.run.records[0]).toMatchObject({ id: "first-iron", tick: 40 });
  });

  it("does not count a built extractor until it is actually producing", () => {
    const run = startRun(0, 0, 10);
    const brownout = recordCheckpoints(
      run,
      context({
        buildings: [
          {
            key: "extractor",
            kind: "extractor",
            status: "brownout",
            powered: false,
          },
        ],
      }),
      1_000,
    );
    expect(brownout.reached).toEqual([]);

    const running = recordCheckpoints(
      run,
      context({
        buildings: [
          {
            key: "extractor",
            kind: "extractor",
            status: "extracting",
            powered: true,
          },
        ],
      }),
      2_000,
    );
    expect(running.reached.map(({ id }) => id)).toEqual(["first-extraction"]);
  });

  it("counts an output-blocked extractor, which is holding what it made", () => {
    // `extracting` is published only while progress is above zero, so a working extractor shows one
    // idle frame per cadence. A blocked one has already produced, which is the same proof.
    const result = recordCheckpoints(
      startRun(0, 0, 10),
      context({
        buildings: [
          {
            key: "extractor",
            kind: "extractor",
            status: "output blocked",
            powered: true,
          },
        ],
      }),
      2_000,
    );
    expect(result.reached.map(({ id }) => id)).toEqual(["first-extraction"]);
  });

  it("requires a composer to be powered, not merely placed", () => {
    const run = startRun(0, 0, 10);
    const placed = recordCheckpoints(
      run,
      context({
        buildings: [
          { key: "composer", kind: "composer", status: "idle", powered: false },
        ],
      }),
      1_000,
    );
    expect(placed.reached).toEqual([]);
  });

  it("records every checkpoint the same snapshot satisfies at once", () => {
    const run = startRun(0, 0, 10);
    const result = recordCheckpoints(
      run,
      context({
        tick: 900,
        contractStage: 1,
        researchedCount: 4,
        carried: { ore: 6, crystal: 3 },
        buildings: [
          {
            key: "extractor",
            kind: "extractor",
            status: "extracting",
            powered: true,
          },
          {
            key: "composer",
            kind: "composer",
            status: "crafting",
            powered: true,
          },
        ],
      }),
      600_000,
    );
    expect(result.reached).toHaveLength(OPENING_CHECKPOINTS.length);
    expect(isRunComplete(result.run)).toBe(true);
  });

  it("keeps taints unique so a report names a reason once", () => {
    let run = startRun(0, 0, 10);
    run = taintRun(run, "speed-changed");
    run = taintRun(run, "speed-changed");
    run = taintRun(run, "loaded-save");
    expect(run.taints).toEqual(["speed-changed", "loaded-save"]);
  });
});

describe("run reporting", () => {
  it("formats elapsed time as minutes and tenths", () => {
    expect(formatElapsed(0)).toBe("0:00.0");
    expect(formatElapsed(9_400)).toBe("0:09.4");
    expect(formatElapsed(605_000)).toBe("10:05.0");
  });

  it("reports splits between consecutive checkpoints", () => {
    let run = startRun(0, 0, 10);
    run = recordCheckpoints(run, context({ carried: { ore: 1 } }), 10_000).run;
    run = recordCheckpoints(run, context({ researchedCount: 1 }), 25_000).run;
    const splits = splitDurations(run);
    expect(splits).toHaveLength(2);
    expect(splits[0]?.elapsedMs).toBe(10_000);
    expect(splits[1]?.elapsedMs).toBe(15_000);
  });

  it("leads a tainted report with the reason it cannot be compared", () => {
    let run = startRun(Date.UTC(2026, 0, 1), 0, 30);
    run = taintRun(run, "speed-changed");
    const report = formatRunReport(run);
    expect(report).toContain("NOT COMPARABLE");
    expect(report).toContain("simulation speed changed mid-run");
    // Every checkpoint is still listed, so an unfinished run reads as a ladder rather than a gap.
    expect(report).toContain("(not reached)");
  });
});

describe("run storage", () => {
  it("round-trips a run", () => {
    const storage = memoryStorage();
    let run = startRun(1_000, 5, 10);
    run = recordCheckpoints(run, context({ carried: { ore: 1 } }), 3_000).run;
    writeRun(storage, run);
    expect(readRun(storage)).toEqual(run);
  });

  it("discards a stored run whose shape does not parse", () => {
    const storage = memoryStorage();
    storage.setItem(RUN_STORAGE_KEY, "{not json");
    expect(readRun(storage)).toBeNull();
    storage.setItem(RUN_STORAGE_KEY, JSON.stringify({ startedAt: "soon" }));
    expect(readRun(storage)).toBeNull();
  });

  it("drops malformed records rather than reporting times that never happened", () => {
    const storage = memoryStorage();
    storage.setItem(
      RUN_STORAGE_KEY,
      JSON.stringify({
        startedAt: 0,
        startedTick: 0,
        startedSpeed: 10,
        taints: ["speed-changed", "nonsense"],
        records: [
          { id: "first-iron", tick: 10, elapsedMs: 500 },
          { id: "first-research", tick: "later" },
        ],
      }),
    );
    const run = readRun(storage);
    expect(run?.records).toHaveLength(1);
    expect(run?.taints).toEqual(["speed-changed"]);
  });
});
