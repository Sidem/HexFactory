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
import passabilityFixture from "../fixtures/terrain-passability.json";
import {
  buildingAvailability,
  technologyAvailability,
} from "../src/core/availability";
import {
  encodeCommand,
  halfTransfer,
  MAX_AIM_COORDINATE,
} from "../src/core/commands";
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
import {
  TERRAIN_INFO,
  TERRAIN_ORDER,
  terrainAccess,
} from "../src/core/terrain";
import type {
  BuildingDefinition,
  EntitySnapshot,
  FactorySnapshot,
  FactorySnapshotDelta,
  ResourceSnapshot,
  Terrain,
  WorldParams,
} from "../src/core/types";
import definitions from "../src/data/definitions.json";
import technologies from "../src/data/technologies.json";
import { HexCamera, isSurveyed } from "../src/rendering/CanvasFactoryRenderer";
import { findLandingHub, homeBearing } from "../src/rendering/landmarks";

const snapshot: FactorySnapshot = {
  scenario: "new-game",
  scenario_name: "New game",
  world_version: 3,
  seed: 1213486160,
  tick: 12,
  checksum: 123,
  delivered: 2,
  delivered_by_item: [{ item_id: 1, quantity: 2 }],
  insight: 4,
  victory: false,
  contract: {
    key: "founding",
    name: "Founding contract",
    stage: 0,
    stages: 2,
    stage_key: "components",
    stage_name: "Prove the line",
    stage_brief: "Deliver three components to the landing hub.",
    requirements: [{ item_id: 2, delivered: 0, required: 3 }],
    complete: false,
  },
  requests: [
    {
      key: "ore-assay",
      name: "Ore assay",
      brief: "A sample of the highland seam.",
      item_id: 1,
      delivered: 2,
      required: 10,
      insight: 10,
    },
  ],
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
    radius: 580,
    action_cooldown_total: 6,
  },
  researched: [1],
  chunks: [
    { chunk_q: 0, chunk_r: 0, entity_count: 1, x: 0, y: 0, span: 16384 },
  ],
  terrain: [
    {
      q: 2,
      r: 1,
      x: 3550,
      y: 1500,
      radius: 1024,
      terrain: "shallow_water",
    },
  ],
  resources: [
    {
      q: 3,
      r: 0,
      x: 5322,
      y: 0,
      radius: 1024,
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
      fuel_charge: 0,
      fuel_required: 0,
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

  it("keeps a following camera on the player when the wheel zooms", () => {
    const camera = new HexCamera();
    const player = { x: 3550, y: -3072 };
    camera.recenter(player);
    expect(camera.following).toBe(true);
    camera.zoomAt(1.6, { x: 80, y: 40 }, 900, 650);
    expect(camera.following).toBe(true);
    expect(camera.pan).toEqual({ x: 0, y: 0 });
    expect(camera.center).toEqual(player);
    camera.follow({ x: 4000, y: 0 });
    expect(camera.center).toEqual({ x: 4000, y: 0 });
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
    // Precision walking is a smaller intent, never a smaller step: the magnitude field native
    // already accepts is what carries it, so no rule about the player's clock moves for it. At
    // full speed a hex column takes about a quarter of a second, which is why holding a direction
    // overshoots one; at 0.4 it is closer to two thirds of a second and a single hex is aimable.
    expect(movementIntent(new Set(["KeyD"]), true)).toEqual({
      type: "move_intent",
      x: 400,
      y: 0,
    });
    expect(movementIntent(new Set(["KeyW", "KeyD"]), true)).toEqual({
      type: "move_intent",
      x: 283,
      y: -283,
    });
    // Still bounded by what the command encoder will accept, precise or not.
    for (const precise of [false, true])
      for (const keys of [["KeyW"], ["KeyW", "KeyD"], ["KeyA", "KeyS"]]) {
        const intent = movementIntent(new Set(keys), precise);
        expect(Math.abs(intent.x)).toBeLessThanOrEqual(1000);
        expect(Math.abs(intent.y)).toBeLessThanOrEqual(1000);
        expect(Number.isInteger(intent.x)).toBe(true);
        expect(Number.isInteger(intent.y)).toBe(true);
      }
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

  it("halves a transfer without ever asking for nothing", () => {
    // The full amount is the default and half is the increment beside it. Rounding up, floored at
    // one, is what keeps the control from being a button that does nothing on a single unit.
    expect(halfTransfer(30)).toBe(15);
    expect(halfTransfer(7)).toBe(4);
    expect(halfTransfer(2)).toBe(1);
    expect(halfTransfer(1)).toBe(1);
    expect(halfTransfer(0)).toBe(1);
    // It is only ever a ceiling: native clamps it to the stock, the carrying room, and the
    // container's capacity, and reports how much actually moved.
    expect(
      encodeCommand({
        type: "store",
        q: 1,
        r: 1,
        item_id: 6,
        quantity: halfTransfer(30),
      }),
    ).toEqual({ opcode: 15, args: [1, 1, 6, 15] });
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
    expect(
      encodeCommand({ type: "set_recipe", q: 1, r: 1, recipe_id: 6 }),
    ).toEqual({ opcode: 11, args: [1, 1, 6] });
    // An upgrade names a hex and nothing else. Which tier it becomes, what it costs, and what it
    // hands back are all native's, so the host cannot describe an upgrade native would not make.
    expect(encodeCommand({ type: "upgrade", q: 3, r: -2 })).toEqual({
      opcode: 13,
      args: [3, -2],
    });
    // A right-click names a hex to harvest and nothing else. Whether it is in reach, and whether
    // it holds anything, are native's — the host never re-derives the gather predicate.
    expect(encodeCommand({ type: "gather_at", q: 4, r: 0 })).toEqual({
      opcode: 14,
      args: [4, 0],
    });
    // Storing is the mirror of withdrawing, on the same ceiling-not-demand contract.
    expect(
      encodeCommand({ type: "store", q: 1, r: 1, item_id: 2, quantity: 7 }),
    ).toEqual({ opcode: 15, args: [1, 1, 2, 7] });
    // An aim carries the world point under the cursor, not a heading: native resolves the facing
    // vector, because facing is a checksum input and normalizing it here would decide one.
    expect(encodeCommand({ type: "aim", x: -4200, y: 1774 })).toEqual({
      opcode: 12,
      args: [-4200, 1774],
    });
    expect(() => encodeCommand({ type: "aim", x: 0.5, y: 0 })).toThrow(
      RangeError,
    );
    expect(() =>
      encodeCommand({ type: "aim", x: MAX_AIM_COORDINATE + 1, y: 0 }),
    ).toThrow(RangeError);

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

  it("keeps the reach and capacity rules native for both new hand actions", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    // A right-click sends the hex it named. The host must not decide whether it is close enough,
    // or which cell "close to the player" resolves to — that is the shared gather predicate.
    expect(main).toContain('type: "gather_at"');
    expect(main).not.toMatch(/EXTRACT_RADIUS|axialDistance\(.*player/);
    // A Put sends what the row's control names as a ceiling, exactly as a Take does. Neither side
    // re-derives the container's remaining room: native clamps and reports what actually moved.
    expect(main).toContain('command: "store"');
    expect(main).toContain('command: "withdraw"');
    expect(main).not.toMatch(/capacity\s*-\s*/);
    // A held right-click repeats through the frame loop and is paced by the native cooldown, the
    // same way a held F is. Sending it only on release would make the player click per unit, and
    // a host-side repeat timer would be the host pacing an action native already paces.
    // It repeats from the same `!input.size` guard the held F uses, so both are paced by the
    // cooldown rather than by a host-side timer of their own.
    const repeat = main.slice(
      main.indexOf("if (!input.size) {"),
      main.indexOf("sendAim();"),
    );
    expect(repeat).toContain('type: "gather_at"');
    expect(repeat).toContain('type: "gather"');
    expect(repeat).not.toMatch(/setInterval|setTimeout/);
    // Take and Put are one function, so the direction is data and not a second copy of the row.
    // Two near-identical renderers is how the two halves drift, and the fractional deposit — which
    // belongs to both — is what would have been written twice.
    expect(main).toContain("function renderTransferRows(");
    expect(main).not.toContain("function renderInspectorActionsRow");
    // That one list carries a control, so it must be patched rather than rebuilt: a
    // `replaceChildren` between pointerdown and pointerup detaches the pressed button and the
    // delegated click resolves to nothing.
    const transfer = main.slice(
      main.indexOf("function renderTransferRows("),
      main.indexOf("function renderInspectorActions("),
    );
    expect(transfer).toContain("syncChildren(");
    expect(transfer).not.toContain("replaceChildren");
  });

  it("keeps the hotbar a preference and never a simulation input", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    // The bar is an arrangement of keys, not a fact about a factory. It lives in localStorage
    // beside no game state, and nothing about it may ever reach a command or the checksum.
    expect(main).toContain("hexfactory:hotbar:v1");
    const hotbarRegion = main.slice(
      main.indexOf("function loadHotbar"),
      main.indexOf("function renderHotbar"),
    );
    // Reading the definition catalogue to validate a stored id is fine; sending anything is not.
    expect(hotbarRegion).not.toContain("enqueue(");
    expect(hotbarRegion).not.toContain("input.");
    expect(main).not.toMatch(/enqueue\(\{[^}]*hotbar/);
    // A stored slot naming a definition this build retired must be dropped, not rendered as a
    // button that selects nothing.
    expect(main).toContain("function sanitiseSlot");
    // The dock no longer enumerates every buildable definition: that list is the catalogue's job
    // and it grew to twenty stamps by v0.14.
    expect(main).not.toMatch(/toolShelf\.append/);
    // Both new lists carry controls, so both are patched rather than rebuilt.
    for (const fn of ["renderHotbarSlots", "renderBuildPanel"]) {
      const region = main.slice(
        main.indexOf(`function ${fn}`),
        main.indexOf(`function ${fn}`) + 1400,
      );
      expect(region, `${fn} patches in place`).toContain("syncChildren(");
      expect(region, `${fn} does not rebuild`).not.toContain("replaceChildren");
    }
  });

  it("keeps panel arrangement a preference and lets panels open independently", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    const css = readFileSync(
      new URL("../src/styles.css", import.meta.url),
      "utf8",
    );
    const html = readFileSync(
      new URL("../index.html", import.meta.url),
      "utf8",
    );
    // Which panels are open is a preference about a screen, on exactly the terms the hotbar sets:
    // localStorage, never saved with the game, never hashed, never sent.
    expect(main).toContain("hexfactory:panels:v1");
    const panelRegion = main.slice(
      main.indexOf("function savePanelState"),
      main.indexOf("function syncPanelToggles"),
    );
    expect(panelRegion).not.toContain("enqueue(");
    expect(panelRegion).not.toContain("host.");
    expect(main).not.toMatch(/enqueue\(\{[^}]*panel/i);
    // Opening a panel no longer closes the rest. Exclusivity survives only below the width where
    // there is genuinely one rectangle to share.
    const toggle = main.slice(
      main.indexOf("function togglePanel("),
      main.indexOf("\n}", main.indexOf("function togglePanel(")),
    );
    expect(toggle).toContain("ONE_PANEL_AT_A_TIME");
    expect(toggle).not.toMatch(/^\s*closePanels\(target\);$/m);
    // The exclusivity was covering for a layout: four panels at one origin. The rails are what
    // replaced it, so no panel may reclaim an absolute origin of its own.
    expect(css).toContain(".panel-rail");
    expect(html).toContain("panel-rail rail-left");
    expect(html).toContain("panel-rail rail-right");
    // `.glass-panel` is a flow child of a rail now, so nothing about a panel positions itself.
    const glass = css.slice(
      css.indexOf(".glass-panel {"),
      css.indexOf("}", css.indexOf(".glass-panel {")),
    );
    expect(glass).not.toMatch(/position:\s*absolute/);
    for (const panel of [
      "inventory-panel",
      "research-panel",
      "quest-panel",
      "build-panel",
      "inspector-panel",
      "session-panel",
    ])
      expect(
        css,
        `.${panel} sits in a rail rather than at an origin`,
      ).not.toMatch(
        // A property boundary, so `margin-left` is not read as an origin.
        new RegExp(
          `\\.${panel}[^{}]*\\{[^}]*[;{\\s](position:\\s*absolute|top:|left:|right:)`,
        ),
      );
    // Escape, a new game, and a load still clear the screen.
    expect(main).toContain("function closePanels(");
  });

  it("draws every item through the one chip component", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    const chip = readFileSync(
      new URL("../src/rendering/itemChip.ts", import.meta.url),
      "utf8",
    );
    // One component is the only place an item glyph, name, or count is written. Eight bespoke
    // shapes is what this replaced, and a ninth would start the drift again.
    expect(chip).toContain('class="item-chip-glyph"');
    expect(main).not.toContain('class="item-chip');
    // An item always shows its glyph: the bare colour swatch on the contract bill and the request
    // board is gone, and colour alone is not an identity in a catalogue with three greys in it.
    expect(main).not.toMatch(/swatch"\)\.style\.background/);
    // One spelling per meaning. `×3` was a third spelling of a plain amount.
    expect(main).not.toContain("`×${quantity}`");
    // Every item drawing goes through the same patcher, so a chip inside a list carrying a control
    // is never rebuilt between pointerdown and pointerup.
    expect(main).toContain("function paintChip(");
    expect(chip).toContain("export function fillItemChip(");
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
      "renderTransferRows",
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
      costLabel: "1 Iron ore",
      cost: [{ item_id: 1, required: 1, held: 3, shortfall: 0 }],
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

  it("names a world by preset or by parameters and re-reads what it got", async () => {
    const { transport, requests } = fakeTransport();
    const preset = {
      key: "basin",
      name: "Basin",
      description: "Great contiguous seas around broad land.",
      params: TEST_WORLD_PARAMS,
    };
    const host = FactoryHost.forTesting(transport, snapshot, 0, 30, [preset]);
    await host.newGame("new-game", 7, "basin");
    const tuned = { ...TEST_WORLD_PARAMS, water_level: 26000 };
    await host.newGame("new-game", 7, tuned);
    expect(requests.map((entry) => entry.payload)).toEqual([
      { scenario: "new-game", seed: 7, worldParams: "basin" },
      { scenario: "new-game", seed: 7, worldParams: tuned },
    ]);

    // The parameters come back from native rather than from what was asked for, and only once:
    // a world's parameters cannot change without a new world.
    expect(await host.worldParams()).toEqual(TEST_WORLD_PARAMS);
    expect(await host.worldParams()).toEqual(TEST_WORLD_PARAMS);
    expect(
      requests.filter((entry) => entry.method === "worldParams"),
    ).toHaveLength(1);

    expect(host.presetKeyFor(TEST_WORLD_PARAMS)).toBe("basin");
    expect(host.presetKeyFor(tuned)).toBeUndefined();
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
      fuel_charge: 0,
      fuel_required: 0,
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
      q: 4,
      r: -2,
      x: 7096,
      y: -3072,
      radius: 1024,
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
    expect(patched.map(({ q, r }) => `${q},${r}`)).toEqual(["3,0", "4,-2"]);
    expect(patched[0]).toEqual(drained);
    expect(patched[1]).toBe(listed[1]);

    // A patch touches the harvested cell and nothing else, including in the negative-coordinate
    // world where a 64-bit id packed from q and r used to round to the same JSON number for a
    // whole column of the field — harvesting one cell then overwrote its neighbours with a copy
    // of it, so hexes the player never touched changed their amount and their position.
    const column: ResourceSnapshot[] = [0, 1, 2, 3].map((r) => ({
      q: -32,
      r,
      x: -56768,
      y: r * 1536,
      radius: 1024,
      item_id: 1,
      quantity: 20,
      initial_quantity: 20,
    }));
    const harvested = { ...column[2]!, quantity: 19 };
    expect(applyResourcesPatch(column, { changed: [harvested] })).toEqual([
      column[0],
      column[1],
      harvested,
      column[3],
    ]);

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
    // The delta leaves native already encoded and is handed over rather than copied. A worker that
    // parsed it here would rebuild the object graph only for the structured clone to copy it
    // again, which is the cost `docs/BENCHMARKS.md` finding 3 priced at about 10 µs per kilobyte.
    expect(workerSource).toContain("factory.snapshot_delta_bytes()");
    expect(workerSource).not.toContain("factory.snapshot_delta_json()");
    expect(workerSource).toContain("result instanceof ArrayBuffer ? [result]");
    expect(hostSource).toContain("decodeSnapshotDelta(result)");
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
    expect(renderer).toContain("player.radius");
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
    // The next step is permanent chrome, not something behind a key a new player has to find.
    expect(html).toContain('id="next-step-title"');
    expect(html).toContain('id="next-step-detail"');
    // Comfort controls exist in the product, both on the bar and in the menu, and the precision
    // walk that fixes single-hex overshoot is documented in the page rather than only in the repo.
    expect(html).toContain('id="sound"');
    expect(html).toContain('id="reduce-motion"');
    expect(html).toContain('id="mute"');
    expect(html).toContain("<kbd>Shift</kbd>");
    // Progressive disclosure has a visible control on both catalogues, so nothing is silently held
    // back from a player who wants the whole tree.
    expect(html).toContain('id="research-scope"');
    expect(html).toContain('id="build-scope"');
    expect(html).toContain('data-move-key="KeyW"');
    expect(html).toContain('data-native-action="gather"');
    expect(html).toContain('aria-label="Current mission"');
    expect(html).toContain('id="surveyed-value"');
    expect(styles).toContain("height: 100dvh");
    expect(styles).toContain("@media (max-width: 720px)");
    expect(styles).toContain("prefers-reduced-motion: reduce");
    // The recipe a machine runs is chosen before placing and changeable after, both reachable by
    // keyboard and both named.
    expect(html).toContain(
      'aria-label="Recipe for the machine about to be built"',
    );
    expect(html).toContain('aria-label="Recipe for the inspected machine"');
  });

  it("pins the terrain passability table to the Rust rule it copies", () => {
    // The host draws impassable ground as one category, so it holds a copy of a rule native owns.
    // A copy drifts; this is what stops it. Rust asserts the same file against
    // `Terrain::blocks_movement` and `Terrain::blocks_construction`.
    const entries = passabilityFixture as {
      terrain: Terrain;
      passable: boolean;
      buildable: boolean;
    }[];
    expect(entries.map(({ terrain }) => terrain)).toEqual(TERRAIN_ORDER);
    for (const entry of entries) {
      const band = TERRAIN_INFO[entry.terrain];
      expect(band.passable, entry.terrain).toBe(entry.passable);
      expect(band.buildable, entry.terrain).toBe(entry.buildable);
      expect(terrainAccess(band), entry.terrain).toBe(
        entry.passable ? "Buildable" : "Impassable",
      );
    }
    // The three the player keeps walking into, named rather than counted.
    expect(
      entries.filter(({ passable }) => !passable).map((e) => e.terrain),
    ).toEqual(["deep_water", "shallow_water", "cliff"]);

    const renderer = readFileSync(
      new URL("../src/rendering/CanvasFactoryRenderer.ts", import.meta.url),
      "utf8",
    );
    // Impassability is drawn from the table, not from a second opinion about which grey is cliff.
    expect(renderer).toContain("if (!band.passable) this.drawImpassable(");
    expect(renderer).not.toContain('case "cliff"');
  });

  it("always knows which way the landing hub is", () => {
    const hub = findLandingHub(snapshot);
    // The hub's world position comes from its axial coordinate at the native lattice scale.
    expect(hub).toEqual({ x: 0, y: 0 });
    expect(findLandingHub({ ...snapshot, buildings: [] })).toBeNull();

    // Due west of the hub, eight hexes out: the bearing points east and says how far in hexes.
    const away = { ...snapshot.player, x: -8 * 1774, y: 0 };
    const bearing = homeBearing(away, hub as { x: number; y: number });
    expect(bearing?.x).toBeCloseTo(1, 6);
    expect(bearing?.y).toBeCloseTo(0, 6);
    expect(bearing?.hexes).toBe(8);
    expect(bearing?.direction).toBe(0);
    // Standing on it names no direction rather than an arbitrary one.
    expect(homeBearing({ x: 0, y: 0 }, { x: 0, y: 0 })).toBeNull();
    // Southeast is direction 1, the same numbering the rest of the game uses.
    expect(homeBearing({ x: 0, y: 0 }, { x: 500, y: 866 })?.direction).toBe(1);
    expect(homeBearing({ x: 0, y: 0 }, { x: -500, y: -866 })?.direction).toBe(
      4,
    );
  });

  it("keeps the world in view and every panel behind its own key", () => {
    const html = readFileSync(
      new URL("../index.html", import.meta.url),
      "utf8",
    );
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    // Pack, research, and the objective guide wait behind I, O, and P.
    expect(main).toContain('KeyI: "inventory-panel"');
    expect(main).toContain('KeyO: "research-panel"');
    expect(main).toContain('KeyP: "quest-panel"');
    // The inspector is the exception: it has no key because it never leaves the world.
    expect(main).not.toContain('"inspector-panel"');
    // Space centres the camera and pause moved off it. A clicked button must not keep Space:
    // activation is on keyup, so keydown alone would both skip recenter and press the control.
    expect(main).toContain('event.code === "Space") renderer.recenter()');
    expect(main).toContain('event.code === "Space"');
    expect(main).toContain("target.blur()");
    expect(main).toContain('event.code === "KeyT") setPlaying(!playing)');
    // Gather and deliver are permanent chrome in the dock, not a panel a new player has to find.
    expect(html).toContain('class="field-actions"');
    expect(html).toContain('id="minimap"');
    expect(html).toContain('id="home-readout"');
    expect(html).toContain("<kbd>Space</kbd>");
    expect(html).toContain("<kbd>I</kbd>");
  });

  it("offers each machine only the recipes of its own category", () => {
    // The host must not hand a machine "the first recipe in the catalog": native would refuse it,
    // and a build tool that cannot place anything is a defect the player has to diagnose.
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    const hostSource = readFileSync(
      new URL("../src/core/FactoryHost.ts", import.meta.url),
      "utf8",
    );
    expect(main).toContain("function recipeChoices(");
    expect(main).not.toContain("host.definitions.recipes[0]");
    expect(hostSource).not.toContain("this.definitions.recipes[0]");

    const byCategory = (key: string): string[] => {
      const definition = definitions.buildings.find(
        (building) => building.key === key,
      ) as BuildingDefinition;
      return definitions.recipes
        .filter(({ category }) => category === definition.recipe_category)
        .map(({ key: recipe }) => recipe);
    };
    expect(byCategory("kiln")).toEqual(["brick", "charcoal"]);
    expect(byCategory("crusher")).toEqual(["gravel"]);
    expect(byCategory("smelter")).toContain("steel");
    expect(byCategory("smelter")).not.toContain("circuit");
  });

  it("draws the harvest wait where the harvest happens, from native numbers", () => {
    const renderer = readFileSync(
      new URL("../src/rendering/CanvasFactoryRenderer.ts", import.meta.url),
      "utf8",
    );
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    // Both the wait outstanding and what a fresh one is worth are published, so the ring is a
    // proportion the host was given rather than a maximum it inferred by watching a value fall.
    expect(renderer).toContain("action_cooldown_total");
    expect(renderer).toContain("drawActionCooldown(");
    expect(renderer).not.toMatch(/action_cooldown\w*\s*=\s*\d/);
    // And the refusal it replaces no longer reaches the message strip.
    expect(main).toContain('SILENT_EVENTS = new Set(["action cooling down"])');
    expect(snapshot.player.action_cooldown_total).toBeGreaterThan(0);
  });

  it("names every surveyed hex, including the band native does not send", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    // Lowland is the default surveyed fill and is deliberately omitted from the terrain group, so
    // a surveyed hex with no entry is lowland — not an unknown tile and not a hole in the world.
    expect(main).toContain('?.terrain ?? "lowland"');
    for (const band of TERRAIN_ORDER) {
      expect(TERRAIN_INFO[band].name, band).toBeTruthy();
      expect(TERRAIN_INFO[band].note, band).toBeTruthy();
    }
    // A field hex leads with what is on it. Band potentials stay on empty ground.
    const inspectorStart = main.indexOf("function renderInspector(");
    expect(main.indexOf("if (resource)", inspectorStart)).toBeLessThan(
      main.indexOf('?.terrain ?? "lowland"', inspectorStart),
    );
  });

  it("reads a clicked hex as cards, not as a text dump", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    const html = readFileSync(
      new URL("../index.html", import.meta.url),
      "utf8",
    );
    const css = readFileSync(
      new URL("../src/styles.css", import.meta.url),
      "utf8",
    );
    // The heading is the hex, not the active tool. Coordinates are a labelled chip.
    expect(html).toContain('id="inspect-title"');
    expect(html).toContain('id="inspect-q"');
    expect(html).toContain('id="inspect-compass"');
    // The field cell is a metered item chip. Static markup names a holder rather than spelling the
    // chip out, so `createItemChip` stays the only place its shape is written down.
    expect(html).toContain('id="inspect-field-chip"');
    expect(html).not.toContain("item-chip-glyph");
    expect(html).not.toContain('id="selected-tool-value"');
    // Direction 0 never reaches the player; the six names and a compass do.
    expect(main).toContain("DIRECTION_NAMES[building.orientation]");
    expect(main).not.toContain("Direction ${building.orientation}");
    expect(main).not.toContain("lines.join");
    // A proportion is both published numbers, same rule as the cooldown ring.
    expect(main).toContain("resource.quantity");
    expect(main).toContain("resource.initial_quantity");
    expect(css).not.toMatch(/\.inspector\s*\{[^}]*white-space:\s*pre-line/);
  });

  it("shrinks the hex lattice on screen and keeps counts off untouched fields", () => {
    const renderer = readFileSync(
      new URL("../src/rendering/CanvasFactoryRenderer.ts", import.meta.url),
      "utf8",
    );
    // More hexes in the viewport is a presentation knob, not another PLAYER_RADIUS bump.
    expect(renderer).toContain("export const BASE_HEX_SIZE = 22");
    expect(renderer).toContain("const drawnFrom");
    expect(renderer).toContain("drawFieldLabel(");
    // The old always-on count is what turned the landscape into a spreadsheet.
    expect(renderer).not.toContain("String(resource.quantity)");
  });

  it("resolves definitions once and keeps the bench out of the game", () => {
    const renderer = readFileSync(
      new URL("../src/rendering/CanvasFactoryRenderer.ts", import.meta.url),
      "utf8",
    );
    const minimap = readFileSync(
      new URL("../src/rendering/MinimapRenderer.ts", import.meta.url),
      "utf8",
    );
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    const vite = readFileSync(
      new URL("../vite.config.ts", import.meta.url),
      "utf8",
    );
    // Per-entity .find() inside the draw loops is the thing the renderer measurement should
    // not have to answer. Lookups are built once from the roster.
    expect(renderer).toContain('from "./terrainLook"');
    expect(renderer).toContain('from "./buildingLook"');
    expect(renderer).toContain("this.itemsById = new Map(");
    expect(renderer).toContain("this.buildingsById = new Map(");
    expect(renderer).not.toContain("definitions.items.find(");
    expect(renderer).not.toContain("definitions.buildings.find(");
    expect(minimap).toContain("this.itemsById = new Map(");
    expect(minimap).not.toContain("definitions.items.find(");
    expect(main).not.toMatch(/from ["'].*bench/);
    expect(renderer).not.toMatch(/from ["'].*bench/);
    expect(minimap).not.toMatch(/from ["'].*bench/);
    // The production build's only HTML entry is index.html. bench.html is served in dev only.
    expect(vite).not.toContain("bench.html");
  });
});

/** The shipped `continental` numbers, as native would report them. */
const TEST_WORLD_PARAMS: WorldParams = {
  elevation_coarse_cell: 8,
  elevation_fine_cell: 3,
  elevation_coarse_weight: 50,
  moisture_cell: 7,
  richness_cell: 5,
  vein_cell: 4,
  water_level: 18000,
  shore_level: 24000,
  hills_level: 33000,
  highland_level: 42000,
  cliff_step: 14000,
  deep_water_moisture: 40000,
  field_rules: [
    {
      terrain: "cliff",
      item_id: 6,
      moisture_min: -1,
      richness_min: 50000,
      vein_min: -1,
      base: 24,
      spread: 25,
    },
  ],
};

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
      if (method === "newGame") return response({ tick: 0 }) as T;
      // A world's parameters change only when the world does, so they are asked for on demand
      // rather than carried in every frame's delta.
      if (method === "worldParams") return TEST_WORLD_PARAMS as T;
      throw new Error(`Unexpected test method ${method}`);
    },
    dispose: vi.fn(),
  };
  return { transport, requests };
}
