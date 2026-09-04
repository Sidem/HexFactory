//! Derived habitat truth.
//!
//! Habitat is neither terrain identity nor a resource field. It is a native answer derived on
//! demand from drainage and the cell's current physical state, so an untouched stable world stores
//! no ecology cache and performs no ecology tick.

use super::*;

/// One unit of rated river discharge supports this many future population units on an intact bank.
/// The discharge ladder is already the measured drainage capacity scale; using it directly avoids
/// inventing a second moisture threshold beside the generator's wet-channel and bench rules.
const CAPACITY_PER_DISCHARGE_CLASS: u16 = 25;

/// What ground watered by anything other than a rated channel is worth.
///
/// The floor of the ladder, deliberately. A hand-cut trench is the poorest thing that can water
/// ground — it carries no drainage of its own, only what somebody let into it — so irrigated land is
/// productive without ever being worth as much per hex as the floodplain the river made itself.
const CANAL_DISCHARGE_CLASS: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FertileRiverbank {
    pub capacity: u16,
    pub discharge_class: u8,
}

fn fertile_riverbank(
    discharge_class: Option<u8>,
    water_depth: i32,
    surface: DefinitionId,
    occupied: bool,
) -> Option<FertileRiverbank> {
    let discharge_class = discharge_class?;
    (discharge_class > 0 && water_depth == 0 && surface == 0 && !occupied).then_some(
        FertileRiverbank {
            capacity: u16::from(discharge_class) * CAPACITY_PER_DISCHARGE_CLASS,
            discharge_class,
        },
    )
}

