//! Measurement of what a seed's landscape contains: counting, invariants and reporting.
//!
//! Split from `terra` because none of it generates ground. The generator answers "what is at
//! this cell"; this answers "is the model true", and only the harnesses and tests ask.

use crate::scale::SEA_LEVEL_QUANTA;
use crate::terra::{
    bed_depth, province_of, province_origin, river_bench_width, river_half_width,
    valley_half_width, Flow, Terra, Water, MQ, PROVINCE_CELL,
};
use crate::{hexes_in_radius, DIRECTIONS};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
/// What a seed's landscape actually contains, counted rather than described.
///
/// Every claim the Phase 8 brief makes about drainage is a claim about proportions or invariants,
/// and both are things to be measured. `cycles` and `uphill_edges` are here to be zero; if they
/// are ever not, the model has been falsified and no amount of tuning is the answer.
#[derive(Clone, Debug)]
pub struct TerraSurvey {
    pub seed: u32,
    /// The province the square is centred on. Recorded because "0 walks reached the sea" means
    /// something entirely different inland than it does on a coast.
    pub centre: (i32, i32),
    pub provinces: u32,
    pub cells: u64,
    pub min_quanta: i32,
    pub max_quanta: i32,
    pub mean_quanta: i32,
    /// Neighbour height differences, bucketed by height quanta: 0, 1, 2-3, 4-7, 8-15, 16+.
    pub slope_histogram: [u64; 6],
    /// Neighbour pairs a player could step between under
    /// [`crate::scale::MAX_WALK_STEP_QUANTA`], and pairs a building pad could span under
    /// [`crate::scale::MAX_BUILD_STEP_QUANTA`].
    ///
    /// These are the numbers that decide whether the scale contract and the generator agree. A
    /// world can satisfy every drainage invariant and still be unplayable, and the only way that
    /// shows up is by asking what fraction of it a person can walk across.
    pub walkable_edges: u64,
    pub buildable_edges: u64,
    pub total_edges: u64,
    pub springs: u32,
    pub lakes: u32,
    pub lake_cells: u64,
    pub frontier_basins: u32,
    pub cycles: u64,
    pub uphill_edges: u64,
    /// Flow edges running from one channel cell to another, how many of those carry the water
    /// surface uphill, and by how much at the worst of them, in milli-quanta.
    ///
    /// `uphill_edges` is about the ground; this is about the river standing on it. A grade line
    /// that gains height downstream is the defect that makes a river read as unphysical, and it is
    /// invisible in a height field that descends perfectly well underneath.
    ///
    /// Not part of [`TerraSurvey::invariants_hold`], because the grade line is interpolated in
    /// integers and a confluence joins two reaches cut to different depths: a few edges rise by
    /// rounding. Measured at 15 mm over 2,411 edges, which is a sixteenth of a height quantum and
    /// below anything the published field can express. A rise of metres would falsify the model.
    pub river_edges: u64,
    pub river_rises: u64,
    pub river_rise_max_mq: i32,
    /// Channel cells the rock stopped short of their graded bed, the flow edges whose water
    /// surface steps down further in one cell than a player can wade, and the largest such step.
    ///
    /// These are the falls. Nothing places one: a sill is a bed a reach lost to, the step is what
    /// the grade line does on the far side of it, and the pool behind it is a depression the lake
    /// solve finds on its own. Counting them is how "the rock varies" stays a claim about the
    /// landscape rather than about the noise field.
    pub river_sills: u64,
    pub river_falls: u64,
    pub river_fall_max_mq: i32,
    /// Channel cells per discharge class.
    pub discharge_histogram: [u64; 8],
    pub sea_cells: u64,
    pub lake_water_cells: u64,
    pub river_cells: u64,
    /// Where a downstream walk ended, over a sampled set of starts.
    pub walks: u64,
    pub reached_sea: u64,
    pub reached_lake: u64,
    pub reached_frontier: u64,
    /// Walks that were still running when they left the surveyed square. Not a failure: a river
    /// crossing the edge of the sample is a river, and following it would mean solving provinces
    /// the survey never asked about.
    pub left_survey: u64,
    pub walk_budget_exhausted: u64,
    pub longest_walk: u32,
    /// Elevation range inside one [`VIEWPORT_CELL`] disc, in quanta, over sampled centres: the
    /// median view and the flattest tenth of views.
    ///
    /// The slope histogram cannot answer the question this does. A field of uncorrelated
    /// centimetre noise and a hillside produce similar neighbour steps, and only one of them is a
    /// landform: the difference is whether the steps accumulate over the distance the camera
    /// frames or cancel out inside it. That is what these two numbers measure, and "the world
    /// looks flat" is a claim about them and about nothing else in this report.
    pub viewport_relief_median: i32,
    pub viewport_relief_p10: i32,
    pub solve_micros: u128,
    pub sweep_micros: u128,
}

