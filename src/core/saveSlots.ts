/**
 * Named local saves. The catalog key does not carry versions: hiding a slot when a number moved
 * made Continue say there was no save, which is how a factory disappeared after a Pages deploy.
 * Each slot records the envelope versions and the world it was started with; incompatibilities
 * stay visible, and native still refuses a load the numbers cannot support. Desktop export is the
 * same HXF1 string (or the catalog JSON); import mints new ids so a file cannot collide with a
 * slot already in the list.
 *
 * `SAVE_VERSION` is the one literal because native does not publish it. Keep it aligned with
 * `factory-wasm/src/lib.rs`. v11 carries `request_fills` beside `request_rounds`; v12 carries
 * `disabled` on every entity, so a factory saved with a machine switched off comes back switched
 * off rather than quietly resuming. v13 carries the creative flag and the pack size, which a
 * creative run can widen — both are run state now rather than fixed properties of the scenario.
 */

export const SAVE_VERSION = 16;
export const SAVE_CATALOG_KEY = "hexfactory:saves:v1";
export const LEGACY_SAVE_PREFIX = "hexfactory:hxf1:";
export const HXF1_PREFIX = "HXF1\n";
export const AUTOSAVE_SLOT_NAME = "Auto-save";
/** One factory, as native wrote it. The catalog JSON is the other downloadable shape. */
export const SAVE_FILE_SUFFIX = ".hxf1";
export const CATALOG_DOWNLOAD_NAME = "hexfactory-saves.json";

export interface StorageLike {
  readonly length: number;
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
  key(index: number): string | null;
}

export interface GameVersions {
  save: number;
  world: number;
  definitions: number;
  technology: number;
  scenario: number;
}

export interface RememberedConfig {
  seed: number;
  scenarioKey: string;
  scenarioName: string;
  worldPreset?: string;
  worldPresetName?: string;
  landformScale?: number;
  seaLevel?: number;
}

export interface SaveSlot {
  id: string;
  name: string;
  savedAt: number;
  payload: string;
  versions: GameVersions;
  config: RememberedConfig;
  /** Legacy localStorage key this slot was imported from, if any. */
  sourceKey?: string;
}

export interface VersionMismatch {
  field: string;
  expected: string;
  found: string;
}

export interface SlotCompatibility {
  compatible: boolean;
  mismatches: VersionMismatch[];
}

export interface CatalogScenario {
  key: string;
  name: string;
  version: number;
}

export interface CatalogPreset {
  key: string;
  name: string;
  params: unknown;
}

export interface CurrentBuild {
  versions: Omit<GameVersions, "scenario">;
  scenarios: readonly CatalogScenario[];
  worldPresets: readonly CatalogPreset[];
}

export interface ParsedEnvelope {
  saveVersion: number;
  worldVersion: number;
  definitionVersion: number;
  technologyVersion: number;
  scenarioKey: string;
  scenarioVersion: number;
  seed: number;
  worldParams: Record<string, unknown> | null;
}

export interface CatalogRead {
  slots: SaveSlot[];
  error?: string;
}

export interface FileImport {
  slots: SaveSlot[];
  error?: string;
}

export interface FileImportOptions {
  fileName?: string;
  now?: number;
}

const CATALOG_FILE_VERSION = 1;

export function parseHxf1(payload: string): ParsedEnvelope | null {
  if (!payload.startsWith(HXF1_PREFIX)) return null;
  try {
    const body: unknown = JSON.parse(payload.slice(HXF1_PREFIX.length));
    if (!isRecord(body)) return null;
    const saveVersion = asUint(body.save_version);
    const worldVersion = asUint(body.world_generator_version);
    const definitionVersion = asUint(body.definition_version);
    const technologyVersion = asUint(body.technology_version);
    const scenarioVersion = asUint(body.scenario_version);
    const scenarioKey = asString(body.scenario_key);
    if (
      saveVersion === undefined ||
      worldVersion === undefined ||
      definitionVersion === undefined ||
      technologyVersion === undefined ||
      scenarioVersion === undefined ||
      scenarioKey === undefined
    ) {
      return null;
    }
    const state = isRecord(body.state) ? body.state : {};
    const seed = asUint(state.seed) ?? 0;
    const worldParams = isRecord(state.world_params)
      ? state.world_params
      : null;
    return {
      saveVersion,
      worldVersion,
      definitionVersion,
      technologyVersion,
      scenarioKey,
      scenarioVersion,
      seed,
      worldParams,
    };
  } catch {
    return null;
  }
}

