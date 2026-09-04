import { axialToPixel } from "@hexlife/embed/hex";
import { recipeOutputs } from "../core/recipes";
import {
  bandAt,
  TERRAIN_INFO,
  terrainAccess,
  waterBand,
} from "../core/terrain";
import { HEIGHT_UNIT_METRES } from "../rendering/sceneScale";
import { DIRECTION_NAMES, TRANSPORT_DIRECTIONS } from "../core/directions";
import type { EntitySnapshot } from "../core/types";
import { BUILDING_COLORS, isSurveyed } from "../rendering/FactoryRenderer";
import { itemTooltip } from "../rendering/itemChip";
import { part, required, syncChildren } from "../ui/dom";
import { machineStockSlots } from "../ui/stockSlots";
import { paintHexFace, setItemGlyph, setMeter } from "../ui/paint";
import type { StockCompartment } from "./runtime";
import { Runtime } from "./runtime";

declare module "./runtime" {
  interface Runtime {
    technologyReach(): Map<number, number>;
    renderTechnologies(): void;
    stockCompartments(building: EntitySnapshot): StockCompartment[];
    renderInspectorActions(building: EntitySnapshot | undefined): void;
    renderInspectorLoad(building: EntitySnapshot | undefined): void;
    renderOutputRouting(building: EntitySnapshot | undefined): void;
    panelsFitAbreast(): boolean;
    offerPackBeside(building: EntitySnapshot | undefined): void;
    renderInspector(): void;
  }
}

Runtime.prototype.technologyReach = function technologyReach(
  this: Runtime,
): Map<number, number> {
  const all = this.host.technologies.technologies;
  const researched = new Set(this.snapshot.researched);
  const depth = new Map<number, number>();
  const measure = (id: number, guard: Set<number>): number => {
    const known = depth.get(id);
    if (known !== undefined) return known;
    if (researched.has(id)) {
      depth.set(id, 0);
      return 0;
    }
    // A cycle is refused natively, so this only guards against a catalogue that never loaded.
    if (guard.has(id)) return Number.MAX_SAFE_INTEGER;
    guard.add(id);
    const technology = all.find((value) => value.id === id);
    const behind = (technology?.prerequisites ?? []).reduce(
      (deepest, prerequisite) =>
        Math.max(deepest, measure(prerequisite, guard)),
      0,
    );
    guard.delete(id);
    const value = behind + 1;
    depth.set(id, value);
    return value;
  };
  for (const technology of all) measure(technology.id, new Set());
  return depth;
};

Runtime.prototype.renderTechnologies = function renderTechnologies(
  this: Runtime,
): void {
  this.researchTree.update(this.snapshot);
};

Runtime.prototype.stockCompartments = function stockCompartments(
  this: Runtime,
  building: EntitySnapshot,
): StockCompartment[] {
  const definition = this.host.definitions.buildings.find(
    ({ id }) => id === building.definition_id,
  );
  const recipe = this.host.definitions.recipes.find(
    ({ id }) => id === building.recipe_id,
  );
  const waterId = this.host.definitions.items.find(
    (item) => item.key === "water",
  )?.id;
  const compartments: StockCompartment[] = [];
  if (building.kind === "container")
    compartments.push({
      stock: "inventory",
      label: "Storage",
      accepts: true,
      expected: definition?.accepted_item_ids ?? [],
      entries: building.inventory,
    });
  else {
    const hasInputs =
      building.kind === "boiler" ||
      Boolean(recipe?.inputs.length) ||
      Boolean(building.input_inventory?.length);
    const firebox =
      (building.kind === "generator" || building.kind === "boiler") &&
      (definition?.power_source === undefined ||
        definition.power_source === "burner");
    const hasFuel =
      Boolean(recipe?.fuel) ||
      firebox ||
      Boolean(building.fuel_inventory?.length);
    const hasOutput =
      building.kind === "composer" ||
      building.kind === "extractor" ||
      building.kind === "pump" ||
      Boolean(building.output_inventory?.length);
    if (hasInputs)
      compartments.push({
        stock: "input",
        label: "Ingredient",
        accepts: true,
        expected: [
          ...(recipe?.inputs.map((input) => input.item_id) ?? []),
          ...(building.kind === "boiler" && waterId ? [waterId] : []),
        ],
        entries: building.input_inventory ?? [],
      });
    if (hasFuel)
      compartments.push({
        stock: "fuel",
        label: "Fuel",
        accepts: true,
        expected: [],
        entries: building.fuel_inventory ?? [],
      });
    if (hasOutput)
      compartments.push({
        stock: "output",
        label: "Output",
        accepts: false,
        expected: [
          ...(recipe
            ? recipeOutputs(recipe).map((output) => output.item_id)
            : []),
          ...(definition?.output_item_id ? [definition.output_item_id] : []),
        ],
        entries: building.output_inventory ?? [],
      });
  }
  return compartments;
};

