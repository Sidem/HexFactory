//! Runs the deterministic capacity ladder and prints measured tiers.
//!
//! `npm run bench` builds this with the same release profile as the shipped wasm artifact. Results
//! are recorded in `docs/BENCHMARKS.md`; this binary never ships to the browser.

use std::fs;
use std::process::ExitCode;

use factory_wasm::capacity;

fn main() -> ExitCode {
    let mut json_path: Option<String> = None;
    let mut specs = capacity::default_tiers();
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--quick" => specs = capacity::quick_tiers(),
            "--json" => match arguments.next() {
                Some(path) => json_path = Some(path),
                None => return usage("--json requires a path"),
            },
            "--help" | "-h" => {
                println!("usage: capacity [--quick] [--json <path>]");
                return ExitCode::SUCCESS;
            }
            other => return usage(&format!("unknown argument {other}")),
        }
    }

    eprintln!("measuring {} capacity tiers", specs.len());
    println!("{}", capacity::table_header());
    let report = capacity::run_with(&specs, |tier| println!("{}", capacity::table_row(tier)));

    if let Some(path) = json_path {
        if let Err(error) = fs::write(&path, format!("{}\n", capacity::format_json(&report))) {
            eprintln!("could not write {path}: {error}");
            return ExitCode::FAILURE;
        }
        eprintln!("wrote {path}");
    }
    ExitCode::SUCCESS
}

fn usage(message: &str) -> ExitCode {
    eprintln!("{message}");
    eprintln!("usage: capacity [--quick] [--json <path>]");
    ExitCode::FAILURE
}
