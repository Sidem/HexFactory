import type { AxialCoordinate } from "@hexlife/embed/hex";

import type {
  BuildingKind,
  BuildingsPatch,
  ChunkSnapshot,
  ContractRequirement,
  EntitySnapshot,
  FactorySnapshotDelta,
  GroundItemSnapshot,
  Ingredient,
  ProjectState,
  RequestSnapshot,
  ResearchAvailability,
  ResourceSnapshot,
  ResourcesPatch,
  Terrain,
  TerrainSnapshot,
} from "./types";

/**
 * The decoder for the binary snapshot delta.
 *
 * `docs/BENCHMARKS.md` finding 3 priced the worker boundary at about 10 µs per kilobyte and found
 * it costing more than the simulation it carried. The encoder is `factory-wasm/src/wire.rs`; this
 * reads what it writes.
 *
 * It produces exactly the object `JSON.parse(snapshot_delta_json())` produced — the same fields in
 * the same shapes, `null` where the JSON path sent `null` — so nothing downstream of
 * {@link FactoryHost} can tell which encoding delivered the frame. That equivalence is not a claim,
 * it is pinned: `fixtures/snapshot-delta-wire.json` carries encoded payloads beside the exact JSON
 * they must decode to, Rust asserts it writes those bytes, and `tests/snapshotWire.test.ts` asserts
 * this reads them back into that JSON.
 *
 * Numbers are LEB128 varints, signed ones zigzagged. They are accumulated by multiplication rather
 * than by `<<`, because JavaScript's bitwise operators truncate to 32 bits and `tick`, `delivered`,
 * and `insight` are all wider than that.
 */

const MAGIC = 0x48584644; // "HXFD"
const VERSION = 15;

/** Wire code is the index. Pinned against Rust by `fixtures/snapshot-delta-wire.json`. */
const KINDS: BuildingKind[] = [
  "extractor",
  "belt",
  "composer",
  "container",
  "consumer",
  "hub",
  "pump",
  "pole",
  "generator",
  "boiler",
  "bridge",
];

const TERRAIN: Terrain[] = [
  "deep_water",
  "shallow_water",
  "shore",
  "lowland",
  "hills",
  "highland",
  "cliff",
];

/** Index is the code `project_state_code` writes; an unknown code reads as `locked`. */
const PROJECT_STATE: ProjectState[] = [
  "locked",
  "available",
  "posted",
  "complete",
];

const STATUSES: string[] = [
  "output blocked",
  "deposit depleted",
  "extracting",
  "no water in reach",
  "pumping",
  "composing",
  "out of fuel",
  "waiting for inputs",
  "buffered",
  "carrying",
  "receiving",
  "landing hub",
  "idle",
  "no power",
  "generating",
  "brownout",
  "no boiler",
  "switched off",
];

const GROUP = {
  scenario: 1 << 0,
  scenarioName: 1 << 1,
  worldVersion: 1 << 2,
  seed: 1 << 3,
  delivered: 1 << 4,
  deliveredByItem: 1 << 5,
  insight: 1 << 6,
  victory: 1 << 7,
  contract: 1 << 8,
  requests: 1 << 9,
  player: 1 << 10,
  researched: 1 << 11,
  researchAvailability: 1 << 18,
  skills: 1 << 19,
  chunks: 1 << 12,
  terrain: 1 << 13,
  resources: 1 << 14,
  buildings: 1 << 15,
  events: 1 << 16,
  groundItems: 1 << 17,
} as const;

const ENTITY_FLAG = {
  recipeId: 1 << 0,
  scenarioOwned: 1 << 1,
  cargo: 1 << 2,
  fuelCharge: 1 << 3,
  fuelRequired: 1 << 4,
  nextId: 1 << 5,
  powerSatisfied: 1 << 6,
  powerDemand: 1 << 7,
  powerCharge: 1 << 8,
  powerCapacity: 1 << 9,
  branchIds: 1 << 10,
  inputInventory: 1 << 11,
  fuelInventory: 1 << 12,
  outputInventory: 1 << 13,
} as const;

const PATCH_REPLACE = 1 << 0;

class Reader {
  private offset = 0;
  private readonly view: DataView;
  private readonly bytes: Uint8Array;
  private readonly text = new TextDecoder();

  constructor(buffer: ArrayBuffer) {
    this.bytes = new Uint8Array(buffer);
    this.view = new DataView(buffer);
  }

  u8(): number {
    const value = this.bytes[this.offset];
    if (value === undefined)
      throw new Error("Snapshot delta buffer ended mid-frame");
    this.offset += 1;
    return value;
  }

