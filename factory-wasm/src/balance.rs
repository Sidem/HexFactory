//! What the shipped numbers actually say the economy is.
//!
//! Balance was the one system in this project with no representation. Hex directions are pinned in
//! two languages, terrain passability is a fixture, the wire format is bytes beside the JSON they
//! decode to — and the economy was twenty buildings, fourteen recipes, and twenty-three items with
//! nothing anywhere stating what the curve was meant to be. A steam turbine outputs 48 and a
//! smelter draws 10, so one turbine runs nearly five smelters. That may be exactly right. Nothing
//! said.
//!
//! This module computes the figures that decide whether an economy works, none of which the data
//! states directly:
//!
//! - what each machine makes per minute at its own cadence, and what it eats to do it;
//! - what each generator carries, and what it burns and drinks to carry it;
//! - **the full raw-material cost of every building expanded through its entire recipe tree**,
//!   which is the number a cost row hides, because a building's own row says nothing about what
//!   its inputs cost to make;
//! - what an extraction site is actually worth under each world preset;
//! - what the opening costs in gathers, research, and machine time.
//!
//! It is derived, never authoritative: every number here is a restatement of `definitions.json`,
//! `technologies.json`, and the rules the tick already implements. It lives in Rust rather than in
//! the host for exactly that reason — items per minute is a restatement of `advance_composer`, and
//! machines carried is a restatement of `power_progress`. A second implementation of a native rule
//! is the defect, not the feature.
//!
//! Measurement code, like the capacity ladder and the survey: native only, never compiled into the
//! wasm artifact, and never a dependency of the game or the production build. `npm run balance`
//! prints it; `fixtures/balance.json` records it, and both languages are pinned to that file.

// `Ordering` is `#[cfg(test)]` at the crate root, so it is imported here rather than inherited.
use std::cmp::Ordering;

use super::*;

const DEFINITIONS: &str = include_str!("../../src/data/definitions.json");
const TECHNOLOGIES: &str = include_str!("../../src/data/technologies.json");
const SCENARIOS: &str = include_str!("../../src/data/scenarios.json");

/// The simulation rate every per-minute figure here is quoted at: the `index.html` default, which
/// is the pace a player who never touches the speed control plays at. It is a unit on the numbers,
/// not a property of the economy — the ratios between machines hold at any speed, and only the
/// comparison against the player's own hands moves, because the player's clock is real time and
/// deliberately not the factory's.
pub const REFERENCE_TICKS_PER_SECOND: u32 = 10;

/// How far the extraction survey samples, matching `survey::DEFAULT_RADIUS` so a yield figure and
/// a density figure are talking about the same disc.
pub const YIELD_RADIUS: i32 = 96;

/// The reaches a site is priced at: the base extractor and the deep one.
const YIELD_REACHES: [i32; 2] = [1, 2];

// ---------------------------------------------------------------------------------------------
// Exact rational arithmetic
//
// A kiln fires three bricks at once, so a pump that wants four of them costs four thirds of a
// batch. Rounding that at every step of a tree and then comparing two buildings would be
// comparing rounding errors, so the expansion is exact and rounds once, at the edge, when it is
// written down.
// ---------------------------------------------------------------------------------------------

