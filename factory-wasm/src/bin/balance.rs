//! Reports what the shipped economy numbers actually add up to.
//!
//! `npm run balance` runs this. A cost row says what a building costs; it says nothing about what
//! its inputs cost to make, what a machine yields per minute, or how many machines a generator
//! carries — so a tuning pass argued from the data file alone is a tuning pass argued from a
//! quarter of the numbers. `fixtures/balance.json` is the recorded form and is asserted in both
//! languages; this binary is the readable one. It never ships to the browser.

use std::fs;
use std::process::ExitCode;

use factory_wasm::balance;

fn main() -> ExitCode {
    let mut json_path: Option<String> = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--json" => match arguments.next() {
                Some(path) => json_path = Some(path),
                None => return usage("--json requires a path"),
            },
            "--help" | "-h" => {
                println!("usage: balance [--json <path>]");
                return ExitCode::SUCCESS;
            }
            other => return usage(&format!("unknown argument {other}")),
        }
    }

    let report = balance::compute();
    print!("{}", balance::format_report(&report));

    if let Some(path) = json_path {
        if let Err(error) = fs::write(&path, format!("{}\n", balance::format_json(&report))) {
            eprintln!("could not write {path}: {error}");
            return ExitCode::FAILURE;
        }
        eprintln!("wrote {path}");
    }
    ExitCode::SUCCESS
}

fn usage(message: &str) -> ExitCode {
    eprintln!("{message}");
    eprintln!("usage: balance [--json <path>]");
    ExitCode::FAILURE
}