  bool(): boolean {
    return this.u8() === 1;
  }

  u32Fixed(): number {
    const value = this.view.getUint32(this.offset, true);
    this.offset += 4;
    return value;
  }

  uvarint(): number {
    let value = 0;
    let scale = 1;
    for (;;) {
      const byte = this.u8();
      value += (byte & 0x7f) * scale;
      if ((byte & 0x80) === 0) return value;
      scale *= 128;
    }
  }

  svarint(): number {
    const raw = this.uvarint();
    // Zigzag, undone without bitwise operators so it survives past 32 bits.
    const magnitude = Math.floor(raw / 2);
    return raw % 2 === 0 ? magnitude : -magnitude - 1;
  }

  string(): string {
    const length = this.uvarint();
    const text = this.text.decode(
      this.bytes.subarray(this.offset, this.offset + length),
    );
    this.offset += length;
    return text;
  }

  ingredients(): Ingredient[] {
    const count = this.uvarint();
    const items: Ingredient[] = new Array<Ingredient>(count);
    for (let index = 0; index < count; index += 1) {
      items[index] = { item_id: this.uvarint(), quantity: this.uvarint() };
    }
    return items;
  }

  atEnd(): boolean {
    return this.offset === this.bytes.length;
  }
}

/**
 * Read one delta out of the buffer the worker transferred.
 *
 * A buffer whose magic or version is not recognised is refused rather than decoded: a host reading
 * a frame from a core it does not match would produce a plausible-looking wrong world, which is
 * worse than a thrown error.
 */
