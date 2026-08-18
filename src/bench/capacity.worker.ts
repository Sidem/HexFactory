/**
 * The capacity ladder's worker.
 *
 * It plays the same role `src/core/factory.worker.ts` plays for the game — the only place wasm is
 * instantiated, driven by ordered RPC — but loads the `--features bench` artifact, which is the
 * only build that carries the measurement code. The round-trip methods deliberately mirror the
 * game worker's `advance` exactly, because their cost is the thing being measured.
 */

import type { FactorySnapshot, NativeFactory } from "../core/types";
import { probeClockResolutionUs } from "./report";
import type { TierSpecSummary } from "./report";

/**
 * The bench artifact, built by `npm run build:wasm:bench` and never by the shipped build. It is
 * resolved against this module rather than the site root so the dev server's configured base
 * applies, and it is loaded dynamically so the repository typechecks without the artifact present.
 */
const BENCH_MODULE_URL = new URL(
  "../../factory-wasm/pkg-bench/factory_wasm_bench.js",
  import.meta.url,
).href;

/** The bounded idle batch a frame with no held key sends, identical to the native harness's. */
const IDLE_COMMANDS = JSON.stringify([{ type: "move_intent", x: 0, y: 0 }]);

interface NativeCapacityBench {
  tier_count(): number;
  tiers_json(): string;
  measure(index: number): string;
  factory(index: number): NativeFactory;
  report_json(): string;
}

interface BenchModule {
  default: () => Promise<unknown>;
  CapacityBench: new (quick: boolean) => NativeCapacityBench;
}

interface WorkerRequest {
  id: number;
  method: string;
  payload?: Record<string, unknown>;
}

interface WorkerScope {
  onmessage: ((event: MessageEvent<WorkerRequest>) => void) | null;
  postMessage(message: unknown, transfer?: Transferable[]): void;
}

const scope = self as unknown as WorkerScope;
let bench: NativeCapacityBench | null = null;
let roundTrip: NativeFactory | null = null;
let operations = Promise.resolve();

scope.onmessage = (event) => {
  operations = operations.then(async () => {
    try {
      const result = await handle(event.data);
      // Transferred exactly as the game worker transfers it, because the crossing is the thing
      // being measured.
      scope.postMessage(
        { id: event.data.id, ok: true, result },
        result instanceof ArrayBuffer ? [result] : [],
      );
    } catch (error) {
      scope.postMessage({
        id: event.data.id,
        ok: false,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  });
};

async function handle(request: WorkerRequest): Promise<unknown> {
  const payload = request.payload ?? {};
  if (request.method === "create") {
    if (bench) throw new Error("Capacity bench is already initialized");
    const module = await load();
    await module.default();
    bench = new module.CapacityBench(Boolean(payload.quick));
    return {
      tiers: JSON.parse(bench.tiers_json()) as TierSpecSummary[],
      clockResolutionUs: probeClockResolutionUs(() => performance.now()),
    };
  }

  const harness = requireBench();
  switch (request.method) {
    case "measure":
      return JSON.parse(harness.measure(Number(payload.index))) as unknown;
    case "roundTripStart": {
      roundTrip?.free();
      roundTrip = harness.factory(Number(payload.index));
      return JSON.parse(roundTrip.snapshot_json()) as FactorySnapshot;
    }
    case "roundTripFrame": {
      const factory = requireRoundTrip();
      // Exactly the game worker's advance: stringify the bounded batch, advance one tick, encode
      // the delta, and hand the buffer over. It parsed the delta and let the structured clone copy
      // it until the binary wire landed; the point of this method is to cost what the game costs,
      // so it moved with it.
      // No player steps: the capacity workload measures the factory, not the walk.
      factory.advance_json(IDLE_COMMANDS, 1, 0);
      const bytes = factory.snapshot_delta_bytes();
      return bytes.byteOffset === 0 &&
        bytes.byteLength === bytes.buffer.byteLength
        ? (bytes.buffer as ArrayBuffer)
        : (bytes.slice().buffer as ArrayBuffer);
    }
    case "roundTripEnd":
      roundTrip?.free();
      roundTrip = null;
      return null;
    case "report":
      return JSON.parse(harness.report_json()) as unknown;
    default:
      throw new Error(`Unknown capacity worker method: ${request.method}`);
  }
}

async function load(): Promise<BenchModule> {
  try {
    return (await import(/* @vite-ignore */ BENCH_MODULE_URL)) as BenchModule;
  } catch (error) {
    throw new Error(
      `Could not load ${BENCH_MODULE_URL} — run "npm run build:wasm:bench" first (${
        error instanceof Error ? error.message : String(error)
      })`,
    );
  }
}

function requireBench(): NativeCapacityBench {
  if (!bench) throw new Error("Capacity bench is not initialized");
  return bench;
}

function requireRoundTrip(): NativeFactory {
  if (!roundTrip) throw new Error("No round-trip factory is open");
  return roundTrip;
}
