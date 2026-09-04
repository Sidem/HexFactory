import { pixelToAxial } from "@hexlife/embed/hex";
import { heldQuantity } from "../core/availability";
import { cueForEvent } from "../audio/feedback";
import { type CurrentBuild } from "../core/saveSlots";
import {
  formatElapsed,
  isRunComplete,
  OPENING_CHECKPOINTS,
  recordCheckpoints,
  startRun,
  taintRun,
  writeRun,
  type CheckpointContext,
} from "../core/checkpoints";
import { DIRECTION_NAMES } from "../core/directions";
import type { FactorySnapshot, ItemDefinition } from "../core/types";
import {
  createItemChip,
  fillItemChip,
  itemTooltip,
  type ItemChipView,
} from "../rendering/itemChip";
import {
  buildingBeside,
  findLandingHub,
  homeBearing,
  WORLD_SCALE,
} from "../rendering/landmarks";
import { part, required, syncChildren } from "../ui/dom";
import { currentBuild as buildInfo } from "./buildInfo";
import type { Tool } from "./runtime";
import { Runtime } from "./runtime";

declare module "./runtime" {
  interface Runtime {
    currentBuild(): CurrentBuild;
    dragOwnsPointer(): boolean;
    loadHotbar(): (Tool | null)[];
    sanitiseSlot(value: unknown): Tool | null;
    saveHotbar(): void;
    checkpointContext(next: FactorySnapshot): CheckpointContext;
    evaluateRun(next: FactorySnapshot): void;
    beginRun(next: FactorySnapshot): void;
    renderRun(): void;
    update(next: FactorySnapshot): void;
    refreshLandingHub(): void;
    syncStandingSelection(buildingsChanged: boolean): void;
    renderHomeReadout(): void;
    sameCarry(
      previous: FactorySnapshot["player"],
      next: FactorySnapshot["player"],
    ): boolean;
    itemById(itemId: number | undefined): ItemDefinition | undefined;
    paintChip(
      holder: HTMLElement,
      itemId: number | undefined,
      view?: ItemChipView,
    ): HTMLElement;
    renderInventory(): void;
    renderCreative(): void;
  }
}

Runtime.prototype.currentBuild = function currentBuild(
  this: Runtime,
): CurrentBuild {
  return buildInfo(this.host, this.snapshot);
};

Runtime.prototype.dragOwnsPointer = function dragOwnsPointer(
  this: Runtime,
): boolean {
  return (
    this.stackDrag !== null ||
    this.panPointer !== null ||
    this.harvestPointer !== null ||
    this.dragBuild !== null
  );
};

Runtime.prototype.loadHotbar = function loadHotbar(
  this: Runtime,
): (Tool | null)[] {
  // Through the same sieve the stored bar goes through. A milestone that retires a definition id
  // leaves the default naming it too, and a default is not more trustworthy than a preference —
  // v0.25.1 retired the riser and the ninth slot rendered as `?18` until this asked the catalogue.
  const defaults = Array.from({ length: this.HOTBAR_SLOTS }, (_, slot) =>
    this.sanitiseSlot(this.DEFAULT_HOTBAR[slot] ?? null),
  );
  try {
    const stored: unknown = JSON.parse(
      window.localStorage.getItem(this.HOTBAR_KEY) ?? "null",
    );
    // A stored bar is taken whole, empty slots included: a slot the player deliberately cleared
    // must not refill itself with a default on the next load.
    if (!Array.isArray(stored)) return defaults;
    return Array.from({ length: this.HOTBAR_SLOTS }, (_, slot) =>
      this.sanitiseSlot(stored[slot]),
    );
  } catch {
    // A corrupt or unreadable preference is not worth failing a boot over.
    return defaults;
  }
};

Runtime.prototype.sanitiseSlot = function sanitiseSlot(
  this: Runtime,
  value: unknown,
): Tool | null {
  if (value === "erase" || value === "rotate" || value === "upgrade")
    return value;
  if (typeof value !== "number") return null;
  const definition = this.host.definitions.buildings.find(
    ({ id }) => id === value,
  );
  return definition?.buildable ? value : null;
};

Runtime.prototype.saveHotbar = function saveHotbar(this: Runtime): void {
  try {
    window.localStorage.setItem(this.HOTBAR_KEY, JSON.stringify(this.hotbar));
  } catch {
    // Private-mode storage refusals must not break the bar for the session in front of us.
  }
};

