import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const outputPath = resolve(root, "docs/AGENT-MAP.md");
const roots = ["factory-wasm/src", "src", "tests"];

const routes = [
  [
    "Fences, gates and edge construction",
    "factory-wasm/src/boundaries.rs; src/ui/boundaries.ts; src/rendering/three/boundaryMeshes.ts",
    "BoundaryEdit, boundary_transaction, BoundaryTool, BoundaryMeshes",
  ],
  [
    "Ground grading, paving and roads",
    "factory-wasm/src/ground.rs; src/ui/ground.ts; src/rendering/three/pavingSurface.ts",
    "GroundEdit, ground_transaction, GroundTool, PavingSurface",
  ],
  [
    "Native tick, determinism",
    "factory-wasm/src/lib.rs",
    "advance_ticks, checksum, Core",
  ],
  [
    "Transport, junctions, arbitration",
    "factory-wasm/src/runtime.rs; factory-wasm/src/lib.rs",
    "compile_graph, transfer_cargo",
  ],
  ["Power", "factory-wasm/src/lib.rs", "compile_power, distribute_power"],
  [
    "World generation and fields",
    "factory-wasm/src/lib.rs",
    "WorldParams, WorldFields, terrain_at",
  ],
  [
    "Save compatibility",
    "factory-wasm/src/save_migrations.rs; factory-wasm/src/lib.rs",
    "migrate, from_save, SavedState",
  ],
  [
    "Binary snapshots",
    "factory-wasm/src/wire.rs; src/core/snapshotWire.ts",
    "encode_snapshot_delta, decodeSnapshotDelta",
  ],
  [
    "Worker/host boundary",
    "src/core/factory.worker.ts; src/core/FactoryHost.ts",
    "handle, applyDelta",
  ],
  ["Frame loop and application wiring", "src/main.ts", "frame, update"],
  [
    "Research tree and icons",
    "src/ui/researchTree.ts; src/ui/researchGraph.ts; src/rendering/researchIcons.ts",
    "ResearchTree, layoutResearch, researchIconSvg",
  ],
  [
    "Skills and surveyed range",
    "factory-wasm/src/skills.rs; src/data/technologies.json; src/ui/skills.ts",
    "SkillEffect, observe_skill_event, SkillsView, skillView",
  ],
  [
    "Panels and keyed DOM",
    "src/ui/panels.ts; src/ui/dom.ts",
    "PanelController, syncChildren",
  ],
  [
    "Input commands",
    "src/core/input.ts; src/core/commands.ts; src/main.ts",
    "BoundedInputQueue, enqueue",
  ],
  [
    "Definitions and balance",
    "src/data/*.json; src/core/definitions.ts; factory-wasm/src/balance.rs",
    "validateDefinitions, Economy",
  ],
  [
    "Petroleum and joint-output recipes",
    "factory-wasm/src/recipes.rs; factory-wasm/src/petroleum_tests.rs; src/data/definitions.json; src/ui/production.ts",
    "outputs, cost_allocation, oil-refining, productionNote",
  ],
  [
    "Contracts, requests and scenarios",
    "src/data/scenarios.json; src/core/guidance.ts; factory-wasm/src/lib.rs",
    "ContractDefinition, request_eligible, advance_contract, nextAction",
  ],
  [
    "Title screen and save catalogue",
    "src/core/saveSlots.ts; src/main.ts; factory-wasm/src/save_migrations.rs",
    "SaveSlot, compatibility, openTitleScreen, migrate",
  ],
  [
    "Guidance and progression UI",
    "src/core/guidance.ts; src/main.ts",
    "nextAction, renderNextAction",
  ],
  [
    "Three.js world",
    "src/rendering/three/ThreeFactoryRenderer.ts; src/rendering/three/worldInstances.ts",
    "ThreeFactoryRenderer, WorldInstances",
  ],
  [
    "Camera, orbit, zoom and graphics profiles",
    "src/rendering/three/HexSceneCamera.ts; src/rendering/three/ThreeFactoryRenderer.ts; src/rendering/three/quality.ts",
    "HexSceneCamera, orbit, zoom, GraphicsProfile",
  ],
  [
    "Machine appearance",
    "src/rendering/shapeGrammar.ts; src/rendering/three/machineMeshes.ts",
    "buildingParts, createMachineMeshes",
  ],
  [
    "Terrain appearance",
    "src/rendering/three/terrainSurface.ts; src/rendering/three/terrainMeshes.ts",
    "terrainSurface, createTerrainMeshes",
  ],
  [
    "Performance ladder",
    "factory-wasm/src/lib.rs; src/bench; docs/BENCHMARKS.md",
    "capacity, browser frame",
  ],
];

