import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { cpus } from "node:os";
import { createServer } from "vite";
import init, { Factory } from "../factory-wasm/pkg/factory_wasm.js";

const [savePath, reportPath] = process.argv.slice(2);
if (!savePath || !reportPath)
  throw new Error(
    "Usage: node scripts/profile-playtest.mjs save.hxf1 report.json",
  );
const save = readFileSync(savePath, "utf8");
const definitions = readFileSync("src/data/definitions.json", "utf8");
await init({
  module_or_path: readFileSync("factory-wasm/pkg/factory_wasm_bg.wasm"),
});
const factory = new Factory(
  definitions,
  readFileSync("src/data/technologies.json", "utf8"),
  readFileSync("src/data/scenarios.json", "utf8"),
  "new-game",
);
factory.load_string(save);
const snapshot = JSON.parse(factory.snapshot_json());
const server = await createServer({
  server: { middlewareMode: true },
  appType: "custom",
});
try {
  const { createWorldMaterials } = await server.ssrLoadModule(
    "/src/rendering/three/materials.ts",
  );
  const { WorldInstanceLayer } = await server.ssrLoadModule(
    "/src/rendering/three/worldInstances.ts",
  );
  const { PopulationMeshes } = await server.ssrLoadModule(
    "/src/rendering/three/populationMeshes.ts",
  );
  const { HexSceneCamera } = await server.ssrLoadModule(
    "/src/rendering/three/HexSceneCamera.ts",
  );
  const { HEIGHT_UNIT_HEIGHT } = await server.ssrLoadModule(
    "/src/rendering/sceneScale.ts",
  );
  const { axialToPixel } = await import("@hexlife/embed/hex");
  const terrain = new Map(
    snapshot.terrain.map((cell) => {
      const point = axialToPixel(cell, 1, { x: 0, y: 0 });
      return [
        `${cell.q},${cell.r}`,
        {
          ...cell,
          x: point.x,
          z: point.y,
          height: cell.height * HEIGHT_UNIT_HEIGHT,
        },
      ];
    }),
  );
  const camera = new HexSceneCamera();
  camera.resize(1280, 720);
  const { heightAtWorld } = await server.ssrLoadModule(
    "/src/rendering/three/terrainMeshes.ts",
  );
  camera.follow(snapshot.player, heightAtWorld(terrain, snapshot.player));
  const materials = createWorldMaterials();
  const world = new WorldInstanceLayer(JSON.parse(definitions), materials);
  const populations = new PopulationMeshes();
  world.setSnapshot(snapshot, terrain, 0);
  populations.update(snapshot.herds, snapshot.tick, terrain, 0);
  const { FrameVisibility } = await server.ssrLoadModule(
    "/src/rendering/three/frameVisibility.ts",
  );
  const visibility = new FrameVisibility();
  visibility.update(camera.camera);
  function measure(action) {
    for (let index = 0; index < 100; index++) action(index * 16.67);
    const samples = [];
    for (let index = 0; index < 500; index++) {
      const started = performance.now();
      action(index * 16.67);
      samples.push((performance.now() - started) * 1000);
    }
    samples.sort((a, b) => a - b);
    return {
      samples: samples.length,
      median_us: samples[249],
      p95_us: samples[474],
      max_us: samples.at(-1),
    };
  }
  const report = {
    recorded: new Date().toISOString(),
    cpu: cpus()[0].model,
    node: process.version,
    save_sha256: createHash("sha256").update(save).digest("hex"),
    wasm_sha256: createHash("sha256")
      .update(readFileSync("factory-wasm/pkg/factory_wasm_bg.wasm"))
      .digest("hex"),
    entities: snapshot.buildings.length,
    herds: snapshot.herds.length,
    animals: snapshot.herds.reduce((sum, herd) => sum + herd.count, 0),
    visibility: Boolean(visibility),
    viewport: [1280, 720],
    limitation:
      "Node CPU preparation only, fixed imported snapshot and camera, no WebGL, GPU, DOM or presentation timing; terrain mesh construction excluded.",
    machines_all: measure((now) => world.update(now, false)),
    machines: measure((now) => world.update(now, false, visibility)),
    herds_all: measure((now) => populations.animate(now)),
    herds_cpu: measure((now) => populations.animate(now, visibility)),
    advance_and_encode: measure(() => {
      factory.advance_json("[]", 1, 0);
      factory.snapshot_delta_bytes();
    }),
  };
  writeFileSync(reportPath, JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify(report, null, 2));
  world.dispose();
  populations.dispose();
  materials.materials.forEach((material) => material.dispose());
} finally {
  factory.free();
  await server.close();
}
