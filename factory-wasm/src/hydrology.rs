//! Phase 8 slice 4: the water a player has moved, and the bounded solve that settles it.
//!
//! Generated hydrology is an *equilibrium*. `terra` publishes a bed and the depth standing on it as
//! a pure function of seed and coordinate, so a world nobody has touched needs no water state at
//! all — the ocean, the lakes and the rivers are answers, not entities. What has to be **saved** is
//! the departure from that equilibrium: the cut that filled, the dam that backed a reach up, the
//! pond a pump took down. This module owns that departure and the solve that returns it to a fixed
//! point.
//!
//! # Why the bounds hold by construction rather than by care
//!
//! `docs/HEXFACTORY-PLAN.md#water-is-equilibrium-plus-sparse-disturbance` forbids a per-cell water
//! kernel, forbids letting a hydrology query grow the generated world, and requires every solve to
//! terminate inside an explicit budget. Each of those is a structural property here, not a rule
//! somebody has to remember:
//!
//! 1. **The solve cannot run forever.** A transfer moves `k` quanta from a cell to a neighbour whose
//!    surface is `d >= 2` lower, with `k <= d/2`. That changes the sum of squared surfaces by
//!    `2k(k - d) <= -kd < 0`, so a transfer inside the region strictly decreases a non-negative
//!    integer potential; a transfer to a boundary strictly decreases the region's total volume,
//!    another non-negative integer. The pair decreases lexicographically on every single transfer,
//!    which is termination without appeal to the sweep budget. [`SETTLE_SWEEP_BUDGET`] is the second
//!    fence, and the report says which one stopped the solve.
//! 2. **The solve cannot generate world.** [`WaterField::surveyed`] is the only question asked about
//!    a cell beyond the frontier. An unsurveyed neighbour is never read for a bed, a depth or a
//!    band: it drains the source cell's own *equilibrium* surface, so the boundary flux is computed
//!    entirely from cells the player has already surveyed. A field that panics on an unsurveyed read
//!    is what the test suite hands the solver.
//! 3. **The region cannot be unbounded.** It starts as the disturbed cells and their rings, and
//!    then grows only where the settling water actually asks for more ground — a wall neighbour a
//!    region cell could pour onto, or a wet one that could pour in. It stops at
//!    [`ACTIVE_CELL_BUDGET`]. A truncated rim is a *wall*, never a sink: water piles against it and
//!    the report says the solve is unfinished, because a budget that quietly ate the water would be
//!    a conservation bug wearing a bound's clothes.
//!
//! # The stated residual
//!
//! Depth is an integer count of 0.25 m quanta and a transfer needs a two-quantum head, so a settled
//! region leaves neighbouring water surfaces at most one quantum apart. That is the model's answer,
//! not its error: a single quantum is 25 cm over 25 m², and a puddle that will not split itself in
//! half across a flat pan is what real ground does too. Every cell still drains *dry* into any
//! neighbour whose bed is a quantum below its own, which is the case a player notices.

use super::*;

/// How many cells one disturbance may put in flight at once.
///
/// Shares its value with [`crate::terra::LAKE_CELL_BUDGET`] deliberately: the generator refuses to
/// close a basin wider than this and calls it a frontier basin, and a player-made pool has no claim
/// to a larger working set than a natural one.
pub(super) const ACTIVE_CELL_BUDGET: usize = crate::terra::LAKE_CELL_BUDGET;

/// How many relaxation sweeps one solve may take before it reports itself unfinished.
///
/// The potential argument in this module's header already forbids an infinite solve, so this is the
/// *latency* fence rather than the termination one: it bounds the work a single tick may spend, and
/// an unfinished region stays scheduled instead of being abandoned. It is counted across the whole
/// solve, growth rounds included, so a spreading sheet cannot buy extra sweeps by claiming ground.
pub(super) const SETTLE_SWEEP_BUDGET: u32 = 1_024;

/// How far standing water may depart from its generated equilibrium, in height quanta: 4 km either
/// way.
///
/// A storage guard rather than a design limit. The departure is kept in an `i16` so the sparse map
/// stays small, and the generator's entire relief span — [`crate::scale::BED_MIN_QUANTA`] to
/// [`crate::scale::BED_MAX_QUANTA`], 9,600 quanta — fits inside this with room over, so no legal
/// dam, cut or flood can reach the wall. A solve that does is reporting a defect, and says so
/// through [`SettleReport::clamped`] rather than wrapping an integer.
pub(super) const DEPARTURE_LIMIT_QUANTA: i32 = 16_000;

/// The largest explicit flood or drain one command may request: eight metres of standing water.
///
/// The ground tool uses the same physical ceiling. Keeping the command bounded independently of
/// the storage guard means a forged input cannot turn the generous corruption fence above into a
/// gameplay-sized allocation or a long-running solve.
pub(super) const WATER_COMMAND_LIMIT_QUANTA: u16 = 32;

/// One bounded request to move standing water at a named surveyed cell.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum WaterAction {
    Flood,
    Drain,
}

/// Departure from the generated equilibrium depth at one cell, in height quanta.
///
/// Signed, because a drained cut and a flooded one are the same kind of fact. Zero is not stored:
/// see [`DisturbedWater::set`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct WaterDelta(i16);

impl WaterDelta {
    pub(super) const fn new(value: i16) -> Self {
        Self(value)
    }

    pub(super) const fn get(self) -> i16 {
        self.0
    }
}

/// Every cell whose water has left the equilibrium, and nothing else.
///
/// This is the saved and checksummed half of Phase 8 water. It is a departure set in the strict
/// sense: a cell that returns to its generated depth is *removed*, so a world that was flooded and
/// drained back hashes identically to one that was never touched. Anything that made the store
/// remember where the player had been would put presentation history into the checksum.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct DisturbedWater {
    cells: BTreeMap<(i32, i32), WaterDelta>,
}

impl DisturbedWater {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn delta_at(&self, q: i32, r: i32) -> WaterDelta {
        self.cells.get(&(q, r)).copied().unwrap_or_default()
    }

    /// Record a departure, or forget the cell when it has none.
    pub(super) fn set(&mut self, q: i32, r: i32, delta: WaterDelta) {
        if delta.get() == 0 {
            self.cells.remove(&(q, r));
        } else {
            self.cells.insert((q, r), delta);
        }
    }