Runtime.prototype.renderInspectorActions = function renderInspectorActions(
  this: Runtime,
  building: EntitySnapshot | undefined,
): void {
  const container = required<HTMLElement>("inspect-stock");
  const list = required<HTMLDivElement>("inspector-actions");
  if (!building || !this.HAND_REACHABLE.has(building.kind)) {
    container.hidden = true;
    syncChildren(list, [], () => document.createElement("section"));
    return;
  }
  const definition = this.host.definitions.buildings.find(
    ({ id }) => id === building.definition_id,
  );
  const compartments = this.stockCompartments(building);
  container.hidden = compartments.length === 0;
  const cards = syncChildren(
    list,
    compartments.map(({ stock }) => stock),
    () => {
      const card = document.createElement("section");
      card.className = "machine-compartment";
      card.innerHTML = `<div class="machine-compartment-header"><span></span><span class="machine-compartment-count"></span></div><div class="machine-stock-grid" role="list"></div>`;
      return card;
    },
  );
  compartments.forEach(
    ({ stock, label, accepts, expected, entries }, index) => {
      const card = cards[index];
      if (!card) return;
      card.className = `machine-compartment ${stock}`;
      part<HTMLElement>(card, ".machine-compartment-header span").textContent =
        label;
      const total = entries.reduce((sum, entry) => sum + entry.quantity, 0);
      // Native bounds ingredients and fuel per item and the rest as one pool, so the count has to
      // read differently: `12 each` is the promise that a stocked slot never crowds out an empty
      // one, where `n / 12` would still claim the compartment is a single shared budget.
      const perItem = stock === "input" || stock === "fuel";
      const capacity = definition?.capacity;
      part<HTMLElement>(card, ".machine-compartment-count").textContent =
        capacity === undefined
          ? String(total)
          : perItem
            ? `${total} · ${capacity} each`
            : `${total} / ${capacity}`;
      const layout = machineStockSlots(
        entries,
        expected,
        accepts,
        capacity,
        perItem,
      );
      const slots = part<HTMLElement>(card, ".machine-stock-grid");
      const cells = syncChildren(
        slots,
        layout.map((slot) => slot.key),
        () => {
          const cell = document.createElement("div");
          cell.className = "machine-stock-slot chip-host";
          cell.setAttribute("role", "listitem");
          cell.tabIndex = 0;
          return cell;
        },
      );
      cells.forEach((cell, slot) => {
        const entry = layout[slot];
        if (!entry) return;
        const filled = entry.quantity > 0;
        cell.classList.toggle("filled", filled);
        cell.classList.toggle("ghost", Boolean(entry.ghost));
        cell.dataset.stackSource = "building";
        cell.dataset.stock = stock;
        cell.dataset.accepts = entry.accepts ? "1" : "0";
        cell.dataset.q = String(building.q);
        cell.dataset.r = String(building.r);
        cell.dataset.itemId =
          filled && entry.item_id ? String(entry.item_id) : "";
        cell.dataset.quantity = String(entry.quantity);
        this.paintChip(cell, entry.item_id, {
          count: filled ? entry.quantity : undefined,
          named: false,
          short: true,
        });
        const item = this.itemById(entry.item_id);
        const slotTooltip =
          item && filled
            ? `${label}: ${itemTooltip(item, item.name, { count: entry.quantity })}${item.fluid ? "\nLoose fluid — use a pipe or barrel station" : ""}`
            : item
              ? `Empty ${label.toLowerCase()} slot for ${item.name}\n${item.description}`
              : `Empty ${label.toLowerCase()} slot`;
        cell.title = slotTooltip;
        cell.setAttribute(
          "aria-label",
          item && filled
            ? `${label}: ${item.name}, ${entry.quantity}`
            : item
              ? `Empty ${label.toLowerCase()} slot for ${item.name}`
              : `Empty ${label.toLowerCase()} slot`,
        );
      });
    },
  );
};

