//! E0 distributions, separate from the historical aggregate ladder. No simulation optimization.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Workload {
    Active,
    Idle,
    /// Every line's sink is shut before the starting state is reached, so the measured window
    /// opens on a factory that is completely backed up, and reopens the sinks partway through.
    Blocked,
    /// The dense splitter/merger/underpass blueprint in [`junction`], running free. A different
    /// factory, not a different mode: transport here is arbitration and crossings rather than the
    /// directed chain the other three measure.
    Junction,
}

impl Workload {
    /// The blueprint this workload runs on. Three of the four are regimes of the same straight
    /// line; the junction workload is a different factory and says so here rather than anywhere
    /// that would have to infer it.
    fn layout(self) -> Layout {
        match self {
            Workload::Active | Workload::Idle | Workload::Blocked => Layout::Line,
            Workload::Junction => Layout::Junction,
        }
    }

    /// Fixed ticks this workload needs on top of the tier's own warmup before its starting state
    /// is the state it claims to measure. A jam has to finish walking back up the line first, and
    /// how long that takes is a property of the line's length and its slowest machine rather than
    /// of anything the sampler does. `a_blocked_line_reaches_a_fixed_point` pins the figure, and
    /// `junction::cargo_crosses_every_junction_and_no_lane_starves` pins the junction's.
    fn extra_warmup_ticks(self) -> u32 {
        match self {
            Workload::Active | Workload::Idle => 0,
            Workload::Blocked => SATURATION_TICKS,
            Workload::Junction => junction::SETTLE_TICKS,
        }
    }
}

/// Ticks a shut line needs to stop changing entirely: the delivery belt fills, the container
/// behind it fills, the composer's output compartment fills and its craft stalls, the belts back
/// up to the extractor, and the extractor stops on a full output.
///
/// It is the *extractor* that sets this, not the length of the line: the jam is complete when
/// every buffer along it is full, and one ore every thirty ticks is how fast they fill. Lines are
/// independent and identical, so the figure is the same at every tier. Measured, not estimated —
/// `a_blocked_line_reaches_a_fixed_point` requires that a line is still working three quarters of
/// the way through this and completely still by the end of it.
const SATURATION_TICKS: u32 = 4096;

/// Sample at which a blocked run reopens its sinks: sixty seconds of factory time at the shipped
/// 60 Hz cadence, which every recorded tier reaches inside a thirty-second window. Fixed as a
/// sample index rather than a wall-clock fraction so every run's blocked phase is the same
/// number of identical ticks, whatever the host's throughput.
const REOPEN_AFTER_TICKS: usize = 3600;

#[derive(Debug, Serialize)]
pub struct Distribution {
    pub samples: usize,
    pub median_us: f64,
    pub p95_us: f64,
    pub p99_us: f64,
    pub max_us: f64,
}

/// Nearest-rank percentiles. Raw samples remain in acquisition order in the report.
pub fn distribution(samples: &[f64]) -> Distribution {
    assert!(!samples.is_empty());
    assert!(samples.iter().all(|x| x.is_finite() && *x >= 0.0));
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank = |percent: usize| sorted[(sorted.len() * percent).div_ceil(100) - 1];
    Distribution {
        samples: sorted.len(),
        median_us: rank(50),
        p95_us: rank(95),
        p99_us: rank(99),
        max_us: sorted[sorted.len() - 1],
    }
}

/// One regime inside a run. A workload whose factory changes shape partway through does not have
/// one steady state, and blending its two into a single percentile would describe neither, so
/// each regime carries its own distributions and its own production total.
#[derive(Debug, Serialize)]
pub struct Phase {
    pub key: &'static str,
    pub first_sample: usize,
    pub ticks: usize,
    pub delivered: u64,
    pub tick: Distribution,
    pub advance_encode: Distribution,
}

