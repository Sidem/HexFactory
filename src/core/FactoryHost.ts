import definitionsJson from "../data/definitions.json";
import scenariosJson from "../data/scenarios.json";
import technologiesJson from "../data/technologies.json";
import { validateDefinitions, validateTechnologies } from "./definitions";
import { applySnapshotDelta } from "./snapshotDelta";
import type {
  Definitions,
  FactorySnapshot,
  FactorySnapshotDelta,
  NativeInputCommand,
  PlacementPreview,
  Scenarios,
  Technologies,
} from "./types";

export type FactoryWorkerMethod =
  | "create"
  | "advance"
  | "reset"
  | "newGame"
  | "placementPreview"
  | "save"
  | "load";

export interface FactoryTransport {
  request<T>(method: FactoryWorkerMethod, payload?: unknown): Promise<T>;
  dispose(): void;
}

interface InitialSnapshot {
  snapshot: FactorySnapshot;
  revision: number;
}

interface WorkerResponse {
  id: number;
  ok: boolean;
  result?: unknown;
  error?: string;
}

class WorkerTransport implements FactoryTransport {
  private nextId = 1;
  private readonly pending = new Map<
    number,
    { resolve: (value: unknown) => void; reject: (reason: Error) => void }
  >();

  constructor(private readonly worker: Worker) {
    worker.addEventListener(
      "message",
      (event: MessageEvent<WorkerResponse>) => {
        const request = this.pending.get(event.data.id);
        if (!request) return;
        this.pending.delete(event.data.id);
        if (event.data.ok) request.resolve(event.data.result);
        else
          request.reject(
            new Error(event.data.error ?? "Factory worker failed"),
          );
      },
    );
    worker.addEventListener("error", (event) => {
      const error = new Error(event.message || "Factory worker crashed");
      for (const request of this.pending.values()) request.reject(error);
      this.pending.clear();
    });
  }

  request<T>(method: FactoryWorkerMethod, payload?: unknown): Promise<T> {
    const id = this.nextId;
    this.nextId += 1;
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, {
        resolve: (value) => resolve(value as T),
        reject,
      });
      this.worker.postMessage({ id, method, payload });
    });
  }

  dispose(): void {
    const error = new Error("Factory worker disposed");
    for (const request of this.pending.values()) request.reject(error);
    this.pending.clear();
    this.worker.terminate();
  }
}

export class FactoryHost {
  readonly definitions: Definitions;
  readonly technologies: Technologies;
  readonly scenarios: Scenarios;
  private revision: number;

  private constructor(
    private readonly transport: FactoryTransport,
    private currentSnapshot: FactorySnapshot,
    revision: number,
  ) {
    this.revision = revision;
    this.definitions = definitionsJson as Definitions;
    this.technologies = technologiesJson as Technologies;
    this.scenarios = scenariosJson as Scenarios;
  }

  static async create(scenario = "new-game"): Promise<FactoryHost> {
    validateDefinitions(definitionsJson);
    validateTechnologies(technologiesJson, definitionsJson);
    const transport = new WorkerTransport(
      new Worker(new URL("./factory.worker.ts", import.meta.url), {
        type: "module",
      }),
    );
    try {
      const initial = await transport.request<InitialSnapshot>("create", {
        scenario,
      });
      return new FactoryHost(transport, initial.snapshot, initial.revision);
    } catch (error) {
      transport.dispose();
      throw error;
    }
  }

  static forTesting(
    transport: FactoryTransport,
    initial: FactorySnapshot,
    revision = 0,
  ): FactoryHost {
    return new FactoryHost(transport, initial, revision);
  }

  async advance(
    commands: NativeInputCommand[],
    ticks: number,
  ): Promise<FactorySnapshot> {
    return this.applyDelta(
      await this.transport.request<FactorySnapshotDelta>("advance", {
        commands,
        ticks,
      }),
    );
  }

  async tick(count = 1): Promise<FactorySnapshot> {
    return this.advance([], count);
  }

  async reset(): Promise<FactorySnapshot> {
    return this.applyDelta(
      await this.transport.request<FactorySnapshotDelta>("reset"),
    );
  }

  async newGame(
    scenario = "new-game",
    seed?: number,
  ): Promise<FactorySnapshot> {
    return this.applyDelta(
      await this.transport.request<FactorySnapshotDelta>("newGame", {
        scenario,
        seed,
      }),
    );
  }

  placementPreview(
    q: number,
    r: number,
    definitionId: number,
    orientation: number,
  ): Promise<PlacementPreview> {
    const definition = this.definitions.buildings.find(
      ({ id }) => id === definitionId,
    );
    const recipeId =
      definition?.kind === "composer"
        ? this.definitions.recipes[0]?.id
        : undefined;
    return this.transport.request<PlacementPreview>("placementPreview", {
      q,
      r,
      definitionId,
      orientation,
      recipeId,
    });
  }

  save(): Promise<string> {
    return this.transport.request<string>("save");
  }

  async load(save: string): Promise<FactorySnapshot> {
    return this.applyDelta(
      await this.transport.request<FactorySnapshotDelta>("load", { save }),
    );
  }

  snapshot(): FactorySnapshot {
    return this.currentSnapshot;
  }

  dispose(): void {
    this.transport.dispose();
  }

  private applyDelta(delta: FactorySnapshotDelta): FactorySnapshot {
    const next = applySnapshotDelta(this.currentSnapshot, this.revision, delta);
    this.currentSnapshot = next.snapshot;
    this.revision = next.revision;
    return this.currentSnapshot;
  }
}