Runtime.prototype.checkpointContext = function checkpointContext(
  this: Runtime,
  next: FactorySnapshot,
): CheckpointContext {
  const carried: Record<string, number> = {};
  for (const item of this.host.definitions.items) {
    const held =
      next.player.inventory[String(item.id)] ??
      next.player.inventory[item.id] ??
      0;
    if (held > 0) carried[item.key] = held;
  }
  const buildingKeys = new Map(
    this.host.definitions.buildings.map((definition) => [
      definition.id,
      definition.key,
    ]),
  );
  return {
    tick: next.tick,
    contractStage: next.contract.stage,
    researchedCount: next.researched.length,
    carried,
    buildings: next.buildings.map((entity) => ({
      key: buildingKeys.get(entity.definition_id) ?? "",
      kind: entity.kind,
      status: entity.status,
      powered:
        (entity.power_charge ?? 0) > 0 || (entity.power_satisfied ?? 0) > 0,
    })),
  };
};

Runtime.prototype.evaluateRun = function evaluateRun(
  this: Runtime,
  next: FactorySnapshot,
): void {
  if (!this.run) return;
  if (isRunComplete(this.run)) return;
  const result = recordCheckpoints(
    this.run,
    this.checkpointContext(next),
    this.runElapsedMs,
  );
  if (result.reached.length === 0) return;
  this.run = result.run;
  writeRun(localStorage, this.run);
  const last = result.reached.at(-1);
  const checkpoint = last
    ? OPENING_CHECKPOINTS.find(({ id }) => id === last.id)
    : undefined;
  if (last && checkpoint)
    this.showFeedback(`${checkpoint.label} — ${formatElapsed(last.elapsedMs)}`);
  this.renderRun();
};

Runtime.prototype.beginRun = function beginRun(
  this: Runtime,
  next: FactorySnapshot,
): void {
  this.runElapsedMs = 0;
  this.run = startRun(Date.now(), next.tick);
  writeRun(localStorage, this.run);
  this.renderRun();
};

Runtime.prototype.renderRun = function renderRun(this: Runtime): void {
  const conditions = required<HTMLElement>("run-conditions");
  const tainted = (this.run?.taints.length ?? 0) > 0;
  conditions.textContent = !this.run
    ? "No run timed yet. Start a scenario to begin the clock."
    : tainted
      ? `${this.run.startedSpeed} tps · not comparable (${this.run.taints.join(", ")})`
      : `${this.run.startedSpeed} tps · clean`;
  conditions.classList.toggle("run-tainted", tainted);
  // Keyed in place rather than rebuilt, for the reason every other list here is: a row replaced
  // between pointerdown and pointerup eats the click that was already on its way.
  const rows = syncChildren(
    required<HTMLElement>("run-checkpoints"),
    OPENING_CHECKPOINTS.map(({ id }) => id),
    () => {
      const row = document.createElement("li");
      row.className = "run-row";
      const time = document.createElement("strong");
      time.className = "run-time";
      const label = document.createElement("span");
      label.className = "run-label";
      const note = document.createElement("small");
      note.className = "run-note";
      row.append(time, label, note);
      return row;
    },
  );
  const byId = new Map(
    (this.run?.records ?? []).map((record) => [record.id, record]),
  );
  OPENING_CHECKPOINTS.forEach((checkpoint, index) => {
    const row = rows[index];
    if (!row) return;
    const record = byId.get(checkpoint.id);
    row.classList.toggle("run-reached", record !== undefined);
    part<HTMLElement>(row, ".run-time").textContent = record
      ? formatElapsed(record.elapsedMs)
      : "--:--";
    part<HTMLElement>(row, ".run-label").textContent =
      checkpoint.label + (checkpoint.optional ? " (optional)" : "");
    part<HTMLElement>(row, ".run-note").textContent = record
      ? `tick ${record.tick.toLocaleString()}`
      : checkpoint.note;
  });
};