#[derive(Serialize)]
pub struct SteadyRun {
    pub schema: u32,
    pub workload: Workload,
    pub entities: u32,
    pub seed: u32,
    pub warmup_ticks: u32,
    pub thermal_warmup_us: f64,
    pub requested_measurement_us: f64,
    pub elapsed_us: f64,
    pub setup_us: f64,
    pub start_checksum: u32,
    pub end_checksum: u32,
    pub start_delivered: u64,
    pub end_delivered: u64,
    pub ticks: usize,
    /// Sample index of the first tick after the sinks reopened, for a blocked run that reached
    /// it. `None` on every other workload, and on a blocked window that ended too early to reopen
    /// — which is a rejected record rather than a shorter one.
    pub reopen_tick: Option<usize>,
    /// Cost of reopening every line's sink on the isolated factory, through the same public
    /// rotate the player uses. Excluded from every sample span, like setup.
    pub reopen_us: Option<f64>,
    pub delivered_at_reopen: Option<u64>,
    pub phases: Vec<Phase>,
    pub tick: Distribution,
    pub advance_encode: Distribution,
    pub tick_samples_us: Vec<f64>,
    pub advance_encode_samples_us: Vec<f64>,
    pub delta_bytes: Vec<usize>,
    pub entity_dirty_marks: Vec<usize>,
    pub resource_dirty_marks: Vec<usize>,
}

/// The tier a workload measures at an entity count.
///
/// The count is the fixed point of comparison across the whole of E0, so it is the same four
/// figures on either blueprint and the layout supplies the repeat size rather than a divisor
/// written here. The line key is `steady` and unchanged: it is what every recorded record is in
/// terms of, and the key names the scenario.
pub fn spec(entities: u32, workload: Workload) -> TierSpec {
    assert!(matches!(entities, 768 | 3072 | 6144 | 24576));
    let (layout, key, per_repeat) = match workload.layout() {
        Layout::Line => (Layout::Line, "steady", 12),
        Layout::Junction => (
            Layout::Junction,
            "steady-junction",
            junction::ENTITIES_PER_UNIT,
        ),
    };
    assert_eq!(entities % per_repeat, 0);
    tier_on(layout, key, entities / per_repeat, 120, 120, 1, 1)
}

/// The line's last belt: the one that hands the finished component to its consumer.
fn sink_gate_q(spec: &TierSpec) -> i32 {
    EXTRACTOR_CELLS + spec.belt_span as i32 + COMPOSER_CELLS + 2
}

/// Turn every line's sink gate through the public rotate path, one edit per line.
///
/// Reversing from east lands on the one edge heading whose eight-hex output ray leaves the
/// blueprint entirely, so the belt compiles no outlet at all instead of quietly binding to a
/// neighbouring line — which the other five headings, at this row pitch, would do. Nothing here
/// trusts that geometry: every caller checks `linked_gates` outside its timed spans, so a change
/// to the blueprint fails the workload rather than silently measuring a differently shaped
/// factory. The check is not folded in here because one of the two calls is the measurement.
fn turn_gates(core: &mut Core, spec: &TierSpec, shut: bool) {
    for line in 0..spec.lines {
        core.rotate(sink_gate_q(spec), line as i32 * ROW_PITCH, shut)
            .expect("capacity sink gate rotates");
    }
}

/// How many sink gates currently compile an outlet.
fn linked_gates(core: &Core, spec: &TierSpec) -> u32 {
    (0..spec.lines)
        .filter(|line| {
            let index = core
                .entity_at(sink_gate_q(spec), *line as i32 * ROW_PITCH)
                .expect("capacity sink gate exists");
            !core.graph[index].is_empty()
        })
        .count() as u32
}

fn factory(spec: &TierSpec, workload: Workload) -> Factory {
    warmed(
        spec,
        workload,
        spec.warmup_ticks + workload.extra_warmup_ticks(),
    )
}

/// The workload's synthetic initial state, advanced to an explicit tick count. Only the tests
/// name a count of their own; every measured run takes the workload's.
fn warmed(spec: &TierSpec, workload: Workload, ticks: u32) -> Factory {
    let mut cold = *spec;
    cold.warmup_ticks = 0;
    let mut factory = warm_factory(&cold);
    match workload {
        Workload::Active | Workload::Junction => {}
        Workload::Idle => {
            // Explicit synthetic initial state: every switchable machine is suspended before the
            // first tick. Belts and storage start empty. No player command or production change.
            for entity in &mut factory.core.entities {
                if Core::can_be_switched(entity.kind) {
                    entity.disabled = true;
                }
            }
        }
        // Shut before the first tick, so nothing this workload ever measures was delivered and
        // the jam is complete by the time the starting state is reached.
        Workload::Blocked => {
            turn_gates(&mut factory.core, spec, true);
            assert_eq!(
                linked_gates(&factory.core, spec),
                0,
                "a shut line has no sink"
            );
        }
    }
    factory.core.advance_ticks(ticks);
    let _ = factory.snapshot_delta_bytes();
    factory
}

