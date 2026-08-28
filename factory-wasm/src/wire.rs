//! The binary snapshot-delta wire format.
//!
//! `docs/BENCHMARKS.md` finding 3 priced the worker boundary at about 10 µs per kilobyte and found
//! it to be 57–61% of a host frame — more than the simulation it carries. The payload was JSON:
//! every coordinate spelled in decimal, every field name repeated once per entity, every status
//! spelled out in full, and the whole thing parsed and copied again on the main thread. This module
//! is the encoding that replaces it. It attacks all three costs at once, because the buffer is
//! built once here, needs no parse on the far side, and crosses the boundary as a transferable
//! rather than a structured clone.
//!
//! # What this is not
//!
//! It is not a change to what the host is told. The decoder in `src/core/snapshotWire.ts` produces
//! exactly the object `JSON.parse(snapshot_delta_json())` produced, field for field and `null` for
//! `null`, so nothing downstream of `FactoryHost` can tell the difference. That is the whole design
//! constraint: this milestone moves bytes, not meaning. `snapshot_delta_json` is kept as the oracle
//! the encoder is pinned against, and `fixtures/snapshot-delta-wire.json` pins both languages to
//! one artifact the way `fixtures/hex-directions.json` pins the direction table.
//!
//! # The format
//!
//! Little-endian throughout. Integers are LEB128 varints, signed ones zigzagged first, so a small
//! number costs one byte and a coordinate near the origin costs one or two. Three things beyond
//! that carry most of the saving:
//!
//! - **Closed sets are one byte.** `BuildingKind`, `Terrain`, and `EntityStatus` travel as their
//!   declaration index. The `wire_code` matches below are exhaustive on purpose: adding a variant
//!   without assigning it a code is a compile error, not a silently wrong frame.
//! - **Ascending runs are delta-coded.** Entity ids and removal ids arrive sorted, so each costs
//!   the gap to the one before it rather than its absolute value.
//! - **Neighbouring cells are delta-coded.** Terrain and resource lists are built chunk by chunk,
//!   so consecutive entries sit beside each other in the world; `q`, `r`, `x`, and `y` each travel
//!   as the difference from the previous entry. A footprint cell is coded against its own entity's
//!   hex, which makes the single-cell case — nearly every building — two bytes.
//!
//! Versioning is a magic word and a version byte at the head. A host that does not recognise both
//! refuses the buffer rather than decoding a frame from a core it does not match.

use super::*;

/// Head of every buffer, so a stale or foreign payload is rejected rather than misread.
pub(crate) const WIRE_MAGIC: [u8; 4] = *b"HXFD";
/// Bumped whenever the layout below changes in a way an old decoder would misread. Version 3 is
/// the Founding Contract: the objective group became the contract group, and it carries names and
/// a bill rather than one item's three numbers.
///
/// Version 4 is the Power Grid. The per-entity flag field is a uvarint rather than a fixed byte,
/// and carries two more numbers behind it — what a machine has banked and what it banks to. A
/// version-3 decoder would read the first byte of a two-byte flag and mis-frame every field after
/// it, which is exactly the misreading this number exists to prevent.
///
/// Version 5 is Standing Requests. A new group carries the hub's request board, and it is written
/// between the contract and the player — so a version-4 decoder that skipped the group bit would
/// read the board's first string as a player and mis-frame the rest of the buffer.
///
/// Version 6 is Crossings and Canopy. `Bridge` is appended to the kind table and the player group
/// publishes the hand's extraction radius, so an older decoder would leave a trailing byte and
/// could not draw the held-action reach native actually uses.
///
/// Version 7 is the hand switch. `SwitchedOff` is appended to the status table, and a status code
/// is one byte with no length beside it — an older decoder would not mis-frame the buffer, it
/// would simply fail on a code it has no name for, which is a worse way to learn the same thing.
///
/// Version 8 is creative mode. The player group gains a trailing flag byte, so an older decoder
/// would read the group one byte short and leave a trailing byte behind — a mis-framed buffer
/// rather than an honest failure, which is exactly what this number exists to prevent.
///
/// Version 9 is belt junctions. An entity may now compile more than one output, so a tenth entity
/// flag carries the outputs after the first. A version-8 decoder would not know that bit, would not
/// consume the branch list behind it, and would read the next entity out of the middle of this
/// one — the mis-framing this number prevents, and the reason a new flag is still a new version.
///
/// Version 10 is click-to-walk. The player group gains where an autonomous walk is headed and the
/// route it is taking to get there, both written after the creative flag that used to end the group.
/// A version-9 decoder would stop at that flag and read the route as whatever group came next,
/// mis-framing the rest of the buffer — the same failure version 8 was cut for, and the same reason
/// a trailing addition is still a new version.
///
/// Version 11 is compartment storage. The player group gains its cursor-held stack, and entity
/// flags may carry sparse input, fuel, and output inventories. An older decoder would otherwise
/// read the first compartment length as progress and mis-frame every entity after it.
///
/// Version 12 adds ground items for dropped player cargo with 1-minute despawn timers.
/// Version 13 appends the native research availability group.
///
/// Version 14 is Practical Projects. The request group now carries the whole finite catalogue
/// rather than the three posted slots, and every row gains a state byte behind its price. A
/// version-13 decoder would read that byte as the start of the next row's key length and mis-frame
/// the rest of the group — and would in any case draw a locked project as a posted one.
/// Version 15 appends bounded personal skill state and native purchase availability.
pub(crate) const WIRE_VERSION: u8 = 15;