    pub(super) fn len(&self) -> usize {
        self.cells.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (&(i32, i32), &WaterDelta)> {
        self.cells.iter()
    }

    /// The departure set as a file carries it, in key order.
    pub(super) fn cells(&self) -> Vec<WaterCell> {
        self.cells
            .iter()
            .map(|(&(q, r), delta)| WaterCell {
                q,
                r,
                departure: delta.get(),
            })
            .collect()
    }

    /// Restore a departure set. A zero is dropped rather than kept, so a hand-edited file cannot
    /// introduce a cell the running store would never have written.
    pub(super) fn from_cells(cells: &[WaterCell]) -> Self {
        let mut water = Self::new();
        for cell in cells {
            water.set(cell.q, cell.r, WaterDelta::new(cell.departure));
        }
        water
    }

    /// Write a recorded set of departures back, cell by cell.
    pub(super) fn apply(&mut self, cells: &[WaterCell]) {
        for cell in cells {
            self.set(cell.q, cell.r, WaterDelta::new(cell.departure));
        }
    }

    /// What this set holds at every cell a later one disagrees with — the exact write-back that
    /// returns `later` to `self`.
    ///
    /// A cell the later set forgot is recorded at the departure it used to carry, and a cell the
    /// later set invented is recorded as zero, so [`apply`](Self::apply) of the result is a true
    /// inverse rather than an approximate one. That is what lets an earthwork undo put the water
    /// back instead of solving for it a second time and hoping the answer matches.
    pub(super) fn reversal_of(&self, later: &Self) -> Vec<WaterCell> {
        let mut cells: Vec<WaterCell> = later
            .cells
            .iter()
            .filter(|(cell, delta)| self.delta_at(cell.0, cell.1) != **delta)
            .map(|(&(q, r), _)| WaterCell {
                q,
                r,
                departure: self.delta_at(q, r).get(),
            })
            .chain(
                self.cells
                    .iter()
                    .filter(|(cell, _)| !later.cells.contains_key(*cell))
                    .map(|(&(q, r), delta)| WaterCell {
                        q,
                        r,
                        departure: delta.get(),
                    }),
            )
            .collect();
        cells.sort_unstable_by_key(|cell| (cell.q, cell.r));
        cells
    }

    /// The checksum contribution, in the map's own key order so it cannot depend on how the water
    /// got there.
    pub(super) fn hash_into(&self, hash: &mut u32) {
        hash_u64(hash, self.cells.len() as u64);
        for (&(q, r), delta) in &self.cells {
            hash_i32(hash, q);
            hash_i32(hash, r);
            hash_i32(hash, i32::from(delta.get()));
        }
    }
}

/// One saved departure, as the file carries it.
///
/// A cell identified by its coordinates and nothing else, on the same rule `ResourceSnapshot`
/// follows: a `u64` packed from two `i32`s is past the range a JSON number carries exactly, and
/// whole columns of a field once collapsed onto one value because of it.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct WaterCell {
    pub(super) q: i32,
    pub(super) r: i32,
    /// Signed quanta away from the depth the generator publishes here.
    pub(super) departure: i16,
}

/// Refuse a departure set no running solve could have written.
///
/// The same shape as `validate_saved_ground`: one identity per cell, and every departure inside the
/// storage guard. A file that fails this is rejected rather than clamped, because a clamped file is
/// a file whose checksum no longer describes it.
pub(super) fn validate_saved_water(saved: &[WaterCell]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for cell in saved {
        if !seen.insert((cell.q, cell.r))
            || i32::from(cell.departure).abs() > DEPARTURE_LIMIT_QUANTA
        {
            return Err("Invalid saved water identity or departure".into());
        }
    }
    Ok(())
}

/// What the solver is allowed to ask the world.
///
/// Deliberately four narrow questions rather than a `&Core`. The interesting invariant of this
/// module is *which reads it does not perform*, and a trait is how that becomes checkable: the tests
/// hand it a field that panics on an unsurveyed bed, and the "hydrology never generates a chunk"
/// rule stops being a promise.
pub(super) trait WaterField {
    /// Finished bed height in height quanta — generated bed plus earthwork plus erosion.
    fn bed_quanta(&self, q: i32, r: i32) -> i32;

    /// The depth the generator publishes standing on that bed, in height quanta.
    fn equilibrium_depth(&self, q: i32, r: i32) -> i32;

    /// Whether the cell is inside the surveyed frontier. The only question asked about a cell
    /// outside it.
    fn surveyed(&self, q: i32, r: i32) -> bool;

    /// Whether the cell belongs to an unbounded derived body — the ocean. It absorbs any outflow,
    /// supplies no departure and is never simulated.
    fn ocean(&self, q: i32, r: i32) -> bool;
}

/// The cells one disturbance put in flight, in a deterministic order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ActiveRegion {
    cells: BTreeSet<(i32, i32)>,
    truncated: bool,
}

impl ActiveRegion {
    pub(super) fn len(&self) -> usize {
        self.cells.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub(super) fn contains(&self, q: i32, r: i32) -> bool {
        self.cells.contains(&(q, r))
    }

    /// Whether [`ACTIVE_CELL_BUDGET`] cut the region short of the water's own reach.
    pub(super) fn truncated(&self) -> bool {
        self.truncated
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.cells.iter().copied()
    }
}

/// What one solve did, in numbers a benchmark and a test can both read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct SettleReport {
    /// Cells in the active region.
    pub(super) cells: usize,
    /// Relaxation sweeps performed. A settled solve always spends one final sweep proving it.
    pub(super) sweeps: u32,
    /// Individual quanta transfers.
    pub(super) transfers: u64,
    /// Quanta that left the world at the ocean or the surveyed frontier.
    pub(super) outflow_quanta: i64,
    /// Water waiting just beyond the surveyed frontier, keyed by the first unsurveyed cell it
    /// enters. Core stores this as an unsurveyed departure and resumes it when that chunk opens.
    pub(super) frontier: BTreeMap<(i32, i32), i32>,
    /// True when the region reached a fixed point inside every budget.
    pub(super) settled: bool,
    /// True when [`ACTIVE_CELL_BUDGET`] truncated the region.
    pub(super) truncated: bool,
    /// Cells whose departure had to be clamped to [`DEPARTURE_LIMIT_QUANTA`]. Always zero; a
    /// non-zero count is a defect report, not a gameplay outcome.
    pub(super) clamped: usize,
}

/// Take one cell into the region, or report that the budget is spent.
///
/// The ocean and the unsurveyed world are boundaries, never members: admitting either would be the
/// two things the plan forbids outright — simulating the sea, and letting a hydrology query claim
/// ground the player has not surveyed.
fn admit<F: WaterField>(field: &F, region: &mut ActiveRegion, cell: (i32, i32)) -> bool {
    if !field.surveyed(cell.0, cell.1) || field.ocean(cell.0, cell.1) {
        return true;
    }
    if region.cells.contains(&cell) {
        return true;
    }
    if region.cells.len() >= ACTIVE_CELL_BUDGET {
        region.truncated = true;
        return false;
    }
    region.cells.insert(cell);
    true
}

/// The cells one disturbance starts with: the seeds themselves and their immediate rings.
///
/// Deliberately not the water's whole reach. A disturbance is local by definition — a cut, a dam, a
/// pump — and where the water then *goes* is the solve's answer rather than its input. [`settle`]
/// grows this region only where settling water actually asks for more ground, so a dry edit costs
/// seven cells and a flood pays for exactly the ground it covers.
pub(super) fn active_region<F: WaterField>(field: &F, seeds: &[(i32, i32)]) -> ActiveRegion {
    let mut region = ActiveRegion::default();
    for &seed in seeds {
        if !admit(field, &mut region, seed) {
            return region;
        }
        for &(dq, dr) in &DIRECTIONS {
            if !admit(field, &mut region, (seed.0 + dq, seed.1 + dr)) {
                return region;
            }
        }
    }
    region
}

/// The lowest surface a neighbour offers, and whether it is a boundary the water leaves through.
///
/// `None` for a wall: surveyed dry land outside the region, which is ground the region has not
/// claimed and water may not be poured onto without claiming it first.
fn neighbour_surface<F: WaterField>(
    field: &F,
    region: &ActiveRegion,
    depth: &BTreeMap<(i32, i32), i32>,
    from: (i32, i32),
    to: (i32, i32),
) -> Option<(i32, bool)> {
    if region.contains(to.0, to.1) {
        return Some((field.bed_quanta(to.0, to.1) + depth[&to], false));
    }
    if !field.surveyed(to.0, to.1) {
        // The frontier flux, computed without reading a single unsurveyed cell: water standing above
        // this cell's own generated equilibrium runs off the edge of the surveyed world, and water
        // at or below that equilibrium has nowhere to go.
        return Some((
            field.bed_quanta(from.0, from.1) + field.equilibrium_depth(from.0, from.1),
            true,
        ));
    }
    if field.ocean(to.0, to.1) {
        return Some((
            field.bed_quanta(to.0, to.1) + field.equilibrium_depth(to.0, to.1),
            true,
        ));
    }
    None
}

