//! What the ground is made of, and how hard it is to cut through at a given depth.
//!
//! The generator lays a graded long profile because that is the steady state of the stream-power
//! law, `E = K·A^m·S^n`. This module is the `K` in it. Holding erodibility at one constant is what
//! made every valley in the world the same shape; making it a field is what lets the same solve
//! produce a gorge here, a floodplain there, and a step in the profile where a reach meets rock it
//! cannot win against.
//!
//! Nothing here draws a feature. A sill is a bed the river lost to, a fall is the step that leaves
//! it, and the pool above it is a depression the ordinary lake solve finds. All three are
//! consequences of one comparison: rock strength against discharge.
//!
//! Pure, `O(1)` and province-local, like every other field the generator reads.

use crate::terra::{base_mq, MQ, NOISE_MAX};
use crate::{floor_div, value_noise};

/// Wavelength of the rock-type field, in cells: about 1.7 km.
///
/// An outcrop has to be a place, not a texture. Below about a kilometre the beds alternate faster
/// than a valley crosses them and the profile reads as noise rather than as geology.
const ROCK_CELL: i32 = 320;

/// Thickness of one bed, in height quanta: 3 m. Thin enough that a valley cuts through several,
/// which is what puts more than one step in a long profile.
const BED_QUANTA: i32 = 12;

/// Depth of the weathered mantle over bedrock, in height quanta: 3 m.
///
/// Softer near the surface, harder with depth. Every cell's top three metres are loose whatever
/// lies under them, so a stream can always cut its own bed; only a river carries enough power to
/// reach the rock below and keep going.
const REGOLITH_QUANTA: i32 = 12;

/// How fast a valley side climbs away from the channel, in milli-quanta per cell, in the weakest
/// and the strongest rock.
///
/// The floor is the single constant world 14 used, so the weakest rock gives the valley that was
/// already tuned and no valley anywhere is wider than one this world has already shipped. That is
/// not caution about slope, it is about what a wider valley planes away: a span of 3,000..5,600
/// centred on the old constant read better in cross-section and cost the continental opening its
/// coal, the largest patch inside the guarantee disc falling from 36 hexes to 16, under the 19 a
/// base extractor needs to be worth standing on. Deposits sit on the bands the relief decides, so
/// widening a valley is deleting ground.
///
/// The ceiling is a gorge at 1.6 times that, and it is what the variety costs: 25 per mille of the
/// inland sample stops being walkable, because a bank that stands up is a bank you walk around.
const BANK_SOFT_MQ: i32 = 4_000;
const BANK_HARD_MQ: i32 = 6_400;

/// Steps the floor scan may take. The scan interval widens to fit, so a deep cut is resolved more
/// coarsely rather than costing more, and the work per channel cell is bounded whatever the reach
/// crosses.
const SCAN_STEPS: i32 = 12;

const OCT_ROCK: u32 = 0x7E44A_7;
/// Salt step between beds. Large and odd, so consecutive beds are independent fields rather than
/// two samples of one.
const BED_SALT: u32 = 0x9E37_79B1;

/// Strength of the bedrock at a level, `0..=100`.
///
/// Beds are flat-lying and laterally graded: the same level is the same kind of rock for about a
/// kilometre and a half, and a different kind three metres down. That is what makes a resistant
/// layer something a river meets across its whole width at once instead of in one cell.
pub(crate) fn rock_strength(seed: u32, q: i32, r: i32, level_mq: i32) -> i32 {
    let bed = floor_div(level_mq, BED_QUANTA * MQ as i32);
    let octave = OCT_ROCK.wrapping_add((bed as u32).wrapping_mul(BED_SALT));
    value_noise(seed, q, r, ROCK_CELL, octave) * 100 / NOISE_MAX
}

/// Strength of the material actually standing at a level, `0..=100`, given the untouched surface
/// above it. Ramps from nothing at the surface to the bedrock's own over [`REGOLITH_QUANTA`].
fn strength_under(seed: u32, q: i32, r: i32, surface_mq: i32, level_mq: i32) -> i32 {
    let mantle = REGOLITH_QUANTA * MQ as i32;
    let depth = (surface_mq - level_mq).clamp(0, mantle);
    rock_strength(seed, q, r, level_mq) * depth / mantle
}

/// The strength a reach of this discharge class can cut through.
///
/// Discharge is the `A` in the stream-power law and the class counts it, so a bigger river wins
/// against harder rock: the top class cuts anything, and it is the small channels that hang.
///
/// The offset is the tuned number, not the slope, and it is tuned against drainage rather than
/// taste. Every sill is a bed with a pool behind it, and a world of pools is the failure this
/// module could most easily cause: at 30 the smallest streams lost to two beds in three and the
/// sample filled with 84 lakes over 344 cells, at 60 with 30 over 166. Here it is 11 over 58
/// against the 7 over 45 the same sample holds with sills disabled entirely: four more ponds, and
/// every one of the 576 drainage walks still ends exactly where it ended without them. The sills
/// and the falls are bought for four ponds.
pub(crate) fn cut_power(class: u8) -> i32 {
    76 + i32::from(class) * 4
}