fn gcd(a: u128, b: u128) -> u128 {
    if b == 0 {
        a.max(1)
    } else {
        gcd(b, a % b)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ratio {
    num: i128,
    den: i128,
}

impl Ratio {
    const ZERO: Ratio = Ratio { num: 0, den: 1 };

    fn new(num: i128, den: i128) -> Self {
        let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
        let divisor = gcd(num.unsigned_abs(), den.unsigned_abs()) as i128;
        Ratio {
            num: num / divisor,
            den: den / divisor,
        }
    }

    fn whole(value: u32) -> Self {
        Ratio {
            num: i128::from(value),
            den: 1,
        }
    }

    fn add(self, other: Self) -> Self {
        Ratio::new(
            self.num * other.den + other.num * self.den,
            self.den * other.den,
        )
    }

    fn mul(self, other: Self) -> Self {
        Ratio::new(self.num * other.num, self.den * other.den)
    }

    fn over(self, value: u32) -> Self {
        Ratio::new(self.num, self.den * i128::from(value))
    }

    fn is_zero(self) -> bool {
        self.num == 0
    }

    /// Thousandths, rounded half up. One rounding, at the edge, on a number the tree computed
    /// exactly — and the same rule the TypeScript side rounds by, which is what lets a fixture
    /// written by one language be recomputed by the other.
    fn milli(self) -> u64 {
        ((self.num * 1000 + self.den / 2) / self.den).max(0) as u64
    }
}

impl PartialOrd for Ratio {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ratio {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.num * other.den).cmp(&(other.num * self.den))
    }
}

fn divide_up(value: u64, divisor: u64) -> u64 {
    if divisor == 0 {
        0
    } else {
        value.div_ceil(divisor)
    }
}

// ---------------------------------------------------------------------------------------------
// Reported shapes
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct Amount {
    pub item: String,
    pub quantity: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MilliAmount {
    pub item: String,
    /// Thousandths of an item. A recipe that outputs three makes a cost a fraction of a craft.
    pub quantity_milli: u64,
}

/// The units every other number here is quoted in.
#[derive(Clone, Debug, Serialize)]
pub struct Reference {
    pub definition_version: u16,
    pub technology_version: u16,
    pub ticks_per_second: u32,
    pub player_ticks_per_second: u32,
    pub gather_cooldown_steps: u32,
    /// What the player's own hands are worth at the *fastest* material, against the same wall
    /// clock the factory runs on. Wood matches an extractor; everything harder is slower, and
    /// signal crystal has no hand rate at all. `hand_gathers` is the per-material table.
    pub hand_items_per_minute: u32,
    /// One row per item the hand can take, with its own cooldown. Crystal is absent.
    pub hand_gathers: Vec<HandGather>,
    /// Field cells one hex covers at the hand's reach, and at each shipped extractor reach.
    pub cells_in_reach: Vec<u32>,
    /// The densest fuel item. Energy costs are quoted in units of it, because that is the fewest
    /// items any amount of heat can be paid with.
    pub best_fuel_item: String,
    pub best_fuel_value: u32,
    /// What the opening guarantees on every seed, and how far the player walks for it. The
    /// clearing used to carry a hardcoded unit count of every material at once; the generator
    /// places real patches now, so what is fixed is the window and not the quantity.
    pub guaranteed_opening: Vec<Guarantee>,
}

/// One promise the bootstrap pass makes, in the distance a player walks to keep it.
#[derive(Clone, Debug, Serialize)]
pub struct Guarantee {
    pub item: String,
    pub walk_min: u32,
    pub walk_max: u32,
}

/// One material the hand can take, priced on the player's own clock.
#[derive(Clone, Debug, Serialize)]
pub struct HandGather {
    pub item: String,
    pub steps: u32,
    pub items_per_minute: u32,
}

/// One machine running one thing, at its own cadence.
#[derive(Clone, Debug, Serialize)]
pub struct MachineRate {
    pub building: String,
    #[serde(skip_serializing_if = "is_false")]
    pub manual_work: bool,
    pub recipe: Option<String>,
    /// Ticks from the start of one output to the start of the next, unblocked and fully powered:
    /// a source's `cadence`, a composer's recipe `duration`.
    pub ticks_per_cycle: u32,
    pub output_item: Option<String>,
    pub output_per_cycle: u32,
    pub per_minute_milli: u64,
    /// What it eats per minute to hold that rate. This is the pairing number: a smelter's input
    /// rate against an extractor's output rate is how many extractors feed a smelter.
    pub inputs_per_minute: Vec<MilliAmount>,
    pub power_draw: u32,
    /// Electricity one whole cycle costs: the draw against the cycle it is spent over.
    ///
    /// This is the number the Power Grid made real. `power_draw` is a rate against *progress*, so
    /// a machine that spends half its life blocked pays half as much clock but exactly this much
    /// per thing it makes. Divide it by a plant's `grid_energy_per_fuel_item` for what one craft
    /// costs in coal.
    pub grid_energy_per_cycle: u32,
    pub grid_energy_per_minute_milli: u64,
    pub fuel_energy_per_cycle: u32,
    pub fuel_energy_per_minute_milli: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlantCarries {
    pub building: String,
    pub power_draw: u32,
    /// How many of that machine this plant runs at full rate, after its own upkeep.
    pub machines: u32,
}

/// A generator, and everything it takes to keep it live.
#[derive(Clone, Debug, Serialize)]
pub struct PowerPlant {
    pub building: String,
    pub source: String,
    pub output: u32,
    /// The building this one is dead without. A turbine's fuel and water are a boiler's.
    pub requires: Option<String>,
    /// Fuel energy a tick costs *at full output*, which is now a ceiling rather than a standing
    /// charge: a plant is billed for the electricity it actually hands over, so one carrying a
    /// fifth of the load pays a fifth of this.
    pub fuel_energy_per_tick: u32,
    pub water_per_tick: u32,
    /// Ticks one unit of the densest fuel sustains it at full output.
    pub ticks_per_fuel_item: u32,
    /// Electricity one unit of the densest fuel buys through this plant. The exchange rate a
    /// machine's `grid_energy_per_cycle` is priced against, and the reason a wind turbine and a
    /// burner are not the same machine with different numbers.
    pub grid_energy_per_fuel_item: u32,
    /// Pumps needed to keep the water up, at the shipped pump cadence, and what they draw. A
    /// generator whose upkeep eats its own output is not a generator.
    pub pumps_required: u32,
    pub upkeep_draw: u32,
    pub net_output: i64,
    pub carries: Vec<PlantCarries>,
}

/// What a fuel is worth, and what it cost to make.
///
/// Fuel is a property of the item, so a recipe that turns one fuel into another is only worth
/// running if it ends up ahead — and nothing in the data says whether it does. This is the row
/// that answers it, and the answer for a fuel is not a matter of taste: a conversion that returns
/// less energy than it consumed is a recipe with no reason to exist.
#[derive(Clone, Debug, Serialize)]
pub struct FuelConversion {
    pub item: String,
    pub fuel_value: u32,
    pub recipe: Option<String>,
    /// Energy one craft's inputs already carried, plus the heat the craft itself burns.
    pub input_energy: u32,
    /// Energy one craft's output carries.
    pub output_energy: u32,
    /// Output over input, in thousandths. Under 1000 destroys energy; exactly 1000 is a recipe
    /// that costs a machine, a power draw, and a duration to hand back what it was given.
    pub gain_milli: Option<u64>,
}

/// A craftable item, expanded to the ground.
#[derive(Clone, Debug, Serialize)]
pub struct ItemCost {
    pub item: String,
    /// How many recipes deep the tree goes. Zero is a raw material — something a hand or an
    /// extractor takes straight out of the world.
    pub depth: u32,
    pub raw: Vec<MilliAmount>,
    pub raw_units_milli: u64,
    pub fuel_energy_milli: u64,
    /// Machine ticks the whole tree spends to make one, one machine at a time.
    pub machine_ticks_milli: u64,
    /// Raw units plus fuel paid in the densest fuel item: one comparable scalar, in the only
    /// weighting the data itself supplies.
    pub effort_milli: u64,
}

/// A building, expanded to the ground. The number a cost row hides.
#[derive(Clone, Debug, Serialize)]
pub struct BuildingCost {
    pub building: String,
    pub kind: String,
    pub tier: u8,
    pub unlock_technology: Option<String>,
    pub direct: Vec<Amount>,
    pub depth: u32,
    /// Exact proportional cost: what one costs if a batch's leftovers go somewhere useful.
    pub raw: Vec<MilliAmount>,
    pub raw_units_milli: u64,
    pub fuel_energy_milli: u64,
    pub effort_milli: u64,
    /// What a player actually spends building exactly one, crafting whole batches and eating the
    /// leftovers. Never smaller than the proportional cost, and the gap is the lumpiness.
    pub batch: Vec<Amount>,
    pub batch_units: u64,
    pub batch_fuel_energy: u64,
    pub machine_ticks: u64,
}

/// One ordered pair on the curve, and whether the order holds.
#[derive(Clone, Debug, Serialize)]
pub struct CurveStep {
    pub building: String,
    pub follows: String,
    /// `upgrade` — the same machine, one tier up, where the later one must cost strictly more.
    /// `kind` — the same kind of machine unlocked behind the other's technology, where the later
    /// one must not cost less.
    pub relation: String,
    pub effort_milli: u64,
    pub follows_effort_milli: u64,
    pub holds: bool,
}

/// Whether a material can actually be had, from the site the game starts you on.
#[derive(Clone, Debug, Serialize)]
pub struct MaterialAccess {
    pub material: String,
    /// How far the opening guarantees this material, if it guarantees it at all: the distance from
    /// the landing site to the nearest hex of the patch the bootstrap pass placed, under the
    /// default preset. This replaced a hardcoded cell inside the clearing — the generator makes
    /// the promise now, so the promise is measured rather than written down.
    pub guaranteed_walk: Option<u32>,
    pub guaranteed_hexes: u32,
    /// Distance to the nearest generated cell outside the clearing, under the default preset.
    pub nearest_generated: Option<u32>,
    /// Whether something can stand where it reaches that material. Stone sits on cliffs nothing
    /// can stand on and is quarried from the hex beside them, so "there is some" and "you can have
    /// some" are different questions and this is the second one.
    pub reachable: bool,
    /// Which recipes and construction costs bottom out in this material.
    pub required_by: u32,
}

/// What an extraction site is worth, measured rather than argued from the rule table.
#[derive(Clone, Debug, Serialize)]
pub struct SiteYield {
    pub preset: String,
    pub material: String,
    pub sites: u32,
    pub mean_cell_quantity: u32,
    /// Units an extractor standing on a cell of this material can draw before the site is worked
    /// out, summed over every field cell inside its reach — which is what an extractor actually
    /// empties, since it walks its whole candidate list and does not care which material a cell
    /// holds.
    pub mean_site_yield: Vec<u32>,
    /// The share of that which is the material the site was named for. The rest arrives on the
    /// same belt, which is a fact about a factory's layout and not a rounding note.
    pub mean_same_material: Vec<u32>,
}

/// What it costs to get to a thing, from a standing start.
#[derive(Clone, Debug, Serialize)]
pub struct Opening {
    pub name: String,
    pub technologies: Vec<String>,
    pub insight: u32,
    /// Items delivered to the hub to pay for that research, through the cheapest standing request
    /// the landing clearing can supply.
    pub insight_items: u32,
    /// Which request that is. A funding cost with no request named would be a number quoted against
    /// a rate nobody posted.
    pub insight_request: String,
    /// Contract stages this opening has to deliver before it can start, because a technology it
    /// needs is granted by finishing one rather than sold for insight. Empty when every technology
    /// on the path is purchasable.
    pub commissions: Vec<String>,
    pub buildings: Vec<String>,
    /// Dependency order: each station's bill is made only by earlier, powered stations.
    pub construction_order: Vec<String>,
    pub gathers: Vec<Amount>,
    pub fuel_energy: u64,
    /// Units of the densest fuel that energy is paid with, counted into the gather total.
    pub fuel_items: u64,
    pub gather_total: u64,
    pub machine_ticks: u64,
    /// Attended factory-clock ticks, kept separate from unattended machine work.
    pub player_work_ticks: u64,
    /// A floor, not a prediction: hand time at the player's own clock plus machine time at the
    /// reference rate, with no walking, no travel, and one machine of each kind. What it excludes
    /// is what a playtest measures.
    pub hand_seconds_milli: u64,
    pub machine_seconds_milli: u64,
    pub player_work_seconds_milli: u64,
}

/// What the landing hub is asking for, priced the same way an opening is.
///
/// This is the row a founding contract has to face. A bill written from the catalogue alone is a
/// bill argued from a quarter of the numbers: `16 iron plate` says nothing about the thirty-two ore
/// under it, the fuel that smelts them, the machines the chain needs, or the research those
/// machines sit behind. A stage that never reaches this section is a demand nothing has compared
/// against the curve.
#[derive(Clone, Debug, Serialize)]
pub struct ContractCost {
    pub scenario: String,
    pub contract: String,
    pub stage: String,
    /// The bill exactly as the scenario states it, before any expansion.
    pub bill: Vec<Amount>,
    /// How many distinct raw materials the whole tree bottoms out in. A founding project is meant
    /// to need more than one landscape, and this is the number that says whether it does.
    pub raw_materials: usize,
    /// Everything the stage costs from a standing start, priced exactly like an opening.
    pub opening: Opening,
}

/// One standing request, priced through the same tree an opening is.
///
/// A reward on its own is a number with no denominator. What decides whether a request is worth
/// filling — and whether the ladder of them rewards processing at all — is the insight against the
/// raw units and the machine time underneath the item. A tier that pays no better per gather than
/// the tier below it is a tier nobody has a reason to automate.
#[derive(Clone, Debug, Serialize)]
pub struct RequestCost {
    pub request: String,
    pub item: String,
    pub quantity: u32,
    pub insight: u32,
    /// What a later fill pays. Equal to `insight` when the row does not decay.
    pub repeat_insight: u32,
    /// The raw materials the bill bottoms out in.
    pub gathers: Vec<Amount>,
    /// Units of the densest fuel the crafting energy is paid with, counted into the gather total.
    pub fuel_items: u64,
    pub gather_total: u64,
    /// Insight per thousand gathers. A raw request sits at about a thousand — one insight for one
    /// gather, which is the rate the old per-item currency paid for everything.
    pub insight_per_gather_milli: u64,
    pub repeat_insight_per_gather_milli: u64,
    /// First-fill insight per minute of hand time (and machine time, for a processed row), using
    /// each material's own `hand_gather_steps`.
    pub insight_per_minute_milli: u64,
    pub repeat_insight_per_minute_milli: u64,
    pub machine_ticks: u64,
    pub machine_seconds_milli: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BalanceReport {
    pub reference: Reference,
    pub machines: Vec<MachineRate>,
    pub power: Vec<PowerPlant>,
    pub fuel: Vec<FuelConversion>,
    pub items: Vec<ItemCost>,
    pub buildings: Vec<BuildingCost>,
    pub curve: Vec<CurveStep>,
    pub access: Vec<MaterialAccess>,
    pub extraction: Vec<SiteYield>,
    pub openings: Vec<Opening>,
    pub contracts: Vec<ContractCost>,
    pub requests: Vec<RequestCost>,
}

// ---------------------------------------------------------------------------------------------
// The catalogue, read once
// ---------------------------------------------------------------------------------------------

struct Economy {
    definitions: DefinitionsInput,
    technologies: TechnologiesInput,
    scenarios: ScenariosInput,
}

impl Economy {
    fn load() -> Self {
        Economy {
            definitions: serde_json::from_str(DEFINITIONS).expect("shipped definitions parse"),
            technologies: serde_json::from_str(TECHNOLOGIES).expect("shipped technologies parse"),
            scenarios: serde_json::from_str(SCENARIOS).expect("shipped scenarios parse"),
        }
    }

    fn item(&self, id: ItemId) -> Option<&ItemDefinition> {
        self.definitions.items.iter().find(|item| item.id == id)
    }

    fn item_key(&self, id: ItemId) -> String {
        self.item(id)
            .map(|item| item.key.clone())
            .unwrap_or_else(|| format!("item-{id}"))
    }

    /// The recipe that makes this item, or `None` for something the world hands over directly.
    /// Two recipes producing one item would make "the cost of a plate" ambiguous, so that is a
    /// panic rather than a first match: this module would be quietly reporting one of two answers.
    fn recipe_for(&self, id: ItemId) -> Option<&RecipeDefinition> {
        let mut found = self
            .definitions
            .recipes
            .iter()
            .filter(|recipe| recipe.output.item_id == id);
        let first = found.next();
        assert!(
            found.next().is_none(),
            "two recipes produce {} — a tree-expanded cost would be ambiguous",
            self.item_key(id)
        );
        first
    }

    fn building(&self, id: DefinitionId) -> Option<&BuildingDefinition> {
        self.definitions
            .buildings
            .iter()
            .find(|building| building.id == id)
    }

    fn building_by_key(&self, key: &str) -> Option<&BuildingDefinition> {
        self.definitions
            .buildings
            .iter()
            .find(|building| building.key == key)
    }

    fn technology(&self, id: TechnologyId) -> Option<&TechnologyDefinition> {
        self.technologies
            .technologies
            .iter()
            .find(|technology| technology.id == id)
    }

    /// The densest fuel in the catalogue. Energy is quoted in units of it because that is the
    /// fewest items any amount of heat can be paid with, and because "which fuel" is the player's
    /// choice and not the data's.
    fn best_fuel(&self) -> (ItemId, u32) {
        self.definitions
            .items
            .iter()
            .filter_map(|item| item.fuel_value.map(|value| (item.id, value)))
            .max_by_key(|&(id, value)| (value, std::cmp::Reverse(id)))
            .unwrap_or((0, 1))
    }

    /// Every technology this one sits behind, transitively.
    fn ancestors(&self, id: TechnologyId) -> BTreeSet<TechnologyId> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            let Some(technology) = self.technology(current) else {
                continue;
            };
            for &prerequisite in &technology.prerequisites {
                if seen.insert(prerequisite) {
                    stack.push(prerequisite);
                }
            }
        }
        seen
    }

    // --- expansion -----------------------------------------------------------------------

    /// Exact proportional expansion: raw materials and fuel energy for `quantity` of `item`.
    fn expand(
        &self,
        item: ItemId,
        quantity: Ratio,
        raw: &mut BTreeMap<ItemId, Ratio>,
        energy: &mut Ratio,
        ticks: &mut Ratio,
        path: &mut Vec<ItemId>,
    ) {
        assert!(
            !path.contains(&item),
            "recipe cycle through {} — a tree-expanded cost would not terminate",
            self.item_key(item)
        );
        let Some(recipe) = self.recipe_for(item) else {
            let entry = raw.entry(item).or_insert(Ratio::ZERO);
            *entry = entry.add(quantity);
            return;
        };
        let crafts = quantity.over(recipe.output.quantity.max(1));
        *energy = energy.add(crafts.mul(Ratio::whole(recipe.fuel)));
        *ticks = ticks.add(crafts.mul(Ratio::whole(recipe.duration)));
        path.push(item);
        for input in &recipe.inputs {
            self.expand(
                input.item_id,
                crafts.mul(Ratio::whole(input.quantity)),
                raw,
                energy,
                ticks,
                path,
            );
        }
        path.pop();
    }

    /// What a player actually spends: whole crafts, leftovers eaten.
    fn expand_batch(
        &self,
        item: ItemId,
        quantity: u64,
        raw: &mut BTreeMap<ItemId, u64>,
        energy: &mut u64,
        ticks: &mut u64,
    ) {
        let Some(recipe) = self.recipe_for(item) else {
            *raw.entry(item).or_insert(0) += quantity;
            return;
        };
        let crafts = divide_up(quantity, u64::from(recipe.output.quantity.max(1)));
        *energy += crafts * u64::from(recipe.fuel);
        *ticks += crafts * u64::from(recipe.duration);
        for input in &recipe.inputs {
            self.expand_batch(
                input.item_id,
                crafts * u64::from(input.quantity),
                raw,
                energy,
                ticks,
            );
        }
    }

    fn depth_of(&self, item: ItemId) -> u32 {
        match self.recipe_for(item) {
            None => 0,
            Some(recipe) => {
                let inputs: Vec<ItemId> = recipe.inputs.iter().map(|input| input.item_id).collect();
                1 + inputs
                    .into_iter()
                    .map(|input| self.depth_of(input))
                    .max()
                    .unwrap_or(0)
            }
        }
    }

    fn cost_of(&self, ingredients: &[Ingredient]) -> Expansion {
        let mut raw = BTreeMap::new();
        let mut energy = Ratio::ZERO;
        let mut ticks = Ratio::ZERO;
        let mut batch_raw = BTreeMap::new();
        let mut batch_energy = 0u64;
        let mut batch_ticks = 0u64;
        let mut depth = 0;
        for ingredient in ingredients {
            self.expand(
                ingredient.item_id,
                Ratio::whole(ingredient.quantity),
                &mut raw,
                &mut energy,
                &mut ticks,
                &mut Vec::new(),
            );
            self.expand_batch(
                ingredient.item_id,
                u64::from(ingredient.quantity),
                &mut batch_raw,
                &mut batch_energy,
                &mut batch_ticks,
            );
            depth = depth.max(self.depth_of(ingredient.item_id));
        }
        Expansion {
            raw,
            energy,
            ticks,
            batch_raw,
            batch_energy,
            batch_ticks,
            depth,
        }
    }
}

struct Expansion {
    raw: BTreeMap<ItemId, Ratio>,
    energy: Ratio,
    ticks: Ratio,
    batch_raw: BTreeMap<ItemId, u64>,
    batch_energy: u64,
    batch_ticks: u64,
    depth: u32,
}

impl Expansion {
    fn units(&self) -> Ratio {
        self.raw
            .values()
            .fold(Ratio::ZERO, |total, &value| total.add(value))
    }

    /// Materials plus heat, with heat priced in the densest fuel item. Every raw unit counts once,
    /// which is the only weighting the data supplies — an insight value is a research price, not a
    /// scarcity, and inventing a scarcity weight would be tuning by feel inside the tool that
    /// exists to stop it.
    fn effort(&self, best_fuel_value: u32) -> Ratio {
        self.units().add(self.energy.over(best_fuel_value.max(1)))
    }
}

// ---------------------------------------------------------------------------------------------
// The report
// ---------------------------------------------------------------------------------------------

pub fn compute() -> BalanceReport {
    report(&Economy::load())
}

/// The same computation over a catalogue that is not the shipped one. This is what lets a test
/// break a number on purpose and watch the curve fail, which is the only way to know the curve is
/// being checked rather than described.
#[cfg(test)]
pub(crate) fn compute_from(
    definitions: DefinitionsInput,
    technologies: TechnologiesInput,
) -> BalanceReport {
    report(&Economy {
        definitions,
        technologies,
        scenarios: serde_json::from_str(SCENARIOS).expect("shipped scenarios parse"),
    })
}

fn report(economy: &Economy) -> BalanceReport {
    let (best_fuel_id, best_fuel_value) = economy.best_fuel();

    BalanceReport {
        reference: reference(economy, best_fuel_id, best_fuel_value),
        machines: machines(economy),
        power: power(economy, best_fuel_value),
        fuel: fuel(economy),
        items: items(economy, best_fuel_value),
        buildings: buildings(economy, best_fuel_value),
        curve: curve(economy, best_fuel_value),
        access: access(economy),
        extraction: extraction(economy),
        openings: openings(economy, best_fuel_id, best_fuel_value),
        contracts: contracts(economy, best_fuel_id, best_fuel_value),
        requests: requests(economy, best_fuel_id, best_fuel_value),
    }
}

fn cells_in_radius(radius: u32) -> u32 {
    1 + 3 * radius * (radius + 1)
}

fn reference(economy: &Economy, best_fuel_id: ItemId, best_fuel_value: u32) -> Reference {
    let mut reaches: Vec<u32> = vec![EXTRACT_RADIUS as u32];
    for building in &economy.definitions.buildings {
        if let Some(radius) = building.extract_radius {
            if !reaches.contains(&radius) {
                reaches.push(radius);
            }
        }
    }
    reaches.sort_unstable();
    // What the opening promises, as a promise rather than as a measurement: the generator places
    // these patches, so the quantity behind each one is a property of a seed and belongs in the
    // survey. The window is the part that is fixed, and it is the part a balance pass argues from.
    let opening: Vec<Guarantee> = BOOTSTRAP_GUARANTEES
        .iter()
        .map(|&(item_id, floor, ceiling)| Guarantee {
            item: economy.item_key(item_id),
            walk_min: floor as u32,
            walk_max: ceiling as u32,
        })
        .collect();
    Reference {
        definition_version: economy.definitions.version,
        technology_version: economy.technologies.version,
        ticks_per_second: REFERENCE_TICKS_PER_SECOND,
        player_ticks_per_second: PLAYER_TICKS_PER_SECOND,
        gather_cooldown_steps: GATHER_COOLDOWN_STEPS,
        hand_items_per_minute: hand_items_per_minute(GATHER_COOLDOWN_STEPS),
        hand_gathers: hand_gathers(economy),
        cells_in_reach: reaches.into_iter().map(cells_in_radius).collect(),
        best_fuel_item: economy.item_key(best_fuel_id),
        best_fuel_value,
        guaranteed_opening: opening,
    }
}

fn hand_items_per_minute(steps: u32) -> u32 {
    60 * PLAYER_TICKS_PER_SECOND / steps.max(1)
}

fn hand_gathers(economy: &Economy) -> Vec<HandGather> {
    economy
        .definitions
        .items
        .iter()
        .filter_map(|item| {
            item.hand_gather_steps.map(|steps| HandGather {
                item: item.key.clone(),
                steps,
                items_per_minute: hand_items_per_minute(steps),
            })
        })
        .collect()
}

/// Player-clock steps the hand spends on this bill: each raw unit at its own gather rate, plus
/// fuel items at the densest fuel's rate. A machine-only material (crystal) uses the wood/extractor
/// ceiling so a number still exists; the request test that compares grinding to processing skips
/// those rows.
fn hand_ticks_for(
    economy: &Economy,
    raw: &BTreeMap<ItemId, u64>,
    fuel_items: u64,
    best_fuel_id: ItemId,
) -> u64 {
    let steps_of = |id: ItemId| {
        economy
            .item(id)
            .and_then(|item| item.hand_gather_steps)
            .unwrap_or(GATHER_COOLDOWN_STEPS)
    };
    let mut ticks = 0u64;
    for (&item_id, &quantity) in raw {
        ticks += quantity * u64::from(steps_of(item_id));
    }
    ticks += fuel_items * u64::from(steps_of(best_fuel_id));
    ticks
}

fn insight_per_minute_milli(insight: u32, hand_ticks: u64, machine_ticks: u64) -> u64 {
    let seconds =
        Ratio::new(i128::from(hand_ticks), i128::from(PLAYER_TICKS_PER_SECOND)).add(Ratio::new(
            i128::from(machine_ticks),
            i128::from(REFERENCE_TICKS_PER_SECOND),
        ));
    if seconds.is_zero() {
        return 0;
    }
    Ratio::new(i128::from(insight) * 60 * seconds.den, seconds.num).milli()
}

/// Outputs per minute, from a cycle length in ticks.
fn per_minute(output_per_cycle: u32, ticks_per_cycle: u32) -> Ratio {
    if ticks_per_cycle == 0 {
        return Ratio::ZERO;
    }
    Ratio::whole(output_per_cycle * 60 * REFERENCE_TICKS_PER_SECOND).over(ticks_per_cycle)
}

fn machines(economy: &Economy) -> Vec<MachineRate> {
    let mut rows = Vec::new();
    for building in &economy.definitions.buildings {
        if !building.buildable {
            continue;
        }
        let power_draw = building.power_draw.unwrap_or(0);
        match building.kind {
            // An extractor no longer has one rate. Its cycle comes from the material under it,
            // scaled by its own `extract_speed`, so it gets one row per material it can work —
            // which is the only way this file can still answer "is a machine worth building" now
            // that the answer is different for coal than it is for wood.
            BuildingKind::Extractor => {
                let speed = building.extract_speed.unwrap_or(100).max(1);
                for item in &economy.definitions.items {
                    let Some(steps) = item.extract_steps else {
                        continue;
                    };
                    let cadence = ((steps * 100 + speed - 1) / speed).max(1);
                    let rate = per_minute(1, cadence);
                    rows.push(MachineRate {
                        building: building.key.clone(),
                        manual_work: building.manual_work,
                        recipe: None,
                        ticks_per_cycle: cadence,
                        output_item: Some(item.key.clone()),
                        output_per_cycle: 1,
                        per_minute_milli: rate.milli(),
                        inputs_per_minute: Vec::new(),
                        power_draw,
                        grid_energy_per_cycle: power_draw * cadence,
                        grid_energy_per_minute_milli: rate
                            .mul(Ratio::whole(power_draw * cadence))
                            .milli(),
                        fuel_energy_per_cycle: 0,
                        fuel_energy_per_minute_milli: 0,
                    });
                }
            }
            // A pump keeps one rate, because water is the one source with no per-material figure:
            // nothing else draws it and it is never anything but water.
            BuildingKind::Pump => {
                let cadence = building.cadence.unwrap_or(1);
                let rate = per_minute(1, cadence);
                rows.push(MachineRate {
                    building: building.key.clone(),
                    manual_work: building.manual_work,
                    recipe: None,
                    ticks_per_cycle: cadence,
                    output_item: building.output_item_id.map(|id| economy.item_key(id)),
                    output_per_cycle: 1,
                    per_minute_milli: rate.milli(),
                    inputs_per_minute: Vec::new(),
                    power_draw,
                    grid_energy_per_cycle: power_draw * cadence,
                    grid_energy_per_minute_milli: rate
                        .mul(Ratio::whole(power_draw * cadence))
                        .milli(),
                    fuel_energy_per_cycle: 0,
                    fuel_energy_per_minute_milli: 0,
                });
            }
            BuildingKind::Composer => {
                for recipe in &economy.definitions.recipes {
                    if !building.supports_recipe(recipe) {
                        continue;
                    }
                    let duration = building.recipe_duration(recipe);
                    let rate = per_minute(recipe.output.quantity, duration);
                    let cycles = per_minute(1, duration);
                    rows.push(MachineRate {
                        building: building.key.clone(),
                        manual_work: building.manual_work,
                        recipe: Some(recipe.key.clone()),
                        ticks_per_cycle: duration,
                        output_item: Some(economy.item_key(recipe.output.item_id)),
                        output_per_cycle: recipe.output.quantity,
                        per_minute_milli: rate.milli(),
                        inputs_per_minute: recipe
                            .inputs
                            .iter()
                            .map(|input| MilliAmount {
                                item: economy.item_key(input.item_id),
                                quantity_milli: cycles.mul(Ratio::whole(input.quantity)).milli(),
                            })
                            .collect(),
                        power_draw,
                        grid_energy_per_cycle: power_draw * duration,
                        grid_energy_per_minute_milli: cycles
                            .mul(Ratio::whole(power_draw * duration))
                            .milli(),
                        fuel_energy_per_cycle: recipe.fuel,
                        fuel_energy_per_minute_milli: cycles.mul(Ratio::whole(recipe.fuel)).milli(),
                    });
                }
            }
            _ => {}
        }
    }
    rows
}

fn power(economy: &Economy, best_fuel_value: u32) -> Vec<PowerPlant> {
    // A boiler burns one unit of energy and drinks one unit of water every tick it runs, and a
    // turbine is dead without one beside it. That is the turbine's true fuel bill, so it is
    // reported on the turbine rather than left to be inferred from two rows.
    let boiler_energy_per_tick = 1;
    let boiler_water_per_tick = 1;
    let pump = economy
        .definitions
        .buildings
        .iter()
        .find(|building| building.kind == BuildingKind::Pump);
    let pump_cadence = pump.and_then(|building| building.cadence).unwrap_or(1);
    let pump_draw = pump.and_then(|building| building.power_draw).unwrap_or(0);

    let consumers: Vec<(&str, u32)> = economy
        .definitions
        .buildings
        .iter()
        .filter(|building| building.buildable && building.power_draw.unwrap_or(0) > 0)
        .map(|building| (building.key.as_str(), building.power_draw.unwrap_or(0)))
        .collect();

    let mut rows = Vec::new();
    for building in &economy.definitions.buildings {
        if building.kind != BuildingKind::Generator || !building.buildable {
            continue;
        }
        let output = building.power_output.unwrap_or(0);
        let (source, requires, fuel_energy_per_tick, water_per_tick) = match building.power_source {
            Some(PowerSource::Burner) => ("burner", None, 1, 0),
            Some(PowerSource::Wind) => ("wind", None, 0, 0),
            Some(PowerSource::Hydro) => ("hydro", None, 0, 0),
            Some(PowerSource::Turbine) => (
                "turbine",
                economy
                    .definitions
                    .buildings
                    .iter()
                    .find(|other| other.kind == BuildingKind::Boiler)
                    .map(|other| other.key.clone()),
                boiler_energy_per_tick,
                boiler_water_per_tick,
            ),
            None => ("none", None, 0, 0),
        };
        // One pump makes one unit every `cadence` ticks, so a plant drinking one a tick needs a
        // pump per tick of that cadence. The pumps are on the same network and draw from it.
        let pumps_required = water_per_tick * pump_cadence;
        let upkeep_draw = pumps_required * pump_draw;
        let net_output = i64::from(output) - i64::from(upkeep_draw);
        rows.push(PowerPlant {
            building: building.key.clone(),
            source: source.to_string(),
            output,
            requires,
            fuel_energy_per_tick,
            water_per_tick,
            ticks_per_fuel_item: if fuel_energy_per_tick == 0 {
                0
            } else {
                best_fuel_value / fuel_energy_per_tick
            },
            grid_energy_per_fuel_item: if fuel_energy_per_tick == 0 {
                0
            } else {
                best_fuel_value * output / fuel_energy_per_tick
            },
            pumps_required,
            upkeep_draw,
            net_output,
            carries: consumers
                .iter()
                .map(|&(key, draw)| PlantCarries {
                    building: key.to_string(),
                    power_draw: draw,
                    machines: if draw == 0 || net_output <= 0 {
                        0
                    } else {
                        (net_output as u32) / draw
                    },
                })
                .collect(),
        });
    }
    rows
}

fn fuel(economy: &Economy) -> Vec<FuelConversion> {
    economy
        .definitions
        .items
        .iter()
        .filter_map(|item| {
            let fuel_value = item.fuel_value?;
            let recipe = economy.recipe_for(item.id);
            let output_energy = fuel_value * recipe.map_or(1, |recipe| recipe.output.quantity);
            let input_energy = recipe.map_or(0, |recipe| {
                recipe.fuel
                    + recipe
                        .inputs
                        .iter()
                        .map(|input| {
                            input.quantity
                                * economy
                                    .item(input.item_id)
                                    .and_then(|input| input.fuel_value)
                                    .unwrap_or(0)
                        })
                        .sum::<u32>()
            });
            Some(FuelConversion {
                item: item.key.clone(),
                fuel_value,
                recipe: recipe.map(|recipe| recipe.key.clone()),
                input_energy,
                output_energy,
                gain_milli: (input_energy > 0).then(|| {
                    Ratio::new(i128::from(output_energy), i128::from(input_energy)).milli()
                }),
            })
        })
        .collect()
}

fn milli_amounts(economy: &Economy, raw: &BTreeMap<ItemId, Ratio>) -> Vec<MilliAmount> {
    raw.iter()
        .filter(|(_, value)| !value.is_zero())
        .map(|(&item_id, &value)| MilliAmount {
            item: economy.item_key(item_id),
            quantity_milli: value.milli(),
        })
        .collect()
}

fn amounts(economy: &Economy, raw: &BTreeMap<ItemId, u64>) -> Vec<Amount> {
    raw.iter()
        .filter(|(_, &value)| value > 0)
        .map(|(&item_id, &value)| Amount {
            item: economy.item_key(item_id),
            quantity: value,
        })
        .collect()
}

fn items(economy: &Economy, best_fuel_value: u32) -> Vec<ItemCost> {
    economy
        .definitions
        .items
        .iter()
        .map(|item| {
            let expansion = economy.cost_of(&[Ingredient {
                item_id: item.id,
                quantity: 1,
            }]);
            ItemCost {
                item: item.key.clone(),
                depth: expansion.depth,
                raw: milli_amounts(economy, &expansion.raw),
                raw_units_milli: expansion.units().milli(),
                fuel_energy_milli: expansion.energy.milli(),
                machine_ticks_milli: expansion.ticks.milli(),
                effort_milli: expansion.effort(best_fuel_value).milli(),
            }
        })
        .collect()
}

fn buildings(economy: &Economy, best_fuel_value: u32) -> Vec<BuildingCost> {
    economy
        .definitions
        .buildings
        .iter()
        .filter(|building| building.buildable)
        .map(|building| {
            let expansion = economy.cost_of(&building.construction_cost);
            BuildingCost {
                building: building.key.clone(),
                kind: format!("{:?}", building.kind).to_lowercase(),
                tier: building.tier,
                unlock_technology: building
                    .unlock_technology_id
                    .and_then(|id| economy.technology(id))
                    .map(|technology| technology.key.clone()),
                direct: building
                    .construction_cost
                    .iter()
                    .map(|ingredient| Amount {
                        item: economy.item_key(ingredient.item_id),
                        quantity: u64::from(ingredient.quantity),
                    })
                    .collect(),
                depth: expansion.depth,
                raw: milli_amounts(economy, &expansion.raw),
                raw_units_milli: expansion.units().milli(),
                fuel_energy_milli: expansion.energy.milli(),
                effort_milli: expansion.effort(best_fuel_value).milli(),
                batch: amounts(economy, &expansion.batch_raw),
                batch_units: expansion.batch_raw.values().sum(),
                batch_fuel_energy: expansion.batch_energy,
                machine_ticks: expansion.batch_ticks,
            }
        })
        .collect()
}

/// The stated curve.
///
/// Two rules, and only two, because "meant to follow" is a claim about the data and not a mood.
/// A tier follows the tier below it on its own ladder, and a machine follows another machine of
/// the same kind whose technology it is unlocked behind. A cutter does not follow a kiln — they
/// sit beside each other in the same tech tier — and pretending otherwise would tune numbers to
/// satisfy an ordering nothing in the game asserts.
fn curve(economy: &Economy, best_fuel_value: u32) -> Vec<CurveStep> {
    let effort = |building: &BuildingDefinition| {
        economy
            .cost_of(&building.construction_cost)
            .effort(best_fuel_value)
    };
    let mut rows = Vec::new();
    for building in &economy.definitions.buildings {
        if !building.buildable {
            continue;
        }
        if let Some(next) = building.upgrades_to.and_then(|id| economy.building(id)) {
            let (before, after) = (effort(building), effort(next));
            rows.push(CurveStep {
                building: next.key.clone(),
                follows: building.key.clone(),
                relation: "upgrade".to_string(),
                effort_milli: after.milli(),
                follows_effort_milli: before.milli(),
                holds: after > before,
            });
        }
        let Some(technology) = building.unlock_technology_id else {
            continue;
        };
        let ancestors = economy.ancestors(technology);
        for earlier in &economy.definitions.buildings {
            if !earlier.buildable || earlier.kind != building.kind || earlier.id == building.id {
                continue;
            }
            let Some(earlier_technology) = earlier.unlock_technology_id else {
                continue;
            };
            if !ancestors.contains(&earlier_technology) {
                continue;
            }
            let (before, after) = (effort(earlier), effort(building));
            rows.push(CurveStep {
                building: building.key.clone(),
                follows: earlier.key.clone(),
                relation: "kind".to_string(),
                effort_milli: after.milli(),
                follows_effort_milli: before.milli(),
                holds: after >= before,
            });
        }
    }
    rows.sort_by(|left, right| {
        (&left.building, &left.follows, &left.relation).cmp(&(
            &right.building,
            &right.follows,
            &right.relation,
        ))
    });
    rows
}

/// Which raw materials the economy bottoms out in, and whether the default world hands them over.
fn access(economy: &Economy) -> Vec<MaterialAccess> {
    let params = preset_params(DEFAULT_PRESET_KEY).expect("the default preset is in the table");
    let seed = survey::default_seed();

    // Everything the recipe tree and the construction rows actually reach the ground through.
    let mut required: BTreeMap<ItemId, u32> = BTreeMap::new();
    let mut note = |economy: &Economy, ingredients: &[Ingredient]| {
        for (item_id, _) in economy.cost_of(ingredients).raw {
            *required.entry(item_id).or_insert(0) += 1;
        }
    };
    for recipe in &economy.definitions.recipes {
        note(economy, &recipe.inputs);
    }
    for building in &economy.definitions.buildings {
        if building.buildable {
            note(economy, &building.construction_cost);
        }
    }
    // Water is raw and is nobody's field: a pump makes it out of terrain. It reaches the tree
    // through concrete, and a boiler drinks it directly.
    required.entry(WATER_ITEM).or_insert(0);

    let fields = WorldFields::new(&params, seed);
    let guaranteed = guaranteed_patches(&fields);
    let mut rows = Vec::new();
    for (&item_id, &required_by) in &required {
        let promise = guaranteed.get(&item_id).copied();
        let (nearest, reachable) = if item_id == WATER_ITEM {
            nearest_water(&params, seed)
        } else {
            nearest_field(&fields, item_id)
        };
        rows.push(MaterialAccess {
            material: economy.item_key(item_id),
            guaranteed_walk: promise.map(|(walk, _)| walk),
            guaranteed_hexes: promise.map(|(_, hexes)| hexes).unwrap_or(0),
            nearest_generated: nearest,
            // A guaranteed patch is placed by the generator and clipped by the same member test as
            // every other, so unlike the old clearing cells it is reachable only if geography says
            // so — which is why the bootstrap pass measures the patch before it claims a cell.
            reachable,
            required_by,
        });
    }
    rows
}

/// Something can stand here, and what it stands on is not a cliff or a basin.
fn standable(params: &WorldParams, seed: u32, cell: (i32, i32)) -> bool {
    !terrain_at(params, seed, cell.0, cell.1, true).blocks_movement()
}

/// What the bootstrap pass promised, per material: the walk to the nearest hex of the guaranteed
/// patch, and how many hexes that patch holds.
fn guaranteed_patches(fields: &WorldFields) -> BTreeMap<ItemId, (u32, u32)> {
    fields
        .guarantees()
        .into_iter()
        .map(|(item_id, walk, hexes)| (item_id, (walk, hexes)))
        .collect()
}

fn nearest_field(fields: &WorldFields, item_id: ItemId) -> (Option<u32>, bool) {
    let params = &fields.params;
    let seed = fields.seed;
    let mut nearest = None;
    let mut reachable = false;
    for cell in hexes_in_radius((0, 0), YIELD_RADIUS) {
        if axial_distance((0, 0), cell) <= LANDING_CLEAR_RADIUS {
            continue;
        }
        let Some(field) = fields.field_at(cell.0, cell.1, true) else {
            continue;
        };
        if field.item_id != item_id {
            continue;
        }
        let distance = axial_distance((0, 0), cell) as u32;
        nearest = Some(nearest.map_or(distance, |value: u32| value.min(distance)));
        // Stone sits on cliffs. What makes it a material rather than scenery is that a hex beside
        // the cliff covers it, at the one reach every extractor and the player's own hand share.
        reachable = reachable
            || hexes_in_radius(cell, EXTRACT_RADIUS)
                .into_iter()
                .any(|neighbour| standable(params, seed, neighbour));
    }
    (nearest, reachable)
}

fn nearest_water(params: &WorldParams, seed: u32) -> (Option<u32>, bool) {
    let mut nearest = None;
    let mut reachable = false;
    for cell in hexes_in_radius((0, 0), YIELD_RADIUS) {
        if !terrain_at(params, seed, cell.0, cell.1, true).is_water() {
            continue;
        }
        let distance = axial_distance((0, 0), cell) as u32;
        nearest = Some(nearest.map_or(distance, |value: u32| value.min(distance)));
        // A pump stands beside a basin, never in it.
        reachable = reachable
            || hexes_in_radius(cell, PUMP_RADIUS)
                .into_iter()
                .any(|neighbour| standable(params, seed, neighbour));
    }
    (nearest, reachable)
}

/// What a site is worth, per preset, measured on generated geography.
///
/// The landing clearing is excluded for the same reason the survey excludes it: it is a promise
/// rather than a landscape, and it is reported on its own in `reference`.
fn extraction(economy: &Economy) -> Vec<SiteYield> {
    let seed = survey::default_seed();
    let mut rows = Vec::new();
    for preset in world_presets() {
        let world = WorldFields::new(&preset.params, seed);
        let mut fields: BTreeMap<(i32, i32), ResourceState> = BTreeMap::new();
        for cell in hexes_in_radius((0, 0), YIELD_RADIUS) {
            if axial_distance((0, 0), cell) <= LANDING_CLEAR_RADIUS {
                continue;
            }
            if let Some(field) = world.field_at(cell.0, cell.1, true) {
                fields.insert(cell, field);
            }
        }
        // sites, total cell quantity, then per reach: total yield and same-material yield.
        let mut totals: BTreeMap<ItemId, (u64, u64, Vec<u64>, Vec<u64>)> = BTreeMap::new();
        for (&cell, field) in &fields {
            let entry = totals.entry(field.item_id).or_insert_with(|| {
                (
                    0,
                    0,
                    vec![0; YIELD_REACHES.len()],
                    vec![0; YIELD_REACHES.len()],
                )
            });
            entry.0 += 1;
            entry.1 += u64::from(field.quantity);
            for (slot, &reach) in YIELD_REACHES.iter().enumerate() {
                for covered in hexes_in_radius(cell, reach) {
                    let Some(other) = fields.get(&covered) else {
                        continue;
                    };
                    entry.2[slot] += u64::from(other.quantity);
                    if other.item_id == field.item_id {
                        entry.3[slot] += u64::from(other.quantity);
                    }
                }
            }
        }
        for (item_id, (sites, quantity, yields, same)) in totals {
            let mean = |total: u64| (total / sites.max(1)) as u32;
            rows.push(SiteYield {
                preset: preset.key.to_string(),
                material: economy.item_key(item_id),
                sites: sites as u32,
                mean_cell_quantity: mean(quantity),
                mean_site_yield: yields.into_iter().map(mean).collect(),
                mean_same_material: same.into_iter().map(mean).collect(),
            });
        }
    }
    rows
}

/// What the opening actually costs, in the two currencies a player pays it in.
fn openings(economy: &Economy, best_fuel_id: ItemId, best_fuel_value: u32) -> Vec<Opening> {
    let targets: Vec<(&str, Vec<&str>, Vec<(&str, u32)>)> = vec![
        ("first smelter", vec!["smelter"], Vec::new()),
        ("first kiln", vec!["kiln"], Vec::new()),
        ("first cutter", vec!["cutter"], Vec::new()),
        ("first crusher", vec!["crusher"], Vec::new()),
        ("first pump", vec!["pump"], Vec::new()),
        ("first power", vec!["burner-generator", "pole"], Vec::new()),
        // The two stations the essential-bill pass repriced. Each is quoted as what standing one
        // up actually costs — the primitive stations that make its parts, and the generator it
        // cannot run without — rather than as the line in the catalogue.
        ("first extractor", vec!["extractor"], Vec::new()),
        ("first composer", vec!["composer"], Vec::new()),
        ("first circuit", Vec::new(), vec![("circuit", 1)]),
        (
            "first primitive plate",
            vec!["primitive-furnace"],
            vec![("iron-plate", 1)],
        ),
        (
            "first manual frame",
            vec!["manual-workshop"],
            vec![("frame", 1)],
        ),
    ];
    let mut rows: Vec<_> = targets
        .into_iter()
        .map(|(name, building_keys, item_keys)| {
            let wanted = item_keys
                .iter()
                .filter_map(|&(key, quantity)| {
                    economy
                        .definitions
                        .items
                        .iter()
                        .find(|item| item.key == key)
                        .map(|item| Ingredient {
                            item_id: item.id,
                            quantity,
                        })
                })
                .collect();
            opening(
                economy,
                best_fuel_id,
                best_fuel_value,
                name,
                &building_keys,
                wanted,
                None,
            )
        })
        .collect();
    if let Some(belt) = economy.building_by_key("belt") {
        for count in [24, 100] {
            let wanted = belt
                .construction_cost
                .iter()
                .map(|input| Ingredient {
                    item_id: input.item_id,
                    quantity: input.quantity * (count - 1),
                })
                .collect();
            rows.push(opening(
                economy,
                best_fuel_id,
                best_fuel_value,
                &format!("{count} starter belts"),
                &["belt"],
                wanted,
                None,
            ));
        }
    }
    rows
}

/// Every standing request the hub can post, priced through its whole tree.
fn requests(economy: &Economy, best_fuel_id: ItemId, best_fuel_value: u32) -> Vec<RequestCost> {
    economy
        .definitions
        .requests
        .iter()
        .map(|request| {
            let expansion = economy.cost_of(&[Ingredient {
                item_id: request.item_id,
                quantity: request.quantity,
            }]);
            let fuel_items = divide_up(expansion.batch_energy, u64::from(best_fuel_value.max(1)));
            let gather_total = expansion.batch_raw.values().sum::<u64>() + fuel_items;
            let repeat = request.repeat_insight.unwrap_or(request.insight);
            let hand_ticks =
                hand_ticks_for(economy, &expansion.batch_raw, fuel_items, best_fuel_id);
            RequestCost {
                request: request.key.clone(),
                item: economy.item_key(request.item_id),
                quantity: request.quantity,
                insight: request.insight,
                repeat_insight: repeat,
                gathers: amounts(economy, &expansion.batch_raw),
                fuel_items,
                gather_total,
                insight_per_gather_milli: Ratio::new(
                    i128::from(request.insight),
                    i128::from(gather_total.max(1)),
                )
                .milli(),
                repeat_insight_per_gather_milli: Ratio::new(
                    i128::from(repeat),
                    i128::from(gather_total.max(1)),
                )
                .milli(),
                insight_per_minute_milli: insight_per_minute_milli(
                    request.insight,
                    hand_ticks,
                    expansion.batch_ticks,
                ),
                repeat_insight_per_minute_milli: insight_per_minute_milli(
                    repeat,
                    hand_ticks,
                    expansion.batch_ticks,
                ),
                machine_ticks: expansion.batch_ticks,
                machine_seconds_milli: Ratio::new(
                    i128::from(expansion.batch_ticks),
                    i128::from(REFERENCE_TICKS_PER_SECOND),
                )
                .milli(),
            }
        })
        .collect()
}

/// Every contract stage the shipped scenarios state, priced through its whole tree.
fn contracts(economy: &Economy, best_fuel_id: ItemId, best_fuel_value: u32) -> Vec<ContractCost> {
    let mut rows = Vec::new();
    for scenario in &economy.scenarios.scenarios {
        // A synthetic bill exists to never be met, so expanding it would price a workload rather
        // than a game. The threshold is deliberately generous: a real founding stage is tens of
        // items, not tens of thousands.
        for stage in &scenario.contract.stages {
            if stage
                .requirements
                .iter()
                .any(|need| need.quantity > CONTRACT_BILL_LIMIT)
            {
                continue;
            }
            let expansion = economy.cost_of(&stage.requirements);
            rows.push(ContractCost {
                scenario: scenario.key.clone(),
                contract: scenario.contract.key.clone(),
                stage: stage.key.clone(),
                bill: stage
                    .requirements
                    .iter()
                    .map(|need| Amount {
                        item: economy.item_key(need.item_id),
                        quantity: u64::from(need.quantity),
                    })
                    .collect(),
                raw_materials: expansion.batch_raw.len(),
                opening: opening(
                    economy,
                    best_fuel_id,
                    best_fuel_value,
                    &format!("{}: {}", scenario.key, stage.key),
                    &[],
                    stage.requirements.clone(),
                    Some(stage.key.as_str()),
                ),
            });
        }
    }
    rows
}

/// Above this, a stage's line is a sentinel rather than a bill. `u32::MAX` and the demo's standing
/// observation both sit far above it.
const CONTRACT_BILL_LIMIT: u32 = 10_000;

/// Fills of one standing request needed to fund `insight`, replaying the board's decay instead of
/// quoting the whole ladder at the posted reward.
///
/// The raw rows drop from ten insight to two the moment they are filled once. Charging every fill
/// the first-fill price is what made research look cheap: it priced an unlimited supply of a reward
/// the hub pays exactly once.
///
/// `None` when the row cannot reach the total at all. A decayed reward of zero funds nothing after
/// its first fill, and dividing by it would invent a finite answer for an impossible one.
fn request_fills(insight: u32, first: u32, repeat: u32) -> Option<u32> {
    if insight == 0 {
        return Some(0);
    }
    if first >= insight {
        return Some(1);
    }
    if repeat == 0 {
        return None;
    }
    Some(1 + (insight - first).div_ceil(repeat))
}

/// The contract stages a set of buildings owes before it can be built, because the technology that
/// unlocks one of them is granted by finishing a stage rather than sold for insight.
///
/// `own_stage` is the stage currently being priced, if any. A stage does not commission itself:
/// its bill is already the thing being costed, and folding it in again would charge it twice.
fn owed_commissions(
    economy: &Economy,
    needed: &BTreeSet<DefinitionId>,
    own_stage: Option<&str>,
) -> BTreeSet<String> {
    let mut stages = BTreeSet::new();
    for &id in needed {
        let Some(technology) = economy.building(id).and_then(|b| b.unlock_technology_id) else {
            continue;
        };
        let mut path = economy.ancestors(technology);
        path.insert(technology);
        for id in path {
            if let Some(TechnologyGrant::ContractStage { key, .. }) =
                economy.technology(id).map(|t| &t.grant)
            {
                if own_stage != Some(key.as_str()) {
                    stages.insert(key.clone());
                }
            }
        }
    }
    stages
}

/// The bill a named contract stage asks for. Sentinel stages are skipped for the same reason
/// `contracts` skips them: the demo's standing observation is a workload, not a commission.
fn stage_requirements(economy: &Economy, stage_key: &str) -> Vec<Ingredient> {
    economy
        .scenarios
        .scenarios
        .iter()
        .flat_map(|scenario| scenario.contract.stages.iter())
        .find(|stage| {
            stage.key == stage_key
                && !stage
                    .requirements
                    .iter()
                    .any(|need| need.quantity > CONTRACT_BILL_LIMIT)
        })
        .map(|stage| stage.requirements.clone())
        .unwrap_or_default()
}

fn opening(
    economy: &Economy,
    best_fuel_id: ItemId,
    best_fuel_value: u32,
    name: &str,
    building_keys: &[&str],
    requested: Vec<Ingredient>,
    own_stage: Option<&str>,
) -> Opening {
    // A commissioned technology is earned by delivering a stage bill, so an opening that needs one
    // owes that delivery before its own first gather. Folding the bill in can pull in another
    // station, which can owe another stage, so this runs to a fixed point rather than assuming a
    // single pass settles it. The stage set only grows and is finite, so it terminates.
    let mut commissions: BTreeSet<String> = BTreeSet::new();
    let (order, needed, wanted) = loop {
        let mut wanted = requested.clone();
        for key in &commissions {
            wanted.extend(stage_requirements(economy, key));
        }
        let order = opening_build_order(economy, best_fuel_value, building_keys, &wanted);
        let needed: BTreeSet<_> = order.iter().copied().collect();
        let owed = owed_commissions(economy, &needed, own_stage);
        if owed.is_subset(&commissions) {
            break (order, needed, wanted);
        }
        commissions.extend(owed);
    };

    let mut ingredients = wanted.clone();
    for &id in &needed {
        if let Some(building) = economy.building(id) {
            ingredients.extend(building.construction_cost.iter().copied());
        }
    }
    let expansion = economy.cost_of(&ingredients);
    let mut available = BTreeSet::new();
    let mut work = (0, 0);
    for &id in &order {
        let bill = &economy.building(id).unwrap().construction_cost;
        let ticks = opening_work(economy, &available, bill);
        work.0 += ticks.0;
        work.1 += ticks.1;
        available.insert(id);
    }
    let ticks = opening_work(economy, &available, &wanted);
    let (machine_ticks, player_work_ticks) = (work.0 + ticks.0, work.1 + ticks.1);

    // Research is paid in insight, and insight is paid in items delivered to the hub. The best
    // rate the landing clearing offers is the fewest items that research can cost.
    let mut technologies: BTreeSet<TechnologyId> = BTreeSet::new();
    for &id in &needed {
        if let Some(technology) = economy.building(id).and_then(|b| b.unlock_technology_id) {
            technologies.insert(technology);
            technologies.extend(economy.ancestors(technology));
        }
    }
    // Only a purchasable technology is paid for in insight. A commissioned one is already charged
    // as the stage bill folded into `wanted` above, and adding its price here would bill the player
    // twice for the same unlock.
    let insight: u32 = technologies
        .iter()
        .filter_map(|&id| economy.technology(id))
        .filter(|technology| technology.purchasable())
        .map(|technology| technology.cost)
        .sum();
    // Insight stopped being a property of an item: it is paid for filling a request the hub posted.
    // So the cheapest way to fund the research is the standing request that reaches the total in the
    // fewest delivered items, counted over the rows the landing clearing can actually supply and
    // replayed through the decay the board states.
    let opening: BTreeSet<ItemId> = BOOTSTRAP_GUARANTEES
        .iter()
        .map(|&(item_id, _, _)| item_id)
        .collect();
    let (insight_items, insight_request) = economy
        .definitions
        .requests
        .iter()
        .filter(|request| opening.contains(&request.item_id))
        .filter_map(|request| {
            let repeat = request.repeat_insight.unwrap_or(request.insight);
            request_fills(insight, request.insight, repeat)
                .map(|fills| (fills * request.quantity, request.key.clone()))
        })
        .min()
        .unwrap_or_default();

    let fuel_items = divide_up(expansion.batch_energy, u64::from(best_fuel_value.max(1)));
    let gather_total: u64 =
        expansion.batch_raw.values().sum::<u64>() + fuel_items + u64::from(insight_items);
    let mut opening_raw = expansion.batch_raw.clone();
    if insight_items > 0 {
        if let Some(request) = economy
            .definitions
            .requests
            .iter()
            .find(|request| request.key == insight_request)
        {
            *opening_raw.entry(request.item_id).or_default() += u64::from(insight_items);
        }
    }
    let hand_ticks = hand_ticks_for(economy, &opening_raw, fuel_items, best_fuel_id);

    let mut names: Vec<String> = needed
        .iter()
        .filter_map(|&id| economy.building(id))
        .map(|building| building.key.clone())
        .collect();
    names.sort();

    Opening {
        name: name.to_string(),
        technologies: technologies
            .iter()
            .filter_map(|&id| economy.technology(id))
            .map(|technology| technology.key.clone())
            .collect(),
        insight,
        insight_items,
        insight_request,
        commissions: commissions.into_iter().collect(),
        buildings: names,
        construction_order: order
            .iter()
            .map(|&id| economy.building(id).unwrap().key.clone())
            .collect(),
        gathers: amounts(economy, &expansion.batch_raw),
        fuel_energy: expansion.batch_energy,
        fuel_items,
        gather_total,
        machine_ticks,
        player_work_ticks,
        hand_seconds_milli: Ratio::new(i128::from(hand_ticks), i128::from(PLAYER_TICKS_PER_SECOND))
            .milli(),
        machine_seconds_milli: Ratio::new(
            i128::from(machine_ticks),
            i128::from(REFERENCE_TICKS_PER_SECOND),
        )
        .milli(),
        player_work_seconds_milli: Ratio::new(
            i128::from(player_work_ticks),
            i128::from(REFERENCE_TICKS_PER_SECOND),
        )
        .milli(),
    }
}

/// Resolve construction before production. A target cannot make its own bill, and a powered
/// station cannot supply the parts of the first generator. This is one deterministic route,
/// not a global cheapest-factory optimizer; already-built suppliers are reused.
fn opening_build_order(
    economy: &Economy,
    best_fuel_value: u32,
    targets: &[&str],
    wanted: &[Ingredient],
) -> Vec<DefinitionId> {
    struct Plan<'a> {
        economy: &'a Economy,
        fuel: u32,
        order: Vec<DefinitionId>,
        visiting: BTreeSet<DefinitionId>,
    }
    impl Plan<'_> {
        fn supply(&mut self, ingredients: &[Ingredient]) {
            for input in ingredients {
                let Some(recipe) = self.economy.recipe_for(input.item_id) else {
                    continue;
                };
                self.supply(&recipe.inputs);
                if self
                    .order
                    .iter()
                    .any(|&id| self.economy.building(id).unwrap().supports_recipe(recipe))
                {
                    continue;
                }
                let provider = self
                    .economy
                    .definitions
                    .buildings
                    .iter()
                    .filter(|b| {
                        b.buildable && b.supports_recipe(recipe) && !self.visiting.contains(&b.id)
                    })
                    .min_by_key(|b| {
                        (
                            self.economy.cost_of(&b.construction_cost).effort(self.fuel),
                            b.id,
                        )
                    })
                    .expect("opening has a non-circular recipe supplier");
                self.build(provider.id);
            }
        }

        fn build(&mut self, id: DefinitionId) {
            if self.order.contains(&id) {
                return;
            }
            assert!(self.visiting.insert(id), "circular opening construction");
            let building = self.economy.building(id).unwrap();
            self.supply(&building.construction_cost);
            if building.power_draw.unwrap_or(0) > 0
                && !self
                    .order
                    .iter()
                    .any(|&id| self.economy.building(id).unwrap().power_output.unwrap_or(0) > 0)
            {
                let generator = self
                    .economy
                    .definitions
                    .buildings
                    .iter()
                    .filter(|b| {
                        b.buildable
                            && b.power_output.unwrap_or(0) > 0
                            && !self.visiting.contains(&b.id)
                    })
                    .min_by_key(|b| {
                        (
                            self.economy.cost_of(&b.construction_cost).effort(self.fuel),
                            b.id,
                        )
                    })
                    .expect("opening has a non-circular power supplier");
                self.build(generator.id);
            }
            self.visiting.remove(&id);
            self.order.push(id);
        }
    }
    let mut plan = Plan {
        economy,
        fuel: best_fuel_value,
        order: Vec::new(),
        visiting: BTreeSet::new(),
    };
    for key in targets {
        plan.build(
            economy
                .building_by_key(key)
                .expect("opening target exists")
                .id,
        );
    }
    plan.supply(wanted);
    plan.order
}