export function decodeSnapshotDelta(buffer: ArrayBuffer): FactorySnapshotDelta {
  const reader = new Reader(buffer);
  const magic =
    (reader.u8() << 24) |
    (reader.u8() << 16) |
    (reader.u8() << 8) |
    reader.u8();
  if (magic !== MAGIC)
    throw new Error("Snapshot delta buffer is not a HexFactory wire payload");
  const version = reader.u8();
  if (version !== VERSION)
    throw new Error(
      `Unsupported snapshot delta wire version ${version}; this host reads ${VERSION}`,
    );

  const delta: FactorySnapshotDelta = {
    base_revision: reader.uvarint(),
    revision: reader.uvarint(),
    tick: reader.uvarint(),
    checksum: reader.u32Fixed(),
  };
  const mask = reader.uvarint();
  const has = (bit: number): boolean => (mask & bit) !== 0;

  if (has(GROUP.scenario)) delta.scenario = reader.string();
  if (has(GROUP.scenarioName)) delta.scenario_name = reader.string();
  if (has(GROUP.worldVersion)) delta.world_version = reader.uvarint();
  if (has(GROUP.seed)) delta.seed = reader.uvarint();
  if (has(GROUP.delivered)) delta.delivered = reader.uvarint();
  if (has(GROUP.deliveredByItem)) {
    const count = reader.uvarint();
    const items: Ingredient[] = new Array<Ingredient>(count);
    for (let index = 0; index < count; index += 1) {
      items[index] = { item_id: reader.uvarint(), quantity: reader.uvarint() };
    }
    delta.delivered_by_item = items;
  }
  if (has(GROUP.insight)) delta.insight = reader.uvarint();
  if (has(GROUP.victory)) delta.victory = reader.bool();
  if (has(GROUP.contract)) {
    const key = reader.string();
    const name = reader.string();
    const stage = reader.uvarint();
    const stages = reader.uvarint();
    const stage_key = reader.string();
    const stage_name = reader.string();
    const stage_brief = reader.string();
    const count = reader.uvarint();
    const requirements: ContractRequirement[] = new Array<ContractRequirement>(
      count,
    );
    for (let index = 0; index < count; index += 1)
      requirements[index] = {
        item_id: reader.uvarint(),
        delivered: reader.uvarint(),
        required: reader.uvarint(),
      };
    delta.contract = {
      key,
      name,
      stage,
      stages,
      stage_key,
      stage_name,
      stage_brief,
      requirements,
      complete: reader.bool(),
    };
  }
  if (has(GROUP.requests)) {
    const count = reader.uvarint();
    const requests: RequestSnapshot[] = new Array<RequestSnapshot>(count);
    for (let index = 0; index < count; index += 1)
      requests[index] = {
        key: reader.string(),
        name: reader.string(),
        brief: reader.string(),
        item_id: reader.uvarint(),
        delivered: reader.uvarint(),
        required: reader.uvarint(),
        insight: reader.uvarint(),
        state: PROJECT_STATE[reader.u8()] ?? "locked",
      };
    delta.requests = requests;
  }
  if (has(GROUP.player)) delta.player = readPlayer(reader);
  if (has(GROUP.researched)) {
    const count = reader.uvarint();
    const researched: number[] = new Array<number>(count);
    for (let index = 0; index < count; index += 1)
      researched[index] = reader.uvarint();
    delta.researched = researched;
  }
  if (has(GROUP.chunks)) delta.chunks = readChunks(reader);
  if (has(GROUP.terrain)) delta.terrain = readTerrain(reader);
  if (has(GROUP.resources)) delta.resources = readResources(reader);
  if (has(GROUP.buildings)) delta.buildings = readBuildings(reader);
  if (has(GROUP.events)) {
    const count = reader.uvarint();
    const events: string[] = new Array<string>(count);
    for (let index = 0; index < count; index += 1)
      events[index] = reader.string();
    delta.events = events;
  }
  if (has(GROUP.groundItems)) {
    const count = reader.uvarint();
    const groundItems: GroundItemSnapshot[] = new Array<GroundItemSnapshot>(
      count,
    );
    for (let index = 0; index < count; index += 1) {
      groundItems[index] = {
        id: reader.uvarint(),
        q: reader.svarint(),
        r: reader.svarint(),
        item_id: reader.uvarint(),
        quantity: reader.uvarint(),
        despawn_tick: reader.uvarint(),
      };
    }
    delta.ground_items = groundItems;
  }

  if (has(GROUP.researchAvailability)) {
    const count = reader.uvarint();
    if (count > 1024)
      throw new Error("Research availability exceeds catalog bound");
    const availability: ResearchAvailability[] = [];
    for (let index = 0; index < count; index += 1) {
      const technology_id = reader.uvarint();
      const complete = reader.bool();
      const insight_shortfall = reader.uvarint();
      const missingCount = reader.uvarint();
      if (missingCount > 1024)
        throw new Error("Research prerequisites exceed catalog bound");
      const missing_prerequisites: number[] = [];
      for (let missing = 0; missing < missingCount; missing += 1)
        missing_prerequisites.push(reader.uvarint());
      availability.push({
        technology_id,
        complete,
        insight_shortfall,
        missing_prerequisites,
      });
    }
    delta.research_availability = availability;
  }

  if (has(GROUP.skills)) {
    const points = reader.uvarint();
    const sandbox = reader.bool();
    const ids = (): number[] => {
      const count = reader.uvarint();
      if (count > 64) throw new Error("Skills exceed catalog bound");
      return Array.from({ length: count }, () => reader.uvarint());
    };
    const purchased = ids();
    const granted = ids();
    const completed = ids();
    const count = reader.uvarint();
    if (count > 64) throw new Error("Skills exceed catalog bound");
    const availability = Array.from({ length: count }, () => ({
      skill_id: reader.uvarint(),
      complete: reader.bool(),
      points_shortfall: reader.uvarint(),
      current_value: reader.uvarint(),
      resulting_value: reader.uvarint(),
      missing_prerequisites: ids(),
    }));
    delta.skills = {
      points,
      sandbox,
      purchased,
      granted,
      completed,
      availability,
    };
  }

  // A buffer with bytes left over means the two sides disagree about the layout, which would
  // otherwise surface as a subtly wrong frame somewhere downstream.
  if (!reader.atEnd())
    throw new Error("Snapshot delta buffer has trailing bytes");
  return delta;
}

function readPlayer(reader: Reader): FactorySnapshotDelta["player"] {
  const x = reader.svarint();
  const y = reader.svarint();
  const facing_x = reader.svarint();
  const facing_y = reader.svarint();
  const move_x = reader.svarint();
  const move_y = reader.svarint();
  const entries = reader.uvarint();
  // Native holds this keyed by item id and JSON delivered it as an object with string keys, so
  // that is what the host has always seen.
  const inventory: Record<string, number> = {};
  for (let index = 0; index < entries; index += 1) {
    const itemId = reader.uvarint();
    inventory[String(itemId)] = reader.uvarint();
  }
  const action_cooldown = reader.uvarint();
  const build_range = reader.uvarint();
  const carry_slots = reader.uvarint();
  const carry_stacks = reader.ingredients();
  const radius = reader.svarint();
  const action_cooldown_total = reader.uvarint();
  const extract_radius = reader.uvarint();
  const creative = reader.bool();
  const hand = reader.bool()
    ? { item_id: reader.uvarint(), quantity: reader.uvarint() }
    : null;
  const walk_goal = reader.bool()
    ? { q: reader.svarint(), r: reader.svarint() }
    : null;
  // Delta-coded against the goal and then against the previous cell, exactly as `write_player`
  // codes it. The route is native's answer to where the player is walking, not the host's guess at
  // it, which is what lets the ribbon on screen be the path the player will actually take.
  const cells = reader.uvarint();
  const walk_path: AxialCoordinate[] = new Array<AxialCoordinate>(cells);
  let previousQ = walk_goal?.q ?? 0;
  let previousR = walk_goal?.r ?? 0;
  for (let index = 0; index < cells; index += 1) {
    previousQ += reader.svarint();
    previousR += reader.svarint();
    walk_path[index] = { q: previousQ, r: previousR };
  }
  return {
    x,
    y,
    facing_x,
    facing_y,
    move_x,
    move_y,
    inventory,
    hand,
    action_cooldown,
    build_range,
    carry_slots,
    carry_stacks,
    radius,
    action_cooldown_total,
    extract_radius,
    creative,
    walk_goal,
    walk_path,
  };
}