Runtime.prototype.update = function update(
  this: Runtime,
  next: FactorySnapshot,
): void {
  const previousVictory = this.snapshot.victory;
  const previous = this.snapshot;
  this.snapshot = next;
  this.refreshLandingHub();
  this.renderer.setHome(this.landingHub);
  this.renderer.setSnapshot(this.snapshot);
  this.boundaryTool.update(this.snapshot);
  this.groundTool.update(this.snapshot);
  this.syncHoverWithCamera();
  this.minimap.setSnapshot(this.snapshot, this.landingHub);
  this.renderHomeReadout();
  required<HTMLElement>("scenario-value").textContent =
    this.snapshot.scenario_name;
  required<HTMLElement>("tick-value").textContent =
    this.snapshot.tick.toLocaleString();
  this.skillsView.update(this.snapshot);
  required<HTMLElement>("skill-points-value").textContent = String(
    this.snapshot.skills.points,
  );
  required<HTMLElement>("skills-chip").setAttribute(
    "aria-label",
    `Skills: ${this.snapshot.skills.points} Skill Point${this.snapshot.skills.points === 1 ? "" : "s"} (K)`,
  );
  required<HTMLElement>("skills-chip").classList.toggle(
    "has-points",
    this.snapshot.skills.points > 0,
  );
  required<HTMLElement>("insight-value").textContent =
    this.snapshot.insight.toLocaleString();
  required<HTMLElement>("position-value").textContent =
    `${(this.snapshot.player.x / 1024).toFixed(1)}, ${(this.snapshot.player.y / 1024).toFixed(1)}`;
  required<HTMLElement>("surveyed-value").textContent =
    this.snapshot.chunks.length.toLocaleString();
  required<HTMLElement>("checksum-value").textContent = this.snapshot.checksum
    .toString(16)
    .padStart(8, "0")
    .toUpperCase();
  // Walking changes the player every frame. Rebuilding every panel for that is the hitch on a
  // weak machine: the factory HUD only moves when the factory does.
  const packChanged = !this.sameCarry(previous.player, next.player);
  // Switching creative off re-prices every card, and a wider pack changes what fits, so both count
  // as the factory moving even on a tick where nothing was built.
  const creativeChanged =
    previous.player.creative !== next.player.creative ||
    previous.player.carry_slots !== next.player.carry_slots;
  const factoryChanged =
    previous === next ||
    creativeChanged ||
    previous.tick !== next.tick ||
    previous.insight !== next.insight ||
    previous.victory !== next.victory ||
    previous.buildings !== next.buildings ||
    previous.resources !== next.resources ||
    previous.researched !== next.researched ||
    previous.contract !== next.contract ||
    previous.requests !== next.requests ||
    previous.events !== next.events;
  if (packChanged || factoryChanged) {
    this.renderInventory();
    this.renderCreative();
    this.renderHotbar();
    this.renderBuildPanel();
    this.renderRecipePanel();
    this.renderTechnologies();
    this.renderContract();
    this.renderRequests();
    this.renderNextAction();
    // Both kinds of change can complete a checkpoint: the first iron is a pack change and the
    // first powered composer is a factory one, so the clock reads whenever either moved.
    this.evaluateRun(next);
  }
  // Before the inspector renders, so the machine the player just walked up to is on the panel in
  // the same pass rather than a tick later.
  this.syncStandingSelection(
    previous === next || previous.buildings !== next.buildings,
  );
  if (factoryChanged || this.selected) this.renderInspector();
  const latestEvent = this.snapshot.events.at(-1) ?? "";
  if (
    latestEvent &&
    latestEvent !== this.lastEvent &&
    !this.SILENT_EVENTS.has(latestEvent)
  ) {
    this.showFeedback(latestEvent);
    // Sound comes from the same event the toast does, so a delivery made by a belt and one made
    // by hand are the same thing heard as well as read.
    const cue = cueForEvent(latestEvent);
    if (cue) this.audio.play(cue);
  }
  this.lastEvent = latestEvent;
  const victory = required<HTMLDivElement>("victory");
  victory.hidden = !this.snapshot.victory;
  if (!previousVictory && this.snapshot.victory)
    this.showFeedback("Founding contract complete — free play continues");
};

Runtime.prototype.refreshLandingHub = function refreshLandingHub(
  this: Runtime,
): void {
  const key = `${this.snapshot.scenario}:${this.snapshot.seed}`;
  if (key === this.landingHubWorld) return;
  this.landingHubWorld = key;
  this.landingHub = findLandingHub(this.snapshot);
};

Runtime.prototype.syncStandingSelection = function syncStandingSelection(
  this: Runtime,
  buildingsChanged: boolean,
): void {
  const standing = pixelToAxial(this.snapshot.player, WORLD_SCALE);
  const hex = `${standing.q},${standing.r}`;
  if (hex === this.standingHex && !buildingsChanged) return;
  this.standingHex = hex;
  const cell = buildingBeside(this.snapshot);
  const key = cell ? `${cell.q},${cell.r}` : null;
  if (key === this.besideBuilding) return;
  this.besideBuilding = key;
  if (!cell) return;
  this.selected = cell;
  this.renderer.setSelection(cell);
  this.panels.revealInspector();
};