/// Which optional groups the buffer carries, in the order they are written.
mod group {
    pub(super) const SCENARIO: u32 = 1 << 0;
    pub(super) const SCENARIO_NAME: u32 = 1 << 1;
    pub(super) const WORLD_VERSION: u32 = 1 << 2;
    pub(super) const SEED: u32 = 1 << 3;
    pub(super) const DELIVERED: u32 = 1 << 4;
    pub(super) const DELIVERED_BY_ITEM: u32 = 1 << 5;
    pub(super) const INSIGHT: u32 = 1 << 6;
    pub(super) const VICTORY: u32 = 1 << 7;
    pub(super) const CONTRACT: u32 = 1 << 8;
    pub(super) const REQUESTS: u32 = 1 << 9;
    pub(super) const PLAYER: u32 = 1 << 10;
    pub(super) const RESEARCHED: u32 = 1 << 11;
    pub(super) const CHUNKS: u32 = 1 << 12;
    pub(super) const TERRAIN: u32 = 1 << 13;
    pub(super) const RESOURCES: u32 = 1 << 14;
    pub(super) const BUILDINGS: u32 = 1 << 15;
    pub(super) const EVENTS: u32 = 1 << 16;
    pub(super) const GROUND_ITEMS: u32 = 1 << 17;
    pub(super) const RESEARCH_AVAILABILITY: u32 = 1 << 18;
    pub(super) const SKILLS: u32 = 1 << 19;
}

/// Per-entity presence bits, so an absent option costs a bit rather than a field name and a `null`.
///
/// Written as a uvarint rather than the fixed byte it was through wire version 3. The Power Grid
/// needs ten bits and a byte holds eight, and widening to a fixed `u16` would have charged every
/// belt and container in the world a second byte to say nothing. A uvarint charges it only to
/// entities that actually set a high bit — which is machines on a network, the ones already
/// carrying four numbers.
///
/// The low seven bits are therefore the *common* flags on purpose: anything an ordinary belt sets
/// has to stay under `1 << 7` or every belt pays for the ordering.
mod entity_flag {
    pub(super) const RECIPE_ID: u16 = 1 << 0;
    pub(super) const SCENARIO_OWNED: u16 = 1 << 1;
    pub(super) const CARGO: u16 = 1 << 2;
    pub(super) const FUEL_CHARGE: u16 = 1 << 3;
    pub(super) const FUEL_REQUIRED: u16 = 1 << 4;
    pub(super) const NEXT_ID: u16 = 1 << 5;
    pub(super) const POWER_SATISFIED: u16 = 1 << 6;
    pub(super) const POWER_DEMAND: u16 = 1 << 7;
    pub(super) const POWER_CHARGE: u16 = 1 << 8;
    pub(super) const POWER_CAPACITY: u16 = 1 << 9;
    /// The outputs after the first, which only a splitter ever has. A flag rather than an always-
    /// written length, because every other entity in the world would otherwise pay a byte to say
    /// it has none.
    pub(super) const BRANCH_IDS: u16 = 1 << 10;
    pub(super) const INPUT_INVENTORY: u16 = 1 << 11;
    pub(super) const FUEL_INVENTORY: u16 = 1 << 12;
    pub(super) const OUTPUT_INVENTORY: u16 = 1 << 13;
}

/// Set on a group whose list replaces the host's rather than patching it.
const PATCH_REPLACE: u8 = 1 << 0;

fn kind_code(kind: BuildingKind) -> u8 {
    match kind {
        BuildingKind::Extractor => 0,
        BuildingKind::Belt => 1,
        BuildingKind::Composer => 2,
        BuildingKind::Container => 3,
        BuildingKind::Consumer => 4,
        BuildingKind::Hub => 5,
        BuildingKind::Pump => 6,
        BuildingKind::Pole => 7,
        BuildingKind::Generator => 8,
        BuildingKind::Boiler => 9,
        BuildingKind::Bridge => 10,
    }
}

fn terrain_code(terrain: Terrain) -> u8 {
    match terrain {
        Terrain::DeepWater => 0,
        Terrain::ShallowWater => 1,
        Terrain::Shore => 2,
        Terrain::Lowland => 3,
        Terrain::Hills => 4,
        Terrain::Highland => 5,
        Terrain::Cliff => 6,
    }
}

fn project_state_code(state: ProjectState) -> u8 {
    match state {
        ProjectState::Locked => 0,
        ProjectState::Available => 1,
        ProjectState::Posted => 2,
        ProjectState::Complete => 3,
    }
}

/// The inverse. An unknown code is `Locked` rather than a panic: the worst a wrong guess does here
/// is grey out a row the player could have asked for, where a panic would take the frame down.
///
/// Only the round-trip tests decode — the real reader is TypeScript — so this is test-only, and
/// `snapshotWire.ts` carries the mapping that ships.
#[cfg(test)]
fn project_state(code: u8) -> ProjectState {
    match code {
        1 => ProjectState::Available,
        2 => ProjectState::Posted,
        3 => ProjectState::Complete,
        _ => ProjectState::Locked,
    }
}

fn status_code(status: EntityStatus) -> u8 {
    match status {
        EntityStatus::OutputBlocked => 0,
        EntityStatus::DepositDepleted => 1,
        EntityStatus::Extracting => 2,
        EntityStatus::NoWaterInReach => 3,
        EntityStatus::Pumping => 4,
        EntityStatus::Composing => 5,
        EntityStatus::OutOfFuel => 6,
        EntityStatus::WaitingForInputs => 7,
        EntityStatus::Buffered => 8,
        EntityStatus::Carrying => 9,
        EntityStatus::Receiving => 10,
        EntityStatus::LandingHub => 11,
        EntityStatus::Idle => 12,
        EntityStatus::NoPower => 13,
        EntityStatus::Generating => 14,
        EntityStatus::Brownout => 15,
        EntityStatus::NoBoiler => 16,
        EntityStatus::SwitchedOff => 17,
    }
}

