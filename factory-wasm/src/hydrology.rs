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
//!
//!    A live head (see [`settle::live_head`]) is the one term that adds water, and it breaks that
//!    potential rather than decreasing it, so termination is argued in two levels. A head supplies
//!    only *inside* the region, only to ground that is not itself a reach, and only while it stands
//!    at least two quanta above it — so every cell a head can fill is capped at that head's own
//!    surface, and the number of quanta a region can absorb from its heads is finite. The moment a
//!    single quantum leaves the world — at the ocean, over the frontier, or into a reach that
//!    carries it away — every head in the region is switched off for the rest of the solve. So the
//!    solve is a finite number of supply transfers separating phases in which nothing is supplied,
//!    and inside a phase the lexicographic pair above strictly decreases per transfer. Both levels
//!    are finite, so the solve still terminates without appeal to the sweep budget — including the
//!    canal cut from a river to the sea, which used to be the stated exception: it now drains what
//!    the region holds, unfunded, and stops.
//! 2. **The solve cannot generate world.** [`WaterField::surveyed`] is the only question asked about
//!    a cell beyond the frontier. An unsurveyed neighbour is never read for a bed, a depth or a
//!    band: it drains the source cell's own *equilibrium* surface, so the boundary flux is computed
//!    entirely from cells the player has already surveyed. A field that panics on an unsurveyed read
//!    is what the test suite hands the solver.
//! 3. **The region cannot be unbounded.** It starts as the disturbed cells and their rings, and
//!    then grows only where the settling water actually asks for more ground — a wall neighbour a
//!    region cell could pour onto, or a wet one that could pour in. A live reach is claimed only on
//!    the second of those, because a region that merely drains towards a river has no use for it and
//!    following one would hand a single trench the whole connected network. It stops at
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

// The relaxation solver: what the world must answer, the region one disturbance puts in flight,
// and the sweep that returns it to a fixed point. It reads the four `WaterField` questions and
// nothing else, which is why it lives apart from the store above and the Core below.
include!("hydrology/settle.rs");

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

    /// A rated channel still standing on the bed the generator gave it.
    ///
    /// The bed clause is what keeps damming a river a real thing to do. A reach is fed from upstream
    /// only while it is the reach the generator drew; the moment somebody cuts or fills it, it is a
    /// pond of theirs and holds whatever they left in it.
    fn channel(&self, q: i32, r: i32) -> bool {
        let generated = self.generated_ground_at(q, r);
        generated.hydrology.discharge_class > 0
            && generated.hydrology.depth_quanta > 0
            && self.ground_elevation_at(q, r) == generated.bed.get()
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
        // Fertility is a ring question — ground is watered by the water standing beside it — so a
        // cell whose depth moved makes its neighbours' habitat answer stale as well as its own.
        self.dirty
            .habitats
            .extend(report.touched.iter().flat_map(|&(q, r)| {
                std::iter::once((q, r))
                    .chain(DIRECTIONS.iter().map(move |&(dq, dr)| (q + dq, r + dr)))
            }));
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

    // The solver's own tests, on hand-built fields that panic on an unsurveyed read.
    include!("hydrology/settle_tests.rs");

    // The same water seen through a running Core: commands, saves, checksums and undo.
    include!("hydrology/core_tests.rs");
}
