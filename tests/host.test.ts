import { readFileSync } from "node:fs";

import {
  HEX_DIRECTIONS,
  axialNeighbor,
  axialToPixel,
  pixelToAxial,
  rotateHexDirection,
} from "@hexlife/embed/hex";
import { describe, expect, it, vi } from "vitest";

import directionFixture from "../fixtures/hex-directions.json";
import {
  buildingAvailability,
  technologyAvailability,
} from "../src/core/availability";
import { encodeCommand } from "../src/core/commands";
import {
  FactoryHost,
  type FactoryTransport,
  type FactoryWorkerMethod,
} from "../src/core/FactoryHost";
import {
  BoundedInputQueue,
  MAX_INPUT_COMMANDS,
  MOVEMENT_KEYS,
  movementIntent,
} from "../src/core/input";
import {
  applyBuildingsPatch,
  applyResourcesPatch,
  applySnapshotDelta,
} from "../src/core/snapshotDelta";
import type {
  BuildingDefinition,
  EntitySnapshot,
  FactorySnapshot,
  FactorySnapshotDelta,
  ResourceSnapshot,
} from "../src/core/types";
import definitions from "../src/data/definitions.json";
import technologies from "../src/data/technologies.json";
import { HexCamera, isSurveyed } from "../src/rendering/CanvasFactoryRenderer";

const snapshot: FactorySnapshot = {
  scenario: "new-game",
  scenario_name: "New game",
  world_version: 2,
  seed: 1213486160,
  tick: 12,
  checksum: 123,
  delivered: 2,
  delivered_by_item: [{ item_id: 1, quantity: 2 }],
  insight: 4,
  victory: false,
  objective: { item_id: 2, delivered: 0, required: 3 },
  player: {
    x: 1774,
    y: 0,
    facing_x: 1000,
    facing_y: 0,
    move_x: 0,
    move_y: 0,
    inventory: { "1": 3 },
    action_cooldown: 0,
    build_range: 8870,
    carry_slots: 6,
    carry_stacks: [{ item_id: 1, quantity: 3 }],
  },
  researched: [1],
  chunks: [
    { chunk_q: 0, chunk_r: 0, entity_count: 1, x: 0, y: 0, span: 16384 },
  ],
  terrain: [{ x: 3550, y: 1500, radius: 660, terrain: "water" }],
  resources: [
    {
      id: 1,
      x: 5322,
      y: 0,
      radius: 720,
      item_id: 1,
      quantity: 47,
      initial_quantity: 48,
    },
  ],
  buildings: [
    {
      id: 1,
      q: 0,
      r: 0,
      definition_id: 6,
      kind: "hub",
      orientation: 0,
      scenario_owned: true,
      inventory: [],
      progress: 0,
      progress_total: 0,
      status: "landing hub",
      footprint: [
        { q: 0, r: 0 },
        { q: 0, r: 1 },
        { q: -1, r: 1 },
      ],
    },
  ],
  events: [],
};

describe("public hex host contract", () => {
  it("pins TypeScript and Rust to the clockwise six-direction fixture", () => {
    expect(HEX_DIRECTIONS).toEqual(directionFixture);
    expect(axialNeighbor({ q: -2, r: 0 }, 1)).toEqual({ q: -2, r: 1 });
    expect(rotateHexDirection(5, 1)).toBe(0);
  });

  it("round-trips base and pan/zoom camera picking through @hexlife/embed/hex", () => {
    const origin = { x: 410, y: 330 };
    expect(
      pixelToAxial(axialToPixel({ q: -4, r: 2 }, 35, origin), 35, origin),
    ).toEqual({ q: -4, r: 2 });
    const camera = new HexCamera();
    camera.recenter({ x: 3550, y: -3072 });
    const coordinate = { q: -4, r: 5 };
    const screen = camera.project(coordinate, 900, 650);
    expect(camera.pick(screen, 900, 650)).toEqual(coordinate);
    camera.panBy(73, -42);
    camera.zoomAt(1.6, { x: 320, y: 240 }, 900, 650);
    const moved = camera.project(coordinate, 900, 650);
    expect(camera.pick(moved, 900, 650)).toEqual(coordinate);
  });
});

