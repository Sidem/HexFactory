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

    /// Whether the cell is a live channel: a generated river reach still standing on its generated
    /// bed. Its water arrives from upstream rather than from the region, so what a cut takes out of
    /// it comes back.
    fn channel(&self, q: i32, r: i32) -> bool;
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
    /// Quanta that entered the world from a live channel: what it put back into itself, plus what
    /// it supplied to lower ground beside it.
    pub(super) inflow_quanta: i64,
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
    /// Exact bounded region whose resulting depth may differ from its previous value.
    pub(super) touched: Vec<(i32, i32)>,
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

/// Whether a cell is a live head: a generated reach still on its generated bed, holding no more than
/// the depth the generator gave it.
///
/// This is the model's one asymmetry and the whole reason a canal works. A head is not a body of
/// water for the region to share out — it is the downstream end of everything upstream — so it fills
/// lower ground beside it without being drawn down, and it goes back to its generated depth whatever
/// the last sweep took off it. A reach somebody dammed or dug is no longer on that bed and stops
/// being one. A reach carrying a flood is above its depth, and water above a river's depth is
/// ordinary water that runs downhill like any other.
fn live_head<F: WaterField>(
    field: &F,
    depth: &BTreeMap<(i32, i32), i32>,
    (q, r): (i32, i32),
) -> bool {
    field.channel(q, r) && depth[&(q, r)] <= field.equilibrium_depth(q, r)
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
        // A head the solve has already started losing water through is switched off outright: it
        // neither refills nor supplies. A region that is losing water is not a basin being filled,
        // it is a channel running somewhere else, and funding one is a pump with the world on the
        // far end.
        let source = report.outflow_quanta == 0 && live_head(field, depth, (q, r));
        if source {
            let restored = field.equilibrium_depth(q, r) - depth[&(q, r)];
            if restored > 0 {
                report.inflow_quanta += i64::from(restored);
                *depth.get_mut(&(q, r)).expect("the cell is in the region") += restored;
            }
        }
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
            // A head fills ground and does nothing else. Letting one pour into the sea, over the
            // frontier or into the next reach down would close the loop it is refilled by, and a
            // loop with an unbounded supply on one end runs until the sweep budget stops it: a
            // six-quanta trench once left a quarter of a kilometre of river standing past the
            // frontier that way, manufactured a quantum at a time by a river conveying itself.
            if source && (!region.contains(to.0, to.1) || field.channel(to.0, to.1)) {
                continue;
            }
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
        // Finite water needs a two-quantum difference: moving one quantum across a one-quantum
        // difference would merely invert it and oscillate forever. A live head is different. It is
        // the fixed upstream water level, so it can fill a canal exactly to that level without
        // lowering itself or creating an inverse gradient.
        if head < if source { 1 } else { 2 } {
            continue;
        }
        // What a source gives is not limited by what it holds; what anything else gives is.
        let quanta = if source { head } else { held.min(head / 2) };
        if source {
            report.inflow_quanta += i64::from(quanta);
        } else {
            *depth.get_mut(&(q, r)).expect("the cell is in the region") -= quanta;
        }
        if boundary {
            report.outflow_quanta += i64::from(quanta);
            if !field.surveyed(to.0, to.1) {
                *report.frontier.entry(to).or_default() += quanta;
            }
        } else if field.channel(to.0, to.1) {
            // Water poured into a reach is carried off rather than stored: by the next tick that
            // column is somewhere downstream. It leaves the world here, which is also the thing
            // that switches off every source in the region — draining into a river is exactly the
            // far end that must not be allowed to feed back into a supply.
            report.outflow_quanta += i64::from(quanta);
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
            // A reach at the region's edge is a head, not ground to spread onto, so it is worth
            // claiming only when it pours *in* — which is a canal tapping it. Claiming every reach
            // a region merely drains towards is how one trench used to end up owning a whole river
            // network, and every cell of it came back dirty.
            let pours_out =
                depth[&(q, r)] > 0 && surface - bed >= 2 && !field.channel(nq, nr);
            // A live head may fill ground even when its surface is only one quantum higher. That
            // last quarter metre is the difference between a visible canal and a dry trench whose
            // floor is physically below the river.
            let pours_in = held > 0
                && (bed + held) - surface >= if field.channel(nq, nr) { 1 } else { 2 };
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
    report.touched = region.iter().collect();
    report.truncated = region.truncated();
    for (&(q, r), &held) in &depth {
        let departure = held - field.equilibrium_depth(q, r);
        // A live reach is never stored below its generated depth, whatever the solve had to do to
        // reach a fixed point. This is the same statement [`live_head`] makes, made once more where
        // it is durable: a river the player has not dug is a head, and a head does not remember
        // being tapped. Above its depth it remembers everything — that is a flood, and a flood is
        // theirs.
        let departure = if field.channel(q, r) {
            departure.max(0)
        } else {
            departure
        };
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