/// The radius of the ground a player has on screen at a normal zoom, in cells: about 215 m across.
pub const VIEWPORT_CELL: i32 = 40;

impl TerraSurvey {
    pub fn water_per_mille(&self) -> u64 {
        if self.cells == 0 {
            return 0;
        }
        (self.sea_cells + self.lake_water_cells + self.river_cells) * 1_000 / self.cells
    }

    pub fn walkable_per_mille(&self) -> u64 {
        self.walkable_edges * 1_000 / self.total_edges.max(1)
    }

    pub fn buildable_per_mille(&self) -> u64 {
        self.buildable_edges * 1_000 / self.total_edges.max(1)
    }

    /// Channel cells as a share of the sample: the drainage density, which is the number that
    /// separates a river network from a crazed glaze.
    pub fn channel_per_mille(&self) -> u64 {
        self.discharge_histogram.iter().sum::<u64>() * 1_000 / self.cells.max(1)
    }

    /// The invariants the brief names as acceptance. A survey that fails this has falsified the
    /// model rather than found a bug to paper over.
    pub fn invariants_hold(&self) -> bool {
        self.cycles == 0 && self.uphill_edges == 0 && self.walk_budget_exhausted == 0
    }
}

/// How far a downstream walk is allowed to run before the survey calls it unterminated.
const WALK_BUDGET: u32 = 20_000;

/// Surveys a square of `span` by `span` provinces, with the origin province at its centre.
/// Surveys a square of provinces centred on the origin.
pub fn survey(seed: u32, span: i32) -> TerraSurvey {
    survey_at(seed, span, (0, 0))
}