/// Grows one reusable byte buffer. Nothing here allocates per field, which is the other half of
/// what makes this cheaper than building a JSON string.
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(1024),
        }
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn bool(&mut self, value: bool) {
        self.bytes.push(u8::from(value));
    }

    /// Fixed four bytes: a checksum is a hash, so its high bits are set as often as not and a
    /// varint would cost five bytes to say the same thing.
    fn u32_fixed(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn uvarint(&mut self, mut value: u64) {
        while value >= 0x80 {
            self.bytes.push((value as u8) | 0x80);
            value >>= 7;
        }
        self.bytes.push(value as u8);
    }

    /// Zigzag, so a small negative coordinate costs one byte rather than ten.
    fn svarint(&mut self, value: i64) {
        self.uvarint(((value << 1) ^ (value >> 63)) as u64);
    }

    fn string(&mut self, value: &str) {
        self.uvarint(value.len() as u64);
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn ingredient(&mut self, ingredient: &Ingredient) {
        self.uvarint(u64::from(ingredient.item_id));
        self.uvarint(u64::from(ingredient.quantity));
    }

    fn ingredients(&mut self, ingredients: &[Ingredient]) {
        self.uvarint(ingredients.len() as u64);
        for ingredient in ingredients {
            self.ingredient(ingredient);
        }
    }
}

/// Encode one delta. The buffer is self-contained: it names its format, states the revisions it
/// bridges, and then carries only the groups that changed.
pub(crate) fn encode_delta(delta: &SnapshotDelta) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.bytes.extend_from_slice(&WIRE_MAGIC);
    writer.u8(WIRE_VERSION);
    writer.uvarint(delta.base_revision);
    writer.uvarint(delta.revision);
    writer.uvarint(delta.tick);
    writer.u32_fixed(delta.checksum);

    let mut mask = 0u32;
    let mut set = |bit: u32, present: bool| {
        if present {
            mask |= bit;
        }
    };
    set(group::SCENARIO, delta.scenario.is_some());
    set(group::SCENARIO_NAME, delta.scenario_name.is_some());
    set(group::WORLD_VERSION, delta.world_version.is_some());
    set(group::SEED, delta.seed.is_some());
    set(group::DELIVERED, delta.delivered.is_some());
    set(group::DELIVERED_BY_ITEM, delta.delivered_by_item.is_some());
    set(group::INSIGHT, delta.insight.is_some());
    set(group::VICTORY, delta.victory.is_some());
    set(group::CONTRACT, delta.contract.is_some());
    set(group::REQUESTS, delta.requests.is_some());
    set(group::PLAYER, delta.player.is_some());
    set(group::RESEARCHED, delta.researched.is_some());
    set(
        group::RESEARCH_AVAILABILITY,
        delta.research_availability.is_some(),
    );
    set(group::SKILLS, delta.skills.is_some());
    set(group::CHUNKS, delta.chunks.is_some());
    set(group::TERRAIN, delta.terrain.is_some());
    set(group::RESOURCES, delta.resources.is_some());
    set(group::BUILDINGS, delta.buildings.is_some());
    set(group::EVENTS, delta.events.is_some());
    set(group::GROUND_ITEMS, delta.ground_items.is_some());
    writer.uvarint(u64::from(mask));

    if let Some(scenario) = &delta.scenario {
        writer.string(scenario);
    }
    if let Some(name) = &delta.scenario_name {
        writer.string(name);
    }
    if let Some(version) = delta.world_version {
        writer.uvarint(u64::from(version));
    }
    if let Some(seed) = delta.seed {
        writer.uvarint(u64::from(seed));
    }
    if let Some(delivered) = delta.delivered {
        writer.uvarint(delivered);
    }
    if let Some(items) = &delta.delivered_by_item {
        writer.uvarint(items.len() as u64);
        for item in items {
            writer.uvarint(u64::from(item.item_id));
            writer.uvarint(item.quantity);
        }
    }
    if let Some(insight) = delta.insight {
        writer.uvarint(insight);
    }
    if let Some(victory) = delta.victory {
        writer.bool(victory);
    }
    if let Some(contract) = &delta.contract {
        writer.string(&contract.key);
        writer.string(&contract.name);
        writer.uvarint(u64::from(contract.stage));
        writer.uvarint(u64::from(contract.stages));
        writer.string(&contract.stage_key);
        writer.string(&contract.stage_name);
        writer.string(&contract.stage_brief);
        writer.uvarint(contract.requirements.len() as u64);
        for need in &contract.requirements {
            writer.uvarint(u64::from(need.item_id));
            writer.uvarint(u64::from(need.delivered));
            writer.uvarint(u64::from(need.required));
        }
        writer.bool(contract.complete);
    }
    if let Some(requests) = &delta.requests {
        writer.uvarint(requests.len() as u64);
        for request in requests {
            writer.string(&request.key);
            writer.string(&request.name);
            writer.string(&request.brief);
            writer.uvarint(u64::from(request.item_id));
            writer.uvarint(u64::from(request.delivered));
            writer.uvarint(u64::from(request.required));
            writer.uvarint(u64::from(request.insight));
            writer.u8(project_state_code(request.state));
        }
    }
    if let Some(player) = &delta.player {
        write_player(&mut writer, player);
    }
    if let Some(researched) = &delta.researched {
        writer.uvarint(researched.len() as u64);
        for &technology in researched {
            writer.uvarint(u64::from(technology));
        }
    }
    if let Some(chunks) = &delta.chunks {
        write_chunks(&mut writer, chunks);
    }
    if let Some(terrain) = &delta.terrain {
        write_terrain(&mut writer, terrain);
    }
    if let Some(resources) = &delta.resources {
        writer.u8(if resources.replace { PATCH_REPLACE } else { 0 });
        write_resources(&mut writer, &resources.changed);
    }
    if let Some(buildings) = &delta.buildings {
        writer.u8(if buildings.replace { PATCH_REPLACE } else { 0 });
        write_entities(&mut writer, &buildings.changed);
        writer.uvarint(buildings.removed.len() as u64);
        let mut previous = 0u32;
        for &id in &buildings.removed {
            writer.uvarint(u64::from(id - previous));
            previous = id;
        }
    }
    if let Some(events) = &delta.events {
        writer.uvarint(events.len() as u64);
        for event in events {
            writer.string(event);
        }
    }
    if let Some(ground_items) = &delta.ground_items {
        write_ground_items(&mut writer, ground_items);
    }
    if let Some(availability) = &delta.research_availability {
        writer.uvarint(availability.len() as u64);
        for row in availability {
            writer.uvarint(u64::from(row.technology_id));
            writer.bool(row.complete);
            writer.uvarint(row.insight_shortfall);
            writer.uvarint(row.missing_prerequisites.len() as u64);
            for &id in &row.missing_prerequisites {
                writer.uvarint(u64::from(id));
            }
        }
    }
    if let Some(skills) = &delta.skills {
        writer.uvarint(u64::from(skills.state.points));
        writer.bool(skills.state.sandbox);
        for ids in [
            &skills.state.purchased,
            &skills.state.granted,
            &skills.state.completed,
        ] {
            writer.uvarint(ids.len() as u64);
            for id in ids {
                writer.uvarint(u64::from(*id));
            }
        }
        writer.uvarint(skills.availability.len() as u64);
        for row in &skills.availability {
            writer.uvarint(u64::from(row.skill_id));
            writer.bool(row.complete);
            writer.uvarint(u64::from(row.points_shortfall));
            writer.uvarint(u64::from(row.current_value));
            writer.uvarint(u64::from(row.resulting_value));
            writer.uvarint(row.missing_prerequisites.len() as u64);
            for id in &row.missing_prerequisites {
                writer.uvarint(u64::from(*id));
            }
        }
    }
    writer.bytes
}

