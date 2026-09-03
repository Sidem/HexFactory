//! The Phase 8 physical scale contract, as fixed constants.
//!
//! `docs/HEXFACTORY-PLAN.md#phase-8--flowing-water` states these as one system rather than as
//! independent tuning knobs, so they live in one module and derive from each other wherever the
//! physics allows. Nothing here is a world parameter and nothing here is a slider: physical scale
//! is a property of the build, not of a preset.
//!
//! Belt cadence now reads these conversions: an item crosses one 5.37 m hex in
//! [`belt_transit_ticks`] at [`BELT_SPEED_MM_S`]. Placement, walking and generated ground still go
//! through the legacy-unit adapter until the rest of the slice-3 compatibility bundle switches with
//! them. The shipped 1 m² cell and the seven presentation bands remain the live *ground* model, and
//! the two must not be mixed.
//!
//! Every value is an integer in a named unit. Millimetres carry linear measure, because the height
//! quantum is 250 mm and the cell spacing is 5,373 mm — both exact in millimetres and both
//! irrational in metres. Nothing here uses floating point.

/// Plan area of one construction hex, in square metres. The axial integer lattice and pointy-top
/// topology are unchanged; only the physical interpretation of a cell moves, by five in linear
/// scale, from the shipped 1 m².
pub const CELL_AREA_M2: i32 = 25;

/// Distance between the centres of two adjacent cells, in millimetres.
///
/// A pointy-top hex of circumradius `s` has area `(3*sqrt(3)/2) * s^2` and neighbour spacing
/// `sqrt(3) * s`. At 25 m² that is `s = 3.10201 m` and a spacing of `5.37285 m`. This is the only
/// number that converts a count of cells into metres, and [`cells_to_mm`] is the only route.
pub const CELL_SPACING_MM: i32 = 5_373;

/// Circumradius of one cell — centre to corner — in millimetres. Meshes and footprints need it;
/// gameplay reach never does, because reach counts cells.
pub const CELL_CIRCUMRADIUS_MM: i32 = 3_102;

/// One native height quantum, in millimetres. Generated bed elevation, earthwork deltas and
/// erosion deltas are all counted in these.
pub const HEIGHT_QUANTUM_MM: i32 = 250;

/// Sea level, in height quanta. Generated bed elevation is a signed absolute integer against this
/// datum, so a 2,000 m summit is ordinary data rather than a terrain enum.
pub const SEA_LEVEL_QUANTA: i32 = 0;

/// The relief the generator is expected to produce, in height quanta: -400 m to +2,000 m.
///
/// This is a content range, not a storage limit. Absolute height is carried in `i32` so that
/// intermediate drainage arithmetic — accumulated drops, spill levels, catchment sums — cannot
/// overflow before anything is narrowed for the wire.
pub const BED_MIN_QUANTA: i32 = -1_600;
/// Upper end of [`BED_MIN_QUANTA`]'s range.
pub const BED_MAX_QUANTA: i32 = 8_000;

/// The three shipped Raise/Lower depth buttons, in height quanta: 0.5 m, 1.0 m and 1.5 m.
///
/// The buttons do not change in number or in feel; each one now names a distance instead of an
/// abstract band step.
pub const EARTHWORK_STEPS_QUANTA: [i32; 3] = [2, 4, 6];

/// How far a player may move ground from the generated bed, in height quanta: 8 m either way.
///
/// A content limit, deliberately far inside the `i16` the sparse earthwork delta is stored in, so
/// that the first wall a player meets is a design decision they can be told about rather than an
/// arithmetic edge they discover.
pub const EARTHWORK_LIMIT_QUANTA: i32 = 32;

/// One spoil unit — one quarter-metre layer of one cell — in litres, which is thousandths of a
/// cubic metre. 25 m² x 0.25 m = 6.25 m³.
///
/// Cut and fill stay an integer volume ledger; this is the conversion a player-facing figure uses,
/// never a divisor inside the ledger itself.
pub const SPOIL_UNIT_LITRES: i64 = 6_250_000;

/// Player walking and running speed, in millimetres per second.
///
/// What the rescale preserved is the *step*: `PLAYER_SPEED` is unchanged, so the player crosses a
/// hex in the time they always did and it is the metre figures that moved, from 3 and 5 to 15 and
/// 25. Distance is what a 25 m² hex buys; making every journey five times longer in the hand is
/// not, and a biome measured in quarter-hours is the cost this refuses to pay. These are the
/// derived numbers — the constant is `PLAYER_SPEED` in `lib.rs`, and its doc carries the decision.
pub const WALK_SPEED_MM_S: i32 = 20_000;
/// Running counterpart to [`WALK_SPEED_MM_S`].
pub const RUN_SPEED_MM_S: i32 = 25_000;

