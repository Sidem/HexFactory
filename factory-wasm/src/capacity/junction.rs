//! The dense-junction E0 workload: a blueprint whose splitters, mergers and underpasses carry all
//! of its throughput, rather than a straight chain with junction icons dropped onto it.
//!
//! One *unit* is four production lanes merging into a single trunk through three chained mergers,
//! an underpass pair carrying that trunk *under* an independent fifth lane running across it, and
//! a splitter fanning the crossed trunk into three branches. Twenty-four entities, six of which
//! are junction primitives, and every item delivered has passed through at least one merger, one
//! underpass pair and one splitter — so a change that broke any of the three would show up as lost
//! throughput rather than as an unchanged number.
//!
//! Every lane carries its own material, which is the whole reason the assertions below can be
//! exact. `delivered_by_item` is incremented only by a consumer taking delivery, so one material
//! per lane turns the core's own counter into a per-lane throughput meter: `cargo_crosses_every_
//! junction_and_no_lane_starves` reads four merged lanes and one crossing lane off it separately,
//! and `the_underpass_carries_the_trunk_over_an_independent_lane` can state that the hex the trunk
//! passes over never holds a trunk material at all.
use super::*;

/// Entities in one unit. Fixed, and it divides the recorded steady ladder: 768, 3072, 6144 and
/// 24576 entities are 32, 128, 256 and 1024 units.
pub const ENTITIES_PER_UNIT: u32 = 24;

/// Rows one unit occupies, including the empty row that keeps the southernmost extractor's service
/// envelope clear of the next unit. Units share no hex and compile no edge between them, so a tier
/// is one unit stamped `units` times and its cost is that of one unit multiplied.
///
/// Two rows per merged lane, and that spacing is forced rather than chosen. An extractor reserves
/// the hex directly south of its anchor as its service envelope, and a transport ray binds to the
/// first hex it steps onto — a gap ends it — so a lane is a contiguous run west of its merger.
/// Two lanes one row apart would therefore have to put one's belt run through the other's
/// envelope. `units_repeat_without_sharing_a_hex_or_an_edge` holds the blueprint to both rules.
const UNIT_ROWS: i32 = 10;

/// Ticks a cold unit needs before it is running at the rate it will keep.
///
/// Longer than the pipeline is deep, which is the easy half: the trunk's furthest lane is eight
/// hand-offs from a consumer at [`BELT_TRANSIT_TICKS`] each, on top of the twenty ticks its
/// extractor spends on the first item. The other half is arbitration — three chained mergers and a
/// splitter each carry a cursor, and the phase those cursors settle into is a property of the
/// blueprint rather than of any one of them. Measured, not estimated:
/// `cargo_crosses_every_junction_and_no_lane_starves` requires that every lane is already at its
/// full extraction rate from the first tick after this.
pub(crate) const SETTLE_TICKS: u32 = 1024;

/// Technologies the junction blueprint's buildings are unlocked by: belt, extractor, and the
/// shipped `unlock_technology_id` of the splitter and merger (16) and of the underpass (17).
///
/// Scenario-placed buildings are not put through the player's placement rules, so this is
/// documentation the scenario carries rather than a gate it passes — but a workload that claims to
/// measure the shipped junctions should name the research that unlocks them.
pub(crate) const RESEARCHED: [TechnologyId; 6] = [1, 2, 3, 4, 16, 17];

const SPLITTER: DefinitionId = 24;
const MERGER: DefinitionId = 25;
const UNDERPASS: DefinitionId = 26;

/// Edge headings. Direction 0 is east and the table turns clockwise, so 1 is south-east and 4 is
/// north-west. The blueprint uses no corner heading: those are a separate heading family with
/// their own price and their own technology, and a workload is not the place to spend either.
const EAST: u8 = 0;
const SOUTH_EAST: u8 = 1;
const NORTH_WEST: u8 = 4;

