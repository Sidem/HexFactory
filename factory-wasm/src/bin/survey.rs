//! Reports what a world parameter set actually generates.
//!
//! `npm run survey` runs this. A threshold is not a proportion — value noise is not uniformly
//! distributed — so every claim a preset makes about its own landscape comes from here rather than
//! from reading its numbers. Results are recorded in `docs/BENCHMARKS.md`; this binary never
//! ships to the browser.

use std::fs;
use std::process::ExitCode;

use factory_wasm::survey;

fn main() -> ExitCode {
    let mut json_path: Option<String> = None;
    let mut radius: Option<i32> = None;
    let mut seed = survey::default_seed();
    let mut keys: Vec<String> = Vec::new();
    let mut overrides: Vec<(String, i32)> = Vec::new();
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            // `--set water_level=26000` surveys a parameter set nobody shipped, which is how a
            // preset's numbers get chosen instead of guessed.
            "--set" => match arguments.next() {
                Some(pair) => match parse_override(&pair) {
                    Some(entry) => overrides.push(entry),
                    None => return usage("--set takes name=value"),
                },
                None => return usage("--set requires name=value"),
            },
            "--radius" => match arguments.next().and_then(|value| value.parse().ok()) {
                Some(value) => radius = Some(value),
                None => return usage("--radius requires an integer"),
            },
            "--seed" => match arguments.next().and_then(|value| value.parse().ok()) {
                Some(value) => seed = value,
                None => return usage("--seed requires an integer"),
            },
            "--json" => match arguments.next() {
                Some(path) => json_path = Some(path),
                None => return usage("--json requires a path"),
            },
            "--help" | "-h" => {
                println!(
                    "usage: survey [--seed <n>] [--radius <n>] [--set name=value]... \
                     [--json <path>] [preset...]"
                );
                println!("presets: {}", survey::preset_keys().join(", "));
                return ExitCode::SUCCESS;
            }
            other if other.starts_with('-') => return usage(&format!("unknown argument {other}")),
            other => keys.push(other.to_string()),
        }
    }
    if keys.is_empty() {
        keys = survey::preset_keys();
    }

    eprintln!(
        "surveying {} presets{}",
        keys.len(),
        match radius {
            Some(value) => format!(" at radius {value}"),
            None => " at each preset's landform radius".into(),
        }
    );
    let mut surveys = Vec::new();
    for key in &keys {
        let coarse = overrides
            .iter()
            .find(|(name, _)| name == "elevation_coarse_cell")
            .map(|&(_, cell)| cell)
            .or_else(|| survey::preset_coarse_cell(key))
            .unwrap_or(0);
        let sample = radius.unwrap_or_else(|| survey::landscape_radius(coarse));
        match survey::survey_overridden(key, &overrides, seed, sample) {
            Ok(result) => {
                print!("{}", survey::format_report(&result));
                println!();
                surveys.push(result);
            }
            Err(error) => return usage(&error),
        }
    }

    if let Some(path) = json_path {
        let body = surveys
            .iter()
            .map(survey::format_json)
            .collect::<Vec<_>>()
            .join(",\n");
        if let Err(error) = fs::write(&path, format!("[\n{body}\n]\n")) {
            eprintln!("could not write {path}: {error}");
            return ExitCode::FAILURE;
        }
        eprintln!("wrote {path}");
    }
    ExitCode::SUCCESS
}

fn parse_override(pair: &str) -> Option<(String, i32)> {
    let (name, value) = pair.split_once('=')?;
    Some((name.to_string(), value.parse().ok()?))
}

fn usage(message: &str) -> ExitCode {
    eprintln!("{message}");
    eprintln!(
        "usage: survey [--seed <n>] [--radius <n>] [--set name=value]... [--json <path>] [preset...]"
    );
    ExitCode::FAILURE
}