/// How fast a belt carries an item along its lane, in millimetres per second.
///
/// Two metres a second is a heavy-duty industrial conveyor. The shipped belt hands its cargo on
/// after a single tick, which at 10 TPS is one 5.37 m cell every 0.1 s — 54 m/s, a rifle round on a
/// rubber band. The plan asks for a bounded integer transit cadence rather than a relabelling, so
/// this is the speed and every belt number below is derived from it.
pub const BELT_SPEED_MM_S: i32 = 2_000;

/// How far apart two items sit on a belt, in millimetres.
///
/// One item per *shipped* cell — the 1 m² hex the game has had until now, 1.075 m across. Items did
/// not get bigger when the ground did, so the spacing that used to be one hex apart is the spacing
/// they still sit at. Together with [`BELT_SPEED_MM_S`] this fixes throughput at two items a second
/// without either number being chosen for throughput: a belt carries exactly what one extractor
/// produces, which is a ratio a player can read off the machines rather than off a table.
pub const BELT_ITEM_SPACING_MM: i32 = 1_075;

/// Simulation ticks per second. Belt cadence is counted in ticks, so the conversion from metres to
/// ticks needs the rate the ticks arrive at.
pub const TICKS_PER_SECOND: i32 = 10;

/// How many ticks an item takes to cross one belt cell, from the hex it entered to the hex it is
/// offered onward from.
///
/// The latency of a belt, not its throughput. Rounded to nearest so a lane of `n` cells is `n`
/// times this and never drifts.
pub const fn belt_transit_ticks() -> i64 {
    let numerator = CELL_SPACING_MM as i64 * TICKS_PER_SECOND as i64;
    let denominator = BELT_SPEED_MM_S as i64;
    (numerator + denominator / 2) / denominator
}

/// The minimum gap between two items entering the same belt, in ticks.
///
/// How long the belt takes to carry one item spacing past its entrance. A belt accepts an item only
/// once the one before it has travelled that far, so the lane fills at the speed it moves rather
/// than all at once at the entrance. This is the number that actually sets throughput: one item
/// every `belt_slot_ticks()` ticks.
pub const fn belt_slot_ticks() -> i64 {
    let numerator = BELT_ITEM_SPACING_MM as i64 * TICKS_PER_SECOND as i64;
    let denominator = BELT_SPEED_MM_S as i64;
    let ticks = (numerator + denominator / 2) / denominator;
    if ticks < 1 {
        1
    } else {
        ticks
    }
}

/// How many items one belt cell holds while they are crossing it.
///
/// Little's law over the two cadence numbers above, rounded *up*: an item occupies the belt for
/// `belt_transit_ticks()` and one arrives every `belt_slot_ticks()`, so a belt running at cadence
/// is carrying that ratio at any instant. Deriving it from the ticks rather than from
/// `CELL_SPACING_MM / BELT_ITEM_SPACING_MM` is what keeps capacity from throttling the very
/// throughput the other two constants state: those two divisions disagree in the last place,
/// because 5.37 m at 2 m/s is 26.9 ticks rounded to 27, and the belt would then be one item short
/// of the flow it is supposed to sustain. Rounding up means the nominal 1.075 m spacing is the
/// spacing of a *moving* belt; a jammed one packs its items closer, as a real conveyor does.
pub const fn belt_lane_slots() -> i64 {
    let transit = belt_transit_ticks();
    let slot = belt_slot_ticks();
    let slots = (transit + slot - 1) / slot;
    if slots < 1 {
        1
    } else {
        slots
    }
}

/// What a belt carries, in items per minute, at the cadence above. A reporting figure.
pub const fn belt_items_per_minute() -> i64 {
    TICKS_PER_SECOND as i64 * 60 / belt_slot_ticks()
}

/// The largest height difference between neighbours that a *building pad* may span, in height
/// quanta, absent an explicit foundation class. One quantum is 0.25 m over 5.37 m.
///
/// Walking and construction no longer share one threshold. Ordinary machines use this pad;
/// `FoundationClass::Span` may follow [`MAX_WALK_STEP_QUANTA`], and `Retaining` is the exception
/// for walls, stairs and prepared foundations.
pub const MAX_BUILD_STEP_QUANTA: i32 = 2;

