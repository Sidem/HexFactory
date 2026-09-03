import { skillView } from "../src/ui/skills";
import { readFileSync } from "node:fs";

import { describe, expect, it, vi } from "vitest";

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
import { SAVE_VERSION } from "../src/core/saveSlots";
import {
  applyBuildingsPatch,
  applyResourcesPatch,
  applySnapshotDelta,
  applyTerrainPatch,
} from "../src/core/snapshotDelta";
import {
  bandAt,
  cliffQuarried,
  physicalAccess,
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
  Substrate,
  Terrain,
  Technologies,
  WorldParams,
} from "../src/core/types";
import definitions from "../src/data/definitions.json";
import technologiesJson from "../src/data/technologies.json";
import { readStyles } from "./sourceGraph";

const technologies = technologiesJson as unknown as Technologies;
import { isSurveyed } from "../src/rendering/CanvasFactoryRenderer";
import {
  buildingBeside,
  findLandingHub,
  homeBearing,
} from "../src/rendering/landmarks";

const snapshot: FactorySnapshot = {
  boundaries: [],
  ground: [],
  water: [],
  spoil: 0,
  scenario: "new-game",
  scenario_name: "New game",
  world_version: 3,
  seed: 1213486160,
  tick: 12,
  checksum: 123,
  belt_transit_ticks: 27,
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
      state: "posted",
    },
  ],
  player: {
    x: 1774,
    y: 0,
    facing_x: 1000,
    facing_y: 0,
    move_x: 0,
    move_y: 0,
    creative: false,
    inventory: { "1": 3, "24": 3 },
    action_cooldown: 0,
    build_range: 8870,
    carry_slots: 6,
    carry_stacks: [{ item_id: 1, quantity: 3 }],
    radius: 580,
    action_cooldown_total: 6,
    extract_radius: 1,
    walk_goal: null,
    walk_path: [],
  },
  researched: [1],
  skills: {
    points: 0,
    purchased: [],
    granted: [],
    completed: [],
    sandbox: false,
    availability: [],
  },
  research_availability: [
    {
      technology_id: 1,
      complete: true,
      missing_prerequisites: [],
      insight_shortfall: 0,
    },
    {
      technology_id: 3,
      complete: false,
      missing_prerequisites: [2],
      insight_shortfall: 0,
    },
  ],
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
      height: 0,
      substrate: "soil",
      water_depth: 1,
      discharge: 0,
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
  ground_items: [],
  events: [],
};

