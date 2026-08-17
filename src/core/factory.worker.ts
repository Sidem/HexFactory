import init, { Factory } from "../../factory-wasm/pkg/factory_wasm.js";

import definitions from "../data/definitions.json";
import scenarios from "../data/scenarios.json";
import technologies from "../data/technologies.json";
import type {
  FactorySnapshot,
  FactorySnapshotDelta,
  LinePreviewCell,
  NativeFactory,
  NativeInputCommand,
  PlacementPreview,
} from "./types";

interface WorkerRequest {
  id: number;
  method: string;
  payload?: Record<string, unknown>;
}

interface WorkerScope {
  onmessage: ((event: MessageEvent<WorkerRequest>) => void) | null;
  postMessage(message: unknown): void;
}

const scope = self as unknown as WorkerScope;
let native: NativeFactory | null = null;
let operations = Promise.resolve();

scope.onmessage = (event) => {
  operations = operations.then(async () => {
    try {
      const result = await handle(event.data);
      scope.postMessage({ id: event.data.id, ok: true, result });
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
    ) as NativeFactory;
    return {
      snapshot: JSON.parse(native.snapshot_json()) as FactorySnapshot,
      revision: 0,
      // The player's walking cadence is native truth; the host only converts elapsed real time
      // into a step count with it.
      playerTicksPerSecond: Factory.playerTicksPerSecond(),
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
      );
      return delta(factory);
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

function delta(factory: NativeFactory): FactorySnapshotDelta {
  return JSON.parse(factory.snapshot_delta_json()) as FactorySnapshotDelta;
}

function optionalNumber(value: unknown): number | undefined {
  return typeof value === "number" ? value : undefined;
}