/// The largest height difference between neighbours a player may walk, in height quanta. Four
/// quanta is 1.0 m over 5.37 m, about 18.6% or 10.5 degrees.
///
/// Slice 1 proposal, pending the slice 3 balance re-measurement. Walking will read slope, true
/// steps, surface and water depth together; this constant is the slope half of that predicate and
/// is deliberately larger than [`MAX_BUILD_STEP_QUANTA`].
pub const MAX_WALK_STEP_QUANTA: i32 = 4;

/// Water depth, in height quanta, at or above which a cell stops being wadeable. One metre.
///
/// Slice 1 proposal; slice 4 owns the wading, bridge and route-search rules that read it.
pub const WADE_LIMIT_QUANTA: i32 = 4;

/// A count of cells as millimetres along the lattice.
///
/// Rounds to nearest so that a reach quoted in metres and the same reach quoted in cells never
/// disagree by a systematic truncation. Uses `i64` because a continental span in millimetres
/// leaves `i32` well before it leaves the map.
pub const fn cells_to_mm(cells: i64) -> i64 {
    cells * CELL_SPACING_MM as i64
}

/// The inverse of [`cells_to_mm`], rounded to nearest. This is how a metre-derived rule — a reach,
/// a radius, a span — is restated as the cell count the lattice actually works in.
pub const fn mm_to_cells(mm: i64) -> i64 {
    let spacing = CELL_SPACING_MM as i64;
    if mm >= 0 {
        (mm + spacing / 2) / spacing
    } else {
        -((-mm + spacing / 2) / spacing)
    }
}

/// Height quanta as millimetres.
pub const fn quanta_to_mm(quanta: i64) -> i64 {
    quanta * HEIGHT_QUANTUM_MM as i64
}

/// Millimetres as height quanta, rounded to nearest.
pub const fn mm_to_quanta(mm: i64) -> i64 {
    let quantum = HEIGHT_QUANTUM_MM as i64;
    if mm >= 0 {
        (mm + quantum / 2) / quantum
    } else {
        -((-mm + quantum / 2) / quantum)
    }
}

/// How long one cell of walking takes, in milliseconds, on level unmodified ground.
pub const fn walk_step_ms() -> i64 {
    (cells_to_mm(1) * 1_000 + WALK_SPEED_MM_S as i64 / 2) / WALK_SPEED_MM_S as i64
}

/// The running counterpart to [`walk_step_ms`].
pub const fn run_step_ms() -> i64 {
    (cells_to_mm(1) * 1_000 + RUN_SPEED_MM_S as i64 / 2) / RUN_SPEED_MM_S as i64
}