/// Surveys a square of provinces centred anywhere.
///
/// The origin is not a representative place. Whether it is mountain, plain or seabed is a property
/// of the seed, and a survey that only ever looks there will report "no walk reached the sea" for a
/// sample with no sea in it and call that a drainage result. [`coast_province`] finds somewhere the
/// question can actually be asked.
pub fn survey_at(seed: u32, span: i32, centre: (i32, i32)) -> TerraSurvey {
    let span = span.max(1);
    let half = (span - 1) / 2;
    let (cq, cr) = centre;
    let mut terra = Terra::new(seed);

    #[cfg(not(target_arch = "wasm32"))]
    let solve_started = Instant::now();
    let mut provinces = Vec::new();
    for pr in -half..(span - half) {
        for pq in -half..(span - half) {
            provinces.push((cq + pq, cr + pr));
            terra.province(cq + pq, cr + pr);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let solve_micros = solve_started.elapsed().as_micros();
    #[cfg(target_arch = "wasm32")]
    let solve_micros = 0u128;

    #[cfg(not(target_arch = "wasm32"))]
    let sweep_started = Instant::now();
    let mut result = TerraSurvey {
        seed,
        centre,
        provinces: provinces.len() as u32,
        cells: 0,
        min_quanta: i32::MAX,
        max_quanta: i32::MIN,
        mean_quanta: 0,
        slope_histogram: [0; 6],
        walkable_edges: 0,
        buildable_edges: 0,
        total_edges: 0,
        springs: 0,
        lakes: 0,
        lake_cells: 0,
        frontier_basins: 0,
        cycles: 0,
        uphill_edges: 0,
        river_edges: 0,
        river_rises: 0,
        river_rise_max_mq: 0,
        river_sills: 0,
        river_falls: 0,
        river_fall_max_mq: 0,
        discharge_histogram: [0; 8],
        sea_cells: 0,
        lake_water_cells: 0,
        river_cells: 0,
        walks: 0,
        reached_sea: 0,
        reached_lake: 0,
        reached_frontier: 0,
        left_survey: 0,
        walk_budget_exhausted: 0,
        longest_walk: 0,
        viewport_relief_median: 0,
        viewport_relief_p10: 0,
        solve_micros,
        sweep_micros: 0,
    };

    let mut height_total: i64 = 0;
    for &(pq, pr) in &provinces {
        let province = terra.province(pq, pr);
        result.springs += province.springs().len() as u32;
        result.lakes += province.lakes().len() as u32;
        for lake in province.lakes() {
            result.lake_cells += u64::from(lake.cells);
        }
        let (origin_q, origin_r) = province_origin(pq, pr);
        for y in 0..PROVINCE_CELL {
            for x in 0..PROVINCE_CELL {
                let (q, r) = (origin_q + x, origin_r + y);
                let head = province.head(q, r).expect("own cell");
                result.cells += 1;
                height_total += i64::from(head);
                result.min_quanta = result.min_quanta.min(head);
                result.max_quanta = result.max_quanta.max(head);

                for (dq, dr) in DIRECTIONS {
                    if let Some(neighbour) = province.head(q + dq, r + dr) {
                        let step = (head - neighbour).abs();
                        result.slope_histogram[slope_bucket(step)] += 1;
                        result.total_edges += 1;
                        if step <= crate::scale::MAX_WALK_STEP_QUANTA {
                            result.walkable_edges += 1;
                        }
                        if step <= crate::scale::MAX_BUILD_STEP_QUANTA {
                            result.buildable_edges += 1;
                        }
                    }
                }

                if let Some(channel) = province.channel(q, r) {
                    result.discharge_histogram[usize::from(channel.class)] += 1;
                    // A bed the rock held above the depth this class would otherwise have cut.
                    if channel.floor_mq > channel.surface_mq - bed_depth(channel.class) * MQ as i32
                    {
                        result.river_sills += 1;
                    }
                }

                match province.flow(q, r) {
                    Some(Flow::To(direction)) => {
                        let (dq, dr) = DIRECTIONS[direction as usize];
                        let (nq, nr) = (q + dq, r + dr);
                        let neighbour = province.head(nq, nr).expect("halo covers one ring");
                        if neighbour > head {
                            result.uphill_edges += 1;
                        }
                        // Strict decrease in one total order is what forbids a cycle, so the
                        // survey checks the order rather than walking every chain to prove it.
                        // The order is the one flow is decided in, milli-quanta, because two cells
                        // can share a published quantum without being at the same height.
                        let here_mq = province.head_mq(q, r).expect("own cell");
                        let there_mq = province.head_mq(nq, nr).expect("halo covers one ring");
                        if (there_mq, nq, nr) >= (here_mq, q, r) {
                            result.cycles += 1;
                        }
                        if let (Some(here), Some(there)) =
                            (province.channel(q, r), province.channel(nq, nr))
                        {
                            result.river_edges += 1;
                            let rise = there.surface_mq - here.surface_mq;
                            if rise > 0 {
                                result.river_rises += 1;
                                result.river_rise_max_mq = result.river_rise_max_mq.max(rise);
                            } else if -rise >= crate::scale::WADE_LIMIT_QUANTA * MQ as i32 {
                                result.river_falls += 1;
                                result.river_fall_max_mq = result.river_fall_max_mq.max(-rise);
                            }
                        }
                    }
                    Some(Flow::Frontier) => result.frontier_basins += 1,
                    _ => {}
                }
            }
        }
    }
    if result.cells > 0 {
        result.mean_quanta = (height_total / result.cells as i64) as i32;
    }

    // Water and walk termination, sampled: every 16th cell in each direction, which is 1/256 of
    // the sweep and still tens of thousands of starts.
    for &(pq, pr) in &provinces {
        let (origin_q, origin_r) = province_origin(pq, pr);
        for y in (0..PROVINCE_CELL).step_by(4) {
            for x in (0..PROVINCE_CELL).step_by(4) {
                let (q, r) = (origin_q + x, origin_r + y);
                match terra.water(q, r) {
                    Water::Sea { .. } => result.sea_cells += 16,
                    Water::Lake { .. } => result.lake_water_cells += 16,
                    Water::River { .. } => result.river_cells += 16,
                    Water::Dry => {}
                }
            }
        }
        for y in (0..PROVINCE_CELL).step_by(16) {
            for x in (0..PROVINCE_CELL).step_by(16) {
                let (mut q, mut r) = (origin_q + x, origin_r + y);
                result.walks += 1;
                let mut steps = 0u32;
                loop {
                    // Stopping at the sample's edge is what keeps the survey's cost the size of
                    // the square it was asked about: a walk that followed a river out of the
                    // sample would solve provinces nobody asked to see.
                    let (wq, wr) = province_of(q, r);
                    if wq < cq - half
                        || wr < cr - half
                        || wq >= cq + span - half
                        || wr >= cr + span - half
                    {
                        result.left_survey += 1;
                        break;
                    }
                    if terra.head(q, r) < SEA_LEVEL_QUANTA {
                        result.reached_sea += 1;
                        break;
                    }
                    match terra.flow(q, r) {
                        Flow::To(direction) => {
                            let (dq, dr) = DIRECTIONS[direction as usize];
                            q += dq;
                            r += dr;
                            steps += 1;
                            if steps >= WALK_BUDGET {
                                result.walk_budget_exhausted += 1;
                                break;
                            }
                        }
                        Flow::Lake(_) => {
                            result.reached_lake += 1;
                            break;
                        }
                        Flow::Frontier => {
                            result.reached_frontier += 1;
                            break;
                        }
                    }
                }
                result.longest_walk = result.longest_walk.max(steps);
            }
        }
    }
    // Relief at the scale the camera frames. Centres are held one viewport inside the surveyed
    // square so that every disc is answered from provinces this survey already solved.
    let (low_q, low_r) = province_origin(cq - half, cr - half);
    let side = span * PROVINCE_CELL;
    let mut views: Vec<i32> = Vec::new();
    let mut centre_r = VIEWPORT_CELL;
    while centre_r < side - VIEWPORT_CELL {
        let mut centre_q = VIEWPORT_CELL;
        while centre_q < side - VIEWPORT_CELL {
            let origin = (low_q + centre_q, low_r + centre_r);
            let mut low = i32::MAX;
            let mut high = i32::MIN;
            for cell in hexes_in_radius(origin, VIEWPORT_CELL) {
                let head = terra.head(cell.0, cell.1);
                low = low.min(head);
                high = high.max(head);
            }
            views.push(high - low);
            centre_q += VIEWPORT_CELL;
        }
        centre_r += VIEWPORT_CELL;
    }
    if !views.is_empty() {
        views.sort_unstable();
        result.viewport_relief_median = views[views.len() / 2];
        result.viewport_relief_p10 = views[views.len() / 10];
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        result.sweep_micros = sweep_started.elapsed().as_micros();
    }
    result
}

fn slope_bucket(difference: i32) -> usize {
    match difference {
        0 => 0,
        1 => 1,
        2..=3 => 2,
        4..=7 => 3,
        8..=15 => 4,
        _ => 5,
    }
}

/// Height quanta as a readable metre figure, to one decimal place.
fn metres(quanta: i32) -> String {
    let tenths = i64::from(quanta) * i64::from(crate::scale::HEIGHT_QUANTUM_MM) / 100;
    format!("{}.{}", tenths / 10, (tenths % 10).abs())
}

pub fn format_report(survey: &TerraSurvey) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "terra prototype | seed {} | centre ({},{}) | {} provinces | {} cells\n",
        survey.seed, survey.centre.0, survey.centre.1, survey.provinces, survey.cells
    ));
    out.push_str(&format!(
        "  elevation      {} m to {} m, mean {} m\n",
        metres(survey.min_quanta),
        metres(survey.max_quanta),
        metres(survey.mean_quanta)
    ));
    let labels = ["0", "1", "2-3", "4-7", "8-15", "16+"];
    let edges: u64 = survey.slope_histogram.iter().sum::<u64>().max(1);
    out.push_str("  slope (quanta between neighbours)\n");
    for (label, count) in labels.iter().zip(survey.slope_histogram.iter()) {
        out.push_str(&format!(
            "    {label:>5}  {count:>12}  {:>4} per mille\n",
            count * 1_000 / edges
        ));
    }
    out.push_str(&format!(
        "  terrain        {} per mille walkable at {} quanta, {} per mille buildable at {}\n",
        survey.walkable_per_mille(),
        crate::scale::MAX_WALK_STEP_QUANTA,
        survey.buildable_per_mille(),
        crate::scale::MAX_BUILD_STEP_QUANTA
    ));
    out.push_str(&format!(
        "  viewport       {} m of relief across {} m, flattest tenth {} m\n",
        metres(survey.viewport_relief_median),
        i64::from(VIEWPORT_CELL * 2) * i64::from(crate::scale::CELL_SPACING_MM) / 1_000,
        metres(survey.viewport_relief_p10)
    ));
    out.push_str(&format!(
        "  discharge class (channel cells, {} per mille of the sample)\n",
        survey.channel_per_mille()
    ));
    for (class, count) in survey.discharge_histogram.iter().enumerate() {
        if *count == 0 {
            continue;
        }
        out.push_str(&format!(
            "    {class:>5}  {count:>12}  water {} cells, sand bench {}, valley half-width {}, bed {} m\n",
            river_half_width(class as u8) * 2 + 1,
            river_bench_width(class as u8),
            valley_half_width(class as u8),
            metres(bed_depth(class as u8))
        ));
    }
    out.push_str(&format!(
        "  hydrology      {} springs, {} lakes over {} cells, {} frontier basins\n",
        survey.springs, survey.lakes, survey.lake_cells, survey.frontier_basins
    ));
    out.push_str(&format!(
        "  water          {} per mille wet (sea {}, lake {}, river {})\n",
        survey.water_per_mille(),
        survey.sea_cells,
        survey.lake_water_cells,
        survey.river_cells
    ));
    out.push_str(&format!(
        "  invariants     {} cycles, {} uphill edges, {} of {} river edges rise downstream (worst {} mm)\n",
        survey.cycles,
        survey.uphill_edges,
        survey.river_rises,
        survey.river_edges,
        i64::from(survey.river_rise_max_mq) * i64::from(crate::scale::HEIGHT_QUANTUM_MM) / MQ
    ));
    out.push_str(&format!(
        "  geology        {} channel cells sit on a sill, {} edges fall past the wade limit (deepest {} mm)\n",
        survey.river_sills,
        survey.river_falls,
        i64::from(survey.river_fall_max_mq) * i64::from(crate::scale::HEIGHT_QUANTUM_MM) / MQ
    ));
    out.push_str(&format!(
        "  drainage walks {} starts: {} to sea, {} to lake, {} to frontier, {} off the sample, {} unterminated, longest {}\n",
        survey.walks,
        survey.reached_sea,
        survey.reached_lake,
        survey.reached_frontier,
        survey.left_survey,
        survey.walk_budget_exhausted,
        survey.longest_walk
    ));
    let per_province = survey.solve_micros / u128::from(survey.provinces.max(1));
    out.push_str(&format!(
        "  cost           {} ms to solve ({} ms per province), {} ms to sweep\n",
        survey.solve_micros / 1_000,
        per_province / 1_000,
        survey.sweep_micros / 1_000
    ));
    out
}

