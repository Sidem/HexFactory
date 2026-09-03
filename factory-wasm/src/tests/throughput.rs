use super::*;

#[test]
fn capacity_workload_is_deterministic_and_actually_produces() {
    let spec = capacity::quick_tiers()[1];
    let mut first = capacity::warm_core(&spec);
    let mut second = capacity::warm_core(&spec);
    first.advance_ticks(120);
    second.advance_ticks(120);
    assert_eq!(first.checksum(), second.checksum());
    // Pinned so a change to definitions, the workload, or the simulation cannot silently
    // invalidate comparisons against previously recorded tier numbers. A generator-version
    // bump moves this number while the workload does not — as did v0.14 adding the splitter's
    // and merger's arbitration cursors to `checksum` — which is why the delivered total and
    // the entity count below are the assertions that say the run is the same run.
    //
    // 841_205_484 → 3_799_495_709 when sand left the ocean gate and sat on the shore band.
    // The workload's shape, entity count, and delivered total did not move.
    //
    // 3_799_495_709 → 2_222_187_037 when a belt began holding what it was handed until the tick
    // after — see `just_received`. The workload's shape and entity count did not move; a line's
    // cargo now spends a tick on each belt it crosses instead of the whole line in one, so this
    // window's delivered total is the one below.
    //
    // 2_222_187_037 → 3_614_679_184 when project progress moved off the board slot and onto the
    // project, so `checksum` hashes `request_delivered` instead of a per-slot count. The
    // workload's shape, entity count, and delivered total did not move.
    //
    // 3_614_679_184 → 23_080_823 when limestone entered the site table and world generator 9
    // entered the checksum. The workload's shape, entity count, and delivered total did not move.
    // Petroleum roads adds the oil site rule and world generator 10; the transport workload
    // and its delivered total remain unchanged.
    //
    // 1_951_253_762 → 360_047_202 when machines took their physical footprints and the line was
    // respaced around them. The entity count, the chain's hop count and the delivered total did
    // not move — only the empty ground between the machines, and so their coordinates.
    //
    // 360_047_202 → 3_227_239_126 when a belt became 5.37 m of conveyor an item takes
    // `BELT_TRANSIT_TICKS` to cross. The workload's shape, entity count and delivered total did
    // not move — the line is extraction-bound either way — but the pipeline is eight belts and
    // 216 ticks longer to fill, so the warmup moved with it and the lanes are now hashed state.
    //
    // 3_227_239_126 → 2_303_878_214 when a survey began opening a disc around the player's own
    // hex instead of a ring of the chunk lattice. `generated_chunks` is hashed, and this tier
    // opens a different set of them — the same world either way, since `tier_scenario` sets
    // `generated_environment: false` and there is no terrain here to change. The workload's
    // shape, entity count and delivered total did not move.
    //
    // 2_303_878_214 → 1_013_018_297 when the same pass moved `WORLD_GENERATOR_VERSION` to 12,
    // which `checksum_for_world` hashes first. Nothing in the workload moved with it; this is
    // the stamp, not the state.
    // 1_013_018_297 → 1_628_779_640 when noise-shaped site rims moved the stamp to 13. The
    // measurement scenario has generation disabled, so again only the version input changed.
    // 1_628_779_640 → 1_229_625_283 when the graded river profile moved the stamp to 14. Same
    // reason: this scenario generates no terrain, so only the version input changed.
    assert_eq!(first.checksum(), 1_229_625_283);
    assert_eq!(first.entities.len(), spec.entities() as usize);
    // Every line must be running end to end, or the tiers would measure an idle blueprint.
    // Four per line rather than fourteen: the line is now extraction-bound, because a
    // tier-one extractor spends 30 ticks per ore against the 5 this workload was calibrated
    // against. The ladder still times the same entity count moving the same cargo, but a tier
    // number recorded before this change was measured at a different cargo cadence and is not
    // comparable — `docs/BENCHMARKS.md` says so beside the affected rows.
    assert_eq!(first.delivered, u64::from(spec.lines) * 4);
}

