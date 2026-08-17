import definitionsJson from "../data/definitions.json";
import scenariosJson from "../data/scenarios.json";
import technologiesJson from "../data/technologies.json";
import { validateDefinitions, validateTechnologies } from "./definitions";
import { applySnapshotDelta } from "./snapshotDelta";
import type {
  Definitions,
  FactorySnapshot,
  FactorySnapshotDelta,
  LinePreviewCell,
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
  | "linePreview"
  | "save"
  | "load";

export interface FactoryTransport {
  request<T>(method: FactoryWorkerMethod, payload?: unknown): Promise<T>;
  dispose(): void;
}

interface InitialSnapshot {
  snapshot: FactorySnapshot;
  revision: number;
  playerTicksPerSecond: number;
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
  /**
   * The player's fixed walking cadence in steps per real second, reported by the native core. The
   * host converts elapsed real time into a step count with it and invents no rate of its own.
   */
  readonly playerTicksPerSecond: number;
  private revision: number;

  private constructor(
    private readonly transport: FactoryTransport,
    private currentSnapshot: FactorySnapshot,
    revision: number,
    playerTicksPerSecond: number,
  ) {
    this.revision = revision;
    this.playerTicksPerSecond = playerTicksPerSecond;
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
      return new FactoryHost(
        transport,
        initial.snapshot,
        initial.revision,
        initial.playerTicksPerSecond,
      );
    } catch (error) {
      transport.dispose();
      throw error;
    }
  }

  static forTesting(
    transport: FactoryTransport,
    initial: FactorySnapshot,
    revision = 0,
    playerTicksPerSecond = 30,
  ): FactoryHost {
    return new FactoryHost(transport, initial, revision, playerTicksPerSecond);
  }

  /**
   * One frame of native work. `ticks` is what the simulation speed is worth and `playerSteps` what
   * the frame's real time is worth, because the player walks on its own cadence.
   */
  async advance(
    commands: NativeInputCommand[],
    ticks: number,
    playerSteps = 0,
  ): Promise<FactorySnapshot> {
    return this.applyDelta(
      await this.transport.request<FactorySnapshotDelta>("advance", {
        commands,
        ticks,
        playerSteps,
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

  /**
   * Whether native would allow this placement. `recipeId` is the caller's — a machine's legality
   * depends on whether the recipe belongs to its category, so a preview that substituted a recipe
   * of its own would answer a different question from the placement it is previewing.
   */
  placementPreview(
    q: number,
    r: number,
    definitionId: number,
    orientation: number,
    recipeId?: number,
  ): Promise<PlacementPreview> {
    return this.transport.request<PlacementPreview>("placementPreview", {
      q,
      r,
      definitionId,
      orientation,
      recipeId,
    });
  }

  /**
   * The cells a drag between these endpoints would touch. Passing no `definitionId` previews a
   * removal drag. The host never resolves the path itself — see {@link LinePreviewCell}.
   */
  linePreview(
    q: number,
    r: number,
    toQ: number,
    toR: number,
    definitionId?: number,
    orientation = 0,
    recipeId?: number,
  ): Promise<LinePreviewCell[]> {
    return this.transport.request<LinePreviewCell[]>("linePreview", {
      q,
      r,
      toQ,
      toR,
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