fn write_ground_items(writer: &mut Writer, ground_items: &[GroundItem]) {
    writer.uvarint(ground_items.len() as u64);
    for item in ground_items {
        writer.uvarint(u64::from(item.id));
        writer.svarint(i64::from(item.q));
        writer.svarint(i64::from(item.r));
        writer.uvarint(u64::from(item.item_id));
        writer.uvarint(u64::from(item.quantity));
        writer.uvarint(item.despawn_tick);
    }
}

fn write_player(writer: &mut Writer, player: &PlayerSnapshot) {
    let state = &player.state;
    writer.svarint(i64::from(state.x));
    writer.svarint(i64::from(state.y));
    writer.svarint(i64::from(state.facing_x));
    writer.svarint(i64::from(state.facing_y));
    writer.svarint(i64::from(state.move_x));
    writer.svarint(i64::from(state.move_y));
    writer.uvarint(state.inventory.len() as u64);
    for (&item_id, &quantity) in &state.inventory {
        writer.uvarint(u64::from(item_id));
        writer.uvarint(u64::from(quantity));
    }
    writer.uvarint(u64::from(state.action_cooldown));
    writer.uvarint(u64::from(state.build_range));
    writer.uvarint(u64::from(state.carry_slots));
    writer.ingredients(&player.carry_stacks);
    writer.svarint(i64::from(player.radius));
    writer.uvarint(u64::from(player.action_cooldown_total));
    writer.uvarint(u64::from(player.extract_radius));
    writer.bool(player.creative);
    writer.bool(state.hand.is_some());
    if let Some(hand) = state.hand {
        writer.uvarint(u64::from(hand.item_id));
        writer.uvarint(u64::from(hand.quantity));
    }
    writer.bool(state.walk_goal.is_some());
    if let Some(goal) = state.walk_goal {
        writer.svarint(i64::from(goal.q));
        writer.svarint(i64::from(goal.r));
    }
    // The route is a chain of neighbouring hexes, so it delta-codes like the terrain and resource
    // lists above: every step after the first is `-1..=1` on each axis and costs two bytes. The
    // first is coded against the goal rather than against nothing, because a walk near the origin
    // and a walk far from it are the same shape and should cost the same.
    writer.uvarint(player.walk_path.len() as u64);
    let mut previous = state.walk_goal.unwrap_or(Coordinate { q: 0, r: 0 });
    for cell in &player.walk_path {
        writer.svarint(i64::from(cell.q - previous.q));
        writer.svarint(i64::from(cell.r - previous.r));
        previous = *cell;
    }
}

fn write_chunks(writer: &mut Writer, chunks: &[ChunkSnapshot]) {
    writer.uvarint(chunks.len() as u64);
    // Chunk coordinates arrive in generation order rather than sorted, so they are absolute. The
    // list is one entry per generated chunk and never approaches the size of the tile lists.
    for chunk in chunks {
        writer.svarint(i64::from(chunk.chunk_q));
        writer.svarint(i64::from(chunk.chunk_r));
        writer.uvarint(chunk.entity_count as u64);
        writer.svarint(i64::from(chunk.x));
        writer.svarint(i64::from(chunk.y));
        writer.svarint(i64::from(chunk.span));
    }
}

fn write_terrain(writer: &mut Writer, tiles: &[TileSnapshot]) {
    writer.uvarint(tiles.len() as u64);
    let mut previous = Cell::default();
    for tile in tiles {
        previous.write_delta(writer, tile.q, tile.r, tile.x, tile.y);
        writer.uvarint(u64::from(tile.radius));
        writer.u8(terrain_code(tile.terrain));
    }
}

