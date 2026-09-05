/**
 * The browser capacity harness page.
 *
 * `npm run bench` measures the ladder natively; this measures the same ladder as wasm in a module
 * worker, and adds the costs a native run cannot see: the worker RPC round trip, the main-thread
 * delta merge, and the two canvases the game draws. Results are recorded in `docs/BENCHMARKS.md`.
 */

import { validateDefinitions } from "../core/definitions";
import { applySnapshotDelta } from "../core/snapshotDelta";
import { decodeSnapshotDelta } from "../core/snapshotWire";
import type {
  Definitions,
  FactorySnapshot,
  FactorySnapshotDelta,
} from "../core/types";
import rawDefinitions from "../data/definitions.json";
import {
  BASE_HEX_SIZE,
  type FactoryRenderer,
  type GraphicsProfile,
} from "../rendering/FactoryRenderer";
import { findLandingHub } from "../rendering/landmarks";
import { MinimapRenderer } from "../rendering/MinimapRenderer";
import { ThreeFactoryRenderer } from "../rendering/three/ThreeFactoryRenderer";
import {
  RENDER_VIEWPORT,
  TIER_COLUMNS,
  deltaMergeIsIntact,
  mergeBrowserReport,
  probeClockResolutionUs,
  tierRow,
} from "./report";
import { measureRenderPhases } from "./renderTiming";
import type {
  BrowserReport,
  HostTierResult,
  NativeReport,
  NativeTierResult,
  TierSpecSummary,
} from "./report";

interface WorkerResponse {
  id: number;
  ok: boolean;
  result?: unknown;
  error?: string;
}

interface CreateResult {
  tiers: TierSpecSummary[];
  clockResolutionUs: number;
}

class BenchTransport {
  private nextId = 1;
  private readonly pending = new Map<
    number,
    { resolve: (value: unknown) => void; reject: (reason: Error) => void }
  >();

  constructor(private readonly worker: Worker) {
    worker.addEventListener(
      "message",
      (event: MessageEvent<WorkerResponse>) => {
        const request = this.pending.get(event.data.id);
        if (!request) return;
        this.pending.delete(event.data.id);
        if (event.data.ok) {
          // The game's transport decodes an arriving delta buffer here, so this one does too. It
          // puts the decode inside the awaited request and therefore inside the round-trip phase,
          // which is where the cost it replaced used to sit: the worker parsed the JSON before the
          // structured clone copied the object graph across.
          const { result } = event.data;
          request.resolve(
            result instanceof ArrayBuffer
              ? decodeSnapshotDelta(result)
              : result,
          );
        } else
          request.reject(
            new Error(event.data.error ?? "Capacity worker failed"),
          );
      },
    );
    worker.addEventListener("error", (event) => {
      const error = new Error(event.message || "Capacity worker crashed");
      for (const request of this.pending.values()) request.reject(error);
      this.pending.clear();
    });
  }

  request<T>(method: string, payload?: Record<string, unknown>): Promise<T> {
    const id = this.nextId;
    this.nextId += 1;
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve: (value) => resolve(value as T), reject });
      this.worker.postMessage({ id, method, payload });
    });
  }

  dispose(): void {
    this.worker.terminate();
  }
}

const runFull = element<HTMLButtonElement>("run-full");
const runQuick = element<HTMLButtonElement>("run-quick");
const copyJson = element<HTMLButtonElement>("copy-json");
const status = element<HTMLParagraphElement>("status");
const head = element<HTMLTableRowElement>("table-head");
const body = element<HTMLTableSectionElement>("table-body");
const output = element<HTMLPreElement>("json");
const graphicsProfile = element<HTMLSelectElement>("graphics-profile");

let lastReport: BrowserReport | null = null;

for (const column of TIER_COLUMNS) {
  const cell = document.createElement("th");
  cell.textContent = column;
  head.append(cell);
}

runFull.addEventListener("click", () => void run(false));
runQuick.addEventListener("click", () => void run(true));
copyJson.addEventListener("click", () => {
  void navigator.clipboard.writeText(output.textContent ?? "");
});

