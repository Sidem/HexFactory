use super::*;
use std::time::Instant;

#[derive(Debug, Serialize)]
pub struct Report {
    pub seed: u32,
    pub command_quanta: u16,
    pub active_cells: usize,
    pub sweeps: u32,
    pub transfers: u64,
    pub frontier_quanta: i64,
    pub settled: bool,
    pub active_micros: u128,
    pub quiet_ticks: u32,
    pub quiet_micros: u128,
    pub quiet_water_dirty: bool,
    pub quiet_state_changed: bool,
}

pub fn run() -> Report {
    const SEED: u32 = 1_213_486_160;
    const QUIET_TICKS: u32 = 100_000;
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
    let size = core.scenario.chunk_size;
    let (q, r) = core
        .generated_chunks
        .iter()
        .flat_map(|&(chunk_q, chunk_r)| hexes_in_chunk(chunk_q, chunk_r, size))
        .find(|&(q, r)| {
            let ground = core.generated_ground_at(q, r);
            ground.hydrology.depth_quanta == 0
                && !ground.presentation.is_water()
                && !core.terrain_blocks_movement(q, r)
        })
        .expect("the opening shelf contains dry ground");
    let started = Instant::now();
    let active = core
        .edit_water(
            q,
            r,
            hydrology::WaterAction::Flood,
            hydrology::WATER_COMMAND_LIMIT_QUANTA,
        )
        .unwrap();
    let active_micros = started.elapsed().as_micros();

    core.dirty.water = false;
    let before = core.water.clone();
    let quiet_started = Instant::now();
    core.tick_many(QUIET_TICKS);
    let quiet_micros = quiet_started.elapsed().as_micros();
    Report {
        seed: SEED,
        command_quanta: hydrology::WATER_COMMAND_LIMIT_QUANTA,
        active_cells: active.cells,
        sweeps: active.sweeps,
        transfers: active.transfers,
        frontier_quanta: active.outflow_quanta,
        settled: active.settled,
        active_micros,
        quiet_ticks: QUIET_TICKS,
        quiet_micros,
        quiet_water_dirty: core.dirty.water,
        quiet_state_changed: core.water != before,
    }
}

pub fn format(report: &Report) -> String {
    serde_json::to_string_pretty(report).expect("water benchmark serializes")
}
