//! E0 distributions, separate from the historical aggregate ladder. No simulation optimization.
use super::*;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Workload {
    Active,
    Idle,
}

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
    pub tick: Distribution,
    pub advance_encode: Distribution,
    pub tick_samples_us: Vec<f64>,
    pub advance_encode_samples_us: Vec<f64>,
    pub delta_bytes: Vec<usize>,
    pub entity_dirty_marks: Vec<usize>,
    pub resource_dirty_marks: Vec<usize>,
}

pub fn spec(entities: u32) -> TierSpec {
    assert!(matches!(entities, 768 | 3072 | 6144 | 24576));
    tier("steady", entities / 12, 120, 120, 1, 1)
}

fn factory(spec: &TierSpec, workload: Workload) -> Factory {
    let mut cold = *spec;
    cold.warmup_ticks = 0;
    let mut factory = warm_factory(&cold);
    if matches!(workload, Workload::Idle) {
        // Explicit synthetic initial state: every switchable machine is suspended before the
        // first tick. Belts and storage start empty. No player command or production change.
        for entity in &mut factory.core.entities {
            if Core::can_be_switched(entity.kind) {
                entity.disabled = true;
            }
        }
    }
    factory.core.advance_ticks(spec.warmup_ticks);
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
/// Timed runs restart from the same fixed 400-tick state, independent of warmup throughput.
pub fn measure(
    entities: u32,
    workload: Workload,
    clock: &dyn Clock,
    warmup_us: f64,
    measurement_us: f64,
) -> SteadyRun {
    assert!(warmup_us.is_finite() && warmup_us >= 0.0);
    assert!(measurement_us.is_finite() && measurement_us > 0.0);
    let spec = spec(entities);
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
    let started = clock.now_us();
    loop {
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
    SteadyRun {
        schema: 1,
        workload,
        entities,
        seed: 2_071_003_907,
        warmup_ticks: spec.warmup_ticks,
        thermal_warmup_us,
        requested_measurement_us: measurement_us,
        elapsed_us,
        setup_us,
        start_checksum,
        end_checksum,
        start_delivered,
        end_delivered: tick.core.delivered,
        ticks: tick_samples_us.len(),
        tick: distribution(&tick_samples_us),
        advance_encode: distribution(&advance_encode_samples_us),
        tick_samples_us,
        advance_encode_samples_us,
        delta_bytes,
        entity_dirty_marks,
        resource_dirty_marks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let spec = quick_tiers()[0];
        for workload in [Workload::Active, Workload::Idle] {
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
            }
        }
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
    }

    struct TestClock(std::cell::Cell<u32>);
    impl Clock for TestClock {
        fn now_us(&self) -> f64 {
            self.0.set(self.0.get() + 10);
            f64::from(self.0.get())
        }
    }
}
