//! E0 active/idle distributions. Timing stays outside CI; workload/oracle tests run in CI.
use factory_wasm::capacity::{default_clock, steady};
use std::{fs, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        eprintln!("usage: steady <active|idle|blocked> <768|3072|6144|24576> <report.json>");
        return ExitCode::FAILURE;
    }
    let workload = match args[0].as_str() {
        "active" => steady::Workload::Active,
        "idle" => steady::Workload::Idle,
        "blocked" => steady::Workload::Blocked,
        _ => return ExitCode::FAILURE,
    };
    let entities = match args[1].parse::<u32>() {
        Ok(value @ (768 | 3072 | 6144 | 24576)) => value,
        _ => return ExitCode::FAILURE,
    };
    let clock = default_clock();
    let mut runs = Vec::new();
    for index in 1..=5 {
        eprintln!("{workload:?} / {entities}: run {index}/5, warm 5 s + measure 30 s");
        runs.push(steady::measure(
            entities,
            workload,
            clock.as_ref(),
            5e6,
            30e6,
        ));
        // Checkpoint each completed run, so an interrupted collection remains reviewable.
        let report = serde_json::to_string_pretty(&runs).unwrap();
        if let Err(error) = fs::write(&args[2], report + "\n") {
            eprintln!("{}: {error}", args[2]);
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