/// Slope between neighbours as a percentage, from a height difference in quanta. Integer, rounded
/// toward zero; a reporting figure rather than a gate.
pub const fn neighbour_slope_percent(quanta: i32) -> i32 {
    (quanta as i64 * HEIGHT_QUANTUM_MM as i64 * 100 / CELL_SPACING_MM as i64) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spacing is not a free number: it is what 25 m² of pointy-top hex forces. Checking it
    /// against the area keeps a later "round it to 5.4 m for convenience" edit from silently
    /// making every cell 2.5% too big. Spacing and circumradius then have to describe the *same*
    /// hexagon, and the whole linear scale has to be exactly five times the shipped 1 m² cell —
    /// the claim the brief makes and the reason every metre-derived rate was retuned rather than
    /// scaled.
    #[test]
    fn the_cell_geometry_is_what_twenty_five_square_metres_forces() {
        // area = (sqrt(3) / 2) * spacing^2, in mm² then reduced to m².
        let spacing = CELL_SPACING_MM as i64;
        // sqrt(3)/2 as a rational, good to nine digits: 866025404 / 1000000000.
        let area_mm2 = spacing * spacing * 866_025_404 / 1_000_000_000;
        let area_m2 = area_mm2 / 1_000_000;
        assert_eq!(area_m2, CELL_AREA_M2 as i64);

        // spacing = sqrt(3) * circumradius.
        let expected = CELL_CIRCUMRADIUS_MM as i64 * 1_732_050_808 / 1_000_000_000;
        assert!((expected - CELL_SPACING_MM as i64).abs() <= 1);

        assert_eq!(CELL_AREA_M2, 25);
        // The shipped cell was 1 m², so its spacing was 5.373 / 5.
        let shipped_spacing_mm = 1_075;
        assert!((CELL_SPACING_MM - shipped_spacing_mm * 5).abs() <= 2);
    }

    /// Every height figure a player is shown reads back in metres: the Raise/Lower buttons are
    /// still 0.5 m, 1.0 m and 1.5 m after the conversion, the earthwork ceiling is a content limit
    /// of 8 m rather than the storage limit of the `i16` it lives in, one spoil unit is one
    /// quarter-metre layer of one 25 m² cell, and the generated relief spans 400 m below sea level
    /// to 2,000 m above it.
    #[test]
    fn heights_and_volumes_read_back_in_metres() {
        let metres: Vec<i64> = EARTHWORK_STEPS_QUANTA
            .iter()
            .map(|&step| quanta_to_mm(step as i64))
            .collect();
        assert_eq!(metres, vec![500, 1_000, 1_500]);
        assert_eq!(quanta_to_mm(EARTHWORK_LIMIT_QUANTA as i64), 8_000);
        assert!(EARTHWORK_LIMIT_QUANTA < i32::from(i16::MAX));

        let litres = CELL_AREA_M2 as i64 * quanta_to_mm(1) * 1_000;
        assert_eq!(litres, SPOIL_UNIT_LITRES);
        assert_eq!(SPOIL_UNIT_LITRES / 1_000_000, 6); // 6.25 m³, floored.

        assert_eq!(quanta_to_mm(BED_MIN_QUANTA as i64), -400_000);
        assert_eq!(quanta_to_mm(BED_MAX_QUANTA as i64), 2_000_000);
        assert_eq!(SEA_LEVEL_QUANTA, 0);
    }

    /// The belt's cadence is derived from its speed and its item spacing, and neither of those was
    /// chosen to hit a throughput number. What falls out is one belt carrying exactly one
    /// extractor's 120 items a minute, at 5.37 m in 2.7 s — about 2 m/s.
    ///
    /// The property those numbers exist to satisfy is asserted with them: capacity must never be
    /// what limits a flowing belt. A lane has to hold everything in flight when items arrive on
    /// cadence, or the entrance stalls and the stated rate is fiction — and no more than one item
    /// of slack, or the belt is quietly a buffer rather than a lane.
    #[test]
    fn belt_cadence_follows_from_speed_and_spacing() {
        assert_eq!(belt_transit_ticks(), 27);
        assert_eq!(belt_slot_ticks(), 5);
        assert_eq!(belt_lane_slots(), 6);
        assert_eq!(belt_items_per_minute(), 120);
        // The lane's own speed, back out of the cadence, is the speed it was derived from.
        let mm_per_tick = cells_to_mm(1) / belt_transit_ticks();
        let mm_per_second = mm_per_tick * TICKS_PER_SECOND as i64;
        assert!((mm_per_second - BELT_SPEED_MM_S as i64).abs() <= 100);

        assert!(belt_lane_slots() * belt_slot_ticks() >= belt_transit_ticks());
        assert!((belt_lane_slots() - 1) * belt_slot_ticks() < belt_transit_ticks());
    }

    /// Walking is the 1× pace and running is exactly 1.25×. The belt above is the opposite case —
    /// its speed is the fixed thing and its cadence was re-derived.
    ///
    /// The conversions those durations are computed through are inverses within half a cell, in
    /// both directions and across zero; and the two slope thresholds the ground rules read are no
    /// longer one number, with build the stricter of them.
    #[test]
    fn movement_reads_the_lattice_through_round_tripping_conversions() {
        assert_eq!(walk_step_ms(), 269);
        assert_eq!(run_step_ms(), 215);
        assert!(
            run_step_ms() * 5 == walk_step_ms() * 4
                || (run_step_ms() * 5 - walk_step_ms() * 4).abs() <= 4
        );

        for cells in -1_000..=1_000 {
            assert_eq!(mm_to_cells(cells_to_mm(cells)), cells);
        }
        for quanta in -8_000..=8_000 {
            assert_eq!(mm_to_quanta(quanta_to_mm(quanta)), quanta);
        }

        assert!(MAX_BUILD_STEP_QUANTA < MAX_WALK_STEP_QUANTA);
        assert_eq!(neighbour_slope_percent(MAX_BUILD_STEP_QUANTA), 9);
        assert_eq!(neighbour_slope_percent(MAX_WALK_STEP_QUANTA), 18);
    }
}