function walk(directory) {
  const absolute = resolve(root, directory);
  return readdirSync(absolute, { withFileTypes: true }).flatMap((entry) => {
    const next = `${directory}/${entry.name}`;
    return entry.isDirectory() ? walk(next) : [next];
  });
}

function declarations(path, source) {
  const rows = [];
  const patterns = path.endsWith(".rs")
    ? [
        /^\s*(?:pub(?:\([^)]*\))?\s+)?(?:struct|enum|trait|type|mod)\s+([A-Za-z0-9_]+)/,
        /^\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+([A-Za-z0-9_]+)/,
        /^\s*impl(?:<[^>]+>)?\s+([A-Za-z0-9_]+)/,
      ]
    : [
        /^\s*export\s+(?:default\s+)?(?:abstract\s+)?(?:class|interface|type|enum|function|const)\s+([A-Za-z0-9_]+)/,
        /^\s*(?:async\s+)?function\s+([A-Za-z0-9_]+)/,
        /^\s*class\s+([A-Za-z0-9_]+)/,
      ];
  source.split(/\r?\n/).forEach((line, index) => {
    for (const pattern of patterns) {
      const match = line.match(pattern);
      if (match) {
        rows.push(`${match[1]}:${index + 1}`);
        break;
      }
    }
  });
  return rows;
}

function group(path) {
  if (path.startsWith("factory-wasm")) return "Native Rust";
  if (path.startsWith("src/rendering")) return "Rendering";
  if (path.startsWith("src/core")) return "Browser core";
  if (path.startsWith("src/ui")) return "Browser UI";
  if (path.startsWith("src/bench")) return "Browser benchmark";
  if (path.startsWith("src/admin")) return "Browser admin";
  if (path.startsWith("tests")) return "TypeScript tests";
  return "Application";
}

const files = roots
  .flatMap(walk)
  .filter((path) => /\.(?:rs|ts|json)$/.test(path) && !path.includes("/pkg"))
  .sort();

const inventory = files.map((path) => {
  const absolute = resolve(root, path);
  const source = readFileSync(absolute, "utf8");
  return {
    path,
    lines: source.split(/\r?\n/).length,
    bytes: statSync(absolute).size,
    declarations: /\.(?:rs|ts)$/.test(path) ? declarations(path, source) : [],
  };
});

const generated = [];
generated.push("# Agent map (generated)", "");
generated.push(
  "Generated by `npm run agent:map`. Start with the task route, then inspect named declarations with `rg -n`; do not read a large file from top to bottom unless the task truly spans it.",
  "",
  "## Task routes",
  "",
  "| Task | Read first | Localize with |",
  "| --- | --- | --- |",
);
for (const [task, paths, anchors] of routes)
  generated.push(`| ${task} | \`${paths}\` | \`${anchors}\` |`);

for (const section of [...new Set(inventory.map(({ path }) => group(path)))]) {
  generated.push("", `## ${section}`, "");
  for (const file of inventory.filter(({ path }) => group(path) === section)) {
    const size = `${file.lines} lines / ${(file.bytes / 1024).toFixed(1)} KiB`;
    generated.push(`### \`${file.path}\` — ${size}`, "");
    if (file.declarations.length === 0) {
      generated.push(
        "Data or fixture; inspect keys before loading the full file.",
        "",
      );
      continue;
    }
    const chunks = [];
    for (let index = 0; index < file.declarations.length; index += 12)
      chunks.push(file.declarations.slice(index, index + 12).join(", "));
    generated.push(...chunks.map((chunk) => `- ${chunk}`), "");
  }
}

generated.push(
  "## Refresh contract",
  "",
  "`npm run agent:map:check` fails when this map is stale. The complete quality gate runs that check.",
);

const output = `${generated.join("\n")}\n`;
if (process.argv.includes("--check")) {
  const current = readFileSync(outputPath, "utf8");
  if (current !== output) {
    process.stderr.write("docs/AGENT-MAP.md is stale; run npm run agent:map\n");
    process.exitCode = 1;
  }
} else {
  writeFileSync(outputPath, output);
  process.stdout.write(`wrote ${relative(root, outputPath)}\n`);
}