fn write_resources(writer: &mut Writer, resources: &[ResourceSnapshot]) {
    writer.uvarint(resources.len() as u64);
    let mut previous = Cell::default();
    for resource in resources {
        previous.write_delta(writer, resource.q, resource.r, resource.x, resource.y);
        writer.uvarint(u64::from(resource.radius));
        writer.uvarint(u64::from(resource.item_id));
        writer.uvarint(u64::from(resource.quantity));
        writer.uvarint(u64::from(resource.initial_quantity));
    }
}

/// The running cell a tile list is coded against. Both lists are built chunk by chunk, so the step
/// from one entry to the next is a short hop in the world and its world position moves by a fixed
/// multiple of that hop — a byte or two each instead of four full coordinates.
#[derive(Default)]
struct Cell {
    q: i32,
    r: i32,
    x: i32,
    y: i32,
}

impl Cell {
    fn write_delta(&mut self, writer: &mut Writer, q: i32, r: i32, x: i32, y: i32) {
        writer.svarint(i64::from(q) - i64::from(self.q));
        writer.svarint(i64::from(r) - i64::from(self.r));
        writer.svarint(i64::from(x) - i64::from(self.x));
        writer.svarint(i64::from(y) - i64::from(self.y));
        self.q = q;
        self.r = r;
        self.x = x;
        self.y = y;
    }
}

fn write_entities(writer: &mut Writer, entities: &[EntitySnapshot]) {
    writer.uvarint(entities.len() as u64);
    let mut previous_id = 0u32;
    for entity in entities {
        // Ascending by stable id, which the host relies on to merge in one pass; the same ordering
        // makes the id itself cost the gap rather than the value.
        writer.uvarint(u64::from(entity.id - previous_id));
        previous_id = entity.id;
        writer.svarint(i64::from(entity.q));
        writer.svarint(i64::from(entity.r));
        writer.uvarint(u64::from(entity.definition_id));
        writer.u8(kind_code(entity.kind));
        writer.u8(entity.orientation);

        let mut flags = 0u16;
        if entity.recipe_id.is_some() {
            flags |= entity_flag::RECIPE_ID;
        }
        if entity.scenario_owned {
            flags |= entity_flag::SCENARIO_OWNED;
        }
        if entity.cargo.is_some() {
            flags |= entity_flag::CARGO;
        }
        if entity.fuel_charge != 0 {
            flags |= entity_flag::FUEL_CHARGE;
        }
        if entity.fuel_required != 0 {
            flags |= entity_flag::FUEL_REQUIRED;
        }
        if entity.next_id.is_some() {
            flags |= entity_flag::NEXT_ID;
        }
        if !entity.branch_ids.is_empty() {
            flags |= entity_flag::BRANCH_IDS;
        }
        if !entity.input_inventory.is_empty() {
            flags |= entity_flag::INPUT_INVENTORY;
        }
        if !entity.fuel_inventory.is_empty() {
            flags |= entity_flag::FUEL_INVENTORY;
        }
        if !entity.output_inventory.is_empty() {
            flags |= entity_flag::OUTPUT_INVENTORY;
        }
        if entity.power_satisfied != 0 {
            flags |= entity_flag::POWER_SATISFIED;
        }
        if entity.power_demand != 0 {
            flags |= entity_flag::POWER_DEMAND;
        }
        if entity.power_charge != 0 {
            flags |= entity_flag::POWER_CHARGE;
        }
        if entity.power_capacity != 0 {
            flags |= entity_flag::POWER_CAPACITY;
        }
        writer.uvarint(u64::from(flags));

        if let Some(recipe_id) = entity.recipe_id {
            writer.uvarint(u64::from(recipe_id));
        }
        if let Some(cargo) = entity.cargo {
            writer.uvarint(u64::from(cargo.item_id));
            writer.uvarint(u64::from(cargo.quantity));
        }
        writer.ingredients(&entity.inventory);
        if flags & entity_flag::INPUT_INVENTORY != 0 {
            writer.ingredients(&entity.input_inventory);
        }
        if flags & entity_flag::FUEL_INVENTORY != 0 {
            writer.ingredients(&entity.fuel_inventory);
        }
        if flags & entity_flag::OUTPUT_INVENTORY != 0 {
            writer.ingredients(&entity.output_inventory);
        }
        writer.uvarint(u64::from(entity.progress));
        writer.uvarint(u64::from(entity.progress_total));
        if entity.fuel_charge != 0 {
            writer.uvarint(u64::from(entity.fuel_charge));
        }
        if entity.fuel_required != 0 {
            writer.uvarint(u64::from(entity.fuel_required));
        }
        writer.u8(status_code(entity.status));
        if let Some(next_id) = entity.next_id {
            writer.uvarint(u64::from(next_id));
        }
        if !entity.branch_ids.is_empty() {
            writer.uvarint(entity.branch_ids.len() as u64);
            for branch_id in &entity.branch_ids {
                writer.uvarint(u64::from(*branch_id));
            }
        }
        if entity.power_satisfied != 0 {
            writer.uvarint(u64::from(entity.power_satisfied));
        }
        if entity.power_demand != 0 {
            writer.uvarint(u64::from(entity.power_demand));
        }
        if entity.power_charge != 0 {
            writer.uvarint(u64::from(entity.power_charge));
        }
        if entity.power_capacity != 0 {
            writer.uvarint(u64::from(entity.power_capacity));
        }
        // Against the entity's own hex, so the single-cell footprint every belt and machine has
        // costs two bytes rather than two full coordinates.
        writer.uvarint(entity.footprint.len() as u64);
        for cell in &entity.footprint {
            writer.svarint(i64::from(cell.q) - i64::from(entity.q));
            writer.svarint(i64::from(cell.r) - i64::from(entity.r));
        }
    }
}