Runtime.prototype.renderHomeReadout = function renderHomeReadout(
  this: Runtime,
): void {
  const element = required<HTMLElement>("home-readout");
  const text = required<HTMLElement>("home-readout-text");
  if (!this.landingHub) {
    element.classList.remove("away");
    text.textContent = "No landing hub in this world";
    return;
  }
  const bearing = homeBearing(this.snapshot.player, this.landingHub);
  element.classList.toggle("away", bearing !== null);
  if (bearing) {
    const degrees = (Math.atan2(bearing.x, -bearing.y) * 180) / Math.PI;
    element.style.setProperty("--home-bearing", `${degrees}deg`);
  }
  text.textContent = bearing
    ? `Landing hub · ${bearing.hexes} hex ${DIRECTION_NAMES[bearing.direction]}`
    : "Landing hub · you are here";
};

Runtime.prototype.sameCarry = function sameCarry(
  this: Runtime,
  previous: FactorySnapshot["player"],
  next: FactorySnapshot["player"],
): boolean {
  const a = previous.carry_stacks;
  const b = next.carry_stacks;
  if (a === b) return true;
  if (a.length !== b.length) return false;
  return a.every(
    (stack, index) =>
      stack.item_id === b[index]?.item_id &&
      stack.quantity === b[index]?.quantity,
  );
};

Runtime.prototype.itemById = function itemById(
  this: Runtime,
  itemId: number | undefined,
): ItemDefinition | undefined {
  return itemId === undefined
    ? undefined
    : this.host.definitions.items.find(({ id }) => id === itemId);
};

Runtime.prototype.paintChip = function paintChip(
  this: Runtime,
  holder: HTMLElement,
  itemId: number | undefined,
  view: ItemChipView = {},
): HTMLElement {
  let chip = this.holdersChip.get(holder);
  if (!chip) {
    chip = createItemChip();
    holder.append(chip);
    this.holdersChip.set(holder, chip);
  }
  fillItemChip(chip, this.itemById(itemId), itemId, view);
  return chip;
};

Runtime.prototype.renderInventory = function renderInventory(
  this: Runtime,
): void {
  const element = required<HTMLDivElement>("inventory");
  const stacks = this.snapshot.player.carry_stacks;
  const slots = Math.max(this.snapshot.player.carry_slots, stacks.length);
  const cells = syncChildren(
    element,
    Array.from({ length: slots }, (_, index) => `slot-${index}`),
    () => {
      const cell = document.createElement("div");
      cell.className = "inventory-slot";
      cell.setAttribute("role", "listitem");
      return cell;
    },
  );
  cells.forEach((cell, index) => {
    const stack = stacks[index];
    cell.classList.toggle("filled", Boolean(stack));
    // A slot is dense: the glyph is the identity and the name would not fit, so it rides the
    // chip's own label rather than a second aria string written here.
    this.paintChip(cell, stack?.item_id, {
      count: stack?.quantity,
      named: false,
      short: true,
    });
    const item = this.itemById(stack?.item_id);
    cell.dataset.stackSource = "player";
    cell.dataset.itemId = stack ? String(stack.item_id) : "";
    cell.dataset.quantity = stack ? String(stack.quantity) : "0";
    cell.tabIndex = 0;
    cell.title =
      item && stack
        ? itemTooltip(item, item.name, { count: stack.quantity })
        : "Empty carrying slot";
    cell.setAttribute(
      "aria-label",
      item && stack
        ? `${item.name}: ${stack.quantity} of ${item.stack_size}`
        : "Empty carrying slot",
    );
  });
  required<HTMLElement>("carry-value").textContent =
    `${stacks.length} / ${this.snapshot.player.carry_slots}`;
  required<HTMLElement>("carry-detail").textContent =
    `${stacks.length} of ${this.snapshot.player.carry_slots} slots carried.`;
  // The top bar is the glanceable pack: the player can see what changed without opening a panel.
  // It is deliberately only a preview of native's published carry stacks; the full grid remains I.
  const peek = required<HTMLElement>("inventory-peek");
  const visible = stacks.slice(0, 3);
  const preview = syncChildren(
    peek,
    visible.map((stack, index) => `${index}-${stack.item_id}`),
    () => {
      const holder = document.createElement("span");
      holder.className = "inventory-peek-slot chip-host";
      return holder;
    },
  );
  visible.forEach((stack, index) => {
    const holder = preview[index];
    if (!holder) return;
    this.paintChip(holder, stack.item_id, {
      count: stack.quantity,
      named: false,
      short: true,
    });
    const item = this.itemById(stack.item_id);
    if (item) {
      holder.title = itemTooltip(item, item.name, { count: stack.quantity });
    }
  });
  peek.dataset.overflow =
    stacks.length > visible.length ? `+${stacks.length - visible.length}` : "";
  peek.classList.toggle("empty", stacks.length === 0);
  // A lifted drag owns the floating stack, because it is carrying something native has not been
  // told about yet: the pickup is only sent when the drop lands.
  if (this.stackDrag?.lifted) return;
  const cursor = required<HTMLElement>("cursor-stack");
  const hand = this.snapshot.player.hand ?? undefined;
  cursor.hidden = !hand;
  if (hand) {
    this.paintChip(cursor, hand.item_id, {
      count: hand.quantity,
      named: false,
      short: true,
    });
    const item = this.itemById(hand.item_id);
    if (item) {
      cursor.title = itemTooltip(item, item.name, { count: hand.quantity });
    }
  }
};

