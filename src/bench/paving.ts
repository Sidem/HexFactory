/** Focused CPU submission measurement for the Phase 5 materials. See paving-bench.html. */
import init, { Factory } from "../../factory-wasm/pkg/factory_wasm";
import rawDefinitions from "../data/definitions.json";
import technologies from "../data/technologies.json";
import scenarios from "../data/scenarios.json";
import { validateDefinitions } from "../core/definitions";
import type { Definitions, FactorySnapshot, GroundCell } from "../core/types";
import { ThreeFactoryRenderer } from "../rendering/three/ThreeFactoryRenderer";
import { required } from "../ui/dom";

await init();
const native = new Factory(
  JSON.stringify(rawDefinitions),
  JSON.stringify(technologies),
  JSON.stringify(scenarios),
  "new-game",
);
const base = JSON.parse(native.snapshot_json()) as FactorySnapshot;
native.free();
validateDefinitions(rawDefinitions);
const definitions = rawDefinitions as Definitions;
const canvas = required<HTMLCanvasElement>("world");
const renderer = new ThreeFactoryRenderer(canvas, definitions, "low");
const surfaces = [
  { id: 0, key: "untreated", name: "Untreated" },
  ...definitions.surfaces,
];
const select = required<HTMLSelectElement>("surface");
for (const surface of surfaces) {
  const option = document.createElement("option");
  option.value = String(surface.id);
  option.textContent = surface.name;
  select.append(option);
}

function yard(surface: number): FactorySnapshot {
  const ground: GroundCell[] = [];
  for (let q = -12; q <= 12; q += 1)
    for (let r = -12; r <= 12; r += 1)
      if (surface) ground.push({ q, r, surface, elevation: 0, paid: [] });
  return {
    ...base,
    terrain: [],
    resources: [],
    buildings: [],
    boundaries: [],
    ground,
    chunks: [
      {
        chunk_q: 0,
        chunk_r: 0,
        x: -40000,
        y: -40000,
        span: 80000,
        entity_count: 0,
      },
    ],
    player: { ...base.player, x: 0, y: 0 },
  };
}

function show(surface: number): void {
  renderer.setSnapshot(yard(surface));
  renderer.recenter();
  renderer.draw();
}
select.addEventListener("change", () => show(Number(select.value)));
show(0);

const frame = (): Promise<number> => new Promise(requestAnimationFrame);
const run = required<HTMLButtonElement>("run");
const output = required("result");
run.addEventListener("click", () => {
  void measure().catch((error: unknown) => {
    output.textContent = String(error);
    run.disabled = false;
    select.disabled = false;
  });
});

async function measure(): Promise<void> {
  run.disabled = true;
  select.disabled = true;
  const results = [];
  for (const surface of surfaces) {
    output.textContent = `Measuring ${surface.name}`;
    select.value = String(surface.id);
    show(surface.id);
    for (let i = 0; i < 60; i += 1) {
      await frame();
      renderer.draw();
    }
    const samples = [];
    const intervals = [];
    let previous = await frame();
    for (let i = 0; i < 240; i += 1) {
      const now = await frame();
      intervals.push(now - previous);
      previous = now;
      const start = performance.now();
      renderer.draw();
      samples.push((performance.now() - start) * 1000);
    }
    samples.sort((a, b) => a - b);
    intervals.sort((a, b) => a - b);
    results.push({
      surface: surface.key,
      paved_hexes: surface.id ? 625 : 0,
      mean_cpu_us: samples.reduce((a, b) => a + b, 0) / samples.length,
      p95_cpu_us: samples[Math.ceil(samples.length * 0.95) - 1],
      p95_raf_interval_ms: intervals[Math.ceil(intervals.length * 0.95) - 1],
      diagnostics: renderer.getDiagnostics(),
    });
  }
  output.textContent = JSON.stringify(
    {
      measured_at: new Date().toISOString(),
      user_agent: navigator.userAgent,
      viewport: { width: canvas.clientWidth, height: canvas.clientHeight },
      profile: "low",
      warmup_frames: 60,
      measured_frames: 240,
      scope:
        "Synthetic level yard; CPU render submission, no worker or GPU completion timing. RAF includes browser scheduling.",
      results,
    },
    null,
    2,
  );
  run.disabled = false;
  select.disabled = false;
}
