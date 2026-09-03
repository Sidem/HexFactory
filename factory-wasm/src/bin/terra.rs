//! Reports what the Phase 8 drainage-first prototype actually generates.
//!
//! `npm run terra` runs this. The brief for the phase makes claims about drainage — that channels
//! descend, that basins are rare and declared rather than filled away, that a query gives the same
//! answer whoever asks — and each of those is a number, not an opinion. This binary is where they
//! become numbers. Results are recorded in `docs/BENCHMARKS.md`.
//!
//! Slice 1 of the phase ships no production toggle, so nothing here is reachable from the game:
//! `factory_wasm::terra` is compiled out of the wasm artifact entirely.

use std::fs;
use std::process::ExitCode;

use factory_wasm::{terra, terra_survey};

/// The default sample: nine provinces, about 2.1 km on a side, which is wide enough to contain a
/// whole small catchment and every kind of seam.
const DEFAULT_SPAN: i32 = 3;

/// The default seed, matching the world survey's, so the two baselines describe the same run.
const DEFAULT_SEED: u32 = 1_213_486_160;

fn main() -> ExitCode {
    let mut seed = DEFAULT_SEED;
    let mut spans: Vec<i32> = Vec::new();
    let mut json_path: Option<String> = None;
    let mut centre = (0, 0);
    let mut coast = false;
    let mut landing = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--coast" => coast = true,
            "--landing" => landing = true,
            "--centre" => match arguments.next().and_then(parse_centre) {
                Some(value) => centre = value,
                None => return usage("--centre requires <pq>,<pr>"),
            },
            "--seed" => match arguments.next().and_then(|value| value.parse().ok()) {
                Some(value) => seed = value,
                None => return usage("--seed requires an integer"),
            },
            "--span" => match arguments.next().and_then(|value| value.parse().ok()) {
                Some(value) => spans.push(value),
                None => return usage("--span requires an integer number of provinces"),
            },
            "--json" => match arguments.next() {
                Some(path) => json_path = Some(path),
                None => return usage("--json requires a path"),
            },
            "--help" | "-h" => {
                println!("{USAGE}");
                println!(
                    "one province is {} cells on a side, about {} m",
                    terra::PROVINCE_CELL,
                    i64::from(terra::PROVINCE_CELL)
                        * i64::from(factory_wasm::scale::CELL_SPACING_MM)
                        / 1_000
                );
                return ExitCode::SUCCESS;
            }
            other => return usage(&format!("unknown argument {other}")),
        }
    }
    if spans.is_empty() {
        spans.push(DEFAULT_SPAN);
    }

    if coast {
        match terra::coast_province(seed) {
            Some(shore) => centre = shore,
            // Not a failure. A seed can put every coast further away than the search reaches, and
            // saying so beats quietly surveying the origin and labelling it a coast.
            None => eprintln!(
                "no coast within reach of seed {seed}; surveying ({},{}) instead",
                centre.0, centre.1
            ),
        }
    }

    if landing {
        let mut terra = terra::Terra::new(seed);
        let site = terra.landing_site();
        centre = terra::province_of(site.q, site.r);
        eprintln!(
            "landing at cell ({},{}) in province ({},{}), altitude {:.2} m",
            site.q,
            site.r,
            centre.0,
            centre.1,
            f64::from(site.bed_quanta) * 0.25
        );
        if let Some((sea, distance)) = nearest_sea_cell(&mut terra, (site.q, site.r), 512) {
            let metres =
                i64::from(distance) * i64::from(factory_wasm::scale::CELL_SPACING_MM) / 1_000;
            eprintln!(
                "nearest ocean cell is ({},{}) at {} cells (about {} m straight-line)",
                sea.0, sea.1, distance, metres
            );
        }
        if let Some((ocean, distance)) = nearest_ocean_province(seed, centre, 96) {
            let metres = i64::from(distance)
                * i64::from(terra::PROVINCE_CELL)
                * i64::from(factory_wasm::scale::CELL_SPACING_MM)
                / 1_000;
            eprintln!(
                "nearest ocean-ranked province is ({},{}) at {} macro steps (about {:.1} km straight-line)",
                ocean.0,
                ocean.1,
                distance,
                metres as f64 / 1_000.0
            );
        }
        if let Some((lake, distance)) = nearest_lake_cell(&mut terra, (site.q, site.r), 512) {
            let metres =
                i64::from(distance) * i64::from(factory_wasm::scale::CELL_SPACING_MM) / 1_000;
            let basin_cells = terra.lake_at(lake.0, lake.1).map_or(0, |info| info.cells);
            eprintln!(
                "nearest visible lake cell is ({},{}) in a {}-cell basin, at {} cells (about {:.1} km straight-line)",
                lake.0,
                lake.1,
                basin_cells,
                distance,
                metres as f64 / 1_000.0
            );
        } else {
            eprintln!("no visible lake within 512 cells of the landing");
        }
        let mut drainage = centre;
        for steps in 0..10_000 {
            match terra::province_outlet(seed, drainage.0, drainage.1) {
                terra::Outlet::Ocean => {
                    eprintln!(
                        "landing province drains to ocean province ({},{}) after {} macro steps",
                        drainage.0, drainage.1, steps
                    );
                    break;
                }
                terra::Outlet::Basin => {
                    eprintln!(
                        "landing province drains to an inland basin at ({},{}) after {} macro steps",
                        drainage.0, drainage.1, steps
                    );
                    break;
                }
                terra::Outlet::Province { pq, pr } => drainage = (pq, pr),
            }
        }
    }

    eprintln!(
        "surveying the terra prototype at seed {seed}, centred on province ({},{})",
        centre.0, centre.1
    );
    let mut surveys = Vec::new();
    let mut clean = true;
    for span in spans {
        let result = terra_survey::survey_at(seed, span, centre);
        print!("{}", terra_survey::format_report(&result));
        println!();
        // The invariants are the point of the prototype, so failing them fails the run rather than
        // printing a report that reads fine and is wrong.
        if !result.invariants_hold() {
            eprintln!(
                "drainage invariants failed at span {span}: {} cycles, {} uphill edges, {} unterminated walks",
                result.cycles, result.uphill_edges, result.walk_budget_exhausted
            );
            clean = false;
        }
        surveys.push(result);
    }

    if let Some(path) = json_path {
        let body = surveys
            .iter()
            .map(terra_survey::format_json)
            .collect::<Vec<_>>()
            .join(",\n");
        if let Err(error) = fs::write(&path, format!("[\n{body}\n]\n")) {
            eprintln!("could not write {path}: {error}");
            return ExitCode::FAILURE;
        }
        eprintln!("wrote {path}");
    }

    if clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn nearest_ocean_province(seed: u32, centre: (i32, i32), reach: i32) -> Option<((i32, i32), i32)> {
    for distance in 0..=reach {
        for dq in -distance..=distance {
            for dr in -distance..=distance {
                let axial = (dq.abs() + dr.abs() + (dq + dr).abs()) / 2;
                if axial != distance {
                    continue;
                }
                let province = (centre.0 + dq, centre.1 + dr);
                if terra::province_rank(seed, province.0, province.1)
                    < factory_wasm::scale::SEA_LEVEL_QUANTA
                {
                    return Some((province, distance));
                }
            }
        }
    }
    None
}