async function run(quick: boolean): Promise<void> {
  runFull.disabled = true;
  runQuick.disabled = true;
  copyJson.disabled = true;
  body.replaceChildren();
  output.textContent = "";
  lastReport = null;
  const profile = graphicsProfile.value as GraphicsProfile;
  resetRenderSurfaces();
  graphicsProfile.disabled = true;

  const transport = new BenchTransport(
    new Worker(new URL("./capacity.worker.ts", import.meta.url), {
      type: "module",
    }),
  );
  try {
    report("Loading the bench artifact…");
    const created = await transport.request<CreateResult>("create", { quick });
    const hosts: HostTierResult[] = [];

    for (const [index, tier] of created.tiers.entries()) {
      report(
        `Measuring ${tier.key} — ${tier.entities.toLocaleString("en-US")} entities (${index + 1}/${created.tiers.length})…`,
      );
      const measured = await transport.request<NativeTierResult>("measure", {
        index,
      });
      const host = await measureRoundTrip(transport, index, tier, profile);
      hosts.push(host);
      appendRow({ ...measured, host });
      // Yield to the event loop so the page paints each tier as it lands.
      await new Promise((resolve) => setTimeout(resolve, 0));
    }

    const native = await transport.request<NativeReport>("report");
    lastReport = mergeBrowserReport(native, hosts, {
      user_agent: navigator.userAgent,
      hardware_concurrency: navigator.hardwareConcurrency,
      cross_origin_isolated: crossOriginIsolated,
      main_clock_resolution_us: probeClockResolutionUs(() => performance.now()),
      viewport_width: RENDER_VIEWPORT.width,
      viewport_height: RENDER_VIEWPORT.height,
      minimap_size: RENDER_VIEWPORT.minimap,
      device_pixel_ratio: window.devicePixelRatio || 1,
      hex_size: BASE_HEX_SIZE,
      worker_clock_resolution_us: created.clockResolutionUs,
      renderer_name: "Three.js Visual Depth",
      graphics_profile: profile,
      recorded: new Date().toISOString(),
    });
    output.textContent = JSON.stringify(lastReport, null, 2);
    copyJson.disabled = false;
    report(
      deltaMergeIsIntact(lastReport)
        ? `Measured ${lastReport.tiers.length} tiers. Every applied snapshot kept its full entity count.`
        : "Measured, but an applied snapshot lost entities — the delta merge did not reproduce the blueprint.",
    );
  } catch (error) {
    report(error instanceof Error ? error.message : String(error), true);
  } finally {
    transport.dispose();
    resetRenderSurfaces();
    runFull.disabled = false;
    runQuick.disabled = false;
    graphicsProfile.disabled = false;
  }
}

/**
 * One tier's host cost, measured as two aggregate spans rather than per frame.
 *
 * `performance.now` is clamped in a page that is not cross-origin isolated, so timing each frame
 * individually would measure the clock at the smaller tiers. Both phases are therefore timed once
 * around the whole budget: every delta is collected over the RPC first, then merged in arrival
 * order. The cost of a frame is their sum; the interleaving differs from the game's, and
 * `docs/BENCHMARKS.md` records that limit.
 */
async function measureRoundTrip(
  transport: BenchTransport,
  index: number,
  tier: TierSpecSummary,
  profile: GraphicsProfile,
): Promise<HostTierResult> {
  let snapshot = await transport.request<FactorySnapshot>("roundTripStart", {
    index,
  });
  let revision = 0;
  // The first delta after a fresh factory is a complete snapshot. Take it outside the measurement,
  // exactly as the native frame phase does, so what is timed is the steady state.
  const first = await transport.request<FactorySnapshotDelta>("roundTripFrame");
  ({ snapshot, revision } = applySnapshotDelta(snapshot, revision, first));

  const deltas: FactorySnapshotDelta[] = [];
  const roundTripStarted = performance.now();
  for (let frame = 0; frame < tier.frames; frame += 1) {
    deltas.push(
      await transport.request<FactorySnapshotDelta>("roundTripFrame"),
    );
  }
  const roundTripEnded = performance.now();

  const applyStarted = performance.now();
  for (const delta of deltas) {
    ({ snapshot, revision } = applySnapshotDelta(snapshot, revision, delta));
  }
  const applyEnded = performance.now();
  await transport.request("roundTripEnd");

  const roundTripUs =
    ((roundTripEnded - roundTripStarted) * 1000) / tier.frames;
  const applyUs = ((applyEnded - applyStarted) * 1000) / tier.frames;
  const hostFrameUs = roundTripUs + applyUs;
  const rendered = measureRender(snapshot, profile);
  return {
    key: tier.key,
    frames: tier.frames,
    round_trip_us: roundTripUs,
    apply_us: applyUs,
    host_frame_us: hostFrameUs,
    checksum: snapshot.checksum,
    applied_entities: snapshot.buildings.length,
    ...rendered,
    browser_frame_us: hostFrameUs + rendered.render_us,
    isolated_pipeline_us: hostFrameUs + rendered.preparation_submission_us,
  };
}

