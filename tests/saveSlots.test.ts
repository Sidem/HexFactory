import { describe, expect, it } from "vitest";

import {
  AUTOSAVE_SLOT_NAME,
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
  uniqueSlotName,
  catalogDocument,
  CATALOG_DOWNLOAD_NAME,
  fileStem,
  saveFileName,
  slotsFromFileText,
  unsavedRunAtRisk,
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
      saveVersion: SAVE_VERSION,
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

  it("offers the exact released migration chain to native", () => {
    const latest = {
      ...build,
      versions: {
        ...build.versions,
        save: 29,
        definitions: 23,
        technology: 12,
      },
    };
    // Real released scenarios changed at save 23 as well as technologies. Synthetic equal
    // scenario versions used to hide a picker that disabled every older but migratable factory.
    const releasedScenarios = {
      ...latest,
      scenarios: latest.scenarios.map((scenario) => ({
        ...scenario,
        version: 7,
      })),
    };
    for (const [saveVersion, definitionVersion, technologyVersion] of [
      [14, 14, 7],
      [15, 14, 7],
      [16, 15, 7],
      [17, 15, 8],
      [18, 16, 8],
      [19, 17, 8],
      [20, 18, 8],
      [21, 18, 9],
      [22, 18, 10],
      [23, 18, 11],
      [24, 19, 11],
      [25, 20, 11],
      [26, 21, 11],
      [27, 22, 11],
      [28, 22, 12],
    ] as const) {
      const parsed = parseHxf1(
        envelope({
          save_version: saveVersion,
          definition_version: definitionVersion,
          technology_version: technologyVersion,
        }),
      )!;
      expect(compatibility(parsed, latest).compatible).toBe(true);
      expect(
        compatibility({ ...parsed, definitionVersion: 99 }, latest).compatible,
      ).toBe(false);
      expect(
        compatibility({ ...parsed, technologyVersion: 99 }, latest).compatible,
      ).toBe(false);
      expect(
        compatibility(
          {
            ...parsed,
            scenarioVersion: saveVersion < 23 ? 5 : saveVersion < 25 ? 6 : 7,
          },
          releasedScenarios,
        ).compatible,
      ).toBe(true);
      expect(
        compatibility({ ...parsed, scenarioVersion: 4 }, releasedScenarios)
          .compatible,
      ).toBe(false);
      if (saveVersion >= 23)
        expect(
          compatibility({ ...parsed, scenarioVersion: 5 }, releasedScenarios)
            .compatible,
        ).toBe(false);
      if (saveVersion >= 25)
        expect(
          compatibility({ ...parsed, scenarioVersion: 6 }, releasedScenarios)
            .compatible,
        ).toBe(false);
    }
    const current = {
      ...build,
      versions: {
        ...build.versions,
        save: 18,
        definitions: 16,
        technology: 8,
      },
    };
    for (const save_version of [14, 15]) {
      const parsed = parseHxf1(
        envelope({
          save_version,
          definition_version: 14,
          technology_version: 7,
        }),
      )!;
      expect(compatibility(parsed, current)).toEqual({
        compatible: true,
        mismatches: [],
      });
    }
    const version16 = parseHxf1(
      envelope({
        save_version: 16,
        definition_version: 15,
        technology_version: 7,
      }),
    )!;
    expect(compatibility(version16, current)).toEqual({
      compatible: true,
      mismatches: [],
    });
    const version17 = parseHxf1(
      envelope({
        save_version: 17,
        definition_version: 15,
        technology_version: 8,
      }),
    )!;
    expect(compatibility(version17, current).compatible).toBe(true);
    expect(
      compatibility({ ...version17, definitionVersion: 14 }, current)
        .compatible,
    ).toBe(false);
    const unknown = parseHxf1(
      envelope({
        save_version: 13,
        definition_version: 14,
        technology_version: 7,
      }),
    )!;
    expect(compatibility(unknown, current).compatible).toBe(false);
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
      `Save format is 9; this build is ${SAVE_VERSION}.`,
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

  it("updates Auto-save slot without overwriting other named slots", () => {
    const storage = memoryStorage();
    const manual = slotFromPayload(
      envelope(),
      "Landing run",
      build,
      1000,
      "m1",
    )!;
    const auto1 = slotFromPayload(
      envelope(),
      AUTOSAVE_SLOT_NAME,
      build,
      1500,
      "a1",
    )!;
    const auto2 = slotFromPayload(
      envelope(),
      AUTOSAVE_SLOT_NAME,
      build,
      2500,
      "a2",
    )!;
    const initial = [manual, auto1];
    writeCatalog(storage, initial);
    const updated = replaceNamedSlot(readCatalog(storage).slots, auto2);
    writeCatalog(storage, updated);
    const { slots } = readCatalog(storage);
    expect(slots).toHaveLength(2);
    expect(slots.find((slot) => slot.name === "Landing run")?.savedAt).toBe(
      1000,
    );
    expect(
      slots.find((slot) => slot.name === AUTOSAVE_SLOT_NAME)?.savedAt,
    ).toBe(2500);
    expect(latestCompatible(slots, build)?.name).toBe(AUTOSAVE_SLOT_NAME);
  });

  it("steps a defaulted save name aside rather than over an existing factory", () => {
    const taken = [
      slotFromPayload(envelope(), "Landing run", build, 1000, "a")!,
      slotFromPayload(envelope(), "landing run 2", build, 2000, "b")!,
    ];
    expect(uniqueSlotName("Landing run", taken)).toBe("Landing run 3");
    expect(uniqueSlotName("Second landing", taken)).toBe("Second landing");
    expect(uniqueSlotName("Landing run", [])).toBe("Landing run");
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

describe("unsavedRunAtRisk", () => {
  const grace = 30_000;

  it("says nothing when the newest write already covers this tick", () => {
    expect(
      unsavedRunAtRisk({
        tick: 400,
        savedTick: 400,
        savedAt: 0,
        now: 10 * grace,
        graceMs: grace,
      }),
    ).toBe(false);
  });

  it("keeps quiet about the ticks a player runs up moments after saving", () => {
    expect(
      unsavedRunAtRisk({
        tick: 412,
        savedTick: 400,
        savedAt: 1_000,
        now: 1_000 + grace - 1,
        graceMs: grace,
      }),
    ).toBe(false);
  });

  it("asks once a whole grace window of factory is unwritten", () => {
    expect(
      unsavedRunAtRisk({
        tick: 412,
        savedTick: 400,
        savedAt: 1_000,
        now: 1_000 + grace,
        graceMs: grace,
      }),
    ).toBe(true);
  });

  it("stays quiet on an idle run however long the save has stood", () => {
    expect(
      unsavedRunAtRisk({
        tick: 400,
        savedTick: 412,
        savedAt: 0,
        now: 60 * grace,
        graceMs: grace,
      }),
    ).toBe(false);
  });
});

describe("desktop save files", () => {
  it("turns a slot name into a file name a desktop will accept", () => {
    expect(saveFileName("Landing run")).toBe("Landing run.hxf1");
    expect(saveFileName("Foo/bar:baz*qux")).toBe("Foo bar baz qux.hxf1");
    expect(saveFileName("  trailing.  ")).toBe("trailing.hxf1");
    expect(saveFileName("   ")).toBe("hexfactory-save.hxf1");
    expect(fileStem("C:\\\\Downloads\\\\Landing run.hxf1")).toBe("Landing run");
    expect(CATALOG_DOWNLOAD_NAME).toBe("hexfactory-saves.json");
  });

  it("round-trips one HXF1 file, including a BOM and a Windows line ending", () => {
    const payload = envelope();
    const fromNative = slotsFromFileText(payload, build, {
      fileName: "Landing run.hxf1",
      now: 50,
    });
    expect(fromNative.error).toBeUndefined();
    expect(fromNative.slots).toHaveLength(1);
    expect(fromNative.slots[0]?.name).toBe("Landing run");
    expect(fromNative.slots[0]?.payload).toBe(payload);

    const withBom = slotsFromFileText(`\uFEFF${payload}`, build, {
      fileName: "download.hxf1",
      now: 50,
    });
    expect(withBom.slots[0]?.name).toMatch(/^New game · /);
    expect(withBom.slots[0]?.payload.startsWith(HXF1_PREFIX)).toBe(true);

    const crlf = payload.replace("HXF1\n", "HXF1\r\n");
    const fromWindows = slotsFromFileText(crlf, build, {
      fileName: "Hills.hxf1",
      now: 50,
    });
    expect(fromWindows.slots[0]?.name).toBe("Hills");
    expect(parseHxf1(fromWindows.slots[0]!.payload)?.seed).toBe(1213486160);
  });

  it("imports the catalog JSON the browser already stores, skipping broken rows", () => {
    const good = slotFromPayload(envelope(), "Landing", build, 1000, "a")!;
    const document = catalogDocument([
      good,
      { name: "gone" } as unknown as typeof good,
    ]);
    const imported = slotsFromFileText(document, build, {
      fileName: "hexfactory-saves.json",
      now: 2000,
    });
    expect(imported.slots).toHaveLength(1);
    expect(imported.slots[0]?.name).toBe("Landing");
    expect(imported.slots[0]?.savedAt).toBe(1000);
    expect(imported.slots[0]?.id).not.toBe("a");
  });

  it("keeps a name that would not fit a file stem when the JSON wraps the payload", () => {
    const payload = envelope();
    const imported = slotsFromFileText(
      JSON.stringify({ name: "North / West", payload, savedAt: 9 }),
      build,
      { fileName: "hexfactory-save.hxf1", now: 1 },
    );
    expect(imported.slots[0]?.name).toBe("North / West");
    expect(imported.slots[0]?.savedAt).toBe(9);
  });

  it("reads a bare envelope body that lost the HXF1 marker", () => {
    const body = JSON.stringify({
      save_version: SAVE_VERSION,
      world_generator_version: 6,
      definition_version: 10,
      technology_version: 5,
      scenario_key: "new-game",
      scenario_version: 5,
      checksum: 1,
      state: { seed: 7, world_params: continental },
    });
    const imported = slotsFromFileText(body, build, {
      fileName: "untitled.json",
      now: 1,
    });
    expect(imported.slots[0]?.config.seed).toBe(7);
  });

  it("refuses empty and unrelated files rather than inventing a slot", () => {
    expect(slotsFromFileText("", build).error).toBe("The file is empty.");
    expect(slotsFromFileText("{}\n", build).error).toBe(
      "The file is not a HexFactory save.",
    );
    expect(slotsFromFileText('{"slots":[]}', build).error).toBe(
      "The file does not contain a save.",
    );
    expect(slotsFromFileText("not a save", build).error).toBe(
      "The file is not a HexFactory save.",
    );
  });
});
