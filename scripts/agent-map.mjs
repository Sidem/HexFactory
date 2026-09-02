import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const routerPath = resolve(root, "docs/AGENT-MAP.md");
const shardRoot = resolve(root, ".agent");
const roots = ["factory-wasm/src", "src", "tests"];
const MAX_ROUTER_BYTES = 4 * 1024;
const MAX_SHARD_BYTES = 8 * 1024;
const MAX_DECLARATIONS_PER_FILE = 4;

const routes = [
  [
    "Fences, gates and edge construction",
    "native",
    "factory-wasm/src/boundaries.rs; src/ui/boundaries.ts; src/rendering/three/boundaryMeshes.ts",
    "BoundaryEdit, boundary_transaction, BoundaryTool, BoundaryMeshes",
  ],
  [
    "Ground grading, paving and roads",
    "native",
    "factory-wasm/src/ground_spine.rs; factory-wasm/src/ground.rs; src/ui/ground.ts; src/rendering/three/pavingSurface.ts",
    "GroundSpine, FinishedGround, GroundEdit, ground_transaction, GroundTool, PavingSurface",
  ],
  [
    "Native tick and determinism",
    "simulation",
    "factory-wasm/src/core/tick.rs; factory-wasm/src/core/snapshots.rs",
    "advance, checksum",
  ],
  [
    "Transport, junctions and arbitration",
    "simulation",
    "factory-wasm/src/runtime.rs; factory-wasm/src/core/graph.rs; factory-wasm/src/core/transport.rs",
    "compile_graph, transfer_cargo",
  ],
  [
    "Power",
    "simulation",
    "factory-wasm/src/core/power.rs",
    "compile_power, distribute_power",
  ],
  [
    "World generation and fields",
    "native",
    "factory-wasm/src/terra.rs; factory-wasm/src/lib.rs",
    "WorldParams, WorldFields, terrain_at",
  ],
  [
    "Save compatibility",
    "simulation",
    "factory-wasm/src/save_migrations.rs; factory-wasm/src/core/persistence.rs",
    "migrate, from_save, SavedState",
  ],
  [
    "Binary snapshots",
    "browser",
    "factory-wasm/src/wire.rs; src/core/snapshotWire.ts",
    "encode_snapshot_delta, decodeSnapshotDelta",
  ],
  [
    "Worker and host boundary",
    "browser",
    "src/core/factory.worker.ts; src/core/FactoryHost.ts",
    "handle, applyDelta",
  ],
  [
    "Frame loop and application wiring",
    "browser",
    "src/main.ts",
    "frame, update",
  ],
  [
    "Research tree, skills and icons",
    "browser",
    "src/ui/researchTree.ts; src/ui/researchGraph.ts; src/ui/skills.ts; src/rendering/researchIcons.ts",
    "ResearchTree, SkillsView, layoutResearch",
  ],
  [
    "Panels and keyed DOM",
    "browser",
    "src/ui/panels.ts; src/ui/dom.ts",
    "PanelController, syncChildren",
  ],
  [
    "Input commands",
    "browser",
    "src/core/input.ts; src/core/commands.ts; src/main.ts",
    "BoundedInputQueue, enqueue",
  ],
  [
    "Definitions, recipes and balance",
    "native",
    "src/data/*.json; src/core/definitions.ts; factory-wasm/src/balance.rs; factory-wasm/src/recipes.rs",
    "validateDefinitions, Economy, outputs",
  ],
  [
    "Contracts, requests and guidance",
    "browser",
    "src/data/scenarios.json; src/core/guidance.ts; factory-wasm/src/lib.rs",
    "ContractDefinition, advance_contract, nextAction",
  ],
  [
    "Title screen and save catalogue",
    "browser",
    "src/core/saveSlots.ts; src/main.ts",
    "SaveSlot, openTitleScreen, compatibility",
  ],
  [
    "Three.js world and camera",
    "rendering",
    "src/rendering/three/ThreeFactoryRenderer.ts; src/rendering/three/worldInstances.ts; src/rendering/three/HexSceneCamera.ts",
    "ThreeFactoryRenderer, WorldInstances, HexSceneCamera",
  ],
  [
    "Machine appearance",
    "rendering",
    "src/rendering/shapeGrammar.ts; src/rendering/three/machineMeshes.ts",
    "buildingParts, createMachineMeshes",
  ],
  [
    "Terrain appearance",
    "rendering",
    "src/rendering/three/terrainSurface.ts; src/rendering/three/terrainMeshes.ts",
    "terrainSurface, createTerrainMeshes",
  ],
  [
    "Performance measurements",
    "benchmark",
    "factory-wasm/src/capacity.rs; src/bench; docs/BENCHMARKS.md",
    "capacity, browser frame",
  ],
  ["Native tests", "tests", "factory-wasm/src/tests", "nearest named test"],
  ["Browser tests", "tests", "tests", "nearest describe or test"],
];