/// One relaxation pass over the region, in key order. Returns whether anything moved.
fn sweep<F: WaterField>(
    field: &F,
    region: &ActiveRegion,
    depth: &mut BTreeMap<(i32, i32), i32>,
    report: &mut SettleReport,
) -> bool {
    let mut moved = false;
    for (q, r) in region.iter() {
        let held = depth[&(q, r)];
        if held <= 0 {
            continue;
        }
        let bed = field.bed_quanta(q, r);
        let surface = bed + held;
        // Ties go to the lower direction index, which is why the scan is strictly less-than.
        let mut target: Option<(i32, (i32, i32), bool)> = None;
        for &(dq, dr) in &DIRECTIONS {
            let to = (q + dq, r + dr);
            let Some((offered, boundary)) = neighbour_surface(field, region, depth, (q, r), to)
            else {
                continue;
            };
            if target.is_none_or(|(best, _, _)| offered < best) {
                target = Some((offered, to, boundary));
            }
        }
        let Some((lowest, to, boundary)) = target else {
            continue;
        };
        let head = surface - lowest;
        if head < 2 {
            continue;
        }
        let quanta = held.min(head / 2);
        *depth.get_mut(&(q, r)).expect("the cell is in the region") -= quanta;
        if boundary {
            report.outflow_quanta += i64::from(quanta);
            if !field.surveyed(to.0, to.1) {
                *report.frontier.entry(to).or_default() += quanta;
            }
        } else {
            *depth
                .get_mut(&to)
                .expect("a region neighbour is in the map") += quanta;
        }
        report.transfers += 1;
        moved = true;
    }
    moved
}

/// Ground the settled region now wants, in key order: a wall a region cell could pour onto, or a wet
/// wall that could pour in.
fn reachable_walls<F: WaterField>(
    field: &F,
    region: &ActiveRegion,
    depth: &BTreeMap<(i32, i32), i32>,
    water: &DisturbedWater,
) -> BTreeSet<(i32, i32)> {
    let mut walls = BTreeSet::new();
    for (q, r) in region.iter() {
        let surface = field.bed_quanta(q, r) + depth[&(q, r)];
        for &(dq, dr) in &DIRECTIONS {
            let (nq, nr) = (q + dq, r + dr);
            if region.contains(nq, nr) || !field.surveyed(nq, nr) || field.ocean(nq, nr) {
                continue;
            }
            let bed = field.bed_quanta(nq, nr);
            let held = field.equilibrium_depth(nq, nr) + i32::from(water.delta_at(nq, nr).get());
            let pours_out = depth[&(q, r)] > 0 && surface - bed >= 2;
            let pours_in = held > 0 && (bed + held) - surface >= 2;
            if pours_out || pours_in {
                walls.insert((nq, nr));
            }
        }
    }
    walls
}

/// Relax one disturbance to a fixed point and write the result back as departure.
///
/// The solve alternates relaxation with growth: settle what the region holds, then claim whatever
/// ground the settled water has reached, and settle again. It ends when nothing moves and nothing
/// more is reachable — or when a budget stops it, which the report says plainly. A caller that gets
/// `settled: false` has an unfinished region and must reschedule it; dropping one would leave water
/// standing above its own outlet.
pub(super) fn settle<F: WaterField>(
    field: &F,
    water: &mut DisturbedWater,
    seeds: &[(i32, i32)],
) -> SettleReport {
    let mut region = active_region(field, seeds);
    let mut report = SettleReport::default();
    let held_at = |water: &DisturbedWater, (q, r): (i32, i32)| {
        field.equilibrium_depth(q, r) + i32::from(water.delta_at(q, r).get())
    };
    if region.is_empty() {
        report.truncated = region.truncated();
        report.settled = !report.truncated;
        return report;
    }

    let mut depth: BTreeMap<(i32, i32), i32> = region
        .iter()
        .map(|cell| (cell, held_at(water, cell)))
        .collect();

    loop {
        let mut at_rest = false;
        while report.sweeps < SETTLE_SWEEP_BUDGET {
            report.sweeps += 1;
            if !sweep(field, &region, &mut depth, &mut report) {
                at_rest = true;
                break;
            }
        }
        if !at_rest {
            // The latency fence stopped the solve mid-flow. The region is still consistent — every
            // transfer is complete — so the state is written back and the caller reschedules.
            break;
        }
        let walls = reachable_walls(field, &region, &depth, water);
        if walls.is_empty() {
            report.settled = !region.truncated();
            break;
        }
        let mut claimed = false;
        for cell in walls {
            if !admit(field, &mut region, cell) {
                break;
            }
            depth.insert(cell, held_at(water, cell));
            claimed = true;
        }
        if !claimed {
            break;
        }
    }

    report.cells = region.len();
    report.truncated = region.truncated();
    for (&(q, r), &held) in &depth {
        let departure = held - field.equilibrium_depth(q, r);
        let bounded = departure.clamp(-DEPARTURE_LIMIT_QUANTA, DEPARTURE_LIMIT_QUANTA);
        if bounded != departure {
            report.clamped += 1;
        }
        water.set(
            q,
            r,
            WaterDelta::new(i16::try_from(bounded).expect("the departure is clamped to an i16")),
        );
    }

    report
}

/// The running world, answering the solver's four questions and nothing more.
///
/// Every one of them is a fact the Core already publishes. `bed_quanta` is the finished ground the
/// player walks on — generated bed plus earthwork plus erosion — because water stands on what was
/// dug, not on what was generated. `surveyed` is `generated_chunks` itself rather than a second
/// account of it, which is what makes "hydrology cannot grow the world" true by construction: the
/// only set the solver can read is the one the player has already opened.
impl WaterField for Core {
    fn bed_quanta(&self, q: i32, r: i32) -> i32 {
        self.ground_elevation_at(q, r)
    }

    fn equilibrium_depth(&self, q: i32, r: i32) -> i32 {
        self.generated_ground_at(q, r).hydrology.depth_quanta
    }

    fn surveyed(&self, q: i32, r: i32) -> bool {
        let size = self.scenario.chunk_size;
        self.generated_chunks
            .contains(&(floor_div(q, size), floor_div(r, size)))
    }

    /// Standing water whose surface is at or below the datum. That is the whole definition of the
    /// ocean here — the plan's rule is that static water at or below sea level stays static, and
    /// deriving it from the published surface means no second flag can drift away from it.
    fn ocean(&self, q: i32, r: i32) -> bool {
        let hydrology = self.generated_ground_at(q, r).hydrology;
        hydrology.depth_quanta > 0 && hydrology.surface.get() <= crate::scale::SEA_LEVEL_QUANTA
    }
}

impl Core {
    /// The one native water predicate. Movement, construction, wading, route search and pumps read
    /// this and never a terrain band: the band is a picture of the generated equilibrium, and the
    /// player is allowed to have changed it.
    pub(super) fn water_depth_at(&self, q: i32, r: i32) -> i32 {
        self.water_depth_of(self.generated_ground_at(q, r), q, r)
    }

    /// The same answer when the caller already holds the generated facts, so a legality check costs
    /// one trip through the surveyed cache rather than two.
    pub(super) fn water_depth_of(&self, generated: GeneratedGround, q: i32, r: i32) -> i32 {
        (generated.hydrology.depth_quanta + i32::from(self.water.delta_at(q, r).get())).max(0)
    }