/// Both paths execute exactly one tick per sample. They have independent canonical state and
/// must end identically. Parsing the bounded command and encoding are included only in frame.
fn sample(
    tick: &mut Factory,
    frame: &mut Factory,
    clock: &dyn Clock,
) -> (f64, f64, usize, usize, usize) {
    let start = clock.now_us();
    tick.core.advance_ticks(1);
    let tick_us = clock.now_us() - start;
    let entity_marks = tick.core.dirty.entities.len();
    let resource_marks = tick.core.dirty.resources.len();
    // Publication normally consumes marks each frame. Avoid an artificial ever-growing dirty
    // vector in the isolated tick path; reset outside its span, as encoding is measured below.
    tick.core.dirty = SnapshotDirty::default();
    let start = clock.now_us();
    frame.advance_json(IDLE_COMMANDS, 1, 0).unwrap();
    let bytes = frame.snapshot_delta_bytes();
    let frame_us = clock.now_us() - start;
    (tick_us, frame_us, bytes.len(), entity_marks, resource_marks)
}

/// Wall-duration collection; setup and a five-second thermal warmup are explicitly separated.
/// Timed runs restart from the same fixed starting state, independent of warmup throughput.
pub fn measure(
    entities: u32,
    workload: Workload,
    clock: &dyn Clock,
    warmup_us: f64,
    measurement_us: f64,
) -> SteadyRun {
    measure_reopening_after(
        entities,
        workload,
        clock,
        warmup_us,
        measurement_us,
        REOPEN_AFTER_TICKS,
    )
}

fn measure_reopening_after(
    entities: u32,
    workload: Workload,
    clock: &dyn Clock,
    warmup_us: f64,
    measurement_us: f64,
    reopen_after: usize,
) -> SteadyRun {
    assert!(warmup_us.is_finite() && warmup_us >= 0.0);
    assert!(measurement_us.is_finite() && measurement_us > 0.0);
    let spec = spec(entities, workload);
    let setup_start = clock.now_us();
    let mut tick = factory(&spec, workload);
    let mut frame = factory(&spec, workload);
    let first_setup_us = clock.now_us() - setup_start;
    let warm_start = clock.now_us();
    while clock.now_us() - warm_start < warmup_us {
        sample(&mut tick, &mut frame, clock);
    }
    let thermal_warmup_us = clock.now_us() - warm_start;
    let setup_start = clock.now_us();
    tick = factory(&spec, workload);
    frame = factory(&spec, workload);
    let setup_us = first_setup_us + clock.now_us() - setup_start;
    let start_checksum = tick.core.checksum();
    assert_eq!(start_checksum, frame.core.checksum());
    let start_delivered = tick.core.delivered;
    let mut tick_samples_us = Vec::new();
    let mut advance_encode_samples_us = Vec::new();
    let mut delta_bytes = Vec::new();
    let mut entity_dirty_marks = Vec::new();
    let mut resource_dirty_marks = Vec::new();
    let mut reopen_tick = None;
    let mut reopen_us = None;
    let mut delivered_at_reopen = None;
    let started = clock.now_us();
    loop {
        if workload == Workload::Blocked && tick_samples_us.len() == reopen_after {
            delivered_at_reopen = Some(tick.core.delivered);
            let edit_start = clock.now_us();
            turn_gates(&mut tick.core, &spec, false);
            reopen_us = Some(clock.now_us() - edit_start);
            // The published factory pays the same edit, outside the reported figure: one of the
            // two is the measurement, and both have to stay canonically identical.
            turn_gates(&mut frame.core, &spec, false);
            for core in [&tick.core, &frame.core] {
                assert_eq!(linked_gates(core, &spec), spec.lines, "one sink per line");
            }
            reopen_tick = Some(tick_samples_us.len());
        }
        let (tick_us, frame_us, bytes, entities, resources) = sample(&mut tick, &mut frame, clock);
        tick_samples_us.push(tick_us);
        advance_encode_samples_us.push(frame_us);
        delta_bytes.push(bytes);
        entity_dirty_marks.push(entities);
        resource_dirty_marks.push(resources);
        if clock.now_us() - started >= measurement_us {
            break;
        }
    }
    let elapsed_us = clock.now_us() - started;
    let end_checksum = tick.core.checksum();
    assert_eq!(end_checksum, frame.core.checksum());
    let end_delivered = tick.core.delivered;
    SteadyRun {
        schema: 2,
        workload,
        entities,
        seed: 2_071_003_907,
        warmup_ticks: spec.warmup_ticks + workload.extra_warmup_ticks(),
        thermal_warmup_us,
        requested_measurement_us: measurement_us,
        elapsed_us,
        setup_us,
        start_checksum,
        end_checksum,
        start_delivered,
        end_delivered,
        ticks: tick_samples_us.len(),
        reopen_tick,
        reopen_us,
        delivered_at_reopen,
        phases: phases(
            workload,
            reopen_tick,
            &tick_samples_us,
            &advance_encode_samples_us,
            [start_delivered, delivered_at_reopen.unwrap_or_default()],
            end_delivered,
        ),
        tick: distribution(&tick_samples_us),
        advance_encode: distribution(&advance_encode_samples_us),
        tick_samples_us,
        advance_encode_samples_us,
        delta_bytes,
        entity_dirty_marks,
        resource_dirty_marks,
    }
}