const shardNames = [
  "native",
  "simulation",
  "browser",
  "rendering",
  "benchmark",
  "tests",
];

function walk(directory) {
  const absolute = resolve(root, directory);
  return readdirSync(absolute, { withFileTypes: true }).flatMap((entry) => {
    const next = `${directory}/${entry.name}`;
    return entry.isDirectory() ? walk(next) : [next];
  });
}

function declarations(path, source) {
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
  const rows = [];
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

function domain(path) {
  if (path.startsWith("factory-wasm/src/tests/") || path.startsWith("tests/"))
    return "tests";
  if (path.startsWith("factory-wasm/src/core/")) return "simulation";
  if (path.startsWith("factory-wasm/")) return "native";
  if (path.startsWith("src/rendering/")) return "rendering";
  if (path.startsWith("src/bench/") || path.startsWith("src/admin/"))
    return "benchmark";
  return "browser";
}

const inventory = roots
  .flatMap(walk)
  .filter((path) => /\.(?:rs|ts|json)$/.test(path) && !path.includes("/pkg"))
  .sort()
  .map((path) => {
    const source = readFileSync(resolve(root, path), "utf8");
    return {
      path,
      domain: domain(path),
      lines: source.split(/\r?\n/).length,
      bytes: statSync(resolve(root, path)).size,
      declarations: /\.(?:rs|ts)$/.test(path) ? declarations(path, source) : [],
    };
  });

const router = [
  "# Agent map (generated)",
  "",
  "Choose one route. Open only that domain index, then localize its named anchors with `rg -n`.",
  "",
  "| Task | Domain index |",
  "| --- | --- |",
  ...routes.map(
    ([task, route]) => `| ${task} | [${route}](../.agent/${route}.md) |`,
  ),
  "",
  "Run `npm run agent:map` after declarations move; quality fails when any generated index is stale.",
  "",
].join("\n");

function shard(name) {
  const lines = [
    `# ${name} route (generated)`,
    "",
    "Read the named file and a bounded range around the anchor; do not read oversized files end to end.",
    "",
    "## Tasks",
    "",
  ];
  for (const [task, , paths, anchors] of routes.filter(
    ([, route]) => route === name,
  )) {
    lines.push(`- **${task}:** \`${paths}\` — \`${anchors}\``);
  }
  lines.push("", "## Files", "");
  for (const file of inventory.filter((entry) => entry.domain === name)) {
    const sample = file.declarations
      .slice(0, MAX_DECLARATIONS_PER_FILE)
      .join(", ");
    lines.push(
      `- \`${file.path}\` — ${file.lines} lines / ${(file.bytes / 1024).toFixed(1)} KiB${sample ? ` — ${sample}${file.declarations.length > MAX_DECLARATIONS_PER_FILE ? ", …" : ""}` : ""}`,
    );
  }
  lines.push("");
  return lines.join("\n");
}

const outputs = new Map([[routerPath, router]]);
for (const name of shardNames)
  outputs.set(resolve(shardRoot, `${name}.md`), shard(name));

for (const [path, output] of outputs) {
  const limit = path === routerPath ? MAX_ROUTER_BYTES : MAX_SHARD_BYTES;
  if (Buffer.byteLength(output) > limit)
    throw new Error(`${relative(root, path)} exceeds ${limit} bytes`);
}

if (process.argv.includes("--check")) {
  for (const [path, output] of outputs) {
    if (!existsSync(path) || readFileSync(path, "utf8") !== output) {
      process.stderr.write(
        `${relative(root, path)} is stale; run npm run agent:map\n`,
      );
      process.exitCode = 1;
    }
  }
} else {
  mkdirSync(shardRoot, { recursive: true });
  for (const [path, output] of outputs) {
    writeFileSync(path, output);
    process.stdout.write(`wrote ${relative(root, path)}\n`);
  }
}