/// Whether fresh standing water reaches this cell's ring, and so whether the cell is watered at all.
///
/// The sea is excluded and lakes are not. What makes ground fertile here is water it can drink, and
/// the one physical distinction the model already draws between bodies of water is the datum: at or
/// below sea level is the ocean, above it is runoff that got there by falling on land.
fn watered_by(mut depth_and_surface: impl FnMut(i32, i32) -> (i32, i32), q: i32, r: i32) -> bool {
    DIRECTIONS.iter().any(|&(dq, dr)| {
        let (depth, surface) = depth_and_surface(q + dq, r + dr);
        depth > 0 && surface > crate::scale::SEA_LEVEL_QUANTA
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn generated_fertile_riverbank(
    spine: &GroundSpine,
    q: i32,
    r: i32,
) -> Option<FertileRiverbank> {
    let ground = spine.generated_at(q, r);
    let watered = || {
        watered_by(
            |nq, nr| {
                let hydrology = spine.generated_at(nq, nr).hydrology;
                (hydrology.depth_quanta, hydrology.surface.get())
            },
            q,
            r,
        )
    };
    fertile_riverbank(
        irrigation_class(spine.river_bench_class_at(q, r), watered),
        ground.hydrology.depth_quanta,
        0,
        false,
    )
}

/// The drainage class that waters one cell, from the two ways ground gets wet.
///
/// The alluvial bench is the river's own doing and keeps the river's class whether or not the water
/// has reached this particular hex today — that is a floodplain, and it is as rich as the channel is
/// large. Everywhere else the question is only whether water is standing next to it now. That second
/// path is what a canal is: the trench itself is the water, and the ground either side of it drinks.
fn irrigation_class(bench_class: Option<u8>, watered: impl FnOnce() -> bool) -> Option<u8> {
    bench_class.or_else(|| watered().then_some(CANAL_DISCHARGE_CLASS))
}

impl Core {
    /// Current fertile-riverbank capacity at one cell.
    ///
    /// Every input is native physical truth. Drainage comes from the generated bench or from water
    /// standing in the ring; standing water on the cell itself, a prepared surface and an occupied
    /// footprint suppress only the cell where those facts occur. No result is cached or ticked.
    ///
    /// Earthwork is deliberately *not* one of the causes. It used to be — fertility meant the
    /// generated grade was intact — which made the one thing a farmer actually does to a riverbank
    /// the one thing that sterilised it. Fertility follows water now, so a trench cut inland carries
    /// it, and the hexes a canal was dug through are watered by the canal rather than ruined by it.
    pub(super) fn fertile_riverbank_at(&self, q: i32, r: i32) -> Option<FertileRiverbank> {
        let finished = self.finished_ground_at(q, r);
        let watered = || {
            watered_by(
                |nq, nr| (self.water_depth_at(nq, nr), self.water_surface_at(nq, nr)),
                q,
                r,
            )
        };
        fertile_riverbank(
            irrigation_class(self.ground_spine.river_bench_class_at(q, r), watered),
            self.water_depth_of(finished.generated, q, r),
            finished.surface,
            self.runtime.occupied.contains_key(&(q, r)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fertile_riverbank_keeps_each_physical_cause_independent() {
        assert_eq!(
            fertile_riverbank(Some(4), 0, 0, false),
            Some(FertileRiverbank {
                capacity: 100,
                discharge_class: 4,
            })
        );
        assert_eq!(fertile_riverbank(None, 0, 0, false), None);
        assert_eq!(fertile_riverbank(Some(4), 1, 0, false), None);
        assert_eq!(fertile_riverbank(Some(4), 0, 7, false), None);
        assert_eq!(fertile_riverbank(Some(4), 0, 0, true), None);
    }

    /// Grade is no longer one of the causes, and a canal is the second way in.
    #[test]
    fn irrigation_prefers_the_bench_and_falls_back_to_standing_water() {
        assert_eq!(irrigation_class(Some(5), || false), Some(5));
        assert_eq!(irrigation_class(Some(5), || true), Some(5));
        assert_eq!(
            irrigation_class(None, || true),
            Some(CANAL_DISCHARGE_CLASS),
            "a trench waters the ground beside it, at the bottom of the ladder"
        );
        assert_eq!(irrigation_class(None, || false), None);
    }

    /// The sea is the one body that waters nothing.
    #[test]
    fn watered_by_takes_fresh_water_and_refuses_the_sea() {
        let sea = |_: i32, _: i32| (8, crate::scale::SEA_LEVEL_QUANTA);
        let brook = |_: i32, _: i32| (2, crate::scale::SEA_LEVEL_QUANTA + 400);
        assert!(!watered_by(sea, 0, 0));
        assert!(watered_by(brook, 0, 0));
        assert!(!watered_by(
            |_, _| (0, crate::scale::SEA_LEVEL_QUANTA + 400),
            0,
            0
        ));
    }

    #[test]
    fn habitat_is_query_order_invariant_and_crosses_gameplay_chunk_seams() {
        let params = default_world_params();
        let seed = 1_213_486_160;
        let cells = hexes_in_radius((0, 0), 96);
        let forward = GroundSpine::physical(&params, seed, true);
        let readings: Vec<_> = cells
            .iter()
            .map(|&(q, r)| generated_fertile_riverbank(&forward, q, r))
            .collect();
        let backward = GroundSpine::physical(&params, seed, true);
        for (index, &(q, r)) in cells.iter().enumerate().rev() {
            assert_eq!(
                generated_fertile_riverbank(&backward, q, r),
                readings[index],
                "query order changed habitat at {q},{r}"
            );
        }
        let crosses_seam = cells.iter().any(|&(q, r)| {
            generated_fertile_riverbank(&forward, q, r).is_some()
                && DIRECTIONS.iter().any(|&(dq, dr)| {
                    let neighbour = (q + dq, r + dr);
                    (floor_div(q, 8), floor_div(r, 8))
                        != (floor_div(neighbour.0, 8), floor_div(neighbour.1, 8))
                        && generated_fertile_riverbank(&forward, neighbour.0, neighbour.1).is_some()
                })
        });
        assert!(
            crosses_seam,
            "the measured habitat never crosses an 8-cell chunk seam"
        );
    }
}