fn opening_work(
    economy: &Economy,
    machines: &BTreeSet<DefinitionId>,
    ingredients: &[Ingredient],
) -> (u64, u64) {
    fn walk(
        economy: &Economy,
        machines: &BTreeSet<DefinitionId>,
        item: ItemId,
        quantity: u64,
    ) -> (u64, u64) {
        let Some(recipe) = economy.recipe_for(item) else {
            return (0, 0);
        };
        let crafts = divide_up(quantity, u64::from(recipe.output.quantity));
        let machine = machines
            .iter()
            .filter_map(|id| economy.building(*id))
            .filter(|building| building.supports_recipe(recipe))
            .min_by_key(|building| (building.recipe_duration(recipe), building.id))
            .expect("opening resolves a machine for every recipe");
        let ticks = crafts * u64::from(machine.recipe_duration(recipe));
        let mut total = if machine.manual_work {
            (0, ticks)
        } else {
            (ticks, 0)
        };
        for input in &recipe.inputs {
            let child = walk(
                economy,
                machines,
                input.item_id,
                crafts * u64::from(input.quantity),
            );
            total.0 += child.0;
            total.1 += child.1;
        }
        total
    }
    ingredients.iter().fold((0, 0), |total, input| {
        let child = walk(economy, machines, input.item_id, u64::from(input.quantity));
        (total.0 + child.0, total.1 + child.1)
    })
}

