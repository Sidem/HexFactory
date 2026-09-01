import { describe, expect, it } from "vitest";
import json from "../src/data/definitions.json";
import technologiesJson from "../src/data/technologies.json";
import type {
  Definitions,
  EntitySnapshot,
  Technologies,
} from "../src/core/types";
import { productionNote } from "../src/ui/production";
import { researchBenefits } from "../src/ui/researchGraph";
import {
  compatibility,
  parseHxf1,
  type CurrentBuild,
} from "../src/core/saveSlots";

const definitions = json as Definitions;
describe("petroleum player explanations", () => {
  it("names both outputs and explains the full-buffer remedy", () => {
    const entity = {
      recipe_id: 18,
      definition_id: 30,
      status: "composing",
      output_inventory: [],
    } as unknown as EntitySnapshot;
    expect(productionNote(entity, definitions)).toContain(
      "2 Bitumen + 2 Refined fuel",
    );
    entity.status = "output blocked";
    entity.output_inventory = [{ item_id: 30, quantity: 24 }];
    const note = productionNote(entity, definitions);
    expect(note).toContain("holding 24 Refined fuel");
    expect(note).toContain("Free space for the whole batch");
    expect(note).toContain("no inputs are consumed");

    // And it no longer tells the player to empty an ingredient slot to make room. The mixer holds a
    // full compartment's worth of gravel and is still short of bitumen. That used to be a wedge —
    // one shared ingredient budget — and the note explained the way out. Ingredient capacity is per
    // ingredient now, so the bitumen slot has the mixer's whole capacity waiting for it and there
    // is no remedy left to describe.
    const mixer = {
      recipe_id: 19,
      definition_id: 31,
      status: "waiting for inputs",
      input_inventory: [{ item_id: 17, quantity: 24 }],
    } as unknown as EntitySnapshot;
    expect(productionNote(mixer, definitions)).not.toContain(
      "Take some of the other ingredients",
    );
    expect(productionNote(mixer, definitions)).not.toContain(
      "shared by all ingredients",
    );
  });
  it("the atlas names the road itself, not just its production machine", () => {
    const technology = (
      technologiesJson as unknown as Technologies
    ).technologies.find((technology) => technology.id === 22)!;
    expect(researchBenefits(technology, definitions)).toEqual(
      expect.arrayContaining(["Asphalt road", "Asphalt mixer"]),
    );
  });
  it("offers Load for supported masonry saves, without admitting mismatched envelopes", () => {
    const build: CurrentBuild = {
      versions: { save: 32, world: 10, definitions: 26, technology: 14 },
      scenarios: [{ key: "new-game", name: "New game", version: 7 }],
      worldPresets: [],
    };
    const old = parseHxf1(
      "HXF1\n" +
        JSON.stringify({
          save_version: 31,
          world_generator_version: 9,
          definition_version: 25,
          technology_version: 13,
          scenario_key: "new-game",
          scenario_version: 7,
          state: { seed: 1 },
        }),
    )!;
    expect(compatibility(old, build).compatible).toBe(true);
    expect(
      compatibility({ ...old, definitionVersion: 99 }, build).compatible,
    ).toBe(false);
    expect(compatibility({ ...old, worldVersion: 6 }, build).compatible).toBe(
      false,
    );
  });
});