/// The four lanes that merge into the trunk, and the crossing lane, each with the shipped
/// `extract_steps` of its material — which at the base extractor's speed of 100 is exactly the
/// ticks that extractor spends on one item.
///
/// Distinct materials, so the core's own `delivered_by_item` says which lane a delivery came from.
///
/// The four merged rates deliberately add up to more than a belt carries. Together they offer
/// `0.254` items a tick into a trunk that moves `0.2`, so the mergers — not the extractors — are
/// what sets this factory's throughput, every merger has both feeders holding cargo on every tick,
/// and the rotation is arbitrating a real contest rather than waving items through one at a time.
/// A trunk under capacity settles instead into a conflict-free schedule where the two feeders
/// never collide, which measures a chain of belts with merger icons on it.
///
/// It is still a live steady state and not a jam: the trunk delivers at a belt's full rate for as
/// long as it runs, and the backlog behind each merger is bounded by the belts' six lane slots and
/// the extractor's twelve-item output. A factory that has stopped is what `Workload::Blocked`
/// already measures.
///
/// The crossing lane is deliberately the slowest and is the one lane not merging into anything, so
/// it stays clear of its own belt's capacity and keeps flowing throughout.
const LANES: [Lane; 5] = [
    Lane {
        item_id: 7, // sand
        extract_ticks: 13,
    },
    Lane {
        item_id: 8, // clay
        extract_ticks: 13,
    },
    Lane {
        item_id: 6, // stone
        extract_ticks: 20,
    },
    Lane {
        item_id: 26, // limestone
        extract_ticks: 20,
    },
    Lane {
        item_id: 1, // ore
        extract_ticks: 30,
    },
];

/// What one belt carries, from the shipped physical scale rather than a number written here: the
/// trunk's ceiling, and the figure the four merged lanes are chosen to exceed. The blueprint needs
/// no arithmetic to lay a trunk out, so this exists for the assertions that hold the trunk to it.
#[cfg(test)]
fn trunk_items_per_tick() -> f64 {
    1.0 / scale::belt_slot_ticks() as f64
}

#[derive(Clone, Copy)]
struct Lane {
    item_id: ItemId,
    /// Read by the workload's own throughput assertions. The blueprint needs only the material:
    /// the cadence is what the shipped catalogue already says, recorded here so a test can hold
    /// the workload to it rather than to a number it derived from the same source.
    #[cfg_attr(not(test), allow(dead_code))]
    extract_ticks: u32,
}

/// Index into [`LANES`] of the lane that crosses the trunk rather than merging into it.
const CROSSING: usize = 4;

/// One unit, stamped `units` times down the q axis. See [`LANES`] for why the four merged lanes
/// offer the trunk more than it can carry, and why that is a live steady state rather than a jam.
///
/// The trunk runs north up column 3 through three chained mergers, turns east at the last of them,
/// dives under the crossing lane in column 5, and fans out at the splitter in column 7.
pub(crate) fn blueprint(units: u32) -> (Vec<ScenarioResource>, Vec<PlacedBuilding>) {
    let mut resources = Vec::new();
    let mut buildings = Vec::new();
    for unit in 0..units {
        let base = unit as i32 * UNIT_ROWS;
        // The crossing lane: its own material, its own extractor and its own sink on the far side
        // of the trunk, so what it carries and what the trunk carries can never be confused.
        extractor(&mut resources, &mut buildings, 3, base, LANES[CROSSING]);
        buildings.push(at(5, base, BELT, SOUTH_EAST));

        buildings.push(at(5, base + 1, BELT, SOUTH_EAST));
        buildings.push(at(8, base + 1, CONSUMER, EAST));

        // The head of the trunk turns east here, and everything downstream of it has crossed all
        // three mergers.
        extractor(&mut resources, &mut buildings, 1, base + 2, LANES[0]);
        buildings.push(at(3, base + 2, MERGER, EAST));
        buildings.push(at(4, base + 2, UNDERPASS, EAST));
        // The hex the trunk passes over. It belongs to the crossing lane: fed from the north,
        // delivering to the crossing lane's own sink, and bound to the trunk in neither direction.
        buildings.push(at(5, base + 2, BELT, SOUTH_EAST));
        buildings.push(at(6, base + 2, UNDERPASS, EAST));
        buildings.push(at(7, base + 2, SPLITTER, EAST));
        buildings.push(at(8, base + 2, CONSUMER, EAST));

        buildings.push(at(3, base + 3, BELT, NORTH_WEST));
        buildings.push(at(5, base + 3, CONSUMER, EAST));
        buildings.push(at(7, base + 3, CONSUMER, EAST));

        extractor(&mut resources, &mut buildings, 1, base + 4, LANES[1]);
        buildings.push(at(3, base + 4, MERGER, NORTH_WEST));

        buildings.push(at(3, base + 5, BELT, NORTH_WEST));

        extractor(&mut resources, &mut buildings, 0, base + 6, LANES[2]);
        buildings.push(at(2, base + 6, BELT, EAST));
        buildings.push(at(3, base + 6, MERGER, NORTH_WEST));

        buildings.push(at(3, base + 7, BELT, NORTH_WEST));

        extractor(&mut resources, &mut buildings, 0, base + 8, LANES[3]);
        buildings.push(at(2, base + 8, BELT, EAST));
        // The tail of the trunk: the only belt in the chain of three mergers that is not one.
        buildings.push(at(3, base + 8, BELT, NORTH_WEST));
    }
    (resources, buildings)
}

