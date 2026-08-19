import { describe, expect, it } from "vitest";

import {
  compatibility,
  configFromEnvelope,
  describeMismatches,
  formatConfig,
  HXF1_PREFIX,
  importLegacySlots,
  latestCompatible,
  LEGACY_SAVE_PREFIX,
  parseHxf1,
  readCatalog,
  replaceNamedSlot,
  SAVE_CATALOG_KEY,
  SAVE_VERSION,
  slotFromPayload,
  type CurrentBuild,
  type StorageLike,
  writeCatalog,
} from "../src/core/saveSlots";

const continental = {
  elevation_coarse_cell: 24,
  water_level: 28000,
};

const build: CurrentBuild = {
  versions: {
    save: SAVE_VERSION,
    world: 6,
    definitions: 10,
    technology: 5,
  },
  scenarios: [
    { key: "new-game", name: "New game", version: 5 },
    { key: "factory-demo", name: "Factory demo", version: 5 },
  ],
  worldPresets: [
    { key: "continental", name: "Continental", params: continental },
  ],
};

function envelope(overrides: Record<string, unknown> = {}): string {
  return (
    HXF1_PREFIX +
    JSON.stringify({
      save_version: SAVE_VERSION,
      world_generator_version: 6,
      definition_version: 10,
      technology_version: 5,
      scenario_key: "new-game",
      scenario_version: 5,
      checksum: 1,
      state: { seed: 1213486160, world_params: continental },
      ...overrides,
    })
  );
}

function memoryStorage(initial: Record<string, string> = {}): StorageLike {
  const map = new Map(Object.entries(initial));
  return {
    get length() {
      return map.size;
    },
    getItem(key) {
      return map.get(key) ?? null;
    },
    setItem(key, value) {
      map.set(key, value);
    },
    removeItem(key) {
      map.delete(key);
    },
    key(index) {
      return [...map.keys()][index] ?? null;
    },
  };
}

describe("parseHxf1", () => {
  it("reads the versions and starting world from an envelope", () => {
    const parsed = parseHxf1(envelope());
    expect(parsed).toMatchObject({
      saveVersion: 10,
      worldVersion: 6,
      definitionVersion: 10,
      technologyVersion: 5,
      scenarioKey: "new-game",
      scenarioVersion: 5,
      seed: 1213486160,
    });
    expect(configFromEnvelope(parsed!, build)).toMatchObject({
      scenarioName: "New game",
      worldPreset: "continental",
      worldPresetName: "Continental",
      landformScale: 24,
      seaLevel: 28000,
    });
  });

  it("rejects a payload that is not HXF1 JSON", () => {
    expect(parseHxf1("not a save")).toBeNull();
    expect(parseHxf1(`${HXF1_PREFIX}{`)).toBeNull();
  });
});

describe("compatibility", () => {
  it("accepts a slot whose numbers match this build", () => {
    const parsed = parseHxf1(envelope())!;
    expect(compatibility(parsed, build).compatible).toBe(true);
  });

  it("names every number that would make native refuse the load", () => {
    const parsed = parseHxf1(
      envelope({
        save_version: 9,
        definition_version: 9,
        scenario_key: "gone",
      }),
    )!;
    const result = compatibility(parsed, build);
    expect(result.compatible).toBe(false);
    expect(describeMismatches(result.mismatches)).toContain(
      "Save format is 9; this build is 10.",
    );
    expect(describeMismatches(result.mismatches)).toContain(
      "Definitions is 9; this build is 10.",
    );
    expect(describeMismatches(result.mismatches)).toContain(
      "Scenario “gone” is not in this build.",
    );
  });

  it("refuses a scenario whose own version moved, even when the catalogue version did not", () => {
    const parsed = parseHxf1(envelope({ scenario_version: 4 }))!;
    const result = compatibility(parsed, build);
    expect(result.compatible).toBe(false);
    expect(describeMismatches(result.mismatches)).toBe(
      "Scenario new-game is 4; this build is 5.",
    );
  });
});

describe("catalog", () => {
  it("round-trips named slots and overwrites one that already has the name", () => {
    const storage = memoryStorage();
    const first = slotFromPayload(envelope(), "Landing", build, 1000, "a")!;
    const second = slotFromPayload(envelope(), "Landing", build, 2000, "b")!;
    writeCatalog(storage, replaceNamedSlot([first], second));
    const { slots } = readCatalog(storage);
    expect(slots).toHaveLength(1);
    expect(slots[0]?.id).toBe("a");
    expect(slots[0]?.savedAt).toBe(2000);
  });

  it("Continue is the newest compatible slot, not the newest row", () => {
    const compatible = slotFromPayload(envelope(), "Now", build, 100, "now")!;
    const older = slotFromPayload(
      envelope({ save_version: 9 }),
      "Then",
      build,
      999,
      "then",
    )!;
    expect(latestCompatible([compatible, older], build)?.id).toBe("now");
  });

  it("imports leftover versioned keys without deleting them", () => {
    const key = `${LEGACY_SAVE_PREFIX}v9w6d9t5s2`;
    const payload = envelope({ save_version: 9, definition_version: 9 });
    const storage = memoryStorage({ [key]: payload });
    const first = importLegacySlots(storage, build, 50);
    expect(first.imported).toBe(1);
    expect(first.slots[0]?.name).toBe("Previous run · save 9");
    expect(first.slots[0]?.sourceKey).toBe(key);
    expect(storage.getItem(key)).toBe(payload);
    expect(importLegacySlots(storage, build, 60).imported).toBe(0);
    expect(readCatalog(storage).slots).toHaveLength(1);
    storage.removeItem(key);
    writeCatalog(storage, []);
    expect(importLegacySlots(storage, build, 70).imported).toBe(0);
  });

  it("does not wipe a corrupt catalog", () => {
    const storage = memoryStorage({ [SAVE_CATALOG_KEY]: "{not-json" });
    const read = readCatalog(storage);
    expect(read.error).toBe("Save list could not be read.");
    expect(read.slots).toEqual([]);
    expect(storage.getItem(SAVE_CATALOG_KEY)).toBe("{not-json");
  });

  it("names a custom world from the parameters the envelope carried", () => {
    const payload = envelope({
      state: {
        seed: 7,
        world_params: { elevation_coarse_cell: 40, water_level: 1000 },
      },
    });
    const slot = slotFromPayload(payload, "Hills", build)!;
    expect(formatConfig(slot.config)).toBe(
      "New game · seed 7 · custom (land 40, sea 1000)",
    );
  });
});