/// Every figure the ladder reports is arithmetic over a clock it was handed, so the whole of it
/// can be pinned without depending on how long a machine actually takes.
#[test]
fn capacity_is_measured_per_phase_and_per_tier_against_a_supplied_clock() {
    // Capacity phases are reported per sample against the supplied clock.
    let spec = capacity::quick_tiers()[0];
    let clock = StepClock {
        // Each phase reads the clock exactly twice, so one phase always spans one step.
        step_us: 1_000.0,
        readings: std::cell::Cell::new(0),
    };
    let tier = capacity::measure_tier_with(&spec, &clock, capacity::Budget::FIXED);
    // The tick phase spans one 1,000 µs step across `measured_ticks` samples.
    assert_eq!(tier.measured_ticks, spec.measured_ticks);
    assert_eq!(tier.tick_us, 1_000.0 / f64::from(spec.measured_ticks));
    assert_eq!(tier.frame_us, 1_000.0 / f64::from(spec.frames));
    assert_eq!(tier.snapshot_us, 1_000.0 / f64::from(spec.snapshots));
    assert_eq!(tier.ticks_per_second, 1e6 / tier.tick_us);
    // Every phase read the clock, and the workload itself is unchanged by the clock swap.
    assert_eq!(tier.entities, spec.entities() as usize);
    // Seven phases, each spanning exactly one pair of readings.
    assert_eq!(clock.readings.get(), 14);

    // A coarse clock must buy precision with more samples and nothing else: the tier's identity
    // has to survive, or a browser record could not be compared against a native one.
    let spec = capacity::quick_tiers()[1];
    let fixed = capacity::measure_tier_with(
        &spec,
        capacity::default_clock().as_ref(),
        capacity::Budget::FIXED,
    );
    // A step clock that only ever reports 500 µs per reading forces four repeats to reach a
    // 2,000 µs budget, without depending on how fast this machine is.
    let clock = StepClock {
        step_us: 500.0,
        readings: std::cell::Cell::new(0),
    };
    let budgeted = capacity::measure_tier_with(
        &spec,
        &clock,
        capacity::Budget {
            min_phase_us: 2_000.0,
        },
    );
    assert_eq!(budgeted.measured_ticks, spec.measured_ticks * 4);
    assert_eq!(
        budgeted.tick_us,
        2_000.0 / f64::from(budgeted.measured_ticks)
    );
    // The recorded identity of the tier is untouched by the extra samples.
    assert_eq!(budgeted.checksum, fixed.checksum);
    assert_eq!(budgeted.delivered, fixed.delivered);
    assert_eq!(budgeted.entities, fixed.entities);
    assert_eq!(budgeted.tiles, fixed.tiles);

    // Capacity ladder measures tiers independently and reports its platform.
    let specs = capacity::quick_tiers();
    let mut ladder = capacity::Ladder::new(specs.clone());
    let clock = capacity::default_clock();
    assert_eq!(ladder.len(), specs.len());
    assert!(ladder.measure(specs.len(), clock.as_ref()).is_none());
    // A partial run reports only what it measured, so an interrupted browser run still yields
    // an honest record rather than empty tiers.
    let first = ladder
        .measure(0, clock.as_ref())
        .expect("first tier measures");
    assert_eq!(ladder.report().tiers.len(), 1);
    // Re-measuring a tier replaces it instead of recording the same tier twice.
    let again = ladder
        .measure(0, clock.as_ref())
        .expect("first tier re-measures");
    assert_eq!(again.checksum, first.checksum);
    assert_eq!(ladder.report().tiers.len(), 1);

    ladder.measure(1, clock.as_ref()).expect("second tier");
    let report = ladder.report();
    assert_eq!(report.tiers.len(), 2);
    assert_eq!(report.platform, "native");
    assert_eq!(report.schema, capacity::REPORT_SCHEMA);
    assert!(capacity::format_table(&report).contains("native"));

    // The browser harness drives this factory over the ordinary worker RPC, so it must arrive in
    // the same steady state the in-wasm phases measure, and its first delta must be a complete
    // snapshot the host can adopt.
    let spec = capacity::quick_tiers()[1];
    let mut factory = capacity::warm_factory(&spec);
    let warm = capacity::warm_core(&spec);
    assert_eq!(factory.checksum(), warm.checksum());
    assert!(warm.delivered > 0);

    let first: serde_json::Value =
        serde_json::from_str(&factory.snapshot_delta_json()).expect("delta parses");
    assert_eq!(first["base_revision"], 0);
    assert_eq!(first["revision"], 1);
    assert_eq!(first["buildings"]["replace"], true);
    assert_eq!(
        first["buildings"]["changed"]
            .as_array()
            .expect("a first delta carries the complete blueprint")
            .len(),
        spec.entities() as usize
    );

    factory
        .advance_json("[{\"type\":\"move_intent\",\"x\":0,\"y\":0}]", 1, 0)
        .expect("idle batch is accepted");
    let next: serde_json::Value =
        serde_json::from_str(&factory.snapshot_delta_json()).expect("delta parses");
    assert_eq!(next["base_revision"], 1);
    assert_eq!(next["revision"], 2);
    // The steady-state delta is a patch, not another complete blueprint: `replace` is skipped
    // when false, and only the entities that moved travel.
    assert!(next["buildings"]["replace"].is_null());
    let changed = next["buildings"]["changed"]
        .as_array()
        .expect("a steady-state frame changes entities");
    assert!(!changed.is_empty() && changed.len() < spec.entities() as usize);

    // Capacity ladder reports a result for every tier.
    let specs = capacity::quick_tiers();
    let report = capacity::run(&specs);
    assert_eq!(report.schema, capacity::REPORT_SCHEMA);
    assert_eq!(report.tiers.len(), specs.len());
    for (tier, spec) in report.tiers.iter().zip(&specs) {
        assert_eq!(tier.entities, spec.entities() as usize);
        assert!(tier.tick_us > 0.0);
        assert!(tier.frame_us > 0.0);
        assert!(tier.full_compile_us > 0.0);
        assert!(tier.incremental_recompile_us > 0.0);
        assert!(tier.edit_us > 0.0);
        // A steady-state frame always carries at least the tick's changed groups.
        assert!(tier.delta_bytes > 0.0);
    }
    let table = capacity::format_table(&report);
    assert!(specs.iter().all(|spec| table.contains(spec.key)));
    assert!(capacity::format_json(&report).contains("\"schema\""));
}