function readChunks(reader: Reader): ChunkSnapshot[] {
  const count = reader.uvarint();
  const chunks: ChunkSnapshot[] = new Array<ChunkSnapshot>(count);
  for (let index = 0; index < count; index += 1) {
    chunks[index] = {
      chunk_q: reader.svarint(),
      chunk_r: reader.svarint(),
      entity_count: reader.uvarint(),
      x: reader.svarint(),
      y: reader.svarint(),
      span: reader.svarint(),
    };
  }
  return chunks;
}

/**
 * The running cell the tile lists are coded against. Both are built chunk by chunk, so each entry
 * is a short hop from the one before it and travels as that hop rather than as four coordinates.
 */
interface Cell {
  q: number;
  r: number;
  x: number;
  y: number;
}

function stepCell(reader: Reader, cell: Cell): void {
  cell.q += reader.svarint();
  cell.r += reader.svarint();
  cell.x += reader.svarint();
  cell.y += reader.svarint();
}

function readTerrain(reader: Reader): TerrainSnapshot[] {
  const count = reader.uvarint();
  const tiles: TerrainSnapshot[] = new Array<TerrainSnapshot>(count);
  const cell: Cell = { q: 0, r: 0, x: 0, y: 0 };
  for (let index = 0; index < count; index += 1) {
    stepCell(reader, cell);
    tiles[index] = {
      q: cell.q,
      r: cell.r,
      x: cell.x,
      y: cell.y,
      radius: reader.uvarint(),
      terrain: terrainOf(reader.u8()),
    };
  }
  return tiles;
}

function readResources(reader: Reader): ResourcesPatch {
  const replace = (reader.u8() & PATCH_REPLACE) !== 0;
  const count = reader.uvarint();
  const changed: ResourceSnapshot[] = new Array<ResourceSnapshot>(count);
  const cell: Cell = { q: 0, r: 0, x: 0, y: 0 };
  for (let index = 0; index < count; index += 1) {
    stepCell(reader, cell);
    changed[index] = {
      q: cell.q,
      r: cell.r,
      x: cell.x,
      y: cell.y,
      radius: reader.uvarint(),
      item_id: reader.uvarint(),
      quantity: reader.uvarint(),
      initial_quantity: reader.uvarint(),
    };
  }
  const patch: ResourcesPatch = {};
  // Native skips a false flag and an empty list rather than sending them, so neither key was ever
  // present in the JSON the host received. Every reader takes them as `?? []` and `?? false`, but
  // reproducing the omission is what keeps the two encodings exactly interchangeable.
  if (replace) patch.replace = true;
  if (changed.length > 0) patch.changed = changed;
  return patch;
}

