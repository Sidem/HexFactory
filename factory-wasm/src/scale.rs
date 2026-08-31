//! The Phase 8 physical scale contract, as fixed constants.
//!
//! `docs/HEXFACTORY-PLAN.md#phase-8--flowing-water` states these as one system rather than as
//! independent tuning knobs, so they live in one module and derive from each other wherever the
//! physics allows. Nothing here is a world parameter and nothing here is a slider: physical scale
//! is a property of the build, not of a preset.
//!
//! **This module is inert in v0.46.** Slice 1 of the phase is a baseline and a prototype with no
//! production toggle, so these constants are declared, derived and tested but not yet read by
//! placement, walking, generation or the wire. Slice 2 threads typed ground through the native
//! simulation behind a legacy-unit adapter; slice 3 activates these physical conversions at the
//! compatibility boundary. Until then the shipped 1 m² cell and the seven presentation bands
//! remain the live model, and the two must not be mixed.
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

/// Player walking and running speed, in millimetres per second. The stated 3 m/s and 5 m/s are
/// preserved across the rescale by changing world-units per step, not by changing the speed.
pub const WALK_SPEED_MM_S: i32 = 3_000;
/// Running counterpart to [`WALK_SPEED_MM_S`].
pub const RUN_SPEED_MM_S: i32 = 5_000;

/// The largest height difference between neighbours that a *building pad* may span, in height
/// quanta, absent an explicit foundation class. One quantum is 0.25 m over 5.37 m.
///
/// Slice 1 proposal, pending the slice 3 retune. `MAX_BUILD_STEP == MAX_WALK_STEP` does not
/// survive this phase: retaining walls, foundations and stairs are the exceptions, and they are
/// stated per definition rather than as one global number.
pub const MAX_BUILD_STEP_QUANTA: i32 = 1;

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
    /// making every cell 2.5% too big.
    #[test]
    fn spacing_matches_the_stated_cell_area() {
        // area = (sqrt(3) / 2) * spacing^2, in mm² then reduced to m².
        let spacing = CELL_SPACING_MM as i64;
        // sqrt(3)/2 as a rational, good to nine digits: 866025404 / 1000000000.
        let area_mm2 = spacing * spacing * 866_025_404 / 1_000_000_000;
        let area_m2 = area_mm2 / 1_000_000;
        assert_eq!(area_m2, CELL_AREA_M2 as i64);
    }

    /// Spacing and circumradius describe the same hexagon: spacing = sqrt(3) * circumradius.
    #[test]
    fn circumradius_matches_the_spacing() {
        let expected = CELL_CIRCUMRADIUS_MM as i64 * 1_732_050_808 / 1_000_000_000;
        assert!((expected - CELL_SPACING_MM as i64).abs() <= 1);
    }

    /// The linear scale moved by exactly five from the shipped 1 m² cell, which is the claim the
    /// brief makes and the reason every metre-derived rate has to be retuned rather than scaled.
    #[test]
    fn linear_scale_moved_by_five() {
        assert_eq!(CELL_AREA_M2, 25);
        // The shipped cell was 1 m², so its spacing was 5.373 / 5.
        let shipped_spacing_mm = 1_075;
        assert!((CELL_SPACING_MM - shipped_spacing_mm * 5).abs() <= 2);
    }

    /// The Raise/Lower buttons still read 0.5 m, 1.0 m and 1.5 m after the conversion, and the
    /// content limit is 8 m rather than the storage limit of the `i16` it lives in.
    #[test]
    fn earthwork_steps_name_metres() {
        let metres: Vec<i64> = EARTHWORK_STEPS_QUANTA
            .iter()
            .map(|&step| quanta_to_mm(step as i64))
            .collect();
        assert_eq!(metres, vec![500, 1_000, 1_500]);
        assert_eq!(quanta_to_mm(EARTHWORK_LIMIT_QUANTA as i64), 8_000);
        assert!(EARTHWORK_LIMIT_QUANTA < i32::from(i16::MAX));
    }

    /// One spoil unit is one quarter-metre layer of one 25 m² cell.
    #[test]
    fn spoil_unit_is_one_cell_layer() {
        let litres = CELL_AREA_M2 as i64 * quanta_to_mm(1) * 1_000;
        assert_eq!(litres, SPOIL_UNIT_LITRES);
        assert_eq!(SPOIL_UNIT_LITRES / 1_000_000, 6); // 6.25 m³, floored.
    }

    /// Walking is still 3 m/s and running still 5 m/s; what changed is how far one step carries.
    /// A step is now about 1.79 s rather than about 0.36 s, which is the whole reason cadence has
    /// to be re-derived rather than relabelled.
    #[test]
    fn step_durations_follow_from_speed_and_spacing() {
        assert_eq!(walk_step_ms(), 1_791);
        assert_eq!(run_step_ms(), 1_075);
        assert!(
            run_step_ms() * 5 == walk_step_ms() * 3
                || (run_step_ms() * 5 - walk_step_ms() * 3).abs() <= 4
        );
    }

    /// The two conversions are inverses within half a cell, in both directions and across zero.
    #[test]
    fn cell_and_millimetre_conversions_round_trip() {
        for cells in -1_000..=1_000 {
            assert_eq!(mm_to_cells(cells_to_mm(cells)), cells);
        }
        for quanta in -8_000..=8_000 {
            assert_eq!(mm_to_quanta(quanta_to_mm(quanta)), quanta);
        }
    }

    /// The build and walk thresholds are no longer one number, and the build one is the stricter.
    #[test]
    fn build_and_walk_thresholds_have_parted() {
        assert!(MAX_BUILD_STEP_QUANTA < MAX_WALK_STEP_QUANTA);
        assert_eq!(neighbour_slope_percent(MAX_BUILD_STEP_QUANTA), 4);
        assert_eq!(neighbour_slope_percent(MAX_WALK_STEP_QUANTA), 18);
    }

    /// The stated content relief spans 400 m below sea level to 2,000 m above it.
    #[test]
    fn bed_range_spans_the_stated_relief() {
        assert_eq!(quanta_to_mm(BED_MIN_QUANTA as i64), -400_000);
        assert_eq!(quanta_to_mm(BED_MAX_QUANTA as i64), 2_000_000);
        assert_eq!(SEA_LEVEL_QUANTA, 0);
    }
}
