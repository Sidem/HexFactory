import init, { Factory } from "../../factory-wasm/pkg/factory_wasm.js";

import blueprint from "../data/blueprint.json";
import definitionsJson from "../data/definitions.json";
import { validateDefinitions } from "./definitions";
import type { Definitions, FactorySnapshot, NativeFactory } from "./types";

export class FactoryHost {
  readonly definitions: Definitions;
  private readonly native: NativeFactory;

  private constructor(native: NativeFactory, definitions: Definitions) {
    this.native = native;
    this.definitions = definitions;
  }

  static async create(): Promise<FactoryHost> {
    validateDefinitions(definitionsJson);
    await init();
    const native = new Factory(
      JSON.stringify(definitionsJson),
      JSON.stringify(blueprint),
    ) as NativeFactory;
    return new FactoryHost(native, definitionsJson);
  }

  tick(count = 1): FactorySnapshot {
    this.native.tick(count);
    return this.snapshot();
  }

  reset(): FactorySnapshot {
    this.native.reset();
    return this.snapshot();
  }

  place(
    q: number,
    r: number,
    definitionId: number,
    orientation: number,
  ): FactorySnapshot {
    const definition = this.definitions.buildings.find(
      (candidate) => candidate.id === definitionId,
    );
    const recipeId =
      definition?.kind === "composer"
        ? this.definitions.recipes[0]?.id
        : undefined;
    this.native.place(q, r, definitionId, orientation, recipeId);
    return this.snapshot();
  }

  erase(q: number, r: number): FactorySnapshot {
    this.native.erase(q, r);
    return this.snapshot();
  }

  rotate(q: number, r: number): FactorySnapshot {
    this.native.rotate(q, r);
    return this.snapshot();
  }

  snapshot(): FactorySnapshot {
    return JSON.parse(this.native.snapshot_json()) as FactorySnapshot;
  }

  dispose(): void {
    this.native.free();
  }
}
