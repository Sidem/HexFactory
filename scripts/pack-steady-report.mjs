/** Lossless E0 raw-report packaging and independently checked, readable run summaries. */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { basename } from "node:path";
import { gzipSync, gunzipSync } from "node:zlib";

const path = process.argv[2];
assert(
  path?.endsWith(".json"),
  "usage: node scripts/pack-steady-report.mjs <raw.json>",
);
const raw = readFileSync(path);
const runs = JSON.parse(raw.toString("utf8"));
assert.equal(runs.length, 5, "a complete record requires five runs");

/** Every reported percentile, recomputed here from the samples it claims to describe. */
const checkDistribution = (values, summary, ticks) => {
  assert.equal(values.length, ticks);
  assert.equal(summary.samples, ticks);
  assert(values.every((value) => Number.isFinite(value) && value >= 0));
  const sorted = [...values].sort((a, b) => a - b);
  for (const [field, percentile] of [
    ["median_us", 50],
    ["p95_us", 95],
    ["p99_us", 99],
    ["max_us", 100],
  ]) {
    assert.equal(
      summary[field],
      sorted[Math.ceil((sorted.length * percentile) / 100) - 1],
    );
  }
};

const total = (values) => values.reduce((a, b) => a + b, 0);

const summaries = runs.map((run) => {
  assert.equal(run.schema, 2);
  assert(run.thermal_warmup_us >= 5_000_000);
  assert.equal(run.requested_measurement_us, 30_000_000);
  assert(run.elapsed_us >= run.requested_measurement_us);
  assert.equal(run.start_checksum, runs[0].start_checksum);
  assert.equal(run.workload, runs[0].workload);
  assert.equal(run.entities, runs[0].entities);
  assert.equal(run.warmup_ticks, runs[0].warmup_ticks);
  assert(Number.isSafeInteger(run.ticks) && run.ticks > 0);
  checkDistribution(run.tick_samples_us, run.tick, run.ticks);
  checkDistribution(
    run.advance_encode_samples_us,
    run.advance_encode,
    run.ticks,
  );
  for (const values of [
    run.delta_bytes,
    run.entity_dirty_marks,
    run.resource_dirty_marks,
  ]) {
    assert.equal(values.length, run.ticks);
    assert(values.every((value) => Number.isSafeInteger(value) && value >= 0));
  }

  // Phases must tile the window exactly, and each one's percentiles must come from its own
  // samples rather than from the window's.
  let next = 0;
  for (const phase of run.phases) {
    assert.equal(phase.first_sample, next);
    assert(phase.ticks > 0);
    next += phase.ticks;
    const slice = (values) =>
      values.slice(phase.first_sample, phase.first_sample + phase.ticks);
    checkDistribution(slice(run.tick_samples_us), phase.tick, phase.ticks);
    checkDistribution(
      slice(run.advance_encode_samples_us),
      phase.advance_encode,
      phase.ticks,
    );
  }
  assert.equal(next, run.ticks);
  assert.equal(
    total(run.phases.map((phase) => phase.delivered)),
    run.end_delivered - run.start_delivered,
  );

  const blockedPhase = (key) => {
    const phase = run.phases.find((candidate) => candidate.key === key);
    assert(phase, `a blocked record needs a ${key} phase`);
    return phase;
  };
  if (run.workload === "blocked") {
    // A short window that never reached its reopen measured half a workload; the record is
    // rejected rather than reported as a shorter one.
    assert.equal(run.reopen_tick, runs[0].reopen_tick);
    assert(Number.isSafeInteger(run.reopen_tick) && run.reopen_tick > 0);
    assert(run.reopen_tick < run.ticks);
    assert(run.reopen_us > 0);
    assert.deepEqual(
      run.phases.map((phase) => phase.key),
      ["blocked", "reopened"],
    );
    assert.equal(blockedPhase("blocked").first_sample, 0);
    assert.equal(blockedPhase("reopened").first_sample, run.reopen_tick);
    // Saturation: a jammed line delivers nothing and publishes nothing at all.
    assert.equal(run.start_delivered, 0);
    assert.equal(run.delivered_at_reopen, 0);
    assert.equal(blockedPhase("blocked").delivered, 0);
    const marks = (phase, values) =>
      total(values.slice(phase.first_sample, phase.first_sample + phase.ticks));
    assert.equal(marks(blockedPhase("blocked"), run.entity_dirty_marks), 0);
    assert.equal(marks(blockedPhase("blocked"), run.resource_dirty_marks), 0);
    // Resumption: reopening the sinks restarts both production and publication.
    assert(blockedPhase("reopened").delivered > 0);
    assert.equal(run.end_delivered, blockedPhase("reopened").delivered);
    assert(marks(blockedPhase("reopened"), run.entity_dirty_marks) > 0);
  } else {
    assert(
      ["active", "idle", "junction"].includes(run.workload),
      `unknown workload: ${run.workload}`,
    );
    assert.deepEqual(
      run.phases.map((phase) => phase.key),
      ["steady"],
    );
    for (const field of ["reopen_tick", "reopen_us", "delivered_at_reopen"])
      assert.equal(run[field], null, `${field} belongs to a blocked run only`);
    // A record of a producing workload that produced nothing describes a broken factory rather
    // than a fast one, and an idle one that produced anything is not idle.
    if (run.workload === "idle") {
      assert.equal(run.start_delivered, run.end_delivered);
      assert.equal(total(run.entity_dirty_marks), 0);
    } else {
      assert(run.end_delivered > run.start_delivered, "nothing was delivered");
      assert(total(run.entity_dirty_marks) > 0);
    }
  }

  const summary = { ...run };
  for (const key of [
    "tick_samples_us",
    "advance_encode_samples_us",
    "delta_bytes",
    "entity_dirty_marks",
    "resource_dirty_marks",
  ])
    delete summary[key];
  return {
    ...summary,
    entity_dirty_marks_total: total(run.entity_dirty_marks),
    resource_dirty_marks_total: total(run.resource_dirty_marks),
  };
});
const compressed = gzipSync(raw, { level: 9 });
assert.deepEqual(
  gunzipSync(compressed),
  raw,
  "compression must preserve every raw byte",
);
writeFileSync(`${path}.gz`, compressed);
writeFileSync(
  path.replace(/\.json$/, ".summary.json"),
  JSON.stringify(
    {
      schema: 2,
      raw_file: `${basename(path)}.gz`,
      raw_sha256: createHash("sha256").update(raw).digest("hex"),
      runs: summaries,
    },
    null,
    2,
  ) + "\n",
);
console.log(
  `Verified and packed five runs: ${path} (${raw.length} → ${compressed.length} bytes)`,
);