/// The decoder exists only to pin the encoder.
///
/// `src/core/snapshotWire.ts` is the shipped decoder and the fixture is what proves the two
/// languages agree on one artifact. This one buys something the fixture cannot: a round trip over
/// every delta a real workload produces, which reaches entity shapes — a three-cell footprint on a
/// fuelled machine holding cargo, a removal list beside a replace — that no hand-written fixture
/// enumerates. See `wire_round_trips_every_delta_of_a_running_factory`.
#[cfg(test)]
pub(crate) mod decode {
    use super::*;

    pub(crate) struct Reader<'a> {
        bytes: &'a [u8],
        offset: usize,
    }

    impl<'a> Reader<'a> {
        fn u8(&mut self) -> u8 {
            let value = self.bytes[self.offset];
            self.offset += 1;
            value
        }

        fn bool(&mut self) -> bool {
            self.u8() == 1
        }

        fn u32_fixed(&mut self) -> u32 {
            let mut raw = [0u8; 4];
            raw.copy_from_slice(&self.bytes[self.offset..self.offset + 4]);
            self.offset += 4;
            u32::from_le_bytes(raw)
        }

        fn uvarint(&mut self) -> u64 {
            let mut value = 0u64;
            let mut shift = 0u32;
            loop {
                let byte = self.u8();
                value |= u64::from(byte & 0x7f) << shift;
                if byte & 0x80 == 0 {
                    return value;
                }
                shift += 7;
            }
        }

        fn svarint(&mut self) -> i64 {
            let raw = self.uvarint();
            ((raw >> 1) as i64) ^ -((raw & 1) as i64)
        }

        fn string(&mut self) -> String {
            let length = self.uvarint() as usize;
            let text = std::str::from_utf8(&self.bytes[self.offset..self.offset + length]).unwrap();
            self.offset += length;
            text.to_owned()
        }

        fn count(&mut self) -> usize {
            self.uvarint() as usize
        }

        fn ingredients(&mut self) -> Vec<Ingredient> {
            (0..self.count())
                .map(|_| Ingredient {
                    item_id: self.uvarint() as ItemId,
                    quantity: self.uvarint() as u32,
                })
                .collect()
        }
    }

    fn kind_of(code: u8) -> BuildingKind {
        [
            BuildingKind::Extractor,
            BuildingKind::Belt,
            BuildingKind::Composer,
            BuildingKind::Container,
            BuildingKind::Consumer,
            BuildingKind::Hub,
            BuildingKind::Pump,
            BuildingKind::Pole,
            BuildingKind::Generator,
            BuildingKind::Boiler,
        ][usize::from(code)]
    }

    fn terrain_of(code: u8) -> Terrain {
        [
            Terrain::DeepWater,
            Terrain::ShallowWater,
            Terrain::Shore,
            Terrain::Lowland,
            Terrain::Hills,
            Terrain::Highland,
            Terrain::Cliff,
        ][usize::from(code)]
    }

    fn status_of(code: u8) -> EntityStatus {
        [
            EntityStatus::OutputBlocked,
            EntityStatus::DepositDepleted,
            EntityStatus::Extracting,
            EntityStatus::NoWaterInReach,
            EntityStatus::Pumping,
            EntityStatus::Composing,
            EntityStatus::OutOfFuel,
            EntityStatus::WaitingForInputs,
            EntityStatus::Buffered,
            EntityStatus::Carrying,
            EntityStatus::Receiving,
            EntityStatus::LandingHub,
            EntityStatus::Idle,
            EntityStatus::NoPower,
            EntityStatus::Generating,
            EntityStatus::Brownout,
            EntityStatus::NoBoiler,
            EntityStatus::SwitchedOff,
        ][usize::from(code)]
    }

    pub(crate) fn decode_delta(bytes: &[u8]) -> SnapshotDelta {
        let mut reader = Reader { bytes, offset: 0 };
        assert_eq!(&bytes[0..4], &WIRE_MAGIC, "wire magic");
        reader.offset = 4;
        assert_eq!(reader.u8(), WIRE_VERSION, "wire version");
        let base_revision = reader.uvarint();
        let revision = reader.uvarint();
        let tick = reader.uvarint();
        let checksum = reader.u32_fixed();
        let mask = reader.uvarint() as u32;
        let has = |bit: u32| mask & bit != 0;

        let scenario = has(group::SCENARIO).then(|| reader.string());
        let scenario_name = has(group::SCENARIO_NAME).then(|| reader.string());
        let world_version = has(group::WORLD_VERSION).then(|| reader.uvarint() as u16);
        let seed = has(group::SEED).then(|| reader.uvarint() as u32);
        let delivered = has(group::DELIVERED).then(|| reader.uvarint());
        let delivered_by_item = has(group::DELIVERED_BY_ITEM).then(|| {
            (0..reader.count())
                .map(|_| Ingredient64 {
                    item_id: reader.uvarint() as ItemId,
                    quantity: reader.uvarint(),
                })
                .collect()
        });
        let insight = has(group::INSIGHT).then(|| reader.uvarint());
        let victory = has(group::VICTORY).then(|| reader.bool());
        let contract = has(group::CONTRACT).then(|| ContractSnapshot {
            key: reader.string(),
            name: reader.string(),
            stage: reader.uvarint() as u16,
            stages: reader.uvarint() as u16,
            stage_key: reader.string(),
            stage_name: reader.string(),
            stage_brief: reader.string(),
            requirements: (0..reader.count())
                .map(|_| ContractRequirement {
                    item_id: reader.uvarint() as ItemId,
                    delivered: reader.uvarint() as u32,
                    required: reader.uvarint() as u32,
                })
                .collect(),
            complete: reader.bool(),
        });
        let requests = has(group::REQUESTS).then(|| {
            (0..reader.count())
                .map(|_| RequestSnapshot {
                    key: reader.string(),
                    name: reader.string(),
                    brief: reader.string(),
                    item_id: reader.uvarint() as ItemId,
                    delivered: reader.uvarint() as u32,
                    required: reader.uvarint() as u32,
                    insight: reader.uvarint() as u32,
                    state: project_state(reader.u8()),
                })
                .collect()
        });
        let player = has(group::PLAYER).then(|| read_player(&mut reader));
        let researched = has(group::RESEARCHED).then(|| {
            (0..reader.count())
                .map(|_| reader.uvarint() as TechnologyId)
                .collect()
        });
        let chunks = has(group::CHUNKS).then(|| {
            (0..reader.count())
                .map(|_| ChunkSnapshot {
                    chunk_q: reader.svarint() as i32,
                    chunk_r: reader.svarint() as i32,
                    entity_count: reader.uvarint() as usize,
                    x: reader.svarint() as i32,
                    y: reader.svarint() as i32,
                    span: reader.svarint() as i32,
                })
                .collect()
        });
        let terrain = has(group::TERRAIN).then(|| {
            let count = reader.count();
            let mut cell = Cell::default();
            (0..count)
                .map(|_| {
                    let (q, r, x, y) = read_cell(&mut reader, &mut cell);
                    TileSnapshot {
                        q,
                        r,
                        x,
                        y,
                        radius: reader.uvarint() as u32,
                        terrain: terrain_of(reader.u8()),
                    }
                })
                .collect()
        });
        let resources = has(group::RESOURCES).then(|| {
            let replace = reader.u8() & PATCH_REPLACE != 0;
            let count = reader.count();
            let mut cell = Cell::default();
            let changed = (0..count)
                .map(|_| {
                    let (q, r, x, y) = read_cell(&mut reader, &mut cell);
                    ResourceSnapshot {
                        q,
                        r,
                        x,
                        y,
                        radius: reader.uvarint() as u32,
                        item_id: reader.uvarint() as ItemId,
                        quantity: reader.uvarint() as u32,
                        initial_quantity: reader.uvarint() as u32,
                    }
                })
                .collect();
            ResourcesDelta { replace, changed }
        });
        let buildings = has(group::BUILDINGS).then(|| {
            let replace = reader.u8() & PATCH_REPLACE != 0;
            let changed = read_entities(&mut reader);
            let removed_count = reader.count();
            let mut previous = 0u32;
            let removed = (0..removed_count)
                .map(|_| {
                    previous += reader.uvarint() as u32;
                    previous
                })
                .collect();
            BuildingsDelta {
                replace,
                changed,
                removed,
            }
        });
        let events =
            has(group::EVENTS).then(|| (0..reader.count()).map(|_| reader.string()).collect());
        let ground_items = has(group::GROUND_ITEMS).then(|| {
            (0..reader.count())
                .map(|_| GroundItem {
                    id: reader.uvarint() as u32,
                    q: reader.svarint() as i32,
                    r: reader.svarint() as i32,
                    item_id: reader.uvarint() as ItemId,
                    quantity: reader.uvarint() as u32,
                    despawn_tick: reader.uvarint(),
                })
                .collect()
        });

        let research_availability = has(group::RESEARCH_AVAILABILITY).then(|| {
            (0..reader.count())
                .map(|_| {
                    let technology_id = reader.uvarint() as TechnologyId;
                    let complete = reader.bool();
                    let insight_shortfall = reader.uvarint();
                    let missing_prerequisites = (0..reader.count())
                        .map(|_| reader.uvarint() as TechnologyId)
                        .collect();
                    ResearchAvailability {
                        technology_id,
                        complete,
                        insight_shortfall,
                        missing_prerequisites,
                    }
                })
                .collect()
        });

        let skills = has(group::SKILLS).then(|| {
            let points = reader.uvarint() as u32;
            let sandbox = reader.bool();
            let purchased = (0..reader.count())
                .map(|_| reader.uvarint() as u16)
                .collect();
            let granted = (0..reader.count())
                .map(|_| reader.uvarint() as u16)
                .collect();
            let completed = (0..reader.count())
                .map(|_| reader.uvarint() as u16)
                .collect();
            let availability = (0..reader.count())
                .map(|_| {
                    let skill_id = reader.uvarint() as u16;
                    let complete = reader.bool();
                    let points_shortfall = reader.uvarint() as u32;
                    let current_value = reader.uvarint() as u32;
                    let resulting_value = reader.uvarint() as u32;
                    let missing_prerequisites = (0..reader.count())
                        .map(|_| reader.uvarint() as u16)
                        .collect();
                    SkillAvailability {
                        skill_id,
                        complete,
                        points_shortfall,
                        current_value,
                        resulting_value,
                        missing_prerequisites,
                    }
                })
                .collect();
            SkillsSnapshot {
                state: SkillsState {
                    points,
                    sandbox,
                    purchased,
                    granted,
                    completed,
                },
                availability,
            }
        });

        assert_eq!(
            reader.offset,
            bytes.len(),
            "decoder consumed the whole buffer"
        );
        SnapshotDelta {
            base_revision,
            revision,
            tick,
            checksum,
            scenario,
            scenario_name,
            world_version,
            seed,
            delivered,
            delivered_by_item,
            insight,
            victory,
            contract,
            requests,
            player,
            researched,
            research_availability,
            skills,
            chunks,
            terrain,
            resources,
            buildings,
            ground_items,
            events,
        }
    }

    fn read_cell(reader: &mut Reader, cell: &mut Cell) -> (i32, i32, i32, i32) {
        cell.q += reader.svarint() as i32;
        cell.r += reader.svarint() as i32;
        cell.x += reader.svarint() as i32;
        cell.y += reader.svarint() as i32;
        (cell.q, cell.r, cell.x, cell.y)
    }

    fn read_player(reader: &mut Reader) -> PlayerSnapshot {
        let x = reader.svarint() as i32;
        let y = reader.svarint() as i32;
        let facing_x = reader.svarint() as i16;
        let facing_y = reader.svarint() as i16;
        let move_x = reader.svarint() as i16;
        let move_y = reader.svarint() as i16;
        let entries = reader.count();
        let mut inventory = BTreeMap::new();
        for _ in 0..entries {
            let item_id = reader.uvarint() as ItemId;
            inventory.insert(item_id, reader.uvarint() as u32);
        }
        let action_cooldown = reader.uvarint() as u32;
        let build_range = reader.uvarint() as u32;
        let carry_slots = reader.uvarint() as u32;
        let carry_stacks = reader.ingredients();
        let radius = reader.svarint() as i32;
        let action_cooldown_total = reader.uvarint() as u32;
        let extract_radius = reader.uvarint() as u32;
        let creative = reader.bool();
        let hand = reader.bool().then(|| Cargo {
            item_id: reader.uvarint() as ItemId,
            quantity: reader.uvarint() as u32,
        });
        let walk_goal = reader.bool().then(|| Coordinate {
            q: reader.svarint() as i32,
            r: reader.svarint() as i32,
        });
        let cells = reader.count();
        let mut walk_path = Vec::with_capacity(cells);
        let mut previous = walk_goal.unwrap_or(Coordinate { q: 0, r: 0 });
        for _ in 0..cells {
            previous = Coordinate {
                q: previous.q + reader.svarint() as i32,
                r: previous.r + reader.svarint() as i32,
            };
            walk_path.push(previous);
        }
        PlayerSnapshot {
            state: PlayerState {
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
                walk_goal,
            },
            carry_stacks,
            radius,
            action_cooldown_total,
            extract_radius,
            creative,
            walk_path,
        }
    }

    fn read_entities(reader: &mut Reader) -> Vec<EntitySnapshot> {
        let count = reader.count();
        let mut id = 0u32;
        let mut entities = Vec::with_capacity(count);
        for _ in 0..count {
            id += reader.uvarint() as u32;
            let q = reader.svarint() as i32;
            let r = reader.svarint() as i32;
            let definition_id = reader.uvarint() as DefinitionId;
            let kind = kind_of(reader.u8());
            let orientation = reader.u8();
            let flags = reader.uvarint() as u16;
            let recipe_id =
                (flags & entity_flag::RECIPE_ID != 0).then(|| reader.uvarint() as RecipeId);
            let cargo = (flags & entity_flag::CARGO != 0).then(|| Cargo {
                item_id: reader.uvarint() as ItemId,
                quantity: reader.uvarint() as u32,
            });
            let inventory = reader.ingredients();
            let input_inventory = if flags & entity_flag::INPUT_INVENTORY != 0 {
                reader.ingredients()
            } else {
                Vec::new()
            };
            let fuel_inventory = if flags & entity_flag::FUEL_INVENTORY != 0 {
                reader.ingredients()
            } else {
                Vec::new()
            };
            let output_inventory = if flags & entity_flag::OUTPUT_INVENTORY != 0 {
                reader.ingredients()
            } else {
                Vec::new()
            };
            let progress = reader.uvarint() as u32;
            let progress_total = reader.uvarint() as u32;
            let fuel_charge = if flags & entity_flag::FUEL_CHARGE != 0 {
                reader.uvarint() as u32
            } else {
                0
            };
            let fuel_required = if flags & entity_flag::FUEL_REQUIRED != 0 {
                reader.uvarint() as u32
            } else {
                0
            };
            let status = status_of(reader.u8());
            let next_id = (flags & entity_flag::NEXT_ID != 0).then(|| reader.uvarint() as u32);
            let branch_ids = if flags & entity_flag::BRANCH_IDS != 0 {
                let count = reader.uvarint() as usize;
                (0..count).map(|_| reader.uvarint() as u32).collect()
            } else {
                Vec::new()
            };
            let power_satisfied = if flags & entity_flag::POWER_SATISFIED != 0 {
                reader.uvarint() as u32
            } else {
                0
            };
            let power_demand = if flags & entity_flag::POWER_DEMAND != 0 {
                reader.uvarint() as u32
            } else {
                0
            };
            let power_charge = if flags & entity_flag::POWER_CHARGE != 0 {
                reader.uvarint() as u32
            } else {
                0
            };
            let power_capacity = if flags & entity_flag::POWER_CAPACITY != 0 {
                reader.uvarint() as u32
            } else {
                0
            };
            let cells = reader.count();
            let footprint = (0..cells)
                .map(|_| Coordinate {
                    q: q + reader.svarint() as i32,
                    r: r + reader.svarint() as i32,
                })
                .collect();
            entities.push(EntitySnapshot {
                id,
                q,
                r,
                definition_id,
                kind,
                orientation,
                recipe_id,
                scenario_owned: flags & entity_flag::SCENARIO_OWNED != 0,
                cargo,
                inventory,
                input_inventory,
                fuel_inventory,
                output_inventory,
                progress,
                progress_total,
                fuel_charge,
                fuel_required,
                power_satisfied,
                power_demand,
                power_charge,
                power_capacity,
                status,
                next_id,
                branch_ids,
                footprint,
            });
        }
        entities
    }
}