/**
 * Time the two canvases the game actually draws, against the snapshot the merge just produced.
 *
 * The viewport is pinned (1440×900 world, 178 px minimap) so a record is a measurement of the
 * renderer, not of the bench page's layout. Each canvas is warmed once, then repeated until the
 * same 20 ms budget the rest of the browser harness uses.
 */
function measureRender(
  snapshot: FactorySnapshot,
  profile: GraphicsProfile,
): ReturnType<typeof measureRenderPhases> & {
  renderer_name: string;
  graphics_profile: GraphicsProfile;
  draw_calls: number;
  triangles: number;
  geometries: number;
  textures: number;
  cpu_preparation_us: number;
  render_p95_us: number;
  js_heap_bytes?: number;
} {
  const { world, minimap } = renderSurfaces(profile);
  const home = findLandingHub(snapshot);
  world.setHome(home);
  const phases = measureRenderPhases({
    setWorldSnapshot: () => world.setSnapshot(snapshot),
    drawWorld: () => world.draw(),
    setMinimapSnapshot: () => minimap.setSnapshot(snapshot, home),
    drawMinimap: () => minimap.draw(),
  });
  const diagnostics = world.getDiagnostics();
  return {
    ...phases,
    renderer_name: diagnostics.name,
    graphics_profile: diagnostics.profile,
    draw_calls: diagnostics.drawCalls,
    triangles: diagnostics.triangles,
    geometries: diagnostics.geometries,
    textures: diagnostics.textures,
    cpu_preparation_us: diagnostics.cpuPreparationUs,
    render_p95_us: diagnostics.frameP95Us,
    js_heap_bytes: browserHeapBytes(),
  };
}

function browserHeapBytes(): number | undefined {
  const memory = (
    performance as Performance & {
      memory?: { usedJSHeapSize?: number };
    }
  ).memory;
  return memory?.usedJSHeapSize;
}

let surfaces: {
  world: FactoryRenderer;
  minimap: MinimapRenderer;
  profile: GraphicsProfile;
} | null = null;

function renderSurfaces(profile: GraphicsProfile): {
  world: FactoryRenderer;
  minimap: MinimapRenderer;
  profile: GraphicsProfile;
} {
  if (surfaces?.profile === profile) return surfaces;
  resetRenderSurfaces();
  const definitions = loadDefinitions();
  surfaces = {
    world: new ThreeFactoryRenderer(
      element("bench-world"),
      definitions,
      profile,
    ),
    minimap: new MinimapRenderer(element("bench-minimap"), definitions),
    profile,
  };
  return surfaces;
}

function resetRenderSurfaces(): void {
  surfaces?.world.dispose();
  surfaces = null;
}

function loadDefinitions(): Definitions {
  validateDefinitions(rawDefinitions);
  return rawDefinitions;
}

function appendRow(tier: NativeTierResult & { host: HostTierResult }): void {
  const row = document.createElement("tr");
  for (const value of tierRow(tier)) {
    const cell = document.createElement("td");
    cell.textContent = value;
    row.append(cell);
  }
  body.append(row);
}

function report(message: string, failed = false): void {
  status.textContent = message;
  status.dataset.state = failed ? "failed" : "running";
}

function element<T extends HTMLElement>(id: string): T {
  const found = document.getElementById(id);
  if (!found) throw new Error(`Missing bench element #${id}`);
  return found as T;
}
