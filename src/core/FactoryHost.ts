import definitionsJson from "../data/definitions.json";
import scenariosJson from "../data/scenarios.json";
import technologiesJson from "../data/technologies.json";
import { validateDefinitions, validateTechnologies } from "./definitions";
import { applySnapshotDelta } from "./snapshotDelta";
import { decodeSnapshotDelta } from "./snapshotWire";
import type {
  BoundaryEdit,
  BoundaryPreview,
  Definitions,
  FactorySnapshot,
  FactorySnapshotDelta,
  LinePreviewCell,
  NativeInputCommand,
  PlacementPreview,
  Scenarios,
  Technologies,
  WorldParams,
  WorldPreset,
  WorldPreview,
  GroundEdit,
  GroundPreview,
} from "./types";

export type FactoryWorkerMethod =
  | "create"
  | "advance"
  | "reset"
  | "newGame"
  | "boundaryPreview"
  | "groundPreview"
  | "placementPreview"
  | "linePreview"
  | "save"
  | "load"
  | "worldParams"
  | "worldPreview";

/**
 * How a caller names the world a new game is generated with: a preset key, a complete parameter
 * set, or nothing at all, which means whatever the scenario names.
 */
export type WorldChoice = string | WorldParams;

export interface FactoryTransport {
  request<T>(method: FactoryWorkerMethod, payload?: unknown): Promise<T>;
  dispose(): void;
}

interface InitialSnapshot {
  snapshot: FactorySnapshot;
  revision: number;
  playerTicksPerSecond: number;
  worldPresets: WorldPreset[];
  worldParams: WorldParams;
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
        if (event.data.ok) {
          // A result that arrives as a buffer is a snapshot delta: it is the only thing the worker
          // sends in the binary wire format, and it is transferred rather than cloned. Decoding it
          // here keeps the encoding a property of the transport — {@link FactoryHost} is about
          // revisions and merging, and the tests drive it through a transport that hands over the
          // delta directly.
          const { result } = event.data;
          request.resolve(
            result instanceof ArrayBuffer
              ? decodeSnapshotDelta(result)
              : result,
          );
        } else
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
  /**
   * The shipped world presets, as native declares them. The new-world flow is built from this the
   * same way the catalogue is built from the definitions — a copy on this side could drift from
   * the generator it claims to describe.
   */
  readonly worldPresets: WorldPreset[];
  private revision: number;
  private currentWorldParams: WorldParams | null;

  private constructor(
    private readonly transport: FactoryTransport,
    private currentSnapshot: FactorySnapshot,
    revision: number,
    playerTicksPerSecond: number,
    worldPresets: WorldPreset[] = [],
    worldParams: WorldParams | null = null,
  ) {
    this.revision = revision;
    this.playerTicksPerSecond = playerTicksPerSecond;
    this.worldPresets = worldPresets;
    this.currentWorldParams = worldParams;
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
        initial.worldPresets,
        initial.worldParams,
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
    worldPresets: WorldPreset[] = [],
  ): FactoryHost {
    return new FactoryHost(
      transport,
      initial,
      revision,
      playerTicksPerSecond,
      worldPresets,
    );
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

  /**
   * `world` names the generation: a preset key, a complete parameter set, or nothing, which means
   * whatever the scenario names. The parameters the world actually came out with are read back
   * from native afterwards rather than assumed to be what was asked for.
   */
  async newGame(
    scenario = "new-game",
    seed?: number,
    world?: WorldChoice,
    creative = false,
  ): Promise<FactorySnapshot> {
    const snapshot = this.applyDelta(
      await this.transport.request<FactorySnapshotDelta>("newGame", {
        scenario,
        seed,
        worldParams: world,
        creative,
      }),
    );
    this.currentWorldParams = null;
    return snapshot;
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
    const snapshot = this.applyDelta(
      await this.transport.request<FactorySnapshotDelta>("load", { save }),
    );
    // A save carries its own parameters, so the loaded world is not necessarily the one that was
    // on screen a moment ago.
    this.currentWorldParams = null;
    return snapshot;
  }

  snapshot(): FactorySnapshot {
    return this.currentSnapshot;
  }

  /** Read-only native pricing and validation, shared with the eventual construction command. */
  boundaryPreview(edit: BoundaryEdit): Promise<BoundaryPreview> {
    return this.transport.request<BoundaryPreview>("boundaryPreview", edit);
  }

  /** The same for prepared ground: the transaction that prices the preview is the one that commits. */
  groundPreview(edit: GroundEdit): Promise<GroundPreview> {
    return this.transport.request<GroundPreview>("groundPreview", edit);
  }

  /**
   * The parameters the current world was generated from. Cached because it changes only when the
   * world does, and re-read from native rather than remembered from the request that asked for it.
   */
  async worldParams(): Promise<WorldParams> {
    if (!this.currentWorldParams) {
      this.currentWorldParams =
        await this.transport.request<WorldParams>("worldParams");
    }
    return this.currentWorldParams;
  }

  /**
   * A picture of a world nobody has generated yet, for a parameter set the player is still moving
   * sliders on. Answered by the same `terrain_at` a played hex goes through, so a preview and the
   * world the start button generates cannot disagree.
   *
   * Reads nothing about the run in progress and moves nothing: the current world is untouched, and
   * a set the start button would refuse is refused here too rather than drawn.
   */
  worldPreview(
    world: WorldChoice,
    seed: number,
    width: number,
    height: number,
    hexesAcross: number,
  ): Promise<WorldPreview> {
    return this.transport.request<WorldPreview>("worldPreview", {
      worldParams: world,
      seed,
      width,
      height,
      hexesAcross,
    });
  }

  /** The preset whose parameters these are, if any. A hand-tuned set matches none. */
  presetKeyFor(params: WorldParams): string | undefined {
    return this.worldPresets.find(
      (preset) => JSON.stringify(preset.params) === JSON.stringify(params),
    )?.key;
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