function readBuildings(reader: Reader): BuildingsPatch {
  const replace = (reader.u8() & PATCH_REPLACE) !== 0;
  const count = reader.uvarint();
  const changed: EntitySnapshot[] = new Array<EntitySnapshot>(count);
  let id = 0;
  for (let index = 0; index < count; index += 1) {
    // Ascending stable id, so each entity costs the gap rather than the value.
    id += reader.uvarint();
    const q = reader.svarint();
    const r = reader.svarint();
    const definition_id = reader.uvarint();
    const kind = kindOf(reader.u8());
    const orientation = reader.u8();
    // A uvarint since wire version 4, not the fixed byte it was: ten flags do not fit in eight
    // bits, and a fixed pair of bytes would have charged every belt for the two it never sets.
    const flags = reader.uvarint();
    const recipe_id =
      (flags & ENTITY_FLAG.recipeId) !== 0 ? reader.uvarint() : null;
    const cargo =
      (flags & ENTITY_FLAG.cargo) !== 0
        ? { item_id: reader.uvarint(), quantity: reader.uvarint() }
        : null;
    const inventory = reader.ingredients();
    const input_inventory =
      (flags & ENTITY_FLAG.inputInventory) !== 0 ? reader.ingredients() : [];
    const fuel_inventory =
      (flags & ENTITY_FLAG.fuelInventory) !== 0 ? reader.ingredients() : [];
    const output_inventory =
      (flags & ENTITY_FLAG.outputInventory) !== 0 ? reader.ingredients() : [];
    const progress = reader.uvarint();
    const progress_total = reader.uvarint();
    const fuel_charge =
      (flags & ENTITY_FLAG.fuelCharge) !== 0 ? reader.uvarint() : 0;
    const fuel_required =
      (flags & ENTITY_FLAG.fuelRequired) !== 0 ? reader.uvarint() : 0;
    const status = statusOf(reader.u8());
    const next_id =
      (flags & ENTITY_FLAG.nextId) !== 0 ? reader.uvarint() : null;
    const branch_ids: number[] = [];
    if ((flags & ENTITY_FLAG.branchIds) !== 0) {
      const branches = reader.uvarint();
      for (let branch = 0; branch < branches; branch += 1) {
        branch_ids.push(reader.uvarint());
      }
    }
    const power_satisfied =
      (flags & ENTITY_FLAG.powerSatisfied) !== 0 ? reader.uvarint() : 0;
    const power_demand =
      (flags & ENTITY_FLAG.powerDemand) !== 0 ? reader.uvarint() : 0;
    const power_charge =
      (flags & ENTITY_FLAG.powerCharge) !== 0 ? reader.uvarint() : 0;
    const power_capacity =
      (flags & ENTITY_FLAG.powerCapacity) !== 0 ? reader.uvarint() : 0;
    const cells = reader.uvarint();
    const footprint = new Array<{ q: number; r: number }>(cells);
    for (let cell = 0; cell < cells; cell += 1) {
      footprint[cell] = { q: q + reader.svarint(), r: r + reader.svarint() };
    }
    const entity: EntitySnapshot = {
      id,
      q,
      r,
      definition_id,
      kind,
      orientation,
      recipe_id,
      scenario_owned: (flags & ENTITY_FLAG.scenarioOwned) !== 0,
      cargo,
      inventory,
      progress,
      progress_total,
      status,
      next_id,
      footprint,
    };
    if (input_inventory.length > 0) entity.input_inventory = input_inventory;
    if (fuel_inventory.length > 0) entity.fuel_inventory = fuel_inventory;
    if (output_inventory.length > 0) entity.output_inventory = output_inventory;
    // Absent rather than zero, because that is what native sends: two numbers per entity per delta
    // saying "this is not a furnace" cost 86 KB at the largest measured tier, which is why they are
    // skipped in the first place. The flag bit already carried the distinction.
    if (fuel_charge !== 0) entity.fuel_charge = fuel_charge;
    if (fuel_required !== 0) entity.fuel_required = fuel_required;
    if (power_satisfied !== 0) entity.power_satisfied = power_satisfied;
    if (power_demand !== 0) entity.power_demand = power_demand;
    if (power_charge !== 0) entity.power_charge = power_charge;
    if (power_capacity !== 0) entity.power_capacity = power_capacity;
    // Same rule: absent rather than an empty array, because an empty list is what every entity
    // that is not a splitter has, and native skips it for exactly that reason.
    if (branch_ids.length > 0) entity.branch_ids = branch_ids;
    changed[index] = entity;
  }
  const removedCount = reader.uvarint();
  const removed: number[] = new Array<number>(removedCount);
  let removedId = 0;
  for (let index = 0; index < removedCount; index += 1) {
    removedId += reader.uvarint();
    removed[index] = removedId;
  }
  const patch: BuildingsPatch = {};
  // Omitted exactly where native omits them — see the note in `readResources`.
  if (replace) patch.replace = true;
  if (changed.length > 0) patch.changed = changed;
  if (removed.length > 0) patch.removed = removed;
  return patch;
}

function kindOf(code: number): BuildingKind {
  const kind = KINDS[code];
  if (kind === undefined)
    throw new Error(`Unknown building kind code ${code} on the wire`);
  return kind;
}

function terrainOf(code: number): Terrain {
  const terrain = TERRAIN[code];
  if (terrain === undefined)
    throw new Error(`Unknown terrain code ${code} on the wire`);
  return terrain;
}

function statusOf(code: number): string {
  const status = STATUSES[code];
  if (status === undefined)
    throw new Error(`Unknown entity status code ${code} on the wire`);
  return status;
}