describe("bounded host input", () => {
  it("queues a stack gesture entirely or leaves it entirely unsent", () => {
    const queue = new BoundedInputQueue();
    const gesture = [
      { type: "pickup_player_stack", item_id: 1, quantity: 2 },
      { type: "place_player_stack", quantity: 2 },
    ] as const;
    for (let i = 0; i < MAX_INPUT_COMMANDS - 1; i += 1)
      queue.enqueue({ type: "move_intent", x: 0, y: 0 });
    expect(queue.enqueueBatch(gesture)).toBe(false);
    expect(queue.drain()).toHaveLength(MAX_INPUT_COMMANDS - 1);
    expect(queue.enqueueBatch(gesture)).toBe(true);
    expect(queue.drain()).toEqual(gesture);
  });
  it("maps WASD to normalized continuous intent and never exceeds one native batch limit", () => {
    expect(MOVEMENT_KEYS).toEqual({
      KeyW: { x: 0, y: -1 },
      KeyA: { x: -1, y: 0 },
      KeyS: { x: 0, y: 1 },
      KeyD: { x: 1, y: 0 },
    });
    expect(movementIntent(new Set(["KeyW", "KeyD"]))).toEqual({
      type: "move_intent",
      x: 424,
      y: -424,
    });
    // Walking is a smaller intent, never a smaller step: native `PLAYER_SPEED` is the 25 m/s run
    // at intent 1000, and the host sends 600 for the 15 m/s walk. Shift is the run, not a precision
    // crawl — the world is scaled so a biome takes minutes, and getting across it is what Shift is
    // for.
    expect(movementIntent(new Set(["KeyD"]))).toEqual({
      type: "move_intent",
      x: 600,
      y: 0,
    });
    expect(movementIntent(new Set(["KeyD"]), true)).toEqual({
      type: "move_intent",
      x: 1000,
      y: 0,
    });
    expect(movementIntent(new Set(["KeyW", "KeyD"]), true)).toEqual({
      type: "move_intent",
      x: 707,
      y: -707,
    });
    // Still bounded by what the command encoder will accept, walking or running.
    for (const running of [false, true])
      for (const keys of [["KeyW"], ["KeyW", "KeyD"], ["KeyA", "KeyS"]]) {
        const intent = movementIntent(new Set(keys), running);
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
    // A walk is a destination and nothing else. There is no route, no gait, and no reach in it:
    // native searches, prices the water, and steers, so an opcode carrying anything more would be
    // the host deciding something the checksum depends on.
    expect(encodeCommand({ type: "walk_to", q: 6, r: -2 })).toEqual({
      opcode: 22,
      args: [6, -2],
    });
    expect(
      encodeCommand({
        type: "water_edit",
        q: 4,
        r: -1,
        action: "drain",
        quanta: 8,
      }),
    ).toEqual({ opcode: 35, args: [4, -1, 1, 8] });
    expect(() =>
      encodeCommand({
        type: "water_edit",
        q: 0,
        r: 0,
        action: "flood",
        quanta: 33,
      }),
    ).toThrow("Invalid water target");
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
    ).toEqual({ opcode: 15, args: [1, 1, 6, 15, 0] });
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
    expect(encodeCommand({ type: "deposit" })).toEqual({ opcode: 2, args: [] });
    expect(encodeCommand({ type: "deposit", item_id: 1 })).toEqual({
      opcode: 2,
      args: [1],
    });
    expect(encodeCommand({ type: "undo" })).toEqual({ opcode: 9, args: [] });
    expect(encodeCommand({ type: "rotate", q: 2, r: -1 })).toEqual({
      opcode: 5,
      args: [2, -1, 0],
    });
    expect(
      encodeCommand({ type: "rotate", q: 2, r: -1, reverse: true }),
    ).toEqual({ opcode: 5, args: [2, -1, 1] });
    expect(
      encodeCommand({
        type: "set_output_route",
        q: 2,
        r: -1,
        item_id: 30,
        output_q: 1,
        output_r: -1,
        direction: 2,
      }),
    ).toEqual({ opcode: 34, args: [2, -1, 30, 1, -1, 2] });
    expect(() =>
      encodeCommand({
        type: "set_output_route",
        q: 2,
        r: -1,
        item_id: 30,
        output_q: 1,
        output_r: -1,
        direction: 6,
      }),
    ).toThrow("Invalid output route");
    expect(
      encodeCommand({ type: "withdraw", q: 1, r: 1, item_id: 2, quantity: 7 }),
    ).toEqual({ opcode: 10, args: [1, 1, 2, 7, 0] });
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
    ).toEqual({ opcode: 15, args: [1, 1, 2, 7, 0] });
    expect(
      encodeCommand({
        type: "pickup_building_stack",
        q: 1,
        r: 1,
        stock: "fuel",
        item_id: 5,
        quantity: 4,
      }),
    ).toEqual({ opcode: 24, args: [1, 1, 3, 5, 4] });
    expect(
      encodeCommand({ type: "pickup_player_stack", item_id: 5, quantity: 4 }),
    ).toEqual({ opcode: 23, args: [5, 4] });
    expect(encodeCommand({ type: "place_player_stack", quantity: 1 })).toEqual({
      opcode: 25,
      args: [1],
    });
    expect(
      encodeCommand({
        type: "place_building_stack",
        q: 1,
        r: 1,
        stock: "input",
        quantity: 4,
      }),
    ).toEqual({ opcode: 26, args: [1, 1, 2, 4] });
    expect(
      encodeCommand({
        type: "drop_player_stack",
        q: 2,
        r: -1,
        quantity: 5,
      }),
    ).toEqual({ opcode: 27, args: [2, -1, 5] });
    // The switch carries the state it wants rather than a flip. Two presses of "off" have to be
    // one answer, not none — a toggle opcode would make the stream order-dependent and let a
    // coalesced or replayed pair cancel out.
    expect(
      encodeCommand({ type: "set_enabled", q: 3, r: -1, enabled: false }),
    ).toEqual({ opcode: 17, args: [3, -1, 0] });
    expect(
      encodeCommand({ type: "set_enabled", q: 3, r: -1, enabled: true }),
    ).toEqual({ opcode: 17, args: [3, -1, 1] });
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
    // Posting names a project by id and nothing else. Which board slot it displaces is native's
    // call, for the same reason the draw order is: the host does not know what the player has
    // committed where, and demand being finite makes that the expensive thing to get wrong.
    expect(encodeCommand({ type: "post_request", request_id: 4 })).toEqual({
      opcode: 28,
      args: [4],
    });
    expect(encodeCommand({ type: "skip_request", slot: 1 })).toEqual({
      opcode: 16,
      args: [1],
    });

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

  it("walks to a second click without ever finding the way itself", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    const overlays = readFileSync(
      new URL("../src/rendering/three/overlays.ts", import.meta.url),
      "utf8",
    );
    const minimap = readFileSync(
      new URL("../src/rendering/MinimapRenderer.ts", import.meta.url),
      "utf8",
    );
    // The gesture is the second click on a hex that is already selected, and it has to read the old
    // selection before the click replaces it. It is confined to `inspect` because every other tool's
    // second click already means place, erase, rotate, or upgrade again.
    const click = main.slice(
      main.indexOf('canvas.addEventListener("click"'),
      main.indexOf("function draggableTool()"),
    );
    expect(click).toContain('tool === "inspect"');
    expect(click).toMatch(/selected\.q === coordinate\.q/);
    expect(click).toMatch(/selected\.r === coordinate\.r/);
    expect(click.indexOf("const repeat")).toBeLessThan(
      click.indexOf("selected = coordinate"),
    );
    expect(main.match(/type: "walk_to"/g)).toHaveLength(1);
    // Player time accrues only while the player has work, and nobody holds a key while native
    // steers. Leave a standing goal out of that predicate and the route is planned and drawn but
    // never walked, because the frame hands native zero player steps.
    const budget = main.slice(
      main.indexOf("frameClock.update(now, {"),
      main.indexOf("playerTicksPerSecond: host.playerTicksPerSecond"),
    );
    expect(budget).toContain("walk_goal !== null");
    // Both drawings of the route come from native's own remaining path. A host-side search would be
    // a second pathfinder, and the picture would eventually promise a way the simulation would not
    // take — over water it prices differently, or through a wall that went up mid-walk.
    expect(overlays).toContain("walk_path");
    expect(minimap).toContain("walk_path");
    for (const source of [main, overlays, minimap])
      expect(source).not.toMatch(
        /aStar|astar|BinaryHeap|openSet|frontier\.push/,
      );
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
    // A stack gesture names an amount and compartment. Native still clamps reach, compatibility,
    // stock, and remaining room, so the host never turns the displayed capacity into authority.
    expect(main).toContain('type: "place_building_stack"');
    expect(main).toContain('type: "pickup_building_stack"');
    expect(main).toContain('type: "store"');
    expect(main).toContain('type: "withdraw"');
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
    // Pack and building slots are one gesture function, so full, half, single, quick-move, pickup,
    // and placement cannot drift into separate interpretations of the same click.
    expect(main).toContain("function stackGesture(");
    expect(main).not.toContain("function renderInspectorActionsRow");
    // Those slots carry gestures, so they must be patched rather than rebuilt: a `replaceChildren`
    // between pointerdown and pointerup detaches the pressed slot and the delegated click resolves
    // to nothing.
    const transfer = main.slice(
      main.indexOf("function renderInspectorActions("),
      main.indexOf("function renderInspectorRecipe("),
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
    // And so must a default, which is the same defect wearing better clothes: v0.25.1 retired the
    // riser, `DEFAULT_HOTBAR` went on naming its id, and the ninth slot of a first-ever run drew as
    // `?18`. The sieve now covers both lists; this asserts the list they start from.
    const listed = /const DEFAULT_HOTBAR[^=]*=\s*\[([^\]]*)\]/.exec(main)?.[1];
    const ids = (listed ?? "")
      .split(",")
      .map((slot) => slot.trim())
      .filter((slot) => /^\d+$/.test(slot))
      .map(Number);
    expect(ids.length).toBeGreaterThan(0);
    for (const id of ids)
      expect(
        definitions.buildings.find((building) => building.id === id)?.buildable,
        `default slot names definition ${id}`,
      ).toBe(true);
    // The dock no longer enumerates every buildable definition: that list is the catalogue's job
    // and it grew to twenty stamps by v0.14.
    expect(main).not.toMatch(/toolShelf\.append/);
    // Grouping is exhaustive over kinds, so adding a buildable kind cannot compile while silently
    // dropping its card. The bridge belongs beside the transport it supports.
    expect(main).toContain(
      "satisfies Record<BuildingKind, BuildGroupKey | null>",
    );
    expect(main).toContain('bridge: "transport"');
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

  it("keeps the active workspace a preference and makes panel interactions exclusive", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    const css = readStyles();
    const html = readFileSync(
      new URL("../index.html", import.meta.url),
      "utf8",
    );
    const panels = readFileSync(
      new URL("../src/ui/panels.ts", import.meta.url),
      "utf8",
    );
    // Which panels are open is a preference about a screen, on exactly the terms the hotbar sets:
    // localStorage, never saved with the game, never hashed, never sent.
    expect(panels).toContain("hexfactory:panels:v1");
    expect(panels).not.toContain("enqueue(");
    expect(panels).not.toContain("host.");
    expect(main).not.toMatch(/enqueue\(\{[^}]*panel/i);
    // A workspace replaces the previous one at every width. This prevents several tall surfaces
    // from shrinking into unreadable slivers while all claiming to be open.
    expect(panels).toContain("if (opening) this.close(target)");
    expect(main).not.toContain("ONE_PANEL_AT_A_TIME");
    // Walking up to a machine brings the inspector out where it is behind a button, and only when
    // no other workspace is open: a footstep must not close a panel the player pressed for.
    expect(main).toContain("panels.revealInspector()");
    expect(panels).toContain('this.root.querySelector(".glass-panel.open")');
    expect(panels).toContain("ids.slice(-1)");
    // Panels remain flow children of their rails rather than reclaiming absolute origins.
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
    // Escape, a new game, a load, and a deliberate gesture on the world clear the workspace.
    expect(main).toContain("function closePanels(");
    const worldPointer = main.slice(
      main.indexOf('canvas.addEventListener("pointerdown"'),
      main.indexOf('canvas.addEventListener("pointerup"'),
    );
    expect(worldPointer).toContain("closePanels()");
  });

  it("keeps a refused slot clearable and answers a recipe from either side", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    const css = readStyles();
    const html = readFileSync(
      new URL("../index.html", import.meta.url),
      "utf8",
    );
    // A disabled button swallows every pointer event inside it, including the × and the drag-off,
    // so a pin made before its research existed had no gesture left that could remove it. The
    // refusal is spoken by the handler now; the button itself stays reachable.
    const slots = main.slice(
      main.indexOf("function renderHotbarSlots("),
      main.indexOf("function renderHotbar("),
    );
    expect(slots).not.toMatch(/disabled = availability\.locked/);
    expect(slots).toContain(
      'setAttribute("aria-disabled", String(availability.locked))',
    );
    expect(main).toContain('getAttribute("aria-disabled") === "true"');
    // Dimming rides on the children so the clear affordance stays at full strength above it.
    expect(css).toContain('.hotbar-slot[aria-disabled="true"] .hotbar-clear');
    // The search stays on screen through a catalogue that scrolls past nine hundred pixels.
    const find = css.slice(
      css.indexOf(".build-find {"),
      css.indexOf("}", css.indexOf(".build-find {")),
    );
    expect(find).toMatch(/position:\s*sticky/);
    // A card's recipes are an answer to pointing at it, not a wall the list is read through — but
    // only where a pointer can hover, so a touch reader is never left without them.
    expect(css).toContain("@media (hover: hover)");
    expect(css).toContain(".build-card:hover .build-recipes:not([hidden])");
    // The lookup answers "what makes this" and "what spends this" from one query, which is why it
    // matches the item rather than the recipe name: no recipe called "gear" makes a gear.
    expect(main).toContain("function itemsMatching(");
    expect(main).toContain("function renderRecipePanel(");
    expect(main).toContain('renderRecipeGroup("recipe-makes"');
    expect(main).toContain('renderRecipeGroup("recipe-uses"');
    expect(html).toContain('id="recipe-panel"');
    expect(html).toContain('data-panel-target="recipe-panel"');
    expect(main).toContain('KeyL: "recipe-panel"');
    // The lookup is a reader, not a second command path: clicking a row selects the tool the
    // catalogue already selects, and nothing about a search reaches the worker.
    const lookup = main.slice(
      main.indexOf("function renderRecipePanel("),
      main.indexOf("function createLookupRow("),
    );
    expect(lookup).not.toContain("enqueue(");
    expect(main).not.toMatch(/enqueue\(\{[^}]*recipeSearch/);
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
    const clock = readFileSync(
      new URL("../src/core/frameClock.ts", import.meta.url),
      "utf8",
    );
    // The rate is native truth. The host converts elapsed real time into a step count with it and
    // never turns a frame delta into a position.
    expect(main).toContain("playerTicksPerSecond: host.playerTicksPerSecond");
    expect(clock).toContain("elapsed * state.playerTicksPerSecond");
    expect(main).not.toMatch(/player\.(x|y)\s*\+/);
    // Player steps use native's player cadence, independently of the factory's fixed-rate clock.
    expect(clock).not.toMatch(
      /playerAccumulator \+= [^;]*SIMULATION_TICKS_PER_SECOND/,
    );
  });

  it("patches keyed lists in place so a rebuild cannot swallow a click", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    const dom = readFileSync(
      new URL("../src/ui/dom.ts", import.meta.url),
      "utf8",
    );
    // A `replaceChildren` between pointerdown and pointerup destroys the pressed control, the
    // click retargets to the container, and the delegated handler finds nothing. Every list that
    // carries a control goes through the reconciler instead.
    expect(dom).toContain("export function syncChildren(");
    expect(dom).not.toContain("replaceChildren()");
    expect(main).toContain('from "./ui/dom"');
    const research = readFileSync(
      new URL("../src/ui/researchTree.ts", import.meta.url),
      "utf8",
    );
    expect(research).toContain("syncChildren(");
    expect(research).toContain("technologyAvailability(tech, this.snapshot)");
    expect(main).toContain("researchTree.update(snapshot)");
    expect(main).toContain("if (researchDialog.open || skillsDialog.open)");
    const saveList = readFileSync(
      new URL("../src/ui/saveList.ts", import.meta.url),
      "utf8",
    );
    // The hotbar's buttons are built once, so it needs no reconciler — but rewriting their inner
    // nodes on every snapshot loses a click the same way, so it patches text instead.
    const hotbar = main.slice(
      main.indexOf("function renderHotbar("),
      main.indexOf("\n}", main.indexOf("function renderHotbar(")),
    );
    expect(hotbar).not.toMatch(/innerHTML\s*=/);
    for (const renderer of ["renderInventory", "renderInspectorActions"]) {
      const body = main.slice(
        main.indexOf(`function ${renderer}(`),
        main.indexOf("\n}", main.indexOf(`function ${renderer}(`)),
      );
      expect(body, `${renderer} reconciles in place`).toContain(
        "syncChildren(",
      );
    }
    expect(saveList).toContain("function paintSaveSlotList(");
    expect(saveList).toContain("syncChildren(");
  });

  it("provides a Title Screen overlay with dedicated saves catalog and new factory launch", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    const html = readFileSync(
      new URL("../index.html", import.meta.url),
      "utf8",
    );
    const css = readStyles();
    expect(html).toContain('id="title-screen"');
    expect(html).toContain('id="title-continue"');
    expect(html).toContain('id="title-save-slots"');
    expect(html).toContain('id="title-start-game"');
    expect(html).toContain('id="session-main-menu"');
    expect(html).toContain('id="save-file-input"');
    expect(html).toContain('id="export-save"');
    expect(html).toContain('id="import-save"');
    expect(html).toContain('id="title-import-saves"');
    expect(html).toContain('id="title-export-saves"');
    expect(css).toContain(".title-screen");
    expect(css).toContain(".title-modal");
    expect(css).toContain(".title-save-slots");
    expect(main).toContain("function openTitleScreen(");
    expect(main).toContain("function closeTitleScreen(");
    expect(main).toContain("function switchTitleTab(");
    expect(main).toContain("function triggerAutoSave(");
    expect(main).toContain("visibilitychange");
    expect(main).toContain("beforeunload");
    expect(main).toContain("AUTOSAVE_INTERVAL_MS");
  });

  it("mirrors native's reach and switch rules rather than inventing its own", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    const html = readFileSync(
      new URL("../index.html", import.meta.url),
      "utf8",
    );
    const rust = [
      "../factory-wasm/src/core/inventory.rs",
      "../factory-wasm/src/core/configuration.rs",
    ]
      .map((path) => readFileSync(new URL(path, import.meta.url), "utf8"))
      .join("\n");
    // Two lists of building kinds exist in both languages, and they decide different things: native
    // decides whether a transfer happens, the host decides whether a button is drawn. Drifting
    // apart shows a control that earns a refusal, or hides one that would have worked — neither is
    // caught by a type. So the host's copies are read back out of the Rust that defines them.
    const kindsOf = (source: string, fn: string): string[] => {
      const body = source.slice(
        source.indexOf(`fn ${fn}(`),
        source.indexOf("\n    }", source.indexOf(`fn ${fn}(`)),
      );
      return [...body.matchAll(/BuildingKind::(\w+)/g)]
        .map((match) => match[1]!.toLowerCase())
        .sort();
    };
    const setOf = (name: string): string[] => {
      const body = main.slice(
        main.indexOf(`const ${name} = new Set<string>([`),
        main.indexOf("]);", main.indexOf(`const ${name} = new Set<string>([`)),
      );
      return [...body.matchAll(/"([a-z]+)"/g)].map((match) => match[1]!).sort();
    };
    expect(setOf("HAND_REACHABLE")).toEqual(
      kindsOf(rust, "stock_is_reachable_by_hand"),
    );
    expect(setOf("SWITCHABLE")).toEqual(kindsOf(rust, "can_be_switched"));

    // The switch sends the state it wants, never a flip read off the machine: by the time the
    // command lands the snapshot may have moved, and a toggle would then land the wrong way up.
    expect(main).toContain('type: "set_enabled"');
    expect(main).not.toMatch(/enabled:\s*!/);
    expect(html).toContain('id="inspect-power-switch"');
    // The host displays the native compartments directly. It must never try to infer free input or
    // subtract a reservation in presentation code. One function derives them, because three
    // features now ask the same question — the inspector draws them, the pack opens beside a
    // building that takes items, and a demolition names what is inside — and a second derivation
    // would be a second thing to get wrong.
    const compartments = main.slice(
      main.indexOf("function stockCompartments("),
      main.indexOf("\n}", main.indexOf("function stockCompartments(")),
    );
    expect(compartments).toContain("building.input_inventory");
    expect(compartments).toContain("building.fuel_inventory");
    expect(compartments).toContain("building.output_inventory");
    expect(compartments).not.toMatch(/reserved_inputs/);
    const actions = main.slice(
      main.indexOf("function renderInspectorActions("),
      main.indexOf("\n}", main.indexOf("function renderInspectorActions(")),
    );
    expect(actions).toContain("stockCompartments(building)");
    expect(actions).not.toMatch(/reserved_inputs/);
  });

  it("keeps the envelope numbers level with native's, and shows the ones it is running", () => {
    const rust = readFileSync(
      new URL("../factory-wasm/src/model/constants.rs", import.meta.url),
      "utf8",
    );
    const declared = rust.match(/const SAVE_VERSION: u16 = (\d+);/);
    expect(declared).not.toBeNull();
    expect(SAVE_VERSION).toBe(Number(declared![1]));

    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    const html = readFileSync(
      new URL("../index.html", import.meta.url),
      "utf8",
    );
    const saveUi = readFileSync(
      new URL("../src/app/saveUi.ts", import.meta.url),
      "utf8",
    );
    expect(html).toContain('id="title-envelope-info"></span>');
    expect(main).toContain("saveUi.update(");
    expect(saveUi).toContain('required<HTMLElement>("title-envelope-info")');
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
      costLabel: "1 Transport kit",
      cost: [{ item_id: 24, required: 1, held: 3, shortfall: 0 }],
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
      known: true,
      complete: true,
      prerequisitesMet: true,
      affordable: true,
      purchasable: false,
      missingPrerequisites: [],
      insightShortfall: 0,
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
      { scenario: "new-game", seed: 7, worldParams: "basin", creative: false },
      { scenario: "new-game", seed: 7, worldParams: tuned, creative: false },
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

  // One claim, both patched groups: a delta names the entries that moved, and everything else keeps
  // the identity native sent it with. Buildings are keyed by native id and resources by coordinate,
  // and both have to survive insert, remove, replace, an empty patch and an absent group.
  it("patches buildings and deposits entry by entry instead of resending the group", () => {
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
    const deposits = [...snapshot.resources, second];
    const withCrystal: FactorySnapshot = { ...snapshot, resources: deposits };

    // A drawn-from deposit is substituted in place; every other deposit keeps its identity, so
    // the native ordering the host received survives the patch.
    const drained = { ...deposits[0]!, quantity: 46 };
    const patchedDeposits = applyResourcesPatch(deposits, {
      changed: [drained],
    });
    expect(patchedDeposits.map(({ q, r }) => `${q},${r}`)).toEqual([
      "3,0",
      "4,-2",
    ]);
    expect(patchedDeposits[0]).toEqual(drained);
    expect(patchedDeposits[1]).toBe(deposits[1]);

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
    expect(applyResourcesPatch(deposits, { changed: [] })).toBe(deposits);
    expect(
      applyResourcesPatch(deposits, { replace: true, changed: [second] }),
    ).toEqual([second]);

    const drawn = applySnapshotDelta(withCrystal, 0, {
      base_revision: 0,
      revision: 1,
      tick: 13,
      checksum: 456,
      resources: { changed: [drained] },
    });
    expect(drawn.snapshot.resources[0]?.quantity).toBe(46);
    expect(drawn.snapshot.resources[1]).toBe(second);
    expect(drawn.snapshot.buildings).toBe(withCrystal.buildings);
    expect(drawn.snapshot.terrain).toBe(withCrystal.terrain);
    expect(
      applySnapshotDelta(withCrystal, 0, {
        base_revision: 0,
        revision: 1,
        tick: 13,
        checksum: 456,
      }).snapshot.resources,
    ).toBe(deposits);
  });

  it("appends newly surveyed terrain without rewriting the cells already held", () => {
    const held = snapshot.terrain;
    const opened = {
      q: 8,
      r: 0,
      x: 0,
      y: 0,
      radius: 1024,
      terrain: "lowland" as const,
      height: 0,
      substrate: "meadow" as const,
      water_depth: 0,
      discharge: 0,
    };
    const patched = applyTerrainPatch(held, { changed: [opened] });
    expect(patched).toHaveLength(held.length + 1);
    expect(patched[0]).toBe(held[0]);
    expect(patched[patched.length - 1]).toBe(opened);
    expect(applyTerrainPatch(held, { changed: [] })).toBe(held);
    expect(
      applyTerrainPatch(held, { replace: true, changed: [opened] }),
    ).toEqual([opened]);
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
    const worldGl = readFileSync(
      new URL("../src/rendering/gl/WorldGl.ts", import.meta.url),
      "utf8",
    );
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    // Fog is presentation over native chunk truth: the renderer must not invent chunk geometry.
    expect(renderer).toContain("this.drawFog(");
    expect(renderer).toContain("chunk.span");
    expect(worldGl).toContain("chunk.span");
    expect(renderer).not.toMatch(/span\s*=\s*\d/);
    expect(worldGl).not.toMatch(/span\s*=\s*\d/);
    expect(renderer).toContain("player.radius");
    expect(main).toContain("isSurveyed(snapshot.chunks");
  });

  it("ships responsive controls and accessible labels", () => {
    const html = readFileSync(
      new URL("../index.html", import.meta.url),
      "utf8",
    );
    const styles = readStyles();
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    expect(html).toContain('aria-label="Interactive HexFactory world map');
    expect(html).toContain('id="technology-list"');
    expect(html).toContain('id="inventory-peek"');
    // Mission control and research cross-link, and the physical hub exposes both delivery loops.
    expect(html).toContain("Earn insight at the hub");
    expect(html).toContain("Spend insight in Research");
    expect(html).toContain('id="inspect-hub-contract"');
    expect(html).toContain('id="inspect-hub-requests"');
    expect(main).toContain("contract.requirements.forEach");
    expect(html).toContain('id="continue"');
    expect(html).toContain("<kbd>W</kbd>");
    // The drag, copy, and undo bindings are documented in the page itself, not only in the repo.
    expect(html).toContain("<kbd>Q</kbd>");
    expect(html).toContain("<kbd>Ctrl</kbd>+<kbd>Z</kbd>");
    expect(html).toContain('id="next-action-title"');
    // The next step is permanent chrome, not something behind a key a new player has to find.
    expect(html).toContain('id="next-step-title"');
    expect(html).toContain('id="next-step-detail"');
    // Comfort controls exist in the product, both on the bar and in the menu, and the run binding
    // is documented in the page rather than only in the repo.
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
    const fixture = passabilityFixture as {
      bands: { terrain: Terrain; passable: boolean; buildable: boolean }[];
      physical: {
        substrate: Substrate;
        slope: number;
        water_depth: number;
        passable: boolean;
        buildable: boolean;
      }[];
    };
    const entries = fixture.bands;
    expect(entries.map(({ terrain }) => terrain)).toEqual(TERRAIN_ORDER);
    for (const entry of entries) {
      const band = TERRAIN_INFO[entry.terrain];
      expect(band.passable, entry.terrain).toBe(entry.passable);
      expect(band.buildable, entry.terrain).toBe(entry.buildable);
      expect(terrainAccess(band), entry.terrain).toBe(
        entry.passable
          ? entry.buildable
            ? "Buildable"
            : "Walkable"
          : "Impassable",
      );
    }
    // Deep water and cliff are the wall. Shallows are a ford: walkable, not buildable.
    expect(
      entries.filter(({ passable }) => !passable).map((e) => e.terrain),
    ).toEqual(["deep_water", "cliff"]);
    expect(entries.find((entry) => entry.terrain === "shallow_water")).toEqual({
      terrain: "shallow_water",
      passable: true,
      buildable: false,
    });
    for (const entry of fixture.physical) {
      expect(
        physicalAccess(entry.substrate, entry.slope, entry.water_depth),
        `${entry.substrate}/${entry.slope}/${entry.water_depth}`,
      ).toEqual({ passable: entry.passable, buildable: entry.buildable });
    }

    const renderer = readFileSync(
      new URL("../src/rendering/CanvasFactoryRenderer.ts", import.meta.url),
      "utf8",
    );
    const worldGl = readFileSync(
      new URL("../src/rendering/gl/WorldGl.ts", import.meta.url),
      "utf8",
    );
    // Impassability is drawn from the table, not from a second opinion about which grey is cliff.
    // The one thing allowed on top of the table is the grade the player has cut into the hex, which
    // is a fact about that hex rather than about the band.
    expect(worldGl).toContain("info.passable ||");
    expect(worldGl).toContain("passable ? 0 : 1");
    expect(renderer).not.toContain('case "cliff"');
    expect(worldGl).not.toContain('case "cliff"');
  });

  it("lets a quarried cliff stop being a wall, and only a quarried one", () => {
    // Native's `Core::cliff_quarried` and `terrain_blocks_movement`, copied on the same terms as the
    // band table above: a cliff is the one wall made of something the player can take apart, and it
    // is taken apart by cutting the face below the grade the generator drew. `bandAt` is what every
    // panel asks once it is talking about a particular hex rather than about a legend.
    for (const terrain of TERRAIN_ORDER) {
      // Untouched ground is the table, to the letter — which is why a world nobody has dug reads
      // exactly as it always did.
      expect(bandAt(terrain, 0), terrain).toBe(TERRAIN_INFO[terrain]);
      expect(cliffQuarried(terrain, 0), terrain).toBe(false);
      // A cut anywhere else is landscaping, not demolition: deep water stays a wall however deep it
      // is dug, and no amount of filling makes a cliff into one thing or another.
      expect(cliffQuarried(terrain, -1), terrain).toBe(terrain === "cliff");
      expect(cliffQuarried(terrain, 1), terrain).toBe(false);
      expect(bandAt(terrain, 1), terrain).toBe(TERRAIN_INFO[terrain]);
    }

    const quarried = bandAt("cliff", -1);
    expect(quarried.passable).toBe(true);
    expect(quarried.buildable).toBe(true);
    expect(terrainAccess(quarried)).toBe("Buildable");
    // Same rock, same paint: the face has come down, the hex has not become highland.
    expect(quarried.fill).toBe(TERRAIN_INFO.cliff.fill);
    expect(quarried.stroke).toBe(TERRAIN_INFO.cliff.stroke);
    expect(quarried.name).not.toBe(TERRAIN_INFO.cliff.name);
    // The table itself is untouched by the reading — `bandAt` answers, it does not edit.
    expect(TERRAIN_INFO.cliff.passable).toBe(false);
    expect(TERRAIN_INFO.cliff.buildable).toBe(false);
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

  it("knows which building the player is standing beside", () => {
    // The fixture player stands on (1, 0), one step from the hub's footprint cell at the origin.
    expect(buildingBeside(snapshot)).toEqual({ q: 0, r: 0 });
    // Three hexes out is not beside anything, and nothing is claimed rather than the nearest thing
    // in the world being claimed from any distance.
    expect(
      buildingBeside({
        ...snapshot,
        player: { ...snapshot.player, x: 3 * 1774, y: 0 },
      }),
    ).toBeNull();

    const hub = snapshot.buildings[0] as EntitySnapshot;
    const at = (
      id: number,
      kind: EntitySnapshot["kind"],
      q: number,
      r: number,
    ): EntitySnapshot => ({
      ...hub,
      id,
      kind,
      q,
      r,
      scenario_owned: false,
      footprint: [{ q, r }],
    });
    const beside = (buildings: EntitySnapshot[]): unknown =>
      buildingBeside({ ...snapshot, buildings });

    // A belt running past a machine is not what the player walked up to, whichever was built first.
    expect(beside([at(4, "belt", 1, -1), at(9, "composer", 2, 0)])).toEqual({
      q: 2,
      r: 0,
    });
    // Two equal claims resolve on the lower entity ID, so a crowded corner picks the same machine
    // every tick rather than flickering between two while the player stands still.
    expect(beside([at(9, "composer", 2, 0), at(4, "composer", 1, -1)])).toEqual(
      {
        q: 1,
        r: -1,
      },
    );
    // Standing on one outranks standing beside another, even when the other has the lower ID.
    expect(beside([at(9, "composer", 1, 0), at(4, "composer", 2, 0)])).toEqual({
      q: 1,
      r: 0,
    });
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
    const css = readStyles();
    const focus = readFileSync(
      new URL("../src/input/focus.ts", import.meta.url),
      "utf8",
    );
    // Pack, research, and the objective guide wait behind I, O, and P.
    expect(main).toContain('KeyI: "inventory-panel"');
    expect(main).toContain('KeyO: "research-panel"');
    expect(main).toContain('KeyP: "quest-panel"');
    // The inspector is the exception: it has no key because it never leaves the world.
    expect(main).not.toContain('"inspector-panel"');
    // Space centres the camera. A clicked button must not keep Space:
    // activation is on keyup, so keydown alone would both skip recenter and press the control.
    expect(main).toContain('event.code === "Space") renderer.recenter()');
    expect(main).toContain('event.code === "Space"');
    expect(main).toContain('event.code === "ArrowLeft") orbitView(-1)');
    expect(main).toContain('event.code === "ArrowRight") orbitView(1)');
    expect(main).toContain('event.code === "ArrowUp") tiltView(1)');
    expect(main).toContain('event.code === "ArrowDown") tiltView(-1)');
    expect(main).not.toContain('event.code === "Comma"');
    expect(main).not.toContain('event.code === "Period"');
    expect(main).toContain('event.code === "Backspace"');
    expect(main).toContain('event.code === "Delete"');
    expect(main).toContain("deleteBuildingUnderCursorOrSelected()");
    expect(main).toContain("target.blur()");
    expect(focus).toContain('target.tagName === "SUMMARY"');
    expect(main).not.toContain('event.code === "KeyT"');
    expect(main).not.toContain("setPlaying");
    expect(main).not.toContain("speedInput");
    expect(html).not.toContain('id="play"');
    expect(html).not.toContain('id="speed"');
    expect(html).not.toContain('id="step"');
    // Gather and deliver are permanent chrome in the dock, not a panel a new player has to find.
    expect(html).toContain('class="field-actions"');
    expect(html).toContain('id="minimap"');
    expect(html).toContain('id="home-readout"');
    expect(html).toContain('class="home-compass"');
    expect(html).toContain('id="home-readout-text"');
    expect(css).toMatch(/\.minimap-frame\s*\{[^}]*pointer-events:\s*none;/s);
    const update = main.slice(
      main.indexOf("function update("),
      main.indexOf("function sameCarry("),
    );
    expect(update.indexOf("renderer.setSnapshot(snapshot)")).toBeLessThan(
      update.indexOf("syncHoverWithCamera()"),
    );
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
    expect(byCategory("kiln")).toEqual(["brick", "charcoal", "cement"]);
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

  it("names every surveyed hex from the physical terrain payload", () => {
    const main = readFileSync(
      new URL("../src/main.ts", import.meta.url),
      "utf8",
    );
    // Physical ground publishes every surveyed cell because height has no implicit lowland value.
    expect(main).toContain("const terrainSample = surveyed");
    expect(main).toContain("const terrain = terrainSample?.terrain");
    for (const band of TERRAIN_ORDER) {
      expect(TERRAIN_INFO[band].name, band).toBeTruthy();
      expect(TERRAIN_INFO[band].note, band).toBeTruthy();
    }
    // A field hex leads with what is on it. Band potentials stay on empty ground.
    const inspectorStart = main.indexOf("function renderInspector(");
    expect(main.indexOf("if (resource)", inspectorStart)).toBeLessThan(
      main.indexOf("const terrainSample", inspectorStart),
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
    const css = readStyles();
    // The heading is the hex, not the active tool. Coordinates are a labelled chip.
    expect(html).toContain('id="inspect-title"');
    expect(html).toContain('id="inspect-q"');
    expect(html).toContain('id="inspect-alt"');
    expect(main).toContain("terrainSample.height + grade");
    expect(main).toContain("* HEIGHT_UNIT_METRES");
    expect(html).toContain('id="inspect-compass"');
    expect(html).toContain('id="inspect-output-products"');
    expect(html).toContain('id="inspect-output-ports"');
    // The field cell is a metered item chip. Static markup names a holder rather than spelling the
    // chip out, so `createItemChip` stays the only place its shape is written down.
    expect(html).toContain('id="inspect-field-chip"');
    expect(html).not.toContain("item-chip-glyph");
    expect(html).not.toContain('id="selected-tool-value"');
    // Direction 0 never reaches the player; the six names and a compass do.
    expect(main).toContain("DIRECTION_NAMES[building.orientation]");
    expect(main).toContain('type: "set_output_route"');
    expect(main).toContain("footprintKeys.has");
    expect(main).not.toContain("Direction ${building.orientation}");
    expect(main).not.toContain("lines.join");
    // A proportion is both published numbers, same rule as the cooldown ring.
    expect(main).toContain("resource.quantity");
    expect(main).toContain("resource.initial_quantity");
    expect(css).not.toMatch(/\.inspector\s*\{[^}]*white-space:\s*pre-line/);
  });

  it("keeps the visual lattice compact and depleted fields spatial", () => {
    const contract = readFileSync(
      new URL("../src/rendering/FactoryRenderer.ts", import.meta.url),
      "utf8",
    );
    const instances = readFileSync(
      new URL("../src/rendering/three/worldInstances.ts", import.meta.url),
      "utf8",
    );
    // More hexes in the viewport is a presentation knob, not another PLAYER_RADIUS bump.
    expect(contract).toContain("export const BASE_HEX_SIZE = 22");
    expect(instances).toContain('mesh.name = "depleted-field-scars"');
    expect(instances).not.toContain("field-resource-marks");
    // Resource amounts stay in the inspector; the landscape does not become a spreadsheet.
    expect(instances).not.toContain("String(resource.quantity)");
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
    const worldGl = readFileSync(
      new URL("../src/rendering/gl/WorldGl.ts", import.meta.url),
      "utf8",
    );
    expect(worldGl).toContain('from "../terrainLook"');
    expect(renderer).toContain('from "./buildingLook"');
    expect(renderer).toContain('getContext("webgl2"');
    expect(renderer).toContain("this.itemsById = new Map(");
    expect(renderer).toContain("this.buildingsById = new Map(");
    expect(renderer).not.toContain("definitions.items.find(");
    expect(renderer).not.toContain("definitions.buildings.find(");
    expect(minimap).toContain("this.itemsById = new Map(");
    expect(minimap).toContain('getContext("webgl2"');
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
  water_level: 18000,
  shore_level: 24000,
  hills_level: 33000,
  highland_level: 42000,
  cliff_step: 14000,
  deep_water_moisture: 40000,
  site_cell: 12,
  site_jitter: 4,
  river_cell: 32,
  river_width: 1000,
  river_max_elevation: 42000,
  ocean_level: 15000,
  site_rules: [
    {
      terrain: "highland",
      item_id: 6,
      weight: 26,
      radius_min: 3,
      radius_max: 5,
      site_min: -1,
      yield_core: 12,
      yield_rim: 12,
      yield_jitter: 2,
      member: ["highland", "cliff"],
      member_water_within: 0,
      center_ocean: false,
      center_shore: false,
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
    patch: Partial<
      Omit<FactorySnapshot, "buildings" | "resources" | "terrain">
    >,
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

describe("personal skills", () => {
  it("encodes a bounded native purchase", () => {
    expect(encodeCommand({ type: "purchase_skill", skill_id: 2 })).toEqual({
      opcode: 29,
      args: [2],
    });
    for (const skill_id of [0, -1, 65536, 1.5])
      expect(() =>
        encodeCommand({ type: "purchase_skill", skill_id }),
      ).toThrow();
  });
});

it("skill UI uses native availability, keeps currencies separate and explains grants", () => {
  const skill = technologies.skills[0]!;
  const state = structuredClone(snapshot);
  state.insight = 9999;
  state.skills.availability = [
    {
      skill_id: skill.id,
      complete: false,
      points_shortfall: 1,
      current_value: 6,
      resulting_value: 10,
      missing_prerequisites: [],
    },
  ];
  expect(skillView(skill, state)).toMatchObject({
    canPurchase: false,
    status: "Earn 1 more Skill Point",
  });
  state.skills.availability[0]!.points_shortfall = 0;
  expect(skillView(skill, state).canPurchase).toBe(true);
  state.skills.availability[0]!.missing_prerequisites = [2];
  expect(skillView(skill, state).canPurchase).toBe(false);
  state.skills.availability[0]!.complete = true;
  state.skills.granted = [skill.id];
  expect(skillView(skill, state)).toMatchObject({
    canPurchase: false,
    status: "Already unlocked",
  });
  const applied = applySnapshotDelta(snapshot, 0, {
    base_revision: 0,
    revision: 1,
    tick: 1,
    checksum: 1,
    skills: state.skills,
  });
  expect(applied.snapshot.skills).toBe(state.skills);
  expect(applied.snapshot.insight).toBe(snapshot.insight);
});

it("carries bounded boundary transactions as two lattice vertices", () => {
  expect(
    encodeCommand({
      type: "boundary_edit",
      q: -2,
      r: 1,
      corner: 5,
      to_q: 2,
      to_r: 3,
      to_corner: 2,
      shape: "yard",
      definition_id: 1,
      action: "build",
    }),
  ).toEqual({
    opcode: 30,
    args: [-2, 1, 5, 2, 3, 2, 1, 1, 0],
  });
  expect(encodeCommand({ type: "undo_boundary" })).toEqual({
    opcode: 31,
    args: [],
  });
  expect(() =>
    encodeCommand({
      type: "boundary_edit",
      q: 0.5,
      r: 0,
      corner: 0,
      to_q: 0,
      to_r: 0,
      to_corner: 0,
      shape: "line",
      definition_id: 1,
      action: "build",
    }),
  ).toThrow(/boundary/);
  // A corner outside the six is as unsendable as a hex outside the world: native names vertices by
  // hex and corner, so a host that invented a seventh corner would be inventing a lattice.
  expect(() =>
    encodeCommand({
      type: "boundary_edit",
      q: 0,
      r: 0,
      corner: 6,
      to_q: 0,
      to_r: 0,
      to_corner: 0,
      shape: "line",
      definition_id: 1,
      action: "build",
    }),
  ).toThrow(/boundary/);
});

it("carries ground selections as two anchors, a verb, a depth and a deliberate cover", () => {
  // Native has no numeric opcode decoder: the switch in `encodeCommand` is the whole contract for
  // what a ground edit looks like on the wire, so it is pinned here rather than inferred from Rust.
  expect(
    encodeCommand({
      type: "ground_edit",
      q: -2,
      r: 1,
      corner: 3,
      to_q: 2,
      to_r: 3,
      to_corner: 0,
      shape: "rect",
      definition_id: 4,
      action: "level",
      cover: true,
      steps: 1,
      reference: "highest",
    }),
  ).toEqual({
    opcode: 32,
    args: [-2, 1, 3, 2, 3, 0, 2, 4, 4, 1, 1, 2],
  });
  // Every shape rides those same two anchors, so a fill and its outline differ by one field. The
  // codes are pinned because an outline silently decoding as its fill would pave what it meant to
  // edge, at the player's expense.
  expect(
    (["cell", "path", "rect", "frame", "disc", "ring"] as const).map(
      (shape) =>
        encodeCommand({
          type: "ground_edit",
          q: 0,
          r: 0,
          corner: 0,
          to_q: 2,
          to_r: 0,
          to_corner: 0,
          shape,
          definition_id: 1,
          action: "pave",
          cover: false,
          steps: 1,
          reference: "first",
        }).args[6],
    ),
  ).toEqual([0, 1, 2, 3, 4, 5]);
  // `cover` travels, never defaults: sealing a deposit is the one ground change a player cannot
  // walk back by looking at it, so an unconfirmed edit must reach native as an explicit no.
  expect(
    encodeCommand({
      type: "ground_edit",
      q: 0,
      r: 0,
      corner: 0,
      to_q: 0,
      to_r: 0,
      to_corner: 0,
      shape: "cell",
      definition_id: 1,
      action: "pave",
      cover: false,
      steps: 3,
      reference: "first",
    }).args.slice(9),
  ).toEqual([0, 3, 0]);
  expect(encodeCommand({ type: "undo_ground" })).toEqual({
    opcode: 33,
    args: [],
  });
  expect(() =>
    encodeCommand({
      type: "ground_edit",
      q: 0,
      r: 0,
      corner: 0,
      to_q: 1e9,
      to_r: 0,
      to_corner: 0,
      shape: "path",
      definition_id: 1,
      action: "lower",
      cover: false,
      steps: 1,
      reference: "first",
    }),
  ).toThrow(/ground/i);
  // A depth is a count of steps, not a free number: a fractional or absent one would reach native
  // as a clamp rather than as the refusal it is.
  expect(() =>
    encodeCommand({
      type: "ground_edit",
      q: 0,
      r: 0,
      corner: 0,
      to_q: 0,
      to_r: 0,
      to_corner: 0,
      shape: "cell",
      definition_id: 1,
      action: "lower",
      cover: false,
      steps: 0,
      reference: "first",
    }),
  ).toThrow(/ground/i);
});