describe("bounded host input", () => {
  it("maps WASD to normalized continuous intent and never exceeds one native batch limit", () => {
    expect(MOVEMENT_KEYS).toEqual({
      KeyW: { x: 0, y: -1 },
      KeyA: { x: -1, y: 0 },
      KeyS: { x: 0, y: 1 },
      KeyD: { x: 1, y: 0 },
    });
    expect(movementIntent(new Set(["KeyW", "KeyD"]))).toEqual({
      type: "move_intent",
      x: 707,
      y: -707,
    });
    const queue = new BoundedInputQueue();
    for (let index = 0; index < MAX_INPUT_COMMANDS; index += 1)
      expect(queue.enqueue({ type: "move_intent", x: 0, y: -1000 })).toBe(true);
    expect(queue.enqueue({ type: "gather" })).toBe(false);
    expect(queue.drain()).toHaveLength(MAX_INPUT_COMMANDS);
    expect(queue.drain()).toEqual([]);
  });

  it("encodes commands without embedding simulation behavior", () => {
    expect(encodeCommand({ type: "move_intent", x: 707, y: -707 })).toEqual({
      opcode: 0,
      args: [707, -707],
    });
    expect(
      encodeCommand({
        type: "place",
        q: -3,
        r: 2,
        definition_id: 2,
        orientation: 5,
      }),
    ).toEqual({ opcode: 3, args: [-3, 2, 2, 5, 0] });
    expect(() => encodeCommand({ type: "move_intent", x: 1001, y: 0 })).toThrow(
      /-1000\.\.1000/,
    );
  });

  it("sends a drag as two endpoints and never resolves the run itself", () => {
    // One drag is one bounded command carrying only what the pointer did.
    expect(
      encodeCommand({
        type: "place_line",
        q: 2,
        r: 0,
        to_q: 4,
        to_r: 1,
        definition_id: 2,
        orientation: 0,
      }),
    ).toEqual({ opcode: 7, args: [2, 0, 4, 1, 2, 0, 0] });
    expect(
      encodeCommand({ type: "erase_line", q: 2, r: 0, to_q: 4, to_r: 1 }),
    ).toEqual({ opcode: 8, args: [2, 0, 4, 1] });
    expect(encodeCommand({ type: "undo" })).toEqual({ opcode: 9, args: [] });
    expect(
      encodeCommand({ type: "withdraw", q: 1, r: 1, item_id: 2, quantity: 7 }),
    ).toEqual({ opcode: 10, args: [1, 1, 2, 7] });

    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    const renderer = readFileSync(
      new URL("../src/rendering/CanvasFactoryRenderer.ts", import.meta.url),
      "utf8",
    );
    // The path between the endpoints is native truth. The host asks for it and draws the answer;
    // it must not walk hexes, expand a drag into per-cell commands, or import a line traversal.
    expect(main).toContain("host.linePreview(");
    expect(main).not.toMatch(/hexLine|axialLine|for \(const cell of .*path/);
    expect(main.match(/type: "place_line"/g)).toHaveLength(1);
    expect(renderer).toContain("setDragPath(");
    expect(renderer).not.toMatch(/hexLine|axialLine/);
  });

  it("contains no host-side player or progression mutation loop", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    expect(main).not.toMatch(
      /player\.(x|y|inventory|action_cooldown)\s*[+\-=]/,
    );
    expect(
      main.match(/\.advance\(commands, ticks, playerSteps\)/g),
    ).toHaveLength(1);
    expect(main).not.toContain("snapshot.insight =");
    expect(main).not.toContain("scenarioInput.value = snapshot.scenario");
    expect(main).not.toContain("seedInput.value = String(snapshot.seed)");
  });

  it("paces the player from the native cadence and never from the frame delta", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    // The rate is native truth. The host converts elapsed real time into a step count with it and
    // never turns a frame delta into a position.
    expect(main).toContain("elapsed * host.playerTicksPerSecond");
    expect(main).not.toMatch(/player\.(x|y)\s*\+/);
    // Player steps are unscaled by the speed setting and unaffected by the pause state, which is
    // the whole point: the factory's accumulator is the only one that reads either.
    expect(main).not.toMatch(
      /playerAccumulator \+= [^;]*speedInput|playing\s*&&[^;]*playerAccumulator/,
    );
  });

  it("patches keyed lists in place so a rebuild cannot swallow a click", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    // A `replaceChildren` between pointerdown and pointerup destroys the pressed control, the
    // click retargets to the container, and the delegated handler finds nothing. Every list that
    // carries a control goes through the reconciler instead.
    expect(main).toContain("function syncChildren(");
    expect(main).not.toContain("replaceChildren()");
    // The hotbar's buttons are built once, so it needs no reconciler — but rewriting their inner
    // nodes on every snapshot loses a click the same way, so it patches text instead.
    const hotbar = main.slice(
      main.indexOf("function renderHotbar("),
      main.indexOf("\n}", main.indexOf("function renderHotbar(")),
    );
    expect(hotbar).not.toMatch(/innerHTML\s*=/);
    for (const renderer of [
      "renderTechnologies",
      "renderInventory",
      "renderInspectorActions",
    ]) {
      const body = main.slice(
        main.indexOf(`function ${renderer}(`),
        main.indexOf("\n}", main.indexOf(`function ${renderer}(`)),
      );
      expect(body).toContain("syncChildren(");
    }
  });
});