Runtime.prototype.renderInspectorLoad = function renderInspectorLoad(
  this: Runtime,
  building: EntitySnapshot | undefined,
): void {
  void building;
  required<HTMLElement>("inspect-load").hidden = true;
};

Runtime.prototype.renderOutputRouting = function renderOutputRouting(
  this: Runtime,
  building: EntitySnapshot | undefined,
): void {
  const section = required<HTMLElement>("inspect-output-routing");
  const routes = building?.output_routes ?? [];
  if (!building || routes.length === 0) {
    section.hidden = true;
    return;
  }
  section.hidden = false;
  const remembered = this.selectedOutputProduct.get(building.id);
  const selectedItem = routes.some(({ item_id }) => item_id === remembered)
    ? remembered!
    : routes[0]!.item_id;
  this.selectedOutputProduct.set(building.id, selectedItem);
  const selectedRoute = routes.find(({ item_id }) => item_id === selectedItem)!;
  const item = this.itemById(selectedItem);
  section.style.setProperty("--item-color", item?.color ?? "var(--gold)");
  const products = required<HTMLElement>("inspect-output-products");
  const productButtons = syncChildren(
    products,
    routes.map(({ item_id }) => String(item_id)),
    () => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "inspect-output-product chip-host";
      return button;
    },
  );
  routes.forEach((route, index) => {
    const button = productButtons[index]!;
    const routeItem = this.itemById(route.item_id);
    button.dataset.itemId = String(route.item_id);
    button.classList.toggle("active", route.item_id === selectedItem);
    button.style.setProperty("--item-color", routeItem?.color ?? "var(--gold)");
    button.setAttribute("aria-pressed", String(route.item_id === selectedItem));
    button.setAttribute(
      "aria-label",
      `Route ${routeItem?.name ?? `item ${route.item_id}`}`,
    );
    this.paintChip(button, route.item_id, { named: true, short: true });
  });
  required<HTMLElement>("inspect-output-summary").textContent =
    `${item?.name ?? `Item ${selectedItem}`} · ${DIRECTION_NAMES[selectedRoute.direction] ?? "Output"} from q${selectedRoute.q}, r${selectedRoute.r}`;
  const status = required<HTMLElement>("inspect-output-status");
  status.textContent = selectedRoute.target_id ? "Connected" : "Open";
  status.classList.toggle("open", !selectedRoute.target_id);
  const footprint = building.footprint;
  const footprintKeys = new Set(footprint.map(({ q, r }) => `${q},${r}`));
  const centers = footprint.map((cell) => ({
    cell,
    point: axialToPixel(cell, 26, { x: 0, y: 0 }),
  }));
  const minY = Math.min(...centers.map(({ point }) => point.y));
  const maxY = Math.max(...centers.map(({ point }) => point.y));
  const minX = Math.min(...centers.map(({ point }) => point.x));
  const maxX = Math.max(...centers.map(({ point }) => point.x));
  const midX = (minX + maxX) / 2;
  const map = required<HTMLElement>("inspect-output-map");
  map.style.height = `${Math.max(92, maxY - minY + 78)}px`;
  const cells = syncChildren(
    required<HTMLElement>("inspect-output-cells"),
    footprint.map(({ q, r }) => `${q},${r}`),
    () => {
      const cell = document.createElement("div");
      cell.className = "inspect-output-cell";
      return cell;
    },
  );
  centers.forEach(({ cell, point }, index) => {
    const element = cells[index]!;
    element.style.setProperty("--output-x", `${point.x - midX}px`);
    element.style.setProperty("--output-y", `${point.y - minY + 39}px`);
    element.classList.toggle(
      "active",
      cell.q === selectedRoute.q && cell.r === selectedRoute.r,
    );
    element.textContent = `q${cell.q}\nr${cell.r}`;
  });
  const ports = centers.flatMap(({ cell, point }) =>
    TRANSPORT_DIRECTIONS.slice(0, 6).flatMap((direction, index) => {
      if (footprintKeys.has(`${cell.q + direction.q},${cell.r + direction.r}`))
        return [];
      const step = axialToPixel(direction, 26, { x: 0, y: 0 });
      const length = Math.hypot(step.x, step.y) || 1;
      return [
        {
          q: cell.q,
          r: cell.r,
          direction: index,
          x: point.x - midX + (step.x / length) * 30,
          y: point.y - minY + 39 + (step.y / length) * 30,
          angle: Math.atan2(step.y, step.x),
        },
      ];
    }),
  );
  const portButtons = syncChildren(
    required<HTMLElement>("inspect-output-ports"),
    ports.map(({ q, r, direction }) => `${q},${r},${direction}`),
    () => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "inspect-output-port";
      button.innerHTML = "<span>›</span>";
      return button;
    },
  );
  ports.forEach((port, index) => {
    const button = portButtons[index]!;
    const active =
      port.q === selectedRoute.q &&
      port.r === selectedRoute.r &&
      port.direction === selectedRoute.direction;
    button.dataset.q = String(port.q);
    button.dataset.r = String(port.r);
    button.dataset.direction = String(port.direction);
    button.style.setProperty("--output-x", `${port.x}px`);
    button.style.setProperty("--output-y", `${port.y}px`);
    button.style.setProperty("--output-angle", `${port.angle}rad`);
    button.classList.toggle("active", active);
    button.setAttribute("aria-pressed", String(active));
    button.setAttribute(
      "aria-label",
      `Send ${item?.name ?? "product"} ${DIRECTION_NAMES[port.direction]} from footprint tile q${port.q}, r${port.r}`,
    );
    button.title = `${DIRECTION_NAMES[port.direction]} from q${port.q}, r${port.r}`;
  });
};