    /// Where the water's top surface stands, in the same absolute quanta as the ground.
    pub(super) fn water_surface_at(&self, q: i32, r: i32) -> i32 {
        self.ground_elevation_at(q, r) + self.water_depth_at(q, r)
    }

    /// Settle a disturbance and keep whatever it leaves behind.
    ///
    /// The departure set is lifted out for the solve so the field the solver reads is the finished
    /// ground and the generated equilibrium, never a half-updated copy of its own answer.
    pub(super) fn settle_water(&mut self, seeds: &[(i32, i32)]) -> SettleReport {
        let mut water = std::mem::take(&mut self.water);
        let report = settle(&*self, &mut water, seeds);
        // Frontier flux is not discarded. Its target is deliberately still unsurveyed, so adding
        // it needs no bed or equilibrium query: departure changes by exactly the quanta crossing
        // the edge. The cell stays invisible until generation publishes its bed, then
        // `generate_chunk` resumes the solve from it.
        for (&(q, r), &quanta) in &report.frontier {
            let departure = i32::from(water.delta_at(q, r).get()) + quanta;
            let bounded = departure.clamp(-DEPARTURE_LIMIT_QUANTA, DEPARTURE_LIMIT_QUANTA);
            water.set(
                q,
                r,
                WaterDelta::new(
                    i16::try_from(bounded).expect("frontier departure is clamped to an i16"),
                ),
            );
        }
        self.water = water;
        self.dirty.water = true;
        report
    }

    /// Apply one explicit, bounded flood or drain and settle the region it wakes.
    pub(super) fn edit_water(
        &mut self,
        q: i32,
        r: i32,
        action: WaterAction,
        quanta: u16,
    ) -> Result<SettleReport, String> {
        if !self.ground_is_physical() {
            return Err("water edits require physical ground".into());
        }
        if quanta == 0 || quanta > WATER_COMMAND_LIMIT_QUANTA {
            return Err(format!(
                "water depth must be in 1..={WATER_COMMAND_LIMIT_QUANTA} quanta"
            ));
        }
        if !WaterField::surveyed(self, q, r) {
            return Err("water target is outside the surveyed world".into());
        }
        let current = self.water_depth_at(q, r);
        let change = match action {
            WaterAction::Flood => i32::from(quanta),
            WaterAction::Drain => -current.min(i32::from(quanta)),
        };
        if change == 0 {
            return Err("there is no water there to drain".into());
        }
        let departure = i32::from(self.water.delta_at(q, r).get()) + change;
        let departure = departure.clamp(-DEPARTURE_LIMIT_QUANTA, DEPARTURE_LIMIT_QUANTA);
        self.water.set(
            q,
            r,
            WaterDelta::new(i16::try_from(departure).expect("water command is bounded")),
        );
        Ok(self.settle_water(&[(q, r)]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand-built patch of ground. Anything outside `surveyed` panics on a bed or depth read, so
    /// "the solver never looks past the frontier" is checked by every test in this module at once.
    struct TestField {
        beds: BTreeMap<(i32, i32), i32>,
        equilibrium: BTreeMap<(i32, i32), i32>,
        ocean: BTreeSet<(i32, i32)>,
        default_bed: i32,
        /// Cells outside this set are unsurveyed. `None` surveys everything the map names.
        surveyed: Option<BTreeSet<(i32, i32)>>,
    }

    impl TestField {
        /// A flat pan of the given radius at height `bed`.
        fn flat(radius: i32, bed: i32) -> Self {
            let beds = hexes_in_radius((0, 0), radius)
                .into_iter()
                .map(|cell| (cell, bed))
                .collect();
            Self {
                beds,
                equilibrium: BTreeMap::new(),
                ocean: BTreeSet::new(),
                default_bed: bed,
                surveyed: None,
            }
        }

        fn bed(mut self, q: i32, r: i32, height: i32) -> Self {
            self.beds.insert((q, r), height);
            self
        }

        fn water(mut self, q: i32, r: i32, depth: i32) -> Self {
            self.equilibrium.insert((q, r), depth);
            self
        }

        fn sea(mut self, q: i32, r: i32) -> Self {
            self.ocean.insert((q, r));
            self.beds.insert((q, r), crate::scale::SEA_LEVEL_QUANTA - 8);
            self.equilibrium.insert((q, r), 8);
            self
        }

        fn survey(mut self, cells: &[(i32, i32)]) -> Self {
            self.surveyed = Some(cells.iter().copied().collect());
            self
        }

        fn assert_surveyed(&self, q: i32, r: i32) {
            assert!(
                self.surveyed(q, r),
                "the solver read an unsurveyed cell at {q},{r}"
            );
        }

        fn total_depth(&self, water: &DisturbedWater) -> i32 {
            self.beds
                .keys()
                .filter(|cell| !self.ocean.contains(cell))
                .map(|&(q, r)| self.equilibrium_depth(q, r) + i32::from(water.delta_at(q, r).get()))
                .sum()
        }

        fn surface(&self, water: &DisturbedWater, q: i32, r: i32) -> i32 {
            self.bed_quanta(q, r)
                + self.equilibrium_depth(q, r)
                + i32::from(water.delta_at(q, r).get())
        }
    }

    impl WaterField for TestField {
        fn bed_quanta(&self, q: i32, r: i32) -> i32 {
            self.assert_surveyed(q, r);
            self.beds.get(&(q, r)).copied().unwrap_or(self.default_bed)
        }

        fn equilibrium_depth(&self, q: i32, r: i32) -> i32 {
            self.assert_surveyed(q, r);
            self.equilibrium.get(&(q, r)).copied().unwrap_or(0)
        }

        fn surveyed(&self, q: i32, r: i32) -> bool {
            match &self.surveyed {
                Some(cells) => cells.contains(&(q, r)),
                None => self.beds.contains_key(&(q, r)),
            }
        }

        fn ocean(&self, q: i32, r: i32) -> bool {
            self.ocean.contains(&(q, r))
        }
    }

    /// The model's whole statement about a single disturbance, in the order a player produces it:
    /// an untouched world stores nothing, a cut beside a pool levels with it, an odd volume rests
    /// one quantum apart, a real step drains a cell dry, and a reopened outlet empties a bowl down
    /// its channel without losing a quantum on the way.
    #[test]
    fn water_levels_with_what_is_dug_beside_it_and_stores_only_the_difference() {
        let field = TestField::flat(4, 100).water(0, 0, 3).bed(0, 0, 94);
        let mut water = DisturbedWater::new();
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled, "an undisturbed pool is already settled");
        assert_eq!(report.transfers, 0);
        assert!(
            water.is_empty(),
            "equilibrium is not a departure and must not be stored"
        );

        // A pool six quanta deep in a pit, and a neighbouring cell dug to the same floor.
        let field = TestField::flat(4, 100)
            .bed(0, 0, 94)
            .water(0, 0, 6)
            .bed(1, 0, 94);
        let mut water = DisturbedWater::new();
        let before = field.total_depth(&water);
        let report = settle(&field, &mut water, &[(1, 0)]);
        assert!(report.settled);
        assert_eq!(report.outflow_quanta, 0, "nothing reached a boundary");
        assert_eq!(
            field.total_depth(&water),
            before,
            "a transfer between two cells of the region conserves depth"
        );
        assert_eq!(
            field.surface(&water, 0, 0),
            field.surface(&water, 1, 0),
            "two cells on one floor level exactly at an even depth"
        );
        assert_eq!(water.delta_at(1, 0).get(), 3);
        assert_eq!(water.delta_at(0, 0).get(), -3);

        // An odd volume cannot split evenly, which is the residual this model states.
        let field = TestField::flat(4, 100)
            .bed(0, 0, 94)
            .water(0, 0, 7)
            .bed(1, 0, 94);
        let mut water = DisturbedWater::new();
        settle(&field, &mut water, &[(1, 0)]);
        let gap = field.surface(&water, 0, 0) - field.surface(&water, 1, 0);
        assert_eq!(gap.abs(), 1, "an odd volume rests one quantum apart");

        // One quantum standing on ground a quantum above its neighbour: head 2, so it leaves.
        let field = TestField::flat(4, 100)
            .bed(0, 0, 100)
            .water(0, 0, 1)
            .bed(1, 0, 99);
        let mut water = DisturbedWater::new();
        settle(&field, &mut water, &[(0, 0)]);
        assert_eq!(
            field.equilibrium_depth(0, 0) + i32::from(water.delta_at(0, 0).get()),
            0,
            "the cell drained"
        );
        assert_eq!(
            field.equilibrium_depth(1, 0) + i32::from(water.delta_at(1, 0).get()),
            1,
        );

        // A bowl at 94 holding twelve quanta, and a graded channel out of it to a far lower shelf.
        let mut field = TestField::flat(6, 100).bed(0, 0, 94).water(0, 0, 12);
        for step in 1..=5 {
            field = field.bed(step, 0, 94 - step * 2);
        }
        let mut water = DisturbedWater::new();
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled, "{report:?}");
        let held = field.equilibrium_depth(0, 0) + i32::from(water.delta_at(0, 0).get());
        assert_eq!(held, 0, "the cut drained down its reopened outlet");
        assert_eq!(
            field.total_depth(&water),
            12,
            "the water went down the channel rather than out of existence"
        );
    }

    /// The two boundaries a solve can reach, and what each of them is allowed to do with the water
    /// that arrives: the ocean absorbs it and never itself departs, and the surveyed frontier lets
    /// it leave while naming what left — without either reading past the frontier or mistaking a
    /// generated pool sitting on it for a leak.
    #[test]
    fn water_leaves_at_a_boundary_without_the_boundary_being_simulated() {
        let field = TestField::flat(4, 100)
            .bed(0, 0, 20)
            .water(0, 0, 40)
            .sea(1, 0);
        let mut water = DisturbedWater::new();
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled);
        assert!(report.outflow_quanta > 0, "water reached the sea");
        assert_eq!(
            water.delta_at(1, 0),
            WaterDelta::default(),
            "the ocean is a boundary condition and never departs"
        );

        // Everything but this single cell is unsurveyed, so any read beyond it panics in the field.
        let field = TestField::flat(2, 100).bed(0, 0, 100).survey(&[(0, 0)]);
        let mut water = DisturbedWater::new();
        // Nine quanta of departure standing on dry generated ground, with nowhere surveyed to go.
        water.set(0, 0, WaterDelta::new(9));
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled);
        assert_eq!(
            report.outflow_quanta, 8,
            "water above the generated equilibrium runs off the surveyed edge"
        );
        assert_eq!(
            report.frontier.values().copied().sum::<i32>(),
            8,
            "frontier outflow is named for later survey rather than discarded"
        );
        assert_eq!(
            water.delta_at(0, 0).get(),
            1,
            "the stated residual holds at a boundary too: the last quantum has no two-quantum head"
        );
        assert_eq!(field.surface(&water, 0, 0), 101);

