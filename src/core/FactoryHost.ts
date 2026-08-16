import init, { Factory } from "../../factory-wasm/pkg/factory_wasm.js";

import definitionsJson from "../data/definitions.json";
import scenariosJson from "../data/scenarios.json";
import technologiesJson from "../data/technologies.json";
import { validateDefinitions, validateTechnologies } from "./definitions";
import type {
  Definitions,
  FactorySnapshot,
  NativeFactory,
  NativeInputCommand,
  PlacementPreview,
  Scenarios,
  Technologies,
} from "./types";

export class FactoryHost {
  readonly definitions: Definitions;
  readonly technologies: Technologies;
  readonly scenarios: Scenarios;
  private readonly native: NativeFactory;

  private constructor(native: NativeFactory) {
    this.native = native;
    this.definitions = definitionsJson as Definitions;
    this.technologies = technologiesJson as Technologies;
    this.scenarios = scenariosJson as Scenarios;
  }

  static async create(scenario = "new-game"): Promise<FactoryHost> {
    validateDefinitions(definitionsJson);
    validateTechnologies(technologiesJson, definitionsJson);
    await init();
    const native = new Factory(
      JSON.stringify(definitionsJson),
      JSON.stringify(technologiesJson),
      JSON.stringify(scenariosJson),
      scenario,
    ) as NativeFactory;
    return new FactoryHost(native);
  }

  static forTesting(native: NativeFactory): FactoryHost {
    return new FactoryHost(native);
  }

  tick(count = 1): FactorySnapshot {
    this.native.tick(count);
    return this.snapshot();
  }

  reset(): FactorySnapshot {
    this.native.reset();
    return this.snapshot();
  }

  newGame(scenario = "new-game", seed?: number): FactorySnapshot {
    this.native.new_game(scenario, seed);
    return this.snapshot();
  }

  apply(commands: NativeInputCommand[]): FactorySnapshot {
    this.native.apply_commands_json(JSON.stringify(commands));
    return this.snapshot();
  }

  placementPreview(
    q: number,
    r: number,
    definitionId: number,
    orientation: number,
  ): PlacementPreview {
    const definition = this.definitions.buildings.find(
      ({ id }) => id === definitionId,
    );
    const recipeId =
      definition?.kind === "composer"
        ? this.definitions.recipes[0]?.id
        : undefined;
    return JSON.parse(
      this.native.placement_preview_json(
        q,
        r,
        definitionId,
        orientation,
        recipeId,
      ),
    ) as PlacementPreview;
  }

  save(): string {
    return this.native.save_string();
  }

  load(save: string): FactorySnapshot {
    this.native.load_string(save);
    return this.snapshot();
  }

  snapshot(): FactorySnapshot {
    return JSON.parse(this.native.snapshot_json()) as FactorySnapshot;
  }

  dispose(): void {
    this.native.free();
  }
}