pub fn format_json(survey: &TerraSurvey) -> String {
    serde_json::json!({
        "seed": survey.seed,
        "centre_pq": survey.centre.0,
        "centre_pr": survey.centre.1,
        "provinces": survey.provinces,
        "cells": survey.cells,
        "min_quanta": survey.min_quanta,
        "max_quanta": survey.max_quanta,
        "mean_quanta": survey.mean_quanta,
        "slope_histogram": survey.slope_histogram,
        "walkable_per_mille": survey.walkable_per_mille(),
        "buildable_per_mille": survey.buildable_per_mille(),
        "channel_per_mille": survey.channel_per_mille(),
        "springs": survey.springs,
        "lakes": survey.lakes,
        "lake_cells": survey.lake_cells,
        "frontier_basins": survey.frontier_basins,
        "cycles": survey.cycles,
        "uphill_edges": survey.uphill_edges,
        "river_edges": survey.river_edges,
        "river_rises": survey.river_rises,
        "river_rise_max_mq": survey.river_rise_max_mq,
        "river_sills": survey.river_sills,
        "river_falls": survey.river_falls,
        "river_fall_max_mq": survey.river_fall_max_mq,
        "discharge_histogram": survey.discharge_histogram,
        "sea_cells": survey.sea_cells,
        "lake_water_cells": survey.lake_water_cells,
        "river_cells": survey.river_cells,
        "water_per_mille": survey.water_per_mille(),
        "walks": survey.walks,
        "reached_sea": survey.reached_sea,
        "reached_lake": survey.reached_lake,
        "reached_frontier": survey.reached_frontier,
        "left_survey": survey.left_survey,
        "walk_budget_exhausted": survey.walk_budget_exhausted,
        "longest_walk": survey.longest_walk,
        "viewport_cell": VIEWPORT_CELL,
        "viewport_relief_median": survey.viewport_relief_median,
        "viewport_relief_p10": survey.viewport_relief_p10,
        "solve_micros": survey.solve_micros,
        "sweep_micros": survey.sweep_micros,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: u32 = 0x5EED_A17E;

    /// The survey is the falsification instrument for a generated world, so it has to run and it
    /// has to report the invariants as clean. One province keeps this quick; three is the smallest
    /// sample with a seam in it, and is where the relief and grade-line claims are checked.
    #[test]
    fn the_survey_runs_reports_and_finds_the_invariants_clean() {
        let result = survey(SEED, 1);
        assert_eq!(result.provinces, 1);
        assert_eq!(result.cells, (PROVINCE_CELL * PROVINCE_CELL) as u64);
        assert_eq!(result.cycles, 0);
        assert_eq!(result.uphill_edges, 0);
        assert_eq!(result.walk_budget_exhausted, 0);
        assert!(result.invariants_hold());
        assert!(result.max_quanta > result.min_quanta);
        assert!(!format_report(&result).is_empty());
        assert!(format_json(&result).contains("\"uphill_edges\":0"));

        // Relief has to be there locally, or the world is a single smooth ramp with nothing to walk
        // around. Three provinces is 2.1 km, six per cent of a continental wavelength; asking that
        // for hundreds of metres would only be asking for a noisier generator.
        let wider = survey(SEED, 3);
        let local = wider.max_quanta - wider.min_quanta;
        assert!(
            local > 100,
            "only {local} quanta of relief across three provinces"
        );

        // The grade line is the point of world 14. A few edges may round up at a confluence; a
        // metre of rise would mean a river running uphill.
        assert!(
            wider.river_edges > 0 && wider.river_rise_max_mq < MQ as i32,
            "{} of {} river edges rise, worst {} milli-quanta",
            wider.river_rises,
            wider.river_edges,
            wider.river_rise_max_mq
        );

        // A fall needs somewhere to fall to, and [`SEED`]'s origin is seabed: every channel there
        // is drowned, so the steps are under water and none of them is a drop a walker meets. The
        // rock and fall claims belong on dry land, which is what this one province of upland is.
        // World 16 deliberately removes class-1 gullies; this sample keeps the variable-hardness
        // assertion on a river that survives that hierarchy instead of pinning a discarded twig.
        //
        // Nine provinces rather than one. Sills are what the small channels do, and the one province
        // at this centre now carries nothing but two trunks: recalibrating the discharge ladder onto
        // catchments this generator actually produces moved it from classes 2 and 3 to 6 and 7,
        // which by design cut nearly everything. Asking one province for a sill would be asking the
        // sample to hold a headwater it does not have.
        let upland = survey(1_213_486_160, 3);
        assert!(
            upland.river_sills > 0 && upland.river_falls > 0,
            "{} sills and {} falls in a province of upland",
            upland.river_sills,
            upland.river_falls
        );
        assert!(upland.river_fall_max_mq >= crate::scale::WADE_LIMIT_QUANTA * MQ as i32);
    }
}
