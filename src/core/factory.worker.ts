import init, { Factory } from "../../factory-wasm/pkg/factory_wasm.js";

import definitions from "../data/definitions.json";
import scenarios from "../data/scenarios.json";
import technologies from "../data/technologies.json";
import type {
  FactorySnapshot,
  LinePreviewCell,
  NativeFactory,
  NativeInputCommand,
  PlacementPreview,
  WorldParams,
  WorldPreset,
} from "./types";

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
let native: NativeFactory | null = null;
let operations = Promise.resolve();

scope.onmessage = (event) => {
  operations = operations.then(async () => {
    try {
      const result = await handle(event.data);
      // A delta is an `ArrayBuffer` and is handed over rather than copied. `docs/BENCHMARKS.md`
      // finding 3 measured the crossing at about 10 µs per kilobyte, and a structured clone of a
      // buffer this size is a straight memcpy of every one of them.
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
    if (native) throw new Error("Factory worker is already initialized");
    await init();
    native = new Factory(
      JSON.stringify(definitions),
      JSON.stringify(technologies),
      JSON.stringify(scenarios),
      String(payload.scenario ?? "new-game"),
      undefined,
      worldParamsJson(payload.worldParams),
    ) as NativeFactory;
    return {
      snapshot: JSON.parse(native.snapshot_json()) as FactorySnapshot,
      revision: 0,
      // The player's walking cadence is native truth; the host only converts elapsed real time
      // into a step count with it.
      playerTicksPerSecond: Factory.playerTicksPerSecond(),
      // The preset table is native's, the same way the catalogue is: the host renders it rather
      // than keeping a copy that can drift from the generator it describes.
      worldPresets: JSON.parse(Factory.world_presets_json()) as WorldPreset[],
      worldParams: JSON.parse(native.world_params_json()) as WorldParams,
    };
  }

  const factory = requireFactory();
  switch (request.method) {
    case "advance": {
      const commands = (payload.commands ?? []) as NativeInputCommand[];
      const ticks = Number(payload.ticks ?? 0);
      const playerSteps = Number(payload.playerSteps ?? 0);
      factory.advance_json(JSON.stringify(commands), ticks, playerSteps);
      return delta(factory);
    }
    case "reset":
      factory.reset();
      return delta(factory);
    case "newGame":
      factory.new_game(
        String(payload.scenario ?? "new-game"),
        optionalNumber(payload.seed),
        worldParamsJson(payload.worldParams),
      );
      return delta(factory);
    case "worldParams":
      // Not part of the per-frame delta: a world's parameters change only when the world does,
      // so the host asks after `newGame` and `load` rather than paying for them every frame.
      return JSON.parse(factory.world_params_json()) as WorldParams;
    case "placementPreview":
      return JSON.parse(
        factory.placement_preview_json(
          Number(payload.q),
          Number(payload.r),
          Number(payload.definitionId),
          Number(payload.orientation),
          optionalNumber(payload.recipeId),
        ),
      ) as PlacementPreview;
    case "linePreview":
      return JSON.parse(
        payload.definitionId === undefined
          ? factory.erase_line_preview_json(
              Number(payload.q),
              Number(payload.r),
              Number(payload.toQ),
              Number(payload.toR),
            )
          : factory.line_preview_json(
              Number(payload.q),
              Number(payload.r),
              Number(payload.toQ),
              Number(payload.toR),
              Number(payload.definitionId),
              Number(payload.orientation),
              optionalNumber(payload.recipeId),
            ),
      ) as LinePreviewCell[];
    case "save":
      return factory.save_string();
    case "load":
      factory.load_string(String(payload.save ?? ""));
      return delta(factory);
    default:
      throw new Error(`Unknown factory worker method: ${request.method}`);
  }
}

function requireFactory(): NativeFactory {
  if (!native) throw new Error("Factory worker is not initialized");
  return native;
}

/**
 * The delta as the buffer the host decodes, not as a parsed object.
 *
 * Nothing in the worker looks inside it, so decoding here and letting the structured clone rebuild
 * the object graph on the far side would be paying for the crossing twice. wasm-bindgen returns a
 * `Uint8Array` copied out of wasm memory, and its buffer is exactly this delta's, so handing over
 * the whole buffer transfers nothing extra.
 */
function delta(factory: NativeFactory): ArrayBuffer {
  const bytes = factory.snapshot_delta_bytes();
  // wasm-bindgen copies a returned `Vec<u8>` out of wasm memory into an array of its own, so the
  // buffer is this delta's alone and may be handed over whole. The check is the safety margin on
  // that: transferring a view *into* wasm memory would detach the module's heap, so anything that
  // is not a standalone buffer is copied instead of given away.
  return bytes.byteOffset === 0 && bytes.byteLength === bytes.buffer.byteLength
    ? (bytes.buffer as ArrayBuffer)
    : (bytes.slice().buffer as ArrayBuffer);
}

function optionalNumber(value: unknown): number | undefined {
  return typeof value === "number" ? value : undefined;
}

/**
 * What the host may name a world with: a preset key, or a complete parameter set. Native accepts
 * both and decides which it was given, so the two are one code path rather than two.
 */
function worldParamsJson(value: unknown): string | undefined {
  if (typeof value === "string") return JSON.stringify({ preset: value });
  if (value && typeof value === "object") return JSON.stringify(value);
  return undefined;
}