export function compatibility(
  envelope: ParsedEnvelope,
  build: CurrentBuild,
): SlotCompatibility {
  const mismatches: VersionMismatch[] = [];
  // Native has explicit adjacent migrations from the two definition-14 releases into the current
  // compartment-storage envelope. The catalog has to mirror that narrow entrance or it disables
  // Load before native can perform the migration. This is deliberately a pair, not "older is OK":
  // every other historical shape remains a refusal at the native boundary.
  const migratesToCompartmentStorage =
    build.versions.save === 16 &&
    build.versions.definitions === 15 &&
    (envelope.saveVersion === 14 || envelope.saveVersion === 15) &&
    envelope.definitionVersion === 14;
  const expect = (
    field: string,
    expected: string | number,
    found: string | number,
  ): void => {
    if (String(expected) === String(found)) return;
    mismatches.push({
      field,
      expected: String(expected),
      found: String(found),
    });
  };
  if (!migratesToCompartmentStorage) {
    expect("save format", build.versions.save, envelope.saveVersion);
    expect(
      "definitions",
      build.versions.definitions,
      envelope.definitionVersion,
    );
  }
  expect("world generator", build.versions.world, envelope.worldVersion);
  expect("technology", build.versions.technology, envelope.technologyVersion);
  const scenario = build.scenarios.find(
    (entry) => entry.key === envelope.scenarioKey,
  );
  if (!scenario) {
    mismatches.push({
      field: "scenario",
      expected: "a shipped scenario",
      found: envelope.scenarioKey,
    });
  } else {
    expect(
      `scenario ${scenario.key}`,
      scenario.version,
      envelope.scenarioVersion,
    );
  }
  return { compatible: mismatches.length === 0, mismatches };
}

export function describeMismatches(mismatches: VersionMismatch[]): string {
  return mismatches
    .map((item) => {
      if (item.expected === "a shipped scenario") {
        return `Scenario “${item.found}” is not in this build.`;
      }
      return `${capitalize(item.field)} is ${item.found}; this build is ${item.expected}.`;
    })
    .join(" ");
}

export function configFromEnvelope(
  envelope: ParsedEnvelope,
  build: CurrentBuild,
): RememberedConfig {
  const scenario = build.scenarios.find(
    (entry) => entry.key === envelope.scenarioKey,
  );
  const preset = envelope.worldParams
    ? build.worldPresets.find(
        (entry) =>
          JSON.stringify(entry.params) === JSON.stringify(envelope.worldParams),
      )
    : undefined;
  return {
    seed: envelope.seed,
    scenarioKey: envelope.scenarioKey,
    scenarioName: scenario?.name ?? envelope.scenarioKey,
    worldPreset: preset?.key,
    worldPresetName: preset?.name,
    landformScale: asUint(envelope.worldParams?.elevation_coarse_cell),
    seaLevel: asUint(envelope.worldParams?.water_level),
  };
}

export function slotFromPayload(
  payload: string,
  name: string,
  build: CurrentBuild,
  savedAt = Date.now(),
  id = newSaveId(),
): SaveSlot | null {
  const envelope = parseHxf1(payload);
  if (!envelope) return null;
  return {
    id,
    name:
      name.trim() ||
      defaultSaveName(configFromEnvelope(envelope, build), savedAt),
    savedAt,
    payload,
    versions: {
      save: envelope.saveVersion,
      world: envelope.worldVersion,
      definitions: envelope.definitionVersion,
      technology: envelope.technologyVersion,
      scenario: envelope.scenarioVersion,
    },
    config: configFromEnvelope(envelope, build),
  };
}