fn nearest_lake_cell(
    terra: &mut terra::Terra,
    centre: (i32, i32),
    reach: i32,
) -> Option<((i32, i32), i32)> {
    for distance in 0..=reach {
        for dq in -distance..=distance {
            for dr in -distance..=distance {
                let axial = (dq.abs() + dr.abs() + (dq + dr).abs()) / 2;
                if axial != distance {
                    continue;
                }
                let cell = (centre.0 + dq, centre.1 + dr);
                if matches!(terra.water(cell.0, cell.1), terra::Water::Lake { .. }) {
                    return Some((cell, distance));
                }
            }
        }
    }
    None
}

fn nearest_sea_cell(
    terra: &mut terra::Terra,
    centre: (i32, i32),
    reach: i32,
) -> Option<((i32, i32), i32)> {
    for distance in 0..=reach {
        for dq in -distance..=distance {
            for dr in -distance..=distance {
                let axial = (dq.abs() + dr.abs() + (dq + dr).abs()) / 2;
                if axial != distance {
                    continue;
                }
                let cell = (centre.0 + dq, centre.1 + dr);
                if matches!(terra.water(cell.0, cell.1), terra::Water::Sea { .. }) {
                    return Some((cell, distance));
                }
            }
        }
    }
    None
}

fn usage(message: &str) -> ExitCode {
    eprintln!("{message}");
    eprintln!("{USAGE}");
    ExitCode::FAILURE
}

const USAGE: &str = "usage: terra [--seed <n>] [--span <provinces>]... [--centre <pq>,<pr>] [--coast] [--landing] [--json <path>]";

/// `--centre 12,-4`. Separate from the span so that where a sample is taken and how big it is stay
/// independent questions.
fn parse_centre(text: String) -> Option<(i32, i32)> {
    let (left, right) = text.split_once(',')?;
    Some((left.trim().parse().ok()?, right.trim().parse().ok()?))
}