/// One lane's source: a deposit under the extractor's anchor, and the extractor on it.
fn extractor(
    resources: &mut Vec<ScenarioResource>,
    buildings: &mut Vec<PlacedBuilding>,
    q: i32,
    r: i32,
    lane: Lane,
) {
    resources.push(ScenarioResource {
        q,
        r,
        item_id: lane.item_id,
        quantity: DEPOSIT_QUANTITY,
    });
    buildings.push(at(q, r, EXTRACTOR, EAST));
}

fn at(q: i32, r: i32, definition_id: DefinitionId, orientation: u8) -> PlacedBuilding {
    PlacedBuilding {
        q,
        r,
        definition_id,
        orientation,
        recipe_id: None,
        // Left unowned, exactly as the line blueprint leaves it, so nothing here is a special case
        // the ordinary edit paths cannot touch.
        scenario_owned: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cold junction tier: the blueprint, validated and compiled, advanced by `ticks`.
    fn core(units: u32, ticks: u32) -> Core {
        let mut spec = tier_on(Layout::Junction, "junction", units, 120, 1, 1, 1);
        spec.warmup_ticks = ticks;
        warm_core(&spec)
    }

    fn entity(core: &Core, q: i32, r: i32) -> usize {
        core.entity_at(q, r)
            .unwrap_or_else(|| panic!("the junction blueprint occupies ({q}, {r})"))
    }

    /// Every entity that compiled an outgoing edge into `target`, in the ascending entity id that
    /// arbitration used before mergers existed. Derived from the compiled graph rather than read
    /// out of the tick's own scratch index, so it is an independent statement about the shape.
    fn feeders(core: &Core, target: usize) -> Vec<usize> {
        let mut sources: Vec<usize> = (0..core.entities.len())
            .filter(|&index| core.graph[index].iter().any(|edge| edge == target))
            .collect();
        sources.sort_by_key(|&index| core.entities[index].id);
        sources
    }

    /// The same thing as hexes, in a fixed order: a statement about the shape rather than about
    /// the order arbitration happens to walk it in.
    fn cell_feeders(core: &Core, q: i32, r: i32) -> Vec<(i32, i32)> {
        let mut cells: Vec<(i32, i32)> = feeders(core, entity(core, q, r))
            .into_iter()
            .map(|source| {
                let placed = core.entities[source].placed;
                (placed.q, placed.r)
            })
            .collect();
        cells.sort_unstable();
        cells
    }

    fn primary(core: &Core, q: i32, r: i32) -> Option<(i32, i32)> {
        core.graph[entity(core, q, r)].primary().map(|target| {
            let placed = core.entities[target].placed;
            (placed.q, placed.r)
        })
    }

    /// Everything one entity is holding, exit slot and lane together.
    fn held(core: &Core, q: i32, r: i32) -> Vec<ItemId> {
        Core::belt_contents(&core.entities[entity(core, q, r)])
            .map(|cargo| cargo.item_id)
            .collect()
    }

    fn delivered(core: &Core, item_id: ItemId) -> u64 {
        core.delivered_by_item.get(&item_id).copied().unwrap_or(0)
    }

    /// The unit is the graph it claims to be: three chained mergers with two feeders each, an
    /// underpass pair bound to each other over a hex that feeds neither, and a splitter with three
    /// branches. Asserted on the compiled graph, so a blueprint edit that silently rerouted the
    /// trunk down an ordinary belt fails here rather than quietly measuring a straight chain.
    #[test]
    fn the_unit_compiles_three_mergers_an_underpass_pair_and_a_three_way_splitter() {
        let core = core(1, 0);
        assert_eq!(core.entities.len(), ENTITIES_PER_UNIT as usize);

        // The trunk, from its tail to the last merger. Each merger takes its own lane and the
        // merger behind it: two feeders each, which is the arbitration this workload exists to
        // exercise.
        assert_eq!(primary(&core, 3, 8), Some((3, 7)));
        assert_eq!(cell_feeders(&core, 3, 6), vec![(2, 6), (3, 7)]);
        assert_eq!(cell_feeders(&core, 3, 4), vec![(1, 4), (3, 5)]);
        assert_eq!(cell_feeders(&core, 3, 2), vec![(1, 2), (3, 3)]);

        // The crossing. The entrance binds to its partner two hexes ahead, and the hex between
        // them belongs entirely to the crossing lane: fed only from the north, delivering only to
        // its own sink, with no edge to or from the trunk in either direction.
        assert_eq!(primary(&core, 4, 2), Some((6, 2)));
        assert_eq!(cell_feeders(&core, 5, 2), vec![(5, 1)]);
        assert_eq!(primary(&core, 5, 2), Some((5, 3)));
        assert_eq!(cell_feeders(&core, 6, 2), vec![(4, 2)]);
        // The exit found no partner of its own and so behaves as an ordinary belt.
        assert_eq!(primary(&core, 6, 2), Some((7, 2)));

        // Three branches, all of them consumers, and nothing else compiles more than one edge.
        let splitter = entity(&core, 7, 2);
        let branches: Vec<(i32, i32)> = core.graph[splitter]
            .iter()
            .map(|target| {
                let placed = core.entities[target].placed;
                (placed.q, placed.r)
            })
            .collect();
        assert_eq!(branches, vec![(8, 2), (7, 3), (8, 1)]);
        assert_eq!(
            core.entities
                .iter()
                .enumerate()
                .filter(|(index, _)| core.graph[*index].iter().count() > 1)
                .count(),
            1
        );
        // Every entity but the four consumers delivers somewhere: no lane ends in the dirt.
        assert_eq!(
            (0..core.entities.len())
                .filter(|&index| core.graph[index].is_empty())
                .count(),
            4
        );
    }

    /// Units repeat without touching: no shared hex, no reserved service envelope under another
    /// unit's building, and no compiled edge between one unit and the next. A tier's cost is one
    /// unit's multiplied only if that is true.
    #[test]
    fn units_repeat_without_sharing_a_hex_or_an_edge() {
        let (resources, buildings) = blueprint(3);
        assert_eq!(buildings.len(), 3 * ENTITIES_PER_UNIT as usize);
        assert_eq!(resources.len(), 15);

        let core = core(3, 0);
        for index in 0..core.entities.len() {
            let unit = core.entities[index].placed.r.div_euclid(UNIT_ROWS);
            for target in core.graph[index].iter() {
                assert_eq!(
                    core.entities[target].placed.r.div_euclid(UNIT_ROWS),
                    unit,
                    "an edge left its unit"
                );
            }
        }

        // Extraction stays inside its own lane. An extractor draws from every deposit within one
        // hex of any cell it stands on, so two lanes placed a row apart would quietly feed each
        // other their materials and the per-lane counter would stop meaning anything.
        let distance = |(aq, ar): (i32, i32), (bq, br): (i32, i32)| {
            let (dq, dr) = (aq - bq, ar - br);
            (dq.abs() + dr.abs() + (dq + dr).abs()) / 2
        };
        for resource in &resources {
            let reaching = buildings
                .iter()
                .filter(|building| building.definition_id == EXTRACTOR)
                .filter(|building| {
                    [(0, 0), (1, 0)].into_iter().any(|(dq, dr)| {
                        distance((building.q + dq, building.r + dr), (resource.q, resource.r)) <= 1
                    })
                })
                .count();
            assert_eq!(
                reaching, 1,
                "deposit at ({}, {}) is in reach of {reaching} extractors",
                resource.q, resource.r
            );
        }
    }

    /// Delivered items per lane over `window` ticks, counted at the consumers, from a unit warmed
    /// for `warmup`.
    fn lane_throughput(warmup: u32, window: u32) -> [u64; 5] {
        let mut core = core(1, warmup);
        let before: Vec<u64> = LANES
            .iter()
            .map(|lane| delivered(&core, lane.item_id))
            .collect();
        core.advance_ticks(window);
        let mut moved = [0; 5];
        for (index, lane) in LANES.iter().enumerate() {
            moved[index] = delivered(&core, lane.item_id) - before[index];
        }
        moved
    }

    /// Cargo actually crosses the junctions, and the trunk runs at a belt's full rate: every lane's
    /// material reaches a consumer past a merger, an underpass pair and a splitter, and the four
    /// merged lanes together saturate the trunk.
    ///
    /// The crossing lane is the one competing for nothing, so it is held to the stricter claim —
    /// everything its extractor produced in the window arrived.
    ///
    /// The same window measured after twice the warmup gives the same answer, which is what makes
    /// [`SETTLE_TICKS`] a measured figure rather than a hopeful one.
    #[test]
    fn cargo_crosses_every_junction_and_the_trunk_runs_full() {
        let window = 3000;
        let moved = lane_throughput(SETTLE_TICKS, window);
        for (lane, moved) in LANES.iter().zip(moved) {
            assert!(moved > 0, "item {} never arrived", lane.item_id);
        }
        assert_eq!(moved, lane_throughput(SETTLE_TICKS * 2, window));

        let crossing = u64::from(window / LANES[CROSSING].extract_ticks);
        // One item of slack: a window boundary can fall between an extraction and the delivery it
        // becomes, and what is in flight is otherwise constant.
        assert!(
            moved[CROSSING] + 1 >= crossing && moved[CROSSING] <= crossing + 1,
            "the crossing lane delivered {} of {crossing}",
            moved[CROSSING]
        );

        let trunk: u64 = moved[..CROSSING].iter().sum();
        assert_eq!(trunk as f64, f64::from(window) * trunk_items_per_tick());
        // Pinned, so a change to any junction's physics has to be recorded here rather than
        // absorbed by the tolerances below.
        assert_eq!(moved, [231, 185, 92, 92, 100]);
    }

    /// The trunk passes *over* the crossing lane rather than through it. Nothing in the crossed hex
    /// is ever a trunk material and nothing past the underpass exit is ever the crossing one, for
    /// every tick of a full window — which no ordinary belt in that hex could manage.
    #[test]
    fn the_underpass_carries_the_trunk_over_an_independent_lane() {
        let mut core = core(1, SETTLE_TICKS);
        let crossing = LANES[CROSSING].item_id;
        let mut seen_crossing = 0;
        let mut seen_trunk = 0;
        for _ in 0..1200 {
            core.advance_ticks(1);
            for item in held(&core, 5, 2) {
                assert_eq!(item, crossing, "a trunk material entered the crossed hex");
                seen_crossing += 1;
            }
            for item in held(&core, 6, 2).into_iter().chain(held(&core, 7, 2)) {
                assert_ne!(
                    item, crossing,
                    "the crossing lane leaked past the underpass"
                );
                seen_trunk += 1;
            }
        }
        // Both statements are about a hex that was actually carrying something.
        assert!(seen_crossing > 0 && seen_trunk > 0);
    }

    /// Round-robin arbitration is what decides this factory's output, and the workload would read
    /// completely differently without it.
    ///
    /// The trunk is oversubscribed, so the two deeper mergers have both feeders holding cargo on
    /// every tick: each is a standing contest rather than an occasional one. Under the entity-id
    /// order every other junction in the game arbitrates by, whichever feeder happened to be built
    /// first would win that contest every single time and the other would deliver *nothing at
    /// all*. The shape of the outcome is therefore the proof, and no tolerance can be widened to
    /// cover the difference between a half and a zero.
    ///
    /// Each merger passes what it can take evenly between its two feeders, so the last one splits
    /// the two deepest lanes equally and the one above it gives the third lane as much as those
    /// two together. The head of the trunk is the exception, and a second thing worth pinning: its
    /// lane cannot fill half a belt on its own, so it delivers everything it extracts and the
    /// rotation hands the slack to the lanes below rather than idling on a feeder with nothing.
    ///
    /// Read off the consumers, past the underpass and the splitter, from the core's own per-item
    /// counter — not from any merger's cursor, so a cursor that advanced without changing who was
    /// served would still fail here.
    #[test]
    fn round_robin_halves_the_trunk_at_each_merger_instead_of_starving_a_lane() {
        let window = 3000;
        let moved = lane_throughput(SETTLE_TICKS, window);
        let close = |a: u64, b: u64| {
            let (a, b) = (a as f64, b as f64);
            assert!(f64::abs(a - b) / a < 0.01, "{a} against {b}");
        };
        // The last merger, between the two lanes deepest in the chain.
        close(moved[2], moved[3]);
        // The one above it, between the third lane and everything below.
        close(moved[1], moved[2] + moved[3]);
        // The head lane is supply-limited rather than share-limited: it delivers every item it
        // extracted, which is less than the half a rotation would have offered it.
        let extracted = u64::from(window / LANES[0].extract_ticks);
        close(moved[0], extracted);
        assert!(moved[0] < moved[1] + moved[2] + moved[3]);
    }
}