/// Split a run's samples into the regimes it actually contained. One `steady` phase covers a
/// workload whose factory never changed shape; a blocked run that reopened carries `blocked` and
/// `reopened` instead. A blocked window that ended before its reopen keeps the one phase it
/// measured and no `reopen_tick`, so the packer can reject it as short rather than average it.
fn phases(
    workload: Workload,
    reopen_tick: Option<usize>,
    tick_samples_us: &[f64],
    advance_encode_samples_us: &[f64],
    [start_delivered, delivered_at_reopen]: [u64; 2],
    end_delivered: u64,
) -> Vec<Phase> {
    let phase = |key, first_sample: usize, last: usize, delivered| Phase {
        key,
        first_sample,
        ticks: last - first_sample,
        delivered,
        tick: distribution(&tick_samples_us[first_sample..last]),
        advance_encode: distribution(&advance_encode_samples_us[first_sample..last]),
    };
    let ticks = tick_samples_us.len();
    match (workload, reopen_tick) {
        (Workload::Blocked, Some(reopen)) => vec![
            phase("blocked", 0, reopen, delivered_at_reopen - start_delivered),
            phase(
                "reopened",
                reopen,
                ticks,
                end_delivered - delivered_at_reopen,
            ),
        ],
        (Workload::Blocked, None) => {
            vec![phase("blocked", 0, ticks, end_delivered - start_delivered)]
        }
        _ => vec![phase("steady", 0, ticks, end_delivered - start_delivered)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything the lines are holding: belt lanes, hand-off slots, and every compartment.
    fn cargo_on_the_line(core: &Core) -> u64 {
        core.entities
            .iter()
            .map(|entity| {
                let moving: u64 = Core::belt_contents(entity)
                    .map(|cargo| u64::from(cargo.quantity))
                    .sum();
                let stored: u64 = [
                    &entity.inventory,
                    &entity.input_inventory,
                    &entity.fuel_inventory,
                    &entity.output_inventory,
                ]
                .into_iter()
                .flat_map(|compartment| compartment.values())
                .map(|&quantity| u64::from(quantity))
                .sum();
                moving + stored
            })
            .sum()
    }

    #[test]
    fn nearest_rank_keeps_tail_outliers_and_sample_count() {
        let samples: Vec<f64> = (1..=100).rev().map(f64::from).collect();
        let summary = distribution(&samples);
        assert_eq!(summary.samples, 100);
        assert_eq!(summary.median_us, 50.0);
        assert_eq!(summary.p95_us, 95.0);
        assert_eq!(summary.p99_us, 99.0);
        assert_eq!(summary.max_us, 100.0);
        assert_eq!(distribution(&[7.0]).p99_us, 7.0);
    }

    #[test]
    fn workloads_produce_or_rest_and_tick_matches_advance_encode() {
        let line = quick_tiers()[0];
        // One junction unit, so the workload's own blueprint is driven through the same sampler
        // rather than asserted about only in `junction`'s tests.
        let unit = tier_on(Layout::Junction, "steady-junction", 1, 20, 5, 5, 6);
        for workload in [
            Workload::Active,
            Workload::Idle,
            Workload::Blocked,
            Workload::Junction,
        ] {
            let spec = if workload.layout() == Layout::Junction {
                unit
            } else {
                line
            };
            let mut tick = factory(&spec, workload);
            let mut frame = factory(&spec, workload);
            let before = tick.core.delivered;
            let clock = TestClock(std::cell::Cell::new(0));
            for _ in 0..120 {
                let (tick_us, frame_us, bytes, _, _) = sample(&mut tick, &mut frame, &clock);
                assert_eq!((tick_us, frame_us), (10.0, 10.0));
                assert!(bytes > 0);
                assert_eq!(tick.core.checksum(), frame.core.checksum());
            }
            match workload {
                Workload::Active => {
                    assert!(tick.core.delivered > before);
                    assert_eq!(tick.core.delivered, 4);
                    assert_eq!(tick.core.checksum(), 1_483_737_616);
                }
                Workload::Idle => {
                    assert_eq!(tick.core.delivered, 0);
                    assert_eq!(tick.core.checksum(), 3_427_945_081);
                    assert!(tick
                        .core
                        .entities
                        .iter()
                        .all(|entity| entity.cargo.is_none()
                            && entity.lane.is_empty()
                            && entity.progress == 0));
                }
                Workload::Blocked => {
                    assert_eq!((before, tick.core.delivered), (0, 0));
                    assert_eq!(tick.core.checksum(), 2_518_484_691);
                    assert_eq!(cargo_on_the_line(&tick.core), 96);
                }
                // A saturated trunk at one item every five ticks, plus the unmerged crossing lane
                // at one every thirty: twenty-eight items over a hundred and twenty ticks, every
                // one of them past a merger or an underpass pair and a splitter.
                Workload::Junction => {
                    assert!(tick.core.delivered > before);
                    assert_eq!(tick.core.delivered - before, 24 + 4);
                    assert_eq!(tick.core.checksum(), 3_505_976_921);
                }
            }
        }
    }

    /// The saturation half of the workload's claim: a shut line is still working most of the way
    /// through the saturation warmup, and by the time the blocked starting state is reached it has
    /// stopped entirely — no delivery, not one publishable change, and not one entity that differs
    /// after six hundred more ticks, for as long as it is left shut.
    #[test]
    fn a_blocked_line_reaches_a_fixed_point() {
        let spec = quick_tiers()[0];
        let mut early = warmed(
            &spec,
            Workload::Blocked,
            spec.warmup_ticks + SATURATION_TICKS * 3 / 4,
        );
        assert_eq!(linked_gates(&early.core, &spec), 0);
        early.core.dirty = SnapshotDirty::default();
        // A whole extractor cycle, so this reads the line's pace rather than one arbitrary tick.
        early.core.advance_ticks(60);
        assert!(
            !early.core.dirty.entities.is_empty(),
            "a line already still three quarters of the way through SATURATION_TICKS means the \
             constant is far larger than the jam it waits for"
        );

        let mut blocked = factory(&spec, Workload::Blocked);
        assert_eq!(linked_gates(&blocked.core, &spec), 0);
        assert_eq!(blocked.core.delivered, 0);
        let held = cargo_on_the_line(&blocked.core);
        assert!(held > 0);
        let settled = blocked.core.entities.clone();
        blocked.core.dirty = SnapshotDirty::default();
        blocked.core.advance_ticks(600);
        assert!(blocked.core.dirty.entities.is_empty());
        assert!(blocked.core.dirty.resources.is_empty());
        assert_eq!(blocked.core.delivered, 0);
        assert_eq!(cargo_on_the_line(&blocked.core), held);
        // Nothing moved at all: a jammed factory is a fixed point, not a slowly drifting one.
        assert_eq!(blocked.core.entities, settled);
    }

    /// The resumption half: reopening the sinks restores exactly one outlet per line, the
    /// backlog drains, and the line delivers again.
    #[test]
    fn reopened_sinks_drain_the_backlog_and_deliver_again() {
        let spec = quick_tiers()[0];
        let mut blocked = factory(&spec, Workload::Blocked);
        let held = cargo_on_the_line(&blocked.core);
        turn_gates(&mut blocked.core, &spec, false);
        assert_eq!(linked_gates(&blocked.core, &spec), spec.lines);
        blocked.core.dirty = SnapshotDirty::default();
        blocked.core.advance_ticks(600);
        assert!(!blocked.core.dirty.entities.is_empty());
        assert!(blocked.core.delivered > 0);
        assert!(cargo_on_the_line(&blocked.core) < held);
        // A reopened line is the same line the active workload measures, so it converges on the
        // active workload's own production rate rather than on some drained-out one.
        let mut active = factory(&spec, Workload::Active);
        let before = active.core.delivered;
        active.core.advance_ticks(600);
        let steady = active.core.delivered - before;
        let resumed = blocked.core.delivered;
        assert!(resumed >= steady, "{resumed} < {steady}");
    }

    #[test]
    fn duration_collection_excludes_setup_and_retains_every_raw_sample() {
        let clock = TestClock(std::cell::Cell::new(0));
        let report = measure(768, Workload::Idle, &clock, 50.0, 100.0);
        assert_eq!(report.tick_samples_us, vec![10.0, 10.0]);
        assert_eq!(report.advance_encode_samples_us, vec![10.0, 10.0]);
        assert_eq!(report.ticks, 2);
        assert_eq!(report.delta_bytes.len(), 2);
        assert_eq!(report.entity_dirty_marks, vec![0, 0]);
        assert_eq!(report.resource_dirty_marks, vec![0, 0]);
        assert_eq!(report.tick.samples, 2);
        assert!(report.elapsed_us >= report.requested_measurement_us);
        assert!(report.setup_us > 0.0);
        assert!(report.thermal_warmup_us >= 50.0);
        assert_eq!(report.start_delivered, report.end_delivered);
        assert_eq!(report.reopen_tick, None);
        let phases = &report.phases;
        assert_eq!(phases.len(), 1);
        assert_eq!((phases[0].key, phases[0].first_sample), ("steady", 0));
        assert_eq!((phases[0].ticks, phases[0].delivered), (2, 0));
    }

    /// A blocked run reports its two regimes apart, and each phase's percentiles come from that
    /// phase's own samples.
    #[test]
    fn a_blocked_run_splits_its_window_at_the_reopen() {
        let clock = TestClock(std::cell::Cell::new(0));
        let report = measure_reopening_after(768, Workload::Blocked, &clock, 0.0, 300.0, 2);
        assert_eq!(report.reopen_tick, Some(2));
        assert_eq!(report.reopen_us, Some(10.0));
        assert_eq!(report.delivered_at_reopen, Some(0));
        assert_eq!(report.start_delivered, 0);
        assert!(report.ticks > 2);
        let phases = &report.phases;
        assert_eq!(phases.len(), 2);
        assert_eq!((phases[0].key, phases[0].first_sample), ("blocked", 0));
        assert_eq!((phases[0].ticks, phases[0].delivered), (2, 0));
        assert_eq!((phases[1].key, phases[1].first_sample), ("reopened", 2));
        assert_eq!(phases[1].ticks, report.ticks - 2);
        assert_eq!(
            phases[0].tick.samples + phases[1].tick.samples,
            report.tick.samples
        );
    }

    struct TestClock(std::cell::Cell<u32>);
    impl Clock for TestClock {
        fn now_us(&self) -> f64 {
            self.0.set(self.0.get() + 10);
            f64::from(self.0.get())
        }
    }
}