export function defaultSaveName(
  config: RememberedConfig,
  at = Date.now(),
): string {
  const when = new Date(at).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
  return `${config.scenarioName} · ${when}`;
}

export function formatSavedAt(savedAt: number): string {
  return new Date(savedAt).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

export function formatConfig(config: RememberedConfig): string {
  const world =
    config.worldPresetName ??
    (config.landformScale !== undefined && config.seaLevel !== undefined
      ? `custom (land ${config.landformScale}, sea ${config.seaLevel})`
      : "custom world");
  return `${config.scenarioName} · seed ${config.seed} · ${world}`;
}

export function formatVersions(versions: GameVersions): string {
  return `save ${versions.save} · world ${versions.world} · definitions ${versions.definitions} · technology ${versions.technology} · scenario ${versions.scenario}`;
}

export function readCatalog(storage: StorageLike): CatalogRead {
  const raw = storage.getItem(SAVE_CATALOG_KEY);
  if (!raw) return { slots: [] };
  try {
    const body: unknown = JSON.parse(raw);
    if (!isRecord(body) || !Array.isArray(body.slots)) {
      return { slots: [], error: "Save list could not be read." };
    }
    const slots: SaveSlot[] = [];
    for (const entry of body.slots) {
      const slot = asSlot(entry);
      if (slot) slots.push(slot);
    }
    return { slots };
  } catch {
    return { slots: [], error: "Save list could not be read." };
  }
}

export function catalogDocument(slots: SaveSlot[]): string {
  return JSON.stringify({ version: CATALOG_FILE_VERSION, slots });
}

export function writeCatalog(storage: StorageLike, slots: SaveSlot[]): void {
  storage.setItem(SAVE_CATALOG_KEY, catalogDocument(slots));
}

/**
 * A file name Windows and the common download folders will accept. The stem is the slot name; the
 * suffix is the native envelope. Trailing dots and spaces are the ones Explorer silently strips,
 * so they are already gone before the download lands.
 */
export function saveFileName(name: string): string {
  let cleaned = "";
  for (const char of name.trim()) {
    const code = char.charCodeAt(0);
    cleaned += code < 32 || '<>:"/\\|?*'.includes(char) ? " " : char;
  }
  const stem =
    cleaned
      .replace(/\s+/g, " ")
      .replace(/[. ]+$/g, "")
      .slice(0, 80) || "hexfactory-save";
  return `${stem}${SAVE_FILE_SUFFIX}`;
}

export function fileStem(fileName: string): string {
  const base = fileName.replace(/^.*[/\\]/, "").trim();
  return base.replace(/\.(hxf1|json|txt)$/i, "").trim();
}

/**
 * Read a desktop file back into catalog rows. Native still refuses a load the numbers cannot
 * support; this only has to recognise the two shapes a person can take off the machine: one HXF1
 * envelope, or the catalog JSON the browser already stores.
 *
 * A wrapped `{ name, payload }` object is accepted too, so a name that never fitted a file stem
 * still survives. New ids are minted so an import cannot collide with a slot already in the list.
 */
export function slotsFromFileText(
  text: string,
  build: CurrentBuild,
  options: FileImportOptions = {},
): FileImport {
  const now = options.now ?? Date.now();
  const body = text.replace(/^\uFEFF/, "").trim();
  if (!body) return { slots: [], error: "The file is empty." };

  const payload = asHxf1Payload(body);
  if (payload) {
    const slot = slotFromPayload(
      payload,
      nameFromFile(options.fileName),
      build,
      now,
    );
    if (!slot) {
      return { slots: [], error: "The file is not a readable HXF1 save." };
    }
    return { slots: [slot] };
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(body);
  } catch {
    return { slots: [], error: "The file is not a HexFactory save." };
  }

  if (isRecord(parsed) && Array.isArray(parsed.slots)) {
    const slots: SaveSlot[] = [];
    for (const entry of parsed.slots) {
      const slot = slotFromImportedEntry(entry, build, now);
      if (slot) slots.push(slot);
    }
    if (slots.length === 0) {
      return { slots: [], error: "The file does not contain a save." };
    }
    return { slots };
  }

  if (isRecord(parsed)) {
    const wrapped = asHxf1Payload(asString(parsed.payload) ?? "");
    if (wrapped) {
      const savedAt = asUint(parsed.savedAt) ?? now;
      const slot = slotFromPayload(
        wrapped,
        asString(parsed.name) ?? nameFromFile(options.fileName),
        build,
        savedAt,
      );
      if (!slot) {
        return { slots: [], error: "The file is not a readable HXF1 save." };
      }
      return { slots: [slot] };
    }
    if (asUint(parsed.save_version) !== undefined) {
      const slot = slotFromPayload(
        `${HXF1_PREFIX}${body}`,
        nameFromFile(options.fileName),
        build,
        now,
      );
      if (!slot) {
        return { slots: [], error: "The file is not a readable HXF1 save." };
      }
      return { slots: [slot] };
    }
  }

  return { slots: [], error: "The file is not a HexFactory save." };
}

function asHxf1Payload(text: string): string | null {
  const body = text.trim();
  if (!body) return null;
  if (body.startsWith(HXF1_PREFIX)) return body;
  if (body.startsWith("HXF1\r\n")) return `${HXF1_PREFIX}${body.slice(6)}`;
  if (/^HXF1\r?\n?\{/.test(body)) {
    return `${HXF1_PREFIX}${body.slice(4).replace(/^\r?\n/, "")}`;
  }
  return null;
}

function slotFromImportedEntry(
  value: unknown,
  build: CurrentBuild,
  now: number,
): SaveSlot | null {
  if (!isRecord(value)) return null;
  const payload = asHxf1Payload(asString(value.payload) ?? "");
  if (!payload) return null;
  const savedAt = asUint(value.savedAt) ?? now;
  return slotFromPayload(payload, asString(value.name) ?? "", build, savedAt);
}

function nameFromFile(fileName: string | undefined): string {
  const stem = fileName ? fileStem(fileName) : "";
  if (stem && !GENERIC_FILE_STEMS.has(stem.toLocaleLowerCase())) return stem;
  return "";
}

const GENERIC_FILE_STEMS = new Set([
  "download",
  "save",
  "untitled",
  "hexfactory-save",
  "hexfactory-saves",
]);

export function upsertSlot(slots: SaveSlot[], next: SaveSlot): SaveSlot[] {
  const index = slots.findIndex((slot) => slot.id === next.id);
  if (index < 0) return [...slots, next];
  const copy = slots.slice();
  copy[index] = next;
  return copy;
}

export function replaceNamedSlot(
  slots: SaveSlot[],
  next: SaveSlot,
): SaveSlot[] {
  const match = slots.find(
    (slot) => slot.name.toLocaleLowerCase() === next.name.toLocaleLowerCase(),
  );
  if (!match) return [...slots, next];
  return upsertSlot(slots, { ...next, id: match.id });
}

/**
 * A save name no existing slot already answers to.
 *
 * Writes match slots by name, so a run that takes a name already in the catalogue would overwrite
 * somebody else's factory. That is the right behaviour when the player typed the name — they are
 * saying which save they mean — and the wrong one when the name was defaulted for them, which is
 * what this is for: the default becomes "Landing run 2" rather than eating "Landing run".
 */
export function uniqueSlotName(name: string, slots: SaveSlot[]): string {
  const taken = new Set(slots.map((slot) => slot.name.toLocaleLowerCase()));
  if (!taken.has(name.toLocaleLowerCase())) return name;
  // Two is where a second one starts; the cap is a runaway guard, not a real limit.
  for (let suffix = 2; suffix < 1000; suffix += 1) {
    const candidate = `${name} ${suffix}`;
    if (!taken.has(candidate.toLocaleLowerCase())) return candidate;
  }
  return name;
}

export function removeSlot(slots: SaveSlot[], id: string): SaveSlot[] {
  return slots.filter((slot) => slot.id !== id);
}

export function latestCompatible(
  slots: SaveSlot[],
  build: CurrentBuild,
): SaveSlot | undefined {
  return slots
    .filter((slot) => {
      const envelope = parseHxf1(slot.payload);
      return envelope ? compatibility(envelope, build).compatible : false;
    })
    .sort((left, right) => right.savedAt - left.savedAt)[0];
}

export function slotsNewestFirst(slots: SaveSlot[]): SaveSlot[] {
  return slots.slice().sort((left, right) => right.savedAt - left.savedAt);
}

/**
 * Pull any leftover single-slot keys into the catalog. The old key stays: importing is how a
 * factory that the versioned key hid becomes a named row instead of disappearing.
 */
export function importLegacySlots(
  storage: StorageLike,
  build: CurrentBuild,
  now = Date.now(),
): { slots: SaveSlot[]; imported: number } {
  const read = readCatalog(storage);
  if (read.error) return { slots: read.slots, imported: 0 };
  let slots = read.slots;
  const knownSources = new Set(
    slots.flatMap((slot) => (slot.sourceKey ? [slot.sourceKey] : [])),
  );
  const knownPayloads = new Set(slots.map((slot) => slot.payload));
  const keys: string[] = [];
  for (let index = 0; index < storage.length; index += 1) {
    const key = storage.key(index);
    if (key && key.startsWith(LEGACY_SAVE_PREFIX)) keys.push(key);
  }
  let imported = 0;
  for (const key of keys) {
    if (knownSources.has(key)) continue;
    const payload = storage.getItem(key);
    if (!payload || knownPayloads.has(payload)) continue;
    const envelope = parseHxf1(payload);
    if (!envelope) continue;
    const slot = slotFromPayload(payload, legacyName(envelope), build, now);
    if (!slot) continue;
    slot.sourceKey = key;
    slots = [...slots, slot];
    knownSources.add(key);
    knownPayloads.add(payload);
    imported += 1;
  }
  if (imported > 0) writeCatalog(storage, slots);
  return { slots, imported };
}

export function newSaveId(): string {
  return `s${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

function legacyName(envelope: ParsedEnvelope): string {
  return `Previous run · save ${envelope.saveVersion}`;
}

function asSlot(value: unknown): SaveSlot | null {
  if (!isRecord(value)) return null;
  const id = asString(value.id);
  const name = asString(value.name);
  const savedAt = asUint(value.savedAt);
  const payload = asString(value.payload);
  const versions = asVersions(value.versions);
  const config = asConfig(value.config);
  if (!id || !name || savedAt === undefined || !payload || !versions || !config)
    return null;
  if (!payload.startsWith(HXF1_PREFIX)) return null;
  const sourceKey = asString(value.sourceKey);
  return {
    id,
    name,
    savedAt,
    payload,
    versions,
    config,
    ...(sourceKey ? { sourceKey } : {}),
  };
}

function asVersions(value: unknown): GameVersions | null {
  if (!isRecord(value)) return null;
  const save = asUint(value.save);
  const world = asUint(value.world);
  const definitions = asUint(value.definitions);
  const technology = asUint(value.technology);
  const scenario = asUint(value.scenario);
  if (
    save === undefined ||
    world === undefined ||
    definitions === undefined ||
    technology === undefined ||
    scenario === undefined
  ) {
    return null;
  }
  return { save, world, definitions, technology, scenario };
}

function asConfig(value: unknown): RememberedConfig | null {
  if (!isRecord(value)) return null;
  const seed = asUint(value.seed);
  const scenarioKey = asString(value.scenarioKey);
  const scenarioName = asString(value.scenarioName);
  if (seed === undefined || !scenarioKey || !scenarioName) return null;
  return {
    seed,
    scenarioKey,
    scenarioName,
    worldPreset: asString(value.worldPreset),
    worldPresetName: asString(value.worldPresetName),
    landformScale: asUint(value.landformScale),
    seaLevel: asUint(value.seaLevel),
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asUint(value: unknown): number | undefined {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
    ? value
    : undefined;
}

function asString(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function capitalize(value: string): string {
  const head = value.charAt(0);
  return head === "" ? value : head.toUpperCase() + value.slice(1);
}