        // And a generated pool that happens to sit on the frontier is not a leak: nothing departs
        // and nothing is stored.
        let field = TestField::flat(2, 100)
            .bed(0, 0, 94)
            .water(0, 0, 6)
            .survey(&[(0, 0)]);
        let mut water = DisturbedWater::new();
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled);
        assert_eq!(
            report.outflow_quanta, 0,
            "a generated pool at the frontier is not a leak"
        );
        assert!(water.is_empty());
    }

    /// What the saved departure set is, as distinct from the water: the solve's answer cannot
    /// depend on the order its seeds arrived in, and a cell that is flooded and drained back is
    /// forgotten entirely rather than stored as a zero — it has to hash as untouched, or every
    /// checksum would record work that left no trace on the world.
    #[test]
    fn the_departure_set_is_order_independent_and_forgets_what_returned() {
        let build = || {
            TestField::flat(5, 100)
                .bed(0, 0, 90)
                .water(0, 0, 10)
                .bed(1, 0, 92)
                .bed(-1, 0, 91)
                .bed(0, 1, 93)
        };
        let forward = {
            let mut water = DisturbedWater::new();
            settle(&build(), &mut water, &[(0, 0), (1, 0), (-1, 0), (0, 1)]);
            water
        };
        let reversed = {
            let mut water = DisturbedWater::new();
            settle(&build(), &mut water, &[(0, 1), (-1, 0), (1, 0), (0, 0)]);
            water
        };
        assert_eq!(forward, reversed, "the solve is order independent");

        let untouched = {
            let mut hash = 0x811c_9dc5u32;
            DisturbedWater::new().hash_into(&mut hash);
            hash
        };
        let mut water = DisturbedWater::new();
        water.set(3, -2, WaterDelta::new(7));
        assert_eq!(water.len(), 1);
        let flooded = {
            let mut hash = 0x811c_9dc5u32;
            water.hash_into(&mut hash);
            hash
        };
        assert_ne!(flooded, untouched);
        water.set(3, -2, WaterDelta::new(0));
        assert!(water.is_empty(), "a returned cell leaves the departure set");
        let drained = {
            let mut hash = 0x811c_9dc5u32;
            water.hash_into(&mut hash);
            hash
        };
        assert_eq!(
            drained, untouched,
            "flooding and draining back is not a saved difference"
        );
    }

    /// What one disturbance is allowed to cost. The active region grows with the water and stops
    /// where the water does — seven cells for dry ground and seven for a pit the pool cannot climb
    /// out of — and where a command is wider than the budget the region says so, walls the water in
    /// rather than losing it, and leaves the solve unfinished for rescheduling. The sweep budget is
    /// the other half of the cost, and the shape that stresses it worst — a rough-floored bowl with
    /// one low rim cell, where every cell has somewhere to push and only one leads out — has to
    /// finish inside it without clamping or going negative anywhere.
    #[test]
    fn a_disturbance_claims_only_what_the_water_covers_and_stops_at_its_budget() {
        let field = TestField::flat(12, 100);
        let mut water = DisturbedWater::new();
        let region = active_region(&field, &[(0, 0)]);
        assert_eq!(region.len(), 7, "the seed and its ring, and no further");
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled);
        assert_eq!(report.cells, 7, "dry ground is never claimed");
        assert_eq!(report.transfers, 0);
        assert!(water.is_empty());

        // Ten quanta in a pit, and a wide flat pan around it the water cannot climb onto.
        let field = TestField::flat(12, 100).bed(0, 0, 90).water(0, 0, 10);
        let mut water = DisturbedWater::new();
        let report = settle(&field, &mut water, &[(0, 0)]);
        assert!(report.settled);
        assert_eq!(
            report.cells, 7,
            "growth follows the water, and this water goes nowhere"
        );

        // A flood command wider than the budget: the seeds alone exhaust it.
        let field = TestField::flat(64, 100);
        let seeds = hexes_in_radius((0, 0), 40);
        assert!(seeds.len() > ACTIVE_CELL_BUDGET);
        let region = active_region(&field, &seeds);
        assert!(
            region.truncated(),
            "a disturbance wider than the budget must report the truncation"
        );
        assert_eq!(region.len(), ACTIVE_CELL_BUDGET);

        let mut field = TestField::flat(64, 100);
        for cell in hexes_in_radius((0, 0), 8) {
            field = field.water(cell.0, cell.1, 40);
        }
        let mut water = DisturbedWater::new();
        let before = field.total_depth(&water);
        let report = settle(&field, &mut water, &hexes_in_radius((0, 0), 40));
        assert!(report.truncated);
        assert!(
            !report.settled,
            "a truncated region is unfinished and must be rescheduled"
        );
        assert_eq!(report.outflow_quanta, 0, "no boundary was reached");
        assert_eq!(
            field.total_depth(&water),
            before,
            "the budget is a wall, not a drain"
        );

        // A bowl with a rough floor and a single low rim cell: the worst shape this model meets,
        // because every cell has somewhere to push and only one of them leads out.
        let mut field = TestField::flat(10, 200);
        for (index, cell) in hexes_in_radius((0, 0), 6).into_iter().enumerate() {
            let jitter = (index % 5) as i32;
            field = field.bed(cell.0, cell.1, 150 + jitter).water(
                cell.0,
                cell.1,
                20 + (index % 7) as i32,
            );
        }
        let mut water = DisturbedWater::new();
        let seeds: Vec<_> = hexes_in_radius((0, 0), 6);
        let report = settle(&field, &mut water, &seeds);
        assert!(report.settled, "{report:?}");
        assert!(report.sweeps < SETTLE_SWEEP_BUDGET, "{report:?}");
        assert_eq!(report.clamped, 0);
        for (q, r) in hexes_in_radius((0, 0), 6) {
            let depth = field.equilibrium_depth(q, r) + i32::from(water.delta_at(q, r).get());
            assert!(
                depth >= 0,
                "a settled cell holds no negative depth at {q},{r}"
            );
        }
    }

    /// The three catalogues the shipped game loads, as `Core::new` and `Core::from_save` take them.
    fn catalogues() -> (DefinitionsInput, TechnologiesInput, ScenariosInput) {
        (
            serde_json::from_str(include_str!("../../src/data/definitions.json")).unwrap(),
            serde_json::from_str(include_str!("../../src/data/technologies.json")).unwrap(),
            serde_json::from_str(include_str!("../../src/data/scenarios.json")).unwrap(),
        )
    }

    /// A real opening world on the physical source, surveyed around the landing shelf.
    fn physical_core() -> Core {
        let (definitions, technologies, scenarios) = catalogues();
        let core = Core::new(
            &definitions,
            &technologies,
            &scenarios.scenarios[0],
            None,
            None,
        )
        .unwrap();
        assert!(
            core.ground_is_physical(),
            "the opening is the physical world"
        );
        assert!(
            !core.generated_chunks.is_empty(),
            "the opening surveys the shelf it starts on"
        );
        core
    }

    /// A dry cell in a dry disc: no pool lip refills a pump draw.
    fn dry_cell(core: &Core) -> (i32, i32) {
        let size = core.scenario.chunk_size;
        let dry = |q: i32, r: i32| {
            let c = core.generated_ground_at(q, r);
            c.hydrology.depth_quanta == 0 && !c.presentation.is_water()
        };
        core.generated_chunks
            .iter()
            .flat_map(|&(cq, cr)| hexes_in_chunk(cq, cr, size))
            .find(|&(q, r)| dry(q, r) && DIRECTIONS.iter().all(|&(dq, dr)| dry(q + dq, r + dr)))
            .expect("the opening shelf is dry")
    }

    /// Survey outward from the origin until a surveyed cell holds inland deep water, and name it.
    fn survey_out_to_deep_water(core: &mut Core) -> Option<(i32, i32)> {
        let size = core.scenario.chunk_size;
        for ring in 0..=12 {
            for dq in -ring..=ring {
                for dr in (-ring).max(-dq - ring)..=ring.min(-dq + ring) {
                    core.generate_chunk(dq, dr);
                    let found = hexes_in_chunk(dq, dr, size).find(|&(q, r)| {
                        core.generated_ground_at(q, r).presentation == Terrain::DeepWater
                            && !core.ocean(q, r)
                    });
                    if found.is_some() {
                        return found;
                    }
                }
            }
        }
        None
    }

    /// Depth is the answer and the band is only a drawing. A meadow under water stops a walk its
    /// own band would allow, a drained deep-water cell is ground whatever the band draws, route
    /// cost and wading read the disturbed depth, and the solver reads the same finished bed the
    /// predicate does — the four ways that one claim can be got wrong.
    #[test]
    fn every_water_answer_comes_from_depth_rather_than_the_band() {
        let mut core = physical_core();
        let (q, r) = dry_cell(&core);
        assert!(!core.terrain_blocks_movement(q, r));
        assert!(!core.terrain_blocks_construction(q, r));

        // A ford: deep enough to refuse a foundation, shallow enough to wade.
        core.water.set(
            q,
            r,
            WaterDelta::new(i16::try_from(crate::scale::WADE_LIMIT_QUANTA - 1).unwrap()),
        );
        assert!(
            !core.terrain_blocks_movement(q, r),
            "water under the wade limit is a ford"
        );
        assert!(
            core.terrain_blocks_construction(q, r),
            "any standing water refuses a foundation"
        );

        core.water.set(
            q,
            r,
            WaterDelta::new(i16::try_from(crate::scale::WADE_LIMIT_QUANTA).unwrap()),
        );
        assert!(
            core.terrain_blocks_movement(q, r),
            "the band still says meadow; the predicate is the answer"
        );
        assert!(
            !core.generated_ground_at(q, r).presentation.is_water(),
            "the flood did not rewrite the generated band"
        );

        // Route cost reads the same depth: one quantum is a ford priced as one, and the wade limit
        // stops the route outright.
        core.water.set(q, r, WaterDelta::new(0));
        assert!(!core.shallow_water_at(q, r));
        core.water.set(q, r, WaterDelta::new(1));
        assert!(core.shallow_water_at(q, r), "a flooded meadow is a ford");
        let climb =
            (core.ground_elevation_at(q, r) - core.ground_elevation_at(q - 1, r)).max(0) as u32;
        assert_eq!(
            core.walk_step_cost((q - 1, r), q, r),
            WALK_SHALLOW_COST + climb * WALK_CLIMB_COST,
            "the water part of route cost is the ford cost"
        );
        core.water.set(
            q,
            r,
            WaterDelta::new(i16::try_from(crate::scale::WADE_LIMIT_QUANTA).unwrap()),
        );
        assert!(
            !core.walkable_hex(q, r),
            "deep disturbed water stops the route"
        );

        // And the solver reads the bed the predicate does — the ground the player finished, not the
        // generated bed underneath it.
        core.water.set(q, r, WaterDelta::new(6));
        assert_eq!(core.water_depth_at(q, r), 6);
        assert_eq!(
            core.water_surface_at(q, r),
            core.ground_elevation_at(q, r) + 6,
            "water stands on the ground the player finished, not on the generated bed"
        );
        assert_eq!(
            WaterField::bed_quanta(&core, q, r),
            core.ground_elevation_at(q, r),
            "the solver reads the same bed the predicate does"
        );
        core.water.set(q, r, WaterDelta::new(0));

        // The opening shelf is deliberately dry, so the surveyed rings hold no deep water at all.
        // Walk chunks outward until the generator offers an inland one — the landing site is a
        // translation of an unbounded source, so "there is water somewhere out there" is a property
        // of the generator rather than of this seed's luck.
        let (q, r) = survey_out_to_deep_water(&mut core)
            .expect("the physical generator puts inland deep water within reach of the opening");
        assert!(core.terrain_blocks_movement(q, r));
        let depth = core.water_depth_at(q, r);
        core.water
            .set(q, r, WaterDelta::new(i16::try_from(-depth).unwrap()));
        assert_eq!(core.water_depth_at(q, r), 0);
        assert!(
            !core.terrain_blocks_movement(q, r),
            "a drained cell is ground, whatever the band draws"
        );
    }

    /// A hydrology solve may never insert a gameplay chunk — not while settling, not through a
    /// player's bounded flood or drain command, and not when a survey resumes a departure that was
    /// waiting at the old frontier. Surveying is the player's decision; water arriving somewhere is
    /// not a reason to make it for them.
    #[test]
    fn no_solve_may_survey_a_chunk() {
        let mut core = physical_core();
        let (q, r) = dry_cell(&core);
        core.water.set(q, r, WaterDelta::new(40));
        let surveyed = core.generated_chunks.clone();
        let report = core.settle_water(&[(q, r)]);
        assert!(report.cells > 0);
        assert_eq!(
            core.generated_chunks, surveyed,
            "a hydrology solve may never insert a gameplay chunk"
        );

        // The player's own commands are bounded, reach the same solve, and survey nothing either.
        let mut core = physical_core();
        let (q, r) = dry_cell(&core);
        let surveyed = core.generated_chunks.clone();
        assert!(core
            .edit_water(q, r, WaterAction::Flood, WATER_COMMAND_LIMIT_QUANTA + 1)
            .unwrap_err()
            .contains("1..="));
        let report = core.edit_water(q, r, WaterAction::Flood, 3).unwrap();
        assert!(report.cells > 0);
        assert_eq!(core.generated_chunks, surveyed);
        assert!(core.dirty.water);

        core.creative = true;
        (core.player.x, core.player.y) = axial_world(q, r);
        core.apply_commands(&format!(
            r#"[{{"type":"water_edit","q":{q},"r":{r},"action":"flood","quanta":1}}]"#
        ))
        .unwrap();
        assert!(
            core.events
                .iter()
                .any(|event| event.starts_with("Water settled over")),
            "the JSON command reaches the bounded native edit"
        );

        // And a departure left waiting past the frontier resumes when the player finally surveys
        // there, opening that one chunk and no other.
        let mut core = physical_core();
        let size = core.scenario.chunk_size;
        let chunk = (20, -11);
        let target = (chunk.0 * size, chunk.1 * size);
        assert!(!core.generated_chunks.contains(&chunk));
        core.water.set(target.0, target.1, WaterDelta::new(3));
        core.dirty.water = false;
        let before = core.generated_chunks.len();
        core.generate_chunk(chunk.0, chunk.1);
        assert!(
            core.dirty.water,
            "survey ran the waiting departure through the solve"
        );
        assert_eq!(
            core.generated_chunks.len(),
            before + 1,
            "the resumed solve did not survey past its new frontier"
        );
    }

    #[test]
    fn a_finite_pump_draw_moves_depth_and_a_river_draw_obeys_its_rate() {
        let mut core = physical_core();
        let (q, r) = dry_cell(&core);
        core.water.set(q, r, WaterDelta::new(1));
        let finite = WaterSourceSnapshot {
            q,
            r,
            available: 1,
            discharge: 0,
            rate: 1,
        };
        assert!(core.draw_pump_source(finite));
        assert_eq!(core.water_depth_at(q, r), 0, "the finite cell ran dry");

        let river = WaterSourceSnapshot {
            q,
            r,
            available: 4,
            discharge: 1,
            rate: 1,
        };
        assert!(core.draw_pump_source(river));
        assert!(
            !core.draw_pump_source(river),
            "one discharge class grants one withdrawal in the tick"
        );
        core.water_draws.clear();
        assert!(
            core.draw_pump_source(river),
            "the source replenishes next tick"
        );
    }

    /// Everything the save file has to say about disturbed water: a departure is a checksum input,
    /// a world back at equilibrium hashes as one that never left it, the cells round trip through
    /// the envelope, a version-38 world resumes on the checksum it was written with, and the
    /// storage guard refuses what it must while staying wider than any legal dam.
    #[test]
    fn a_departure_is_saved_checksummed_restored_and_guarded() {
        let mut core = physical_core();
        let baseline = core.checksum();
        let (q, r) = dry_cell(&core);
        core.water.set(q, r, WaterDelta::new(5));
        assert_ne!(
            core.checksum(),
            baseline,
            "disturbed water is a checksum input"
        );
        core.water.set(q, r, WaterDelta::new(0));
        assert_eq!(
            core.checksum(),
            baseline,
            "a world back at its equilibrium hashes as one that never left it"
        );

        core.water.set(q, r, WaterDelta::new(5));
        let saved = core.save_string().expect("the world saves");
        let restored: SaveEnvelope = serde_json::from_str(
            saved
                .strip_prefix(SAVE_PREFIX)
                .expect("the save carries its prefix"),
        )
        .expect("the save parses");
        assert_eq!(restored.state.water, vec![WaterCell { q, r, departure: 5 }]);
        assert_eq!(
            DisturbedWater::from_cells(&restored.state.water),
            core.water
        );

        // The version-39 rung is a stamp and nothing else. A version-38 world could not make a
        // departure, and this version computes the same equilibrium from the same seed, so the file
        // resumes on the checksum it was written with rather than on a recomputed one.
        let core = physical_core();
        let saved = core.save_string().expect("the world saves");
        let old = saved.replace(
            &format!("\"save_version\":{SAVE_VERSION}"),
            "\"save_version\":38",
        );
        assert_ne!(old, saved, "the stamp was found and rewritten");

        let (definitions, technologies, scenarios) = catalogues();
        let restored = Core::from_save(&definitions, &technologies, &scenarios, &old)
            .expect("a version-38 world resumes");
        assert!(restored.water.is_empty(), "it had no departure to carry");
        assert_eq!(
            restored.checksum(),
            core.checksum(),
            "and it hashes exactly what it hashed before hydrology existed"
        );

        // The guard refuses a departure past its limit and a cell named twice, and the limit itself
        // is wider than anything the generator makes but narrower than the integer it guards — so a
        // legal dam can never reach it and the guard can never overflow.
        let past = [WaterCell {
            q: 0,
            r: 0,
            departure: i16::try_from(DEPARTURE_LIMIT_QUANTA + 1).unwrap(),
        }];
        assert!(validate_saved_water(&past).is_err());
        let twice = [
            WaterCell {
                q: 2,
                r: -1,
                departure: 3,
            },
            WaterCell {
                q: 2,
                r: -1,
                departure: -3,
            },
        ];
        assert!(validate_saved_water(&twice).is_err());
        assert!(validate_saved_water(&twice[..1]).is_ok());

        let relief = crate::scale::BED_MAX_QUANTA - crate::scale::BED_MIN_QUANTA;
        assert!(
            DEPARTURE_LIMIT_QUANTA > relief,
            "a legal dam must not be able to reach the storage guard"
        );
        assert!(
            i32::from(i16::MAX) > DEPARTURE_LIMIT_QUANTA,
            "the guard must fit the integer it guards"
        );
    }

    #[test]
    fn a_reversal_puts_back_what_a_solve_forgot_and_drops_what_it_invented() {
        let before = DisturbedWater::from_cells(&[
            WaterCell {
                q: 0,
                r: 0,
                departure: 3,
            },
            WaterCell {
                q: 1,
                r: 0,
                departure: -2,
            },
        ]);
        let after = DisturbedWater::from_cells(&[
            WaterCell {
                q: 1,
                r: 0,
                departure: -2,
            },
            WaterCell {
                q: 2,
                r: 0,
                departure: 5,
            },
        ]);
        assert_eq!(
            before.reversal_of(&after),
            vec![
                WaterCell {
                    q: 0,
                    r: 0,
                    departure: 3,
                },
                WaterCell {
                    q: 2,
                    r: 0,
                    departure: 0,
                },
            ],
            "a cell the solve left alone is not in the record"
        );
        let mut restored = after.clone();
        restored.apply(&before.reversal_of(&after));
        assert_eq!(restored, before, "and applying it is a true inverse");
    }

    /// One hex, lowered by `steps` grade steps, priced and committed the way a player's drag is.
    fn lower(q: i32, r: i32, steps: u8) -> GroundEdit {
        GroundEdit {
            q,
            r,
            to_q: q,
            to_r: r,
            corner: 0,
            to_corner: 0,
            shape: GroundShape::Cell,
            definition_id: 2,
            action: GroundAction::Lower,
            steps,
            reference: GroundReference::default(),
            cover: false,
        }
    }

    /// Whether this hex would take that cut whole — no obstacle, no deposit, no refusal.
    fn diggable(core: &Core, (q, r): (i32, i32), steps: u8) -> bool {
        let preview = core.ground_preview(&lower(q, r, steps));
        preview.error.is_none() && preview.blocked == 0 && preview.changes > 0
    }

    /// A surveyed, dry, diggable hex whose six neighbours are all surveyed, dry and diggable too, so
    /// a pond dug into one of them has a bank the test can compute rather than guess.
    fn pit_and_bank(core: &Core) -> ((i32, i32), (i32, i32)) {
        let size = core.scenario.chunk_size;
        let dry = |core: &Core, (q, r): (i32, i32)| {
            core.surveyed(q, r)
                && core.water_depth_at(q, r) == 0
                && !core.generated_ground_at(q, r).presentation.is_water()
        };
        core.generated_chunks
            .iter()
            .flat_map(|&(cq, cr)| hexes_in_chunk(cq, cr, size))
            .filter(|&c| dry(core, c) && diggable(core, c, 4))
            .find_map(|(q, r)| {
                let ring: Vec<(i32, i32)> = DIRECTIONS
                    .iter()
                    .map(|&(dq, dr)| (q + dq, r + dr))
                    .collect();
                // The bank has to be the rim itself, not merely a neighbour. The pond stands one
                // quantum under the lowest bed around it, so a cut into any higher neighbour can
                // leave that neighbour still above the water and the test would be asserting the
                // model failed to move water uphill. Picking the lowest neighbour makes "the
                // pond's surface is suddenly above it" true by construction, whatever relief the
                // generator lays down here.
                let rim = ring
                    .iter()
                    .map(|&(cq, cr)| core.ground_elevation_at(cq, cr))
                    .min()
                    .expect("a hex has neighbours");
                ring.iter()
                    .all(|&cell| dry(core, cell))
                    .then(|| {
                        ring.iter()
                            .copied()
                            .find(|&cell| {
                                core.ground_elevation_at(cell.0, cell.1) == rim
                                    && diggable(core, cell, 2)
                            })
                            .map(|bank| ((q, r), bank))
                    })
                    .flatten()
            })
            .expect("the opening shelf has an open pair of dry hexes")
    }

    /// Digging beside standing water floods the cut, and nothing had to ask it to. The earthwork
    /// moved the bed, and the bed is what the water stands on.
    #[test]
    fn a_cut_beside_a_pond_floods_and_the_undo_puts_the_water_back() {
        let mut core = physical_core();
        core.set_creative(true);
        let ((pit_q, pit_r), (bank_q, bank_r)) = pit_and_bank(&core);

        // A pit next door, and a pond in it standing exactly one quantum under the lowest bed around
        // it. One quantum is head the model will not move on, so this is a world already at rest.
        core.edit_ground(&lower(pit_q, pit_r, 4)).unwrap();
        let floor = core.ground_elevation_at(pit_q, pit_r);
        let rim = DIRECTIONS
            .iter()
            .map(|&(dq, dr)| core.ground_elevation_at(pit_q + dq, pit_r + dr))
            .min()
            .expect("a hex has neighbours");
        let depth = rim + 1 - floor;
        assert!(depth > 0, "the cut put the floor below its own rim");
        core.water
            .set(pit_q, pit_r, WaterDelta::new(i16::try_from(depth).unwrap()));
        core.settle_water(&[(pit_q, pit_r)]);
        assert_eq!(
            core.water_depth_at(pit_q, pit_r),
            depth,
            "a pond under its rim has nowhere to go"
        );
        let held: i32 = core.water.iter().map(|(_, d)| i32::from(d.get())).sum();
        assert_eq!(held, depth, "and the shelf around it is dry");
        assert_eq!(
            core.snapshot().water,
            core.water.cells(),
            "the snapshot is the departure set, not a second picture of it"
        );
        let checksum = core.checksum();

        // Now cut the bank. The pond's surface is suddenly above it, and the water finds the cut.
        core.events.clear();
        core.edit_ground(&lower(bank_q, bank_r, 2)).unwrap();
        assert!(
            core.water_depth_at(bank_q, bank_r) > 0,
            "the cut took water nobody handed it"
        );
        assert!(
            core.water_depth_at(pit_q, pit_r) < depth,
            "and the pond is what gave it up"
        );
        assert_eq!(
            core.water
                .iter()
                .map(|(_, d)| i32::from(d.get()))
                .sum::<i32>(),
            held,
            "the water was moved, not made"
        );
        assert_eq!(
            core.snapshot().water,
            core.water.cells(),
            "the flood the solve left is the flood the host is told about"
        );
        assert!(
            core.events
                .iter()
                .any(|event| event.contains("Water found the new grade")),
            "{:?}",
            core.events
        );

        // Undo restores the ground and the water that was standing on it, exactly. The water is put
        // back from the record rather than solved for again, so this is an identity and not a second
        // opinion that happens to agree.
        core.undo_ground().unwrap();
        assert_eq!(core.water_depth_at(bank_q, bank_r), 0);
        assert_eq!(core.water_depth_at(pit_q, pit_r), depth);
        assert_eq!(core.checksum(), checksum, "the world came back exactly");

        // The common case is the other half of the same rule, and it must stay free: a grade with no
        // water anywhere near it leaves no departure, says nothing about water, does not open the
        // world to look, and hashes as if hydrology were not here.
        let mut core = physical_core();
        core.set_creative(true);
        let ((q, r), _) = pit_and_bank(&core);
        let checksum = core.checksum();
        let chunks = core.generated_chunks.len();

        core.events.clear();
        core.edit_ground(&lower(q, r, 1)).unwrap();
        assert!(core.water.is_empty(), "dry ground disturbs no water");
        assert!(
            !core
                .events
                .iter()
                .any(|event| event.contains("Water found the new grade")),
            "{:?}",
            core.events
        );
        assert_eq!(
            core.generated_chunks.len(),
            chunks,
            "and the settle did not open the world to look"
        );

        core.undo_ground().unwrap();
        assert_eq!(core.checksum(), checksum);
    }
}