// ---------------------------------------------------------------------------------------------
// Reports
// ---------------------------------------------------------------------------------------------

pub fn format_json(report: &BalanceReport) -> String {
    serde_json::to_string_pretty(report).expect("balance report serializes")
}

fn milli(value: u64) -> String {
    format!("{}.{:03}", value / 1000, value % 1000)
}

/// The human-readable form a tuning pass is argued from.
pub fn format_report(report: &BalanceReport) -> String {
    let mut out = String::new();
    let reference = &report.reference;
    out.push_str(&format!(
        "definitions v{} | technologies v{} | {} tps | player {} steps/s\n",
        reference.definition_version,
        reference.technology_version,
        reference.ticks_per_second,
        reference.player_ticks_per_second
    ));
    out.push_str(&format!(
        "hand: {} items/min at reach {} cells | fuel quoted in {} ({} energy)\n",
        reference.hand_items_per_minute,
        reference.cells_in_reach.first().copied().unwrap_or(0),
        reference.best_fuel_item,
        reference.best_fuel_value
    ));
    for gather in &reference.hand_gathers {
        out.push_str(&format!(
            "  {:<12} {:>3} steps  {:>3} /min\n",
            gather.item, gather.steps, gather.items_per_minute
        ));
    }
    out.push_str(&format!(
        "guaranteed opening: {}\n\n",
        reference
            .guaranteed_opening
            .iter()
            .map(|entry| format!("{} {}-{}", entry.item, entry.walk_min, entry.walk_max))
            .collect::<Vec<_>>()
            .join(" | ")
    ));

    out.push_str("machine rates (manual stations: attended work, one command per batch)\n");
    out.push_str("  building         recipe            ticks   per min   draw   fuel/cycle\n");
    for machine in &report.machines {
        out.push_str(&format!(
            "  {:<16} {:<16} {:>6}  {:>8}  {:>5}  {:>10}\n",
            machine.building,
            machine.recipe.clone().unwrap_or_else(|| "-".to_string()),
            machine.ticks_per_cycle,
            milli(machine.per_minute_milli),
            machine.power_draw,
            machine.fuel_energy_per_cycle
        ));
    }

    out.push_str("\npower plants\n");
    out.push_str("  building         source    out   fuel/t  water/t  pumps  upkeep   net\n");
    for plant in &report.power {
        out.push_str(&format!(
            "  {:<16} {:<9} {:>3}  {:>6}  {:>7}  {:>5}  {:>6}  {:>4}\n",
            plant.building,
            plant.source,
            plant.output,
            plant.fuel_energy_per_tick,
            plant.water_per_tick,
            plant.pumps_required,
            plant.upkeep_draw,
            plant.net_output
        ));
        for carried in &plant.carries {
            if carried.machines > 0 {
                out.push_str(&format!(
                    "      carries {:>3} x {} (draw {})\n",
                    carried.machines, carried.building, carried.power_draw
                ));
            }
        }
    }

    out.push_str("\nfuel\n");
    out.push_str("  item          value   recipe          energy in   energy out   gain\n");
    for entry in &report.fuel {
        out.push_str(&format!(
            "  {:<12} {:>6}   {:<14} {:>9}   {:>10}   {}\n",
            entry.item,
            entry.fuel_value,
            entry.recipe.clone().unwrap_or_else(|| "-".to_string()),
            entry.input_energy,
            entry.output_energy,
            entry
                .gain_milli
                .map_or_else(|| "-".to_string(), |value| format!("{}x", milli(value)))
        ));
    }

    out.push_str("\nitem cost, expanded through the whole tree\n");
    out.push_str("  item          depth   raw units   fuel   ticks   effort\n");
    for item in &report.items {
        if item.depth == 0 {
            continue;
        }
        out.push_str(&format!(
            "  {:<12} {:>5}  {:>10}  {:>5}  {:>6}  {:>7}\n",
            item.item,
            item.depth,
            milli(item.raw_units_milli),
            milli(item.fuel_energy_milli),
            milli(item.machine_ticks_milli),
            milli(item.effort_milli)
        ));
    }

    out.push_str("\nbuilding cost, expanded through the whole tree\n");
    out.push_str("  building         depth   raw units   fuel   effort   batch  ticks  raw\n");
    for building in &report.buildings {
        let raw = building
            .raw
            .iter()
            .map(|entry| format!("{} {}", milli(entry.quantity_milli), entry.item))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "  {:<16} {:>5}  {:>10}  {:>5}  {:>7}  {:>6}  {:>5}  {}\n",
            building.building,
            building.depth,
            milli(building.raw_units_milli),
            milli(building.fuel_energy_milli),
            milli(building.effort_milli),
            building.batch_units,
            building.machine_ticks,
            raw
        ));
    }

    out.push_str("\ncurve\n");
    for step in &report.curve {
        out.push_str(&format!(
            "  {}  {:<16} {:>8} follows {:<16} {:>8}  {}\n",
            if step.holds { "ok  " } else { "BREAK" },
            step.building,
            milli(step.effort_milli),
            step.follows,
            milli(step.follows_effort_milli),
            step.relation
        ));
    }

    out.push_str("\nmaterial access, default preset\n");
    out.push_str("  material        walk   hexes   nearest   reachable   required by\n");
    for material in &report.access {
        out.push_str(&format!(
            "  {:<12} {:>7}  {:>6}   {:>7}   {:>9}   {:>11}\n",
            material.material,
            material
                .guaranteed_walk
                .map_or_else(|| "none".to_string(), |value| value.to_string()),
            material.guaranteed_hexes,
            material
                .nearest_generated
                .map_or_else(|| "none".to_string(), |value| value.to_string()),
            material.reachable,
            material.required_by
        ));
    }

    out.push_str("\nsite yield, mean units an extractor empties\n");
    out.push_str(
        "  preset         material      sites   per cell   reach 1   reach 2   same 1   same 2\n",
    );
    for site in &report.extraction {
        let at = |values: &Vec<u32>, index: usize| values.get(index).copied().unwrap_or(0);
        out.push_str(&format!(
            "  {:<14} {:<12} {:>6}   {:>8}   {:>7}   {:>7}   {:>6}   {:>6}\n",
            site.preset,
            site.material,
            site.sites,
            site.mean_cell_quantity,
            at(&site.mean_site_yield, 0),
            at(&site.mean_site_yield, 1),
            at(&site.mean_same_material, 0),
            at(&site.mean_same_material, 1)
        ));
    }

    out.push_str("\nopenings, from a standing start (no walking)\n");
    for opening in &report.openings {
        out.push_str(&format!(
            "  {}: {} gathers ({} for research, {} fuel), {} machine ticks\n",
            opening.name,
            opening.gather_total,
            opening.insight_items,
            opening.fuel_items,
            opening.machine_ticks
        ));
        out.push_str(&format!(
            "      buildings: {} | research: {} insight | gather {}s + machine {}s + player work {}s\n",
            opening.buildings.join(", "),
            opening.insight,
            milli(opening.hand_seconds_milli),
            milli(opening.machine_seconds_milli),
            milli(opening.player_work_seconds_milli)
        ));
        if !opening.commissions.is_empty() {
            out.push_str(&format!(
                "      commissions delivered first: {}\n",
                opening.commissions.join(", ")
            ));
        }
    }

    out.push_str("\ncontract stages, what the landing hub is asking for\n");
    for stage in &report.contracts {
        let bill: Vec<String> = stage
            .bill
            .iter()
            .map(|line| format!("{} {}", line.quantity, line.item))
            .collect();
        out.push_str(&format!(
            "  {}/{}: {} | {} raw material(s)\n",
            stage.scenario,
            stage.stage,
            bill.join(" + "),
            stage.raw_materials
        ));
        out.push_str(&format!(
            "      {} gathers ({} for research, {} fuel) | buildings: {} | gather {}s + machine {}s + player work {}s\n",
            stage.opening.gather_total,
            stage.opening.insight_items,
            stage.opening.fuel_items,
            stage.opening.buildings.join(", "),
            milli(stage.opening.hand_seconds_milli),
            milli(stage.opening.machine_seconds_milli),
            milli(stage.opening.player_work_seconds_milli)
        ));
        if !stage.opening.commissions.is_empty() {
            out.push_str(&format!(
                "      commissions delivered first: {}\n",
                stage.opening.commissions.join(", ")
            ));
        }
    }
    out
}
