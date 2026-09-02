use super::*;
use std::time::Instant;

#[derive(Debug, Serialize)]
pub struct Report {
    pub seed: u32,
    pub epoch_ticks: u64,
    pub chunk_budget: usize,
    pub cell_budget: usize,
    pub edge_budget: usize,
    pub change_budget: usize,
    pub surveyed_chunks: usize,
    pub accelerated_epochs: u32,
    pub chunks: usize,
    pub cells: usize,
    pub edges: usize,
    pub bends: usize,
    pub stressed_banks: usize,
    pub changes: usize,
    pub truncated: bool,
    pub elapsed_micros: u128,
    pub save_load_checksum_stable: bool,
}

pub fn run() -> Report {
    const SEED: u32 = 1_213_486_160;
    const MAX_ACCELERATED_EPOCHS: u32 = 512;
    let definitions: DefinitionsInput =
        serde_json::from_str(include_str!("../../src/data/definitions.json")).unwrap();
    let technologies: TechnologiesInput =
        serde_json::from_str(include_str!("../../src/data/technologies.json")).unwrap();
    let scenarios: ScenariosInput =
        serde_json::from_str(include_str!("../../src/data/scenarios.json")).unwrap();
    let scenario = scenarios
        .scenarios
        .iter()
        .find(|scenario| scenario.key == "new-game")
        .unwrap();
    let mut core = Core::new(&definitions, &technologies, scenario, Some(SEED), None).unwrap();
    // Measurement-only survey window: production still opens chunks only through player survey.
    for chunk_r in -5..=5 {
        for chunk_q in -5..=5 {
            core.generate_chunk(chunk_q, chunk_r);
        }
    }
    let started = Instant::now();
    let mut report = geomorphology::EpochReport::default();
    let mut epochs = 0;
    while epochs < MAX_ACCELERATED_EPOCHS && report.changes == 0 {
        report = core.run_geomorphic_epoch();
        epochs += 1;
    }
    let elapsed_micros = started.elapsed().as_micros();
    let saved = core.save_string().unwrap();
    let restored = Core::from_save(&definitions, &technologies, &scenarios, &saved).unwrap();
    Report {
        seed: SEED,
        epoch_ticks: geomorphology::EPOCH_TICKS,
        chunk_budget: geomorphology::CHUNK_BUDGET,
        cell_budget: geomorphology::CELL_BUDGET,
        edge_budget: geomorphology::EDGE_BUDGET,
        change_budget: geomorphology::CHANGE_BUDGET,
        surveyed_chunks: core.generated_chunks.len(),
        accelerated_epochs: epochs,
        chunks: report.chunks,
        cells: report.cells,
        edges: report.edges,
        bends: report.bends,
        stressed_banks: report.stressed_banks,
        changes: report.changes,
        truncated: report.truncated,
        elapsed_micros,
        save_load_checksum_stable: restored.checksum() == core.checksum(),
    }
}

pub fn format(report: &Report) -> String {
    serde_json::to_string_pretty(report).expect("erosion benchmark serializes")
}