describe("availability and expanded snapshot adapter", () => {
  it("derives hotbar costs/locks and technology prerequisites from native truth", () => {
    const belt = definitions.buildings.find(
      ({ key }) => key === "belt",
    ) as BuildingDefinition;
    const extractor = definitions.buildings.find(
      ({ key }) => key === "extractor",
    ) as BuildingDefinition;
    expect(buildingAvailability(belt, snapshot, definitions.items)).toEqual({
      locked: false,
      affordable: true,
      costLabel: "1 ORE",
    });
    expect(
      buildingAvailability(extractor, snapshot, definitions.items),
    ).toMatchObject({
      locked: true,
      affordable: false,
    });
    expect(
      technologyAvailability(technologies.technologies[0]!, snapshot),
    ).toEqual({
      complete: true,
      prerequisitesMet: true,
      affordable: true,
    });
    expect(
      technologyAvailability(technologies.technologies[2]!, snapshot)
        .prerequisitesMet,
    ).toBe(false);
  });

  it("delegates worker commands and applies revision-checked native deltas", async () => {
    const { transport, requests } = fakeTransport();
    const host = FactoryHost.forTesting(transport, snapshot);
    expect(host.snapshot()).toEqual(snapshot);
    expect(await host.save()).toBe("HXF1\n{} ");
    expect((await host.load("HXF1\nrestored")).events).toEqual([
      "HXF1 save restored",
    ]);
    expect((await host.advance([{ type: "gather" }], 2, 5)).tick).toBe(14);
    expect(requests).toEqual([
      { method: "save", payload: undefined },
      { method: "load", payload: { save: "HXF1\nrestored" } },
      {
        method: "advance",
        // Simulation ticks and player steps travel as separate counts, because the player walks
        // on its own cadence rather than on factory time.
        payload: { commands: [{ type: "gather" }], ticks: 2, playerSteps: 5 },
      },
    ]);
  });

  it("applies per-entity buildings patches instead of whole-array replacements", () => {
    const belt: EntitySnapshot = {
      id: 4,
      q: 1,
      r: 0,
      definition_id: 2,
      kind: "belt",
      orientation: 0,
      scenario_owned: false,
      inventory: [],
      progress: 0,
      progress_total: 0,
      status: "idle",
      footprint: [{ q: 1, r: 0 }],
    };
    const extractor: EntitySnapshot = { ...belt, id: 2, kind: "extractor" };
    const listed = [...snapshot.buildings, extractor, belt];
    const withLine: FactorySnapshot = { ...snapshot, buildings: listed };

    // A changed entity is patched in place; every untouched entity keeps its identity.
    const moved = { ...belt, cargo: { item_id: 1, quantity: 1 } };
    const patched = applyBuildingsPatch(listed, { changed: [moved] });
    expect(patched.map(({ id }) => id)).toEqual([1, 2, 4]);
    expect(patched[2]).toEqual(moved);
    expect(patched[0]).toBe(listed[0]);
    expect(patched[1]).toBe(listed[1]);

    // Inserts land in native id order, and removals drop without resending survivors.
    const inserted: EntitySnapshot = { ...belt, id: 3, kind: "container" };
    expect(
      applyBuildingsPatch(listed, { changed: [inserted] }).map(({ id }) => id),
    ).toEqual([1, 2, 3, 4]);
    expect(
      applyBuildingsPatch(listed, { removed: [2, 4] }).map(({ id }) => id),
    ).toEqual([1]);
    expect(
      applyBuildingsPatch(listed, { replace: true, changed: [belt] }),
    ).toEqual([belt]);

    const next = applySnapshotDelta(withLine, 0, {
      base_revision: 0,
      revision: 1,
      tick: 13,
      checksum: 456,
      buildings: { changed: [moved], removed: [2] },
    });
    expect(next.snapshot.buildings.map(({ id }) => id)).toEqual([1, 4]);
    expect(next.snapshot.buildings[1]?.cargo).toEqual({
      item_id: 1,
      quantity: 1,
    });
    expect(next.snapshot.resources).toBe(withLine.resources);
    expect(next.revision).toBe(1);
    // An untouched buildings group leaves the previous list in place.
    expect(
      applySnapshotDelta(withLine, 0, {
        base_revision: 0,
        revision: 1,
        tick: 13,
        checksum: 456,
      }).snapshot.buildings,
    ).toBe(listed);
  });

  it("patches individual deposits without resending the surveyed world's resources", () => {
    const second: ResourceSnapshot = {
      id: 2,
      x: 7096,
      y: -3072,
      radius: 720,
      item_id: 3,
      quantity: 32,
      initial_quantity: 32,
    };
    const listed = [...snapshot.resources, second];
    const withCrystal: FactorySnapshot = { ...snapshot, resources: listed };

    // A drawn-from deposit is substituted in place; every other deposit keeps its identity, so
    // the native ordering the host received survives the patch.
    const drained = { ...listed[0]!, quantity: 46 };
    const patched = applyResourcesPatch(listed, { changed: [drained] });
    expect(patched.map(({ id }) => id)).toEqual([1, 2]);
    expect(patched[0]).toEqual(drained);
    expect(patched[1]).toBe(listed[1]);

    // An empty patch and an untouched group both leave the previous list in place.
    expect(applyResourcesPatch(listed, { changed: [] })).toBe(listed);
    expect(
      applyResourcesPatch(listed, { replace: true, changed: [second] }),
    ).toEqual([second]);

    const next = applySnapshotDelta(withCrystal, 0, {
      base_revision: 0,
      revision: 1,
      tick: 13,
      checksum: 456,
      resources: { changed: [drained] },
    });
    expect(next.snapshot.resources[0]?.quantity).toBe(46);
    expect(next.snapshot.resources[1]).toBe(second);
    expect(next.snapshot.buildings).toBe(withCrystal.buildings);
    expect(next.snapshot.terrain).toBe(withCrystal.terrain);
    expect(
      applySnapshotDelta(withCrystal, 0, {
        base_revision: 0,
        revision: 1,
        tick: 13,
        checksum: 456,
      }).snapshot.resources,
    ).toBe(listed);
  });

  it("rejects missing or out-of-order snapshot revisions", () => {
    expect(() =>
      applySnapshotDelta(snapshot, 3, {
        base_revision: 2,
        revision: 3,
        tick: 13,
        checksum: 456,
      }),
    ).toThrow(/expected 3, received 2/);
    expect(() =>
      applySnapshotDelta(snapshot, 3, {
        base_revision: 3,
        revision: 5,
        tick: 13,
        checksum: 456,
      }),
    ).toThrow(/advance by one/);
  });

  it("keeps Wasm ownership in a module worker and transports native deltas", () => {
    const hostSource = readFileSync(
      new URL("../src/core/FactoryHost.ts", import.meta.url),
      "utf8",
    );
    const workerSource = readFileSync(
      new URL("../src/core/factory.worker.ts", import.meta.url),
      "utf8",
    );
    expect(hostSource).toContain('new Worker(new URL("./factory.worker.ts"');
    expect(hostSource).not.toContain("factory_wasm.js");
    expect(workerSource).toContain(
      'from "../../factory-wasm/pkg/factory_wasm.js"',
    );
    expect(workerSource).toContain("factory.advance_json(");
    expect(workerSource).toContain("factory.snapshot_delta_json()");
  });

  it("derives the fog of war from native chunk bounds only", () => {
    // Inside the one surveyed chunk, on its exclusive far edge, and far outside it.
    expect(isSurveyed(snapshot.chunks, { x: 5322, y: 0 })).toBe(true);
    expect(isSurveyed(snapshot.chunks, { x: 0, y: 0 })).toBe(true);
    expect(isSurveyed(snapshot.chunks, { x: 16384, y: 0 })).toBe(false);
    expect(isSurveyed(snapshot.chunks, { x: -1, y: 4000 })).toBe(false);
    expect(isSurveyed([], { x: 0, y: 0 })).toBe(false);

    const renderer = readFileSync(
      new URL("../src/rendering/CanvasFactoryRenderer.ts", import.meta.url),
      "utf8",
    );
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    // Fog is presentation over native chunk truth: the renderer must not invent chunk geometry.
    expect(renderer).toContain("this.drawFog(");
    expect(renderer).toContain("destination-out");
    expect(renderer).not.toMatch(/span\s*=\s*\d/);
    expect(main).toContain("isSurveyed(snapshot.chunks");
  });

  it("ships responsive controls and accessible labels", () => {
    const html = readFileSync(
      new URL("../index.html", import.meta.url),
      "utf8",
    );
    const styles = readFileSync(
      new URL("../src/styles.css", import.meta.url),
      "utf8",
    );
    expect(html).toContain('aria-label="Interactive HexFactory world map');
    expect(html).toContain('id="technology-list"');
    expect(html).toContain('id="continue"');
    expect(html).toContain("<kbd>W</kbd>");
    // The drag, copy, and undo bindings are documented in the page itself, not only in the repo.
    expect(html).toContain("<kbd>Q</kbd>");
    expect(html).toContain("<kbd>Ctrl</kbd>+<kbd>Z</kbd>");
    expect(html).toContain('id="next-action-title"');
    expect(html).toContain('data-move-key="KeyW"');
    expect(html).toContain('data-native-action="gather"');
    expect(html).toContain('aria-label="Current mission"');
    expect(html).toContain('id="surveyed-value"');
    expect(styles).toContain("height: 100dvh");
    expect(styles).toContain("@media (max-width: 720px)");
    expect(styles).toContain("prefers-reduced-motion: reduce");
  });
});

function fakeTransport(): {
  transport: FactoryTransport;
  requests: Array<{ method: FactoryWorkerMethod; payload: unknown }>;
} {
  const requests: Array<{ method: FactoryWorkerMethod; payload: unknown }> = [];
  let revision = 0;
  const response = (
    patch: Partial<Omit<FactorySnapshot, "buildings" | "resources">>,
  ): FactorySnapshotDelta => ({
    base_revision: revision,
    revision: (revision += 1),
    tick: patch.tick ?? snapshot.tick,
    checksum: patch.checksum ?? snapshot.checksum,
    ...patch,
  });
  const transport: FactoryTransport = {
    request: async <T>(method: FactoryWorkerMethod, payload?: unknown) => {
      requests.push({ method, payload });
      if (method === "save") return "HXF1\n{} " as T;
      if (method === "load")
        return response({ events: ["HXF1 save restored"] }) as T;
      if (method === "advance") return response({ tick: 14 }) as T;
      throw new Error(`Unexpected test method ${method}`);
    },
    dispose: vi.fn(),
  };
  return { transport, requests };
}