Runtime.prototype.panelsFitAbreast = function panelsFitAbreast(
  this: Runtime,
): boolean {
  return (
    getComputedStyle(document.documentElement)
      .getPropertyValue("--panels-abreast")
      .trim() === "1"
  );
};

Runtime.prototype.offerPackBeside = function offerPackBeside(
  this: Runtime,
  building: EntitySnapshot | undefined,
): void {
  const open = this.panels.isOpen(this.INVENTORY_PANEL);
  const takes =
    building !== undefined &&
    this.stockCompartments(building).some(({ accepts }) => accepts);
  const key = takes && building ? `${building.q},${building.r}` : null;
  if (key === this.packOfferedFor) return;
  this.packOfferedFor = key;
  // Narrow layouts put the two panels in the same space, so opening the pack would take away the
  // machine the player just selected. There the pack stays behind its key.
  if (key === null || this.packDeclined || open || !this.panelsFitAbreast())
    return;
  this.panels.reveal(this.INVENTORY_PANEL);
};

Runtime.prototype.renderInspector = function renderInspector(
  this: Runtime,
): void {
  const empty = required<HTMLElement>("inspect-empty");
  const sheet = required<HTMLElement>("inspect-sheet");
  const kicker = required<HTMLElement>("inspect-kicker");
  const title = required<HTMLElement>("inspect-title");
  const status = required<HTMLElement>("inspect-status");
  if (!this.selected) {
    empty.hidden = false;
    sheet.hidden = true;
    kicker.textContent = "World inspector";
    title.textContent = "Select a hex";
    status.hidden = true;
    required<HTMLElement>("inspect-habitat").hidden = true;
    this.renderInspectorActions(undefined);
    this.renderOutputRouting(undefined);
    this.renderInspectorLoad(undefined);
    this.renderInspectorTier(undefined);
    this.renderInspectorRecipe(undefined);
    this.renderInspectorHub(undefined);
    this.offerPackBeside(undefined);
    return;
  }
  const building = this.selected ? this.buildingAt(this.selected) : undefined;
  this.offerPackBeside(building);
  const selectedWorld = axialToPixel(this.selected, 1024, { x: 0, y: 0 });
  // Field cells are addressed by their tile key, exactly as the native patch addresses them.
  const resource = this.snapshot.resources.find(
    ({ q, r }) => q === this.selected?.q && r === this.selected?.r,
  );
  const habitat = this.snapshot.habitats.find(
    ({ q, r }) => q === this.selected?.q && r === this.selected?.r,
  );
  const surveyed = isSurveyed(this.snapshot.chunks, selectedWorld);
  const definition = building
    ? this.host.definitions.buildings.find(
        ({ id }) => id === building.definition_id,
      )
    : undefined;
  const fieldItem = resource
    ? this.host.definitions.items.find(({ id }) => id === resource.item_id)
    : undefined;
  empty.hidden = true;
  sheet.hidden = false;
  required<HTMLElement>("inspect-q").textContent = String(this.selected.q);
  required<HTMLElement>("inspect-r").textContent = String(this.selected.r);
  const field = required<HTMLElement>("inspect-field");
  // The actual field is what a new player needs first. Band potentials belong on empty
  // ground; listing them above an iron cell is how the purple hex stayed anonymous.
  if (resource) {
    field.hidden = false;
    field.classList.toggle("inspect-field-solo", !building);
    field.style.setProperty("--item-color", fieldItem?.color ?? "transparent");
    this.paintChip(
      required<HTMLElement>("inspect-field-chip"),
      resource.item_id,
      {
        progress: {
          have: resource.quantity,
          need: resource.initial_quantity,
        },
        meter: true,
      },
    );
  } else {
    field.hidden = true;
  }
  const habitatPanel = required<HTMLElement>("inspect-habitat");
  habitatPanel.hidden = !habitat;
  if (habitat) {
    required<HTMLElement>("inspect-habitat-name").textContent =
      `Fertile riverbank · capacity ${habitat.capacity}`;
    required<HTMLElement>("inspect-habitat-note").textContent =
      `Dry, unbuilt ground with fresh standing water in its ring, watered at class ` +
      `${habitat.discharge}. A cut canal waters ground the same way a river does.`;
  }
  const terrainSample = surveyed
    ? this.snapshot.terrain.find(
        ({ q, r }) => q === this.selected?.q && r === this.selected?.r,
      )
    : undefined;
  const terrain = terrainSample?.terrain;
  // The band says what the generator drew; the grade on top of it says what the player has since
  // made of the hex. Only the pair answers "can I stand here", which is the question this panel is
  // being asked, so a quarried cliff stops reading Impassable the moment its face is down.
  const ground = this.snapshot.ground.find(
    ({ q, r }) => q === this.selected?.q && r === this.selected?.r,
  );
  const grade = (ground?.elevation ?? 0) + (ground?.erosion ?? 0);
  const altitude = terrainSample
    ? (terrainSample.height + grade) * HEIGHT_UNIT_METRES
    : undefined;
  required<HTMLElement>("inspect-alt").textContent =
    altitude === undefined
      ? "—"
      : `${altitude >= 0 ? "+" : "−"}${Math.abs(altitude).toFixed(1)} m`;
  const waterDeparture = this.snapshot.water.find(
    ({ q, r }) => q === this.selected?.q && r === this.selected?.r,
  )?.departure;
  const waterDepth = terrainSample
    ? Math.max(0, terrainSample.water_depth + (waterDeparture ?? 0))
    : 0;
  // A presentation band is generated equilibrium. Once the player floods a dry band or drains a
  // wet one, the inspector must name the physical state in front of them rather than the old map.
  const band = terrain
    ? waterDepth > 0
      ? TERRAIN_INFO[waterBand(waterDepth)]
      : bandAt(terrain, grade)
    : undefined;
  if (building) {
    kicker.textContent = "Building";
    title.textContent = definition?.name ?? this.titleCase(building.kind);
    status.hidden = false;
    status.textContent =
      definition?.manual_work && building.status === "switched off"
        ? building.progress > 0
          ? "work paused"
          : "awaiting player work"
        : building.status;
    status.className = `inspect-status ${this.STATUS_TONE[building.status] ?? "wait"}`;
  } else if (resource) {
    kicker.textContent = "Field";
    title.textContent = fieldItem?.name ?? "Resource";
    if (fieldItem?.regrowth_ticks) {
      status.hidden = false;
      status.textContent = "regrows";
      status.className = "inspect-status live";
    } else {
      status.hidden = true;
    }
  } else if (habitat) {
    kicker.textContent = "Habitat";
    title.textContent = "Fertile riverbank";
    status.hidden = false;
    status.textContent = `capacity ${habitat.capacity}`;
    status.className = "inspect-status live";
  } else if (!surveyed) {
    kicker.textContent = "Unsurveyed";
    title.textContent = "Fog";
    status.hidden = true;
  } else {
    kicker.textContent = "Ground";
    title.textContent = band?.name ?? "Lowland";
    status.hidden = true;
  }
  const hex = required<HTMLElement>("inspect-hex");
  const mark = required<HTMLElement>("inspect-portrait-mark");
  const facingTick = required<HTMLElement>("inspect-facing-tick");
  if (building) {
    paintHexFace(hex, BUILDING_COLORS[building.kind], "#dce7ef", false);
    mark.textContent = definition?.icon ?? "";
    facingTick.hidden = false;
    facingTick.className = `inspect-facing-tick dir-${building.orientation}`;
  } else if (resource && fieldItem) {
    paintHexFace(
      hex,
      band?.fill ?? fieldItem.color,
      band?.stroke ?? "#f4f7f5",
      !(band?.passable ?? true),
    );
    setItemGlyph(mark, fieldItem.icon, fieldItem.color);
    facingTick.hidden = true;
  } else if (habitat && band) {
    paintHexFace(hex, band.fill, band.stroke, !band.passable);
    mark.textContent = "⋀⋀";
    facingTick.hidden = true;
  } else if (band) {
    paintHexFace(hex, band.fill, band.stroke, !band.passable);
    mark.textContent = "";
    facingTick.hidden = true;
  } else {
    paintHexFace(hex, this.FOG_FILL, this.FOG_STROKE, false);
    mark.textContent = "";
    facingTick.hidden = true;
  }
  const place = required<HTMLElement>("inspect-place");
  if (surveyed && band) {
    place.hidden = false;
    paintHexFace(
      required<HTMLElement>("inspect-band-swatch"),
      band.fill,
      band.stroke,
      !band.passable,
    );
    required<HTMLElement>("inspect-band-name").textContent = band.name;
    const access = required<HTMLElement>("inspect-access");
    access.textContent = terrainAccess(band);
    access.classList.toggle("impassable-label", !band.passable);
    required<HTMLElement>("inspect-band-note").textContent =
      resource || building ? "" : band.note;
  } else if (!surveyed) {
    place.hidden = false;
    paintHexFace(
      required<HTMLElement>("inspect-band-swatch"),
      this.FOG_FILL,
      this.FOG_STROKE,
      false,
    );
    required<HTMLElement>("inspect-band-name").textContent = "Unsurveyed";
    const access = required<HTMLElement>("inspect-access");
    access.textContent = "Fog";
    access.classList.remove("impassable-label");
    required<HTMLElement>("inspect-band-note").textContent =
      "Travel here to lift the fog";
  } else {
    place.hidden = true;
  }
  const machine = required<HTMLElement>("inspect-machine");
  machine.hidden = !building || building.kind === "hub";
  if (building && building.kind !== "hub") {
    setMeter(
      required<HTMLElement>("inspect-progress-meter"),
      required<HTMLElement>("inspect-progress-fill"),
      required<HTMLElement>("inspect-progress-amount"),
      building.progress,
      building.progress_total,
      building.progress_total > 0,
    );
    // Offered only where native will honour it — a composer, part way through a craft, that the
    // scenario does not own — because a button whose whole behaviour is to explain why it refused
    // is worse than no button. The coordinates ride on the element so the click reads the machine
    // it was drawn for rather than whatever the inspector has moved on to.
    const cancelButton = required<HTMLButtonElement>("inspect-cancel-craft");
    cancelButton.hidden =
      building.kind !== "composer" ||
      building.progress === 0 ||
      building.scenario_owned;
    cancelButton.dataset.q = String(building.q);
    cancelButton.dataset.r = String(building.r);
    setMeter(
      required<HTMLElement>("inspect-fuel-meter"),
      required<HTMLElement>("inspect-fuel-fill"),
      required<HTMLElement>("inspect-fuel-amount"),
      building.fuel_charge ?? 0,
      building.fuel_required ?? 0,
      Boolean(building.fuel_required),
    );
    // A machine that banks electricity shows its own bank; anything else on the network — a
    // generator, a pole — shows the grid it is part of. One meter, and in both cases the number
    // the player would act on: a bank that keeps hitting zero wants more generation, and a grid
    // whose draw is over its supply says which.
    const banks = Boolean(building.power_capacity);
    required<HTMLElement>("inspect-power-label").textContent = banks
      ? "Charge"
      : "Power";
    setMeter(
      required<HTMLElement>("inspect-power-meter"),
      required<HTMLElement>("inspect-power-fill"),
      required<HTMLElement>("inspect-power-amount"),
      banks ? (building.power_charge ?? 0) : (building.power_satisfied ?? 0),
      banks ? (building.power_capacity ?? 0) : (building.power_demand ?? 0),
      banks || Boolean(building.power_demand),
    );
    const waterSource = required<HTMLElement>("inspect-water-source");
    waterSource.hidden = building.kind !== "pump" || !building.water_source;
    if (building.kind === "pump" && building.water_source) {
      const source = building.water_source;
      waterSource.textContent =
        source.discharge > 0
          ? `River source ${source.q},${source.r} · class ${source.discharge} · limit ${source.rate}/tick`
          : `Finite water ${source.q},${source.r} · ${source.available} depth quantum${source.available === 1 ? "" : "s"} left`;
    }
    for (const spoke of required<HTMLElement>("inspect-compass").children) {
      const tick = spoke as HTMLElement;
      tick.classList.toggle(
        "on",
        Number(tick.dataset.dir) === building.orientation,
      );
    }
    required<HTMLElement>("inspect-facing-name").textContent =
      DIRECTION_NAMES[building.orientation] ?? `Facing ${building.orientation}`;
    required<HTMLElement>("inspect-protected").hidden =
      !building.scenario_owned;
    this.renderInspectorSwitch(building);
    const cargo = required<HTMLElement>("inspect-cargo");
    cargo.hidden = building.kind !== "belt" || !building.cargo;
    if (building.kind === "belt" && building.cargo)
      this.paintChip(
        required<HTMLElement>("inspect-cargo-chip"),
        building.cargo.item_id,
        { count: building.cargo.quantity },
      );
  }
  this.renderInspectorActions(building);
  this.renderOutputRouting(building);
  this.renderInspectorLoad(building);
  this.renderInspectorTier(building);
  this.renderInspectorRecipe(building);
  this.renderInspectorHub(building);
};