Runtime.prototype.renderCreative = function renderCreative(
  this: Runtime,
): void {
  const { creative, carry_slots } = this.snapshot.player;
  // Creative is a different game, so a creative run is not a comparable one. The mark is applied
  // here rather than trusting title-screen state because loaded and newly created runs arrive as
  // the same snapshot. Once chosen at world creation, the mode cannot change.
  if (creative && this.run) {
    const marked = taintRun(this.run, "creative");
    if (marked !== this.run) {
      this.run = marked;
      writeRun(localStorage, this.run);
      this.renderRun();
    }
  }
  this.creativeChip.hidden = !creative;
  if (!creative && this.panels.isOpen("creative-panel")) this.panels.close();
  this.creativeChip.classList.toggle("creative-on", creative);
  this.creativeChip.title = "Creative tools (C)";
  this.creativeSlotsInput.value = String(carry_slots);
  for (const control of [this.creativeSlotsInput, this.creativeClear])
    control.disabled = !creative;
  const rows = syncChildren(
    this.creativeItems,
    creative ? this.host.definitions.items.map(({ id }) => String(id)) : [],
    () => {
      const row = document.createElement("div");
      row.className = "creative-item";
      row.setAttribute("role", "listitem");
      // The holder the chip is painted into and the box the buttons live in are made once, here,
      // so neither list reconciles against the other: `syncChildren` deletes any child it does not
      // own, and a chip and a button row sharing one parent would take turns deleting each other.
      const holder = document.createElement("div");
      holder.className = "creative-item-chip";
      const actions = document.createElement("div");
      actions.className = "creative-item-actions";
      row.append(holder, actions);
      return row;
    },
  );
  rows.forEach((row, index) => {
    const item = this.host.definitions.items[index];
    const holder = row.firstElementChild as HTMLElement | null;
    const actions = row.lastElementChild as HTMLElement | null;
    if (!item || !holder || !actions) return;
    this.paintChip(holder, item.id, {
      count: heldQuantity(this.snapshot, item.id),
      named: true,
    });
    // Three amounts cover what anybody actually reaches for: one, a stack, and as much as the pack
    // will take. Native clamps each to the room left, so these are ceilings rather than promises —
    // which is why "Fill" can be an absurd number rather than a quantity the host has to work out.
    const amounts: {
      label: string;
      title: string;
      quantity: number;
    }[] = [
      { label: "+1", title: `Give 1 ${item.name}`, quantity: 1 },
      {
        label: `+${item.stack_size}`,
        title: `Give one stack of ${item.name}`,
        quantity: item.stack_size,
      },
      {
        label: "Fill",
        title: `Fill the pack with ${item.name}`,
        quantity: this.CREATIVE_FILL,
      },
    ];
    const buttons = syncChildren(
      actions,
      amounts.map(({ label }) => label),
      () => document.createElement("button"),
    );
    buttons.forEach((button, slot) => {
      const amount = amounts[slot];
      if (!amount || !(button instanceof HTMLButtonElement)) return;
      button.type = "button";
      button.textContent = amount.label;
      button.title = amount.title;
      button.setAttribute("aria-label", amount.title);
      button.dataset.itemId = String(item.id);
      button.dataset.quantity = String(amount.quantity);
    });
  });
};
