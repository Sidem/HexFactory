import { describe, expect, it } from "vitest";

import definitions from "../src/data/definitions.json";
import { validateDefinitions } from "../src/core/definitions";

describe("data definitions", () => {
  it("accepts the shipped dynamic item, recipe, and building IDs", () => {
    expect(() => validateDefinitions(definitions)).not.toThrow();
    expect(definitions.recipes[0]?.inputs).toEqual([
      { item_id: 1, quantity: 2 },
    ]);
    expect(definitions.recipes[0]?.output).toEqual({ item_id: 2, quantity: 1 });
  });

  it("rejects duplicate IDs and invalid recipe references", () => {
    const duplicate = structuredClone(definitions);
    duplicate.items[1]!.id = duplicate.items[0]!.id;
    expect(() => validateDefinitions(duplicate)).toThrow(/positive and unique/);

    const badReference = structuredClone(definitions);
    badReference.recipes[0]!.output.item_id = 999;
    expect(() => validateDefinitions(badReference)).toThrow(
      /invalid ingredient/,
    );
  });
});