/// How far down a reach of this class actually gets, in milli-quanta.
///
/// Scans from the untouched surface toward the bed the grade line asks for and stops at the first
/// material it cannot cut. Returning the target unchanged is the ordinary case; returning higher is
/// a sill, and everything a sill causes follows from the bed being where this says it is.
pub(crate) fn erodible_floor_mq(seed: u32, q: i32, r: i32, class: u8, target_mq: i32) -> i32 {
    let surface = base_mq(seed, q, r);
    if surface <= target_mq {
        return target_mq;
    }
    let power = cut_power(class);
    let step = ((surface - target_mq) / SCAN_STEPS).max(BED_QUANTA * MQ as i32 / 2);
    let mut level = surface;
    while level > target_mq {
        let next = (level - step).max(target_mq);
        if strength_under(seed, q, r, surface, next) > power {
            return level;
        }
        level = next;
    }
    target_mq
}

/// How fast a valley side climbs away from the channel, in milli-quanta per cell.
///
/// Weak ground slumps to a wide open floor; strong ground stands as a wall. Sampled below the
/// mantle, because a bank is held up by the rock in it and not by the soil on top. `surface_mq` is
/// the cell's untouched height, which the carving solve already has to hand.
pub(crate) fn bank_grade_mq(seed: u32, q: i32, r: i32, surface_mq: i32) -> i32 {
    let level = surface_mq - REGOLITH_QUANTA * MQ as i32;
    BANK_SOFT_MQ + (BANK_HARD_MQ - BANK_SOFT_MQ) * rock_strength(seed, q, r, level) / 100
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terra::CHANNEL_CLASS_MIN;

    const SEED: u32 = 1_213_486_160;

    /// The field has to be a field: bounded everywhere, varying laterally, and layered with depth.
    #[test]
    fn rock_is_bounded_layered_and_locally_varied() {
        let mut seen = std::collections::BTreeSet::new();
        for q in -400..400 {
            let strength = rock_strength(SEED, q, 0, 0);
            assert!(
                (0..=100).contains(&strength),
                "strength {strength} out of range"
            );
            seen.insert(strength / 10);
        }
        assert!(
            seen.len() > 3,
            "one traverse should cross several rock types"
        );

        // Two levels a bed apart are independent samples; two inside one bed are the same rock.
        let bed = BED_QUANTA * MQ as i32;
        assert_eq!(
            rock_strength(SEED, 0, 0, bed * 3),
            rock_strength(SEED, 0, 0, bed * 3 + bed / 2)
        );
        let differs = (0..64)
            .filter(|level| {
                rock_strength(SEED, 0, 0, level * bed)
                    != rock_strength(SEED, 0, 0, (level + 1) * bed)
            })
            .count();
        assert!(
            differs > 48,
            "beds should mostly differ from their neighbours"
        );
    }

    /// The mantle is soft whatever is under it, and discharge decides what the rock below costs.
    #[test]
    fn the_mantle_is_soft_and_discharge_decides_what_the_rock_costs() {
        let surface = base_mq(SEED, 0, 0);
        let mantle = REGOLITH_QUANTA * MQ as i32;
        assert_eq!(strength_under(SEED, 0, 0, surface, surface), 0);
        assert!(
            strength_under(SEED, 0, 0, surface, surface - mantle / 2)
                < rock_strength(SEED, 0, 0, surface - mantle / 2).max(1)
        );
        assert!(cut_power(7) > 100, "the largest river cuts anything");
        assert!(cut_power(CHANNEL_CLASS_MIN) < cut_power(7));

        // A class the rock can stop never cuts below one it cannot, anywhere along a traverse.
        let mut sills = 0;
        for q in 0..256 {
            let target = base_mq(SEED, q, 0) - 20 * MQ as i32;
            let small = erodible_floor_mq(SEED, q, 0, CHANNEL_CLASS_MIN, target);
            let large = erodible_floor_mq(SEED, q, 0, 7, target);
            assert!(
                small >= target && large >= target,
                "a floor is never below the bed asked for"
            );
            assert!(small >= large, "a stream cannot outcut a river at ({q},0)");
            assert_eq!(large, target, "the top class reaches its graded bed");
            sills += i32::from(small > target);
        }
        assert!(
            (1..128).contains(&sills),
            "sills should be real but uncommon; found {sills} in 256 cells"
        );
    }

    /// A bank grade is always a climb, and always inside the span the width cap was tuned for.
    #[test]
    fn bank_grades_stay_inside_the_measured_span() {
        for q in -200..200 {
            let grade = bank_grade_mq(SEED, q, q / 3, base_mq(SEED, q, q / 3));
            assert!((BANK_SOFT_MQ..=BANK_HARD_MQ).contains(&grade));
        }
    }
}
