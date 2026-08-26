import type { AxialCoordinate } from "@hexlife/embed/hex";
import type {
  BuildingDefinition,
  BuildingKind,
  Ingredient,
  OrientationAxis,
  PlacementRule,
  PowerSource,
} from "../../core/types";
import { itemIconSvg } from "../../rendering/icons";
import { FootprintEditor } from "../footprintEditor";
import type { AdminStore } from "../state";
import { showToast } from "../toast";

const BUILDING_KINDS: BuildingKind[] = [
  "extractor",
  "belt",
  "composer",
  "container",
  "consumer",
  "hub",
  "pump",
  "pole",
  "generator",
  "boiler",
  "bridge",
];

const PLACEMENT_RULES: PlacementRule[] = [
  "ground",
  "resource",
  "water",
  "elevated",
  "shallows",
];

const POWER_SOURCES: PowerSource[] = ["burner", "wind", "hydro", "turbine"];

export function renderBuildingsView(
  container: HTMLElement,
  store: AdminStore,
): void {
  container.innerHTML = "";

  const view = document.createElement("div");
  view.className = "admin-view buildings-view";

  const itemMap = new Map(store.definitions.items.map((i) => [i.id, i]));
  const techMap = new Map(
    store.technologies.technologies.map((t) => [t.id, t]),
  );

  // Toolbar
  const toolbar = document.createElement("div");
  toolbar.className = "view-toolbar";

  // Search input
  const searchInput = document.createElement("input");
  searchInput.type = "search";
  searchInput.placeholder =
    "Search buildings by name, key, kind, category, description...";
  searchInput.className = "search-input";
  searchInput.value = store.searchQuery;
  searchInput.oninput = () => store.setSearchQuery(searchInput.value.trim());
  toolbar.appendChild(searchInput);

  // Kind filter chips
  const filterChips = document.createElement("div");
  filterChips.className = "filter-chips";

  const allChip = document.createElement("button");
  allChip.type = "button";
  allChip.className = `chip-btn ${store.selectedFilter === "all" ? "active" : ""}`;
  allChip.textContent = "All Kinds";
  allChip.onclick = () => store.setSelectedFilter("all");
  filterChips.appendChild(allChip);

  for (const k of BUILDING_KINDS) {
    const chip = document.createElement("button");
    chip.type = "button";
    chip.className = `chip-btn ${store.selectedFilter === k ? "active" : ""}`;
    chip.textContent = k.charAt(0).toUpperCase() + k.slice(1);
    chip.onclick = () => store.setSelectedFilter(k);
    filterChips.appendChild(chip);
  }
  toolbar.appendChild(filterChips);

  // Add Building button
  const addBtn = document.createElement("button");
  addBtn.type = "button";
  addBtn.className = "btn btn-primary";
  addBtn.innerHTML = "<span>+</span> Add Building";
  addBtn.onclick = () => {
    const nextId = store.getNextBuildingId();
    const firstItemId = store.definitions.items[0]?.id ?? 1;
    const newBuilding: BuildingDefinition = {
      id: nextId,
      key: `building-${nextId}`,
      name: `New Building ${nextId}`,
      kind: "composer",
      description: "Description of this machine.",
      icon: "BLD",
      recipe_category: "assembly",
      placement_rule: "ground",
      buildable: true,
      blocks_movement: true,
      footprint: [{ q: 0, r: 0 }],
      construction_cost: [{ item_id: firstItemId, quantity: 5 }],
    };
    store.setEditingTarget({
      type: "building",
      data: newBuilding,
      isNew: true,
    });
  };
  toolbar.appendChild(addBtn);

  view.appendChild(toolbar);

  // Filter buildings
  const query = store.searchQuery.toLowerCase();
  const filterKind = store.selectedFilter;

  const filteredBuildings = store.definitions.buildings.filter((b) => {
    if (filterKind !== "all" && b.kind !== filterKind) return false;
    if (query) {
      const matchText =
        `${b.id} ${b.key} ${b.name} ${b.kind} ${b.recipe_category ?? ""} ${b.description}`.toLowerCase();
      if (!matchText.includes(query)) return false;
    }
    return true;
  });

  // Count bar
  const countBar = document.createElement("div");
  countBar.className = "view-count-bar";
  countBar.innerHTML = `Showing <strong>${filteredBuildings.length}</strong> of ${store.definitions.buildings.length} buildings`;
  view.appendChild(countBar);

  // Buildings Grid
  const grid = document.createElement("div");
  grid.className = "buildings-grid";

  for (const building of filteredBuildings) {
    const card = document.createElement("div");
    card.className = "building-card";

    const unlockTech = building.unlock_technology_id
      ? techMap.get(building.unlock_technology_id)
      : undefined;
    const nextTierBuilding = building.upgrades_to
      ? store.definitions.buildings.find((b) => b.id === building.upgrades_to)
      : undefined;

    card.innerHTML = `
      <div class="card-header">
        <div class="building-badge-icon">${building.icon}</div>
        <div class="building-title-group">
          <div class="card-title-row">
            <h3 class="card-title">${building.name}</h3>
            <span class="id-badge">#${building.id}</span>
          </div>
          <div class="building-meta-row">
            <code class="item-key">${building.key}</code>
            <span class="badge badge-kind">${building.kind}</span>
            ${building.tier !== undefined ? `<span class="badge badge-tier">Tier ${building.tier}</span>` : ""}
            <span class="badge badge-footprint">${building.footprint.length} hex${building.footprint.length > 1 ? "es" : ""}</span>
          </div>
        </div>
      </div>

      <p class="card-description">${building.description}</p>

      <!-- Mini Footprint Visualizer -->
      <div class="mini-footprint-container" data-id="${building.id}"></div>

      <!-- Stats Grid -->
      <div class="building-stats-grid">
        ${building.power_draw ? `<div class="stat-cell"><small>Power Draw</small><strong>⚡ ${building.power_draw} W</strong></div>` : ""}
        ${building.power_output ? `<div class="stat-cell"><small>Power Output</small><strong class="mint-text">⚡ +${building.power_output} W (${building.power_source})</strong></div>` : ""}
        ${building.extract_radius ? `<div class="stat-cell"><small>Reach Radius</small><strong>⛏ ${building.extract_radius} hex</strong></div>` : ""}
        ${building.extract_speed ? `<div class="stat-cell"><small>Speed Rate</small><strong>🚀 ${building.extract_speed}%</strong></div>` : ""}
        ${building.recipe_category ? `<div class="stat-cell"><small>Recipe Category</small><strong class="gold-text">⚙ ${building.recipe_category}</strong></div>` : ""}
        ${building.underpass_span ? `<div class="stat-cell"><small>Underpass Span</small><strong>🚇 ${building.underpass_span} hexes</strong></div>` : ""}
        ${building.pole_reach ? `<div class="stat-cell"><small>Pole Reach</small><strong>🗼 ${building.pole_reach} (supply: ${building.supply_radius})</strong></div>` : ""}
        ${building.placement_rule ? `<div class="stat-cell"><small>Placement</small><strong>📍 ${building.placement_rule}</strong></div>` : ""}
      </div>

      <!-- Construction Cost -->
      <div class="building-cost-section">
        <span class="cost-label">Construction Cost:</span>
        <div class="cost-pills">
          ${building.construction_cost
            .map((c) => {
              const it = itemMap.get(c.item_id);
              const color = it?.color ?? "#888";
              const svg = itemIconSvg(it?.icon ?? "ore", color);
              return `<span class="cost-pill" style="border-color: ${color}50; background: ${color}12;">${svg} ${c.quantity}× ${it?.name ?? `#${c.item_id}`}</span>`;
            })
            .join("")}
        </div>
      </div>

      ${
        nextTierBuilding
          ? `<div class="upgrade-link-row"><span>⬆ Upgrades to:</span> <strong>${nextTierBuilding.name}</strong> (Tier ${nextTierBuilding.tier ?? 1})</div>`
          : ""
      }
      ${
        unlockTech
          ? `<div class="tech-unlock-row"><span>🔬 Unlocked by:</span> <strong>${unlockTech.name}</strong> (${unlockTech.cost} Insight)</div>`
          : `<div class="tech-unlock-row default-unlocked"><span>🔓 Available at start</span></div>`
      }

      <div class="card-actions">
        <button type="button" class="btn btn-sm btn-edit">Edit</button>
        <button type="button" class="btn btn-sm btn-subtle btn-dup">Duplicate</button>
        <button type="button" class="btn btn-sm btn-danger btn-del">Delete</button>
      </div>
    `;

    // Render mini footprint
    const fpContainer = card.querySelector<HTMLElement>(
      ".mini-footprint-container",
    );
    if (fpContainer) {
      new FootprintEditor({
        container: fpContainer,
        initialFootprint: building.footprint,
        gridRadius: 1,
        readOnly: true,
      });
    }

    card.querySelector(".btn-edit")?.addEventListener("click", () => {
      store.setEditingTarget({
        type: "building",
        data: structuredClone(building),
        isNew: false,
      });
    });

    card.querySelector(".btn-dup")?.addEventListener("click", () => {
      const cloned = store.duplicateBuilding(building.id);
      if (cloned)
        showToast(`Duplicated ${building.name} as #${cloned.id}`, "success");
    });

    card.querySelector(".btn-del")?.addEventListener("click", () => {
      if (confirm(`Delete building "${building.name}"?`)) {
        store.deleteBuilding(building.id);
        showToast(`Deleted ${building.name}`, "info");
      }
    });

    grid.appendChild(card);
  }

  view.appendChild(grid);
  container.appendChild(view);

  // If currently editing a building, render modal
  if (store.editingTarget?.type === "building") {
    renderBuildingModal(
      store.editingTarget.data,
      store.editingTarget.isNew,
      store,
    );
  }
}

function renderBuildingModal(
  building: BuildingDefinition,
  isNew: boolean,
  store: AdminStore,
): void {
  const modalOverlay = document.createElement("div");
  modalOverlay.className = "modal-overlay";

  const modal = document.createElement("div");
  modal.className = "modal building-modal";

  const currentBuilding: BuildingDefinition = structuredClone(building);

  function renderCostList(): void {
    const listEl = modal.querySelector<HTMLElement>("#building-cost-list");
    if (!listEl) return;
    listEl.innerHTML = "";

    currentBuilding.construction_cost.forEach((cost, index) => {
      const row = document.createElement("div");
      row.className = "ingredient-builder-row";

      const select = document.createElement("select");
      select.required = true;
      for (const it of store.definitions.items) {
        const opt = document.createElement("option");
        opt.value = String(it.id);
        opt.textContent = `${it.name} (#${it.id})`;
        if (it.id === cost.item_id) opt.selected = true;
        select.appendChild(opt);
      }
      select.onchange = () => {
        cost.item_id = Number(select.value);
      };

      const qtyInput = document.createElement("input");
      qtyInput.type = "number";
      qtyInput.min = "1";
      qtyInput.max = "1000";
      qtyInput.value = String(cost.quantity);
      qtyInput.required = true;
      qtyInput.oninput = () => {
        cost.quantity = Math.max(1, Number(qtyInput.value));
      };

      const removeBtn = document.createElement("button");
      removeBtn.type = "button";
      removeBtn.className = "btn btn-sm btn-danger";
      removeBtn.textContent = "×";
      removeBtn.disabled = currentBuilding.construction_cost.length <= 1;
      removeBtn.onclick = () => {
        if (currentBuilding.construction_cost.length > 1) {
          currentBuilding.construction_cost.splice(index, 1);
          renderCostList();
        }
      };

      row.appendChild(select);
      row.appendChild(qtyInput);
      row.appendChild(removeBtn);
      listEl.appendChild(row);
    });
  }

  modal.innerHTML = `
    <div class="modal-header">
      <h2>${isNew ? "Create New Building" : `Edit Building #${building.id}`}</h2>
      <button type="button" class="modal-close-btn">&times;</button>
    </div>
    <div class="modal-body">
      <form class="entity-form" id="building-form">
        <div class="form-row form-row-3">
          <label>
            <span>Building ID *</span>
            <input type="number" name="id" value="${currentBuilding.id}" min="1" required ${isNew ? "" : "readonly"} />
          </label>
          <label>
            <span>Key Identifier *</span>
            <input type="text" name="key" value="${currentBuilding.key}" required pattern="[a-z0-9-_]+" />
          </label>
          <label>
            <span>Icon Tag *</span>
            <input type="text" name="icon" value="${currentBuilding.icon}" maxlength="4" required />
            <small class="field-hint">e.g. EXT, BLT, CMP</small>
          </label>
        </div>

        <div class="form-row form-row-2">
          <label>
            <span>Building Name *</span>
            <input type="text" name="name" value="${currentBuilding.name}" required />
          </label>
          <label>
            <span>Building Kind *</span>
            <select name="kind" id="building-kind-select" required>
              ${BUILDING_KINDS.map(
                (k) =>
                  `<option value="${k}" ${k === currentBuilding.kind ? "selected" : ""}>${k.charAt(0).toUpperCase() + k.slice(1)}</option>`,
              ).join("")}
            </select>
          </label>
        </div>

        <div class="form-row">
          <label>
            <span>Description *</span>
            <textarea name="description" rows="2" required>${currentBuilding.description}</textarea>
          </label>
        </div>

        <!-- Footprint Section -->
        <div class="form-section">
          <span class="section-title">Axial Hex Footprint Designer *</span>
          <div id="modal-footprint-editor"></div>
        </div>

        <!-- Dynamic Kind Fields -->
        <div class="form-section kind-specific-section" id="kind-fields-section"></div>

        <!-- Power & General Mechanics -->
        <div class="form-section">
          <span class="section-title">General Mechanics & Power</span>
          <div class="form-row form-row-3">
            <label>
              <span>Placement Rule *</span>
              <select name="placement_rule" required>
                ${PLACEMENT_RULES.map(
                  (p) =>
                    `<option value="${p}" ${p === currentBuilding.placement_rule ? "selected" : ""}>${p.charAt(0).toUpperCase() + p.slice(1)}</option>`,
                ).join("")}
              </select>
            </label>
            <label>
              <span>Power Draw (Watts/tick)</span>
              <input type="number" name="power_draw" value="${currentBuilding.power_draw ?? ""}" min="0" placeholder="e.g. 4" />
            </label>
            <label>
              <span>Capacity</span>
              <input type="number" name="capacity" value="${currentBuilding.capacity ?? ""}" min="0" placeholder="e.g. 12" />
            </label>
          </div>

          <div class="form-row form-row-2">
            <label class="checkbox-label">
              <input type="checkbox" name="buildable" ${currentBuilding.buildable ? "checked" : ""} />
              <span>Buildable by player</span>
            </label>
            <label class="checkbox-label">
              <input type="checkbox" name="blocks_movement" ${currentBuilding.blocks_movement ? "checked" : ""} />
              <span>Blocks player movement</span>
            </label>
          </div>
        </div>

        <!-- Upgrade Ladder & Tech Unlocks -->
        <div class="form-section">
          <span class="section-title">Progression & Upgrades</span>
          <div class="form-row form-row-3">
            <label>
              <span>Tier</span>
              <input type="number" name="tier" value="${currentBuilding.tier ?? ""}" min="0" placeholder="0 (Base)" />
            </label>
            <label>
              <span>Upgrades To</span>
              <select name="upgrades_to">
                <option value="">-- None (Max Tier) --</option>
                ${store.definitions.buildings
                  .filter((b) => b.id !== currentBuilding.id)
                  .map(
                    (b) =>
                      `<option value="${b.id}" ${b.id === currentBuilding.upgrades_to ? "selected" : ""}>${b.name} (#${b.id}) [Tier ${b.tier ?? 0}]</option>`,
                  )
                  .join("")}
              </select>
            </label>
            <label>
              <span>Unlock Technology</span>
              <select name="unlock_technology_id">
                <option value="">-- Available at start --</option>
                ${store.technologies.technologies
                  .map(
                    (t) =>
                      `<option value="${t.id}" ${t.id === currentBuilding.unlock_technology_id ? "selected" : ""}>${t.name} (#${t.id})</option>`,
                  )
                  .join("")}
              </select>
            </label>
          </div>
        </div>

        <!-- Construction Costs -->
        <div class="form-section">
          <div class="section-title-row">
            <span class="section-title">Construction Cost *</span>
            <button type="button" class="btn btn-sm btn-subtle" id="add-cost-btn">+ Add Item</button>
          </div>
          <div id="building-cost-list" class="ingredients-list"></div>
        </div>

        <div class="modal-actions">
          <button type="button" class="btn btn-subtle modal-cancel-btn">Cancel</button>
          <button type="submit" class="btn btn-primary">Save Building</button>
        </div>
      </form>
    </div>
  `;

  // Mount Footprint Editor
  const fpContainer = modal.querySelector<HTMLElement>(
    "#modal-footprint-editor",
  )!;
  const footprintEditorInstance = new FootprintEditor({
    container: fpContainer,
    initialFootprint: currentBuilding.footprint,
    gridRadius: 2,
    readOnly: false,
    onChange: (fp) => {
      currentBuilding.footprint = fp;
    },
  });

  function updateKindSpecificFields(): void {
    const kindFields = modal.querySelector<HTMLElement>(
      "#kind-fields-section",
    )!;
    const kind = currentBuilding.kind;

    let html = `<span class="section-title">${kind.toUpperCase()} Specific Settings</span>`;

    if (kind === "composer") {
      const cats = Array.from(
        new Set(store.definitions.recipes.map((r) => r.category)),
      );
      html += `
        <div class="form-row form-row-2">
          <label>
            <span>Recipe Category *</span>
            <select name="recipe_category" required>
              ${cats.map((c) => `<option value="${c}" ${c === currentBuilding.recipe_category ? "selected" : ""}>${c}</option>`).join("")}
            </select>
            <small class="field-hint">Defines what recipes this machine can execute</small>
          </label>
          <label>
            <span>Cadence</span>
            <input type="number" name="cadence" value="${currentBuilding.cadence ?? ""}" min="1" placeholder="e.g. 5" />
          </label>
        </div>
      `;
    } else if (kind === "generator") {
      html += `
        <div class="form-row form-row-2">
          <label>
            <span>Power Source *</span>
            <select name="power_source" required>
              ${POWER_SOURCES.map((s) => `<option value="${s}" ${s === currentBuilding.power_source ? "selected" : ""}>${s}</option>`).join("")}
            </select>
          </label>
          <label>
            <span>Power Output (Watts) *</span>
            <input type="number" name="power_output" value="${currentBuilding.power_output ?? 10}" min="1" required />
          </label>
        </div>
      `;
    } else if (kind === "pump") {
      html += `
        <div class="form-row form-row-2">
          <label>
            <span>Pump Output Item *</span>
            <select name="output_item_id" required>
              ${store.definitions.items.map((it) => `<option value="${it.id}" ${it.id === currentBuilding.output_item_id ? "selected" : ""}>${it.name} (#${it.id})</option>`).join("")}
            </select>
          </label>
          <label>
            <span>Extract Reach Radius (1-4)</span>
            <input type="number" name="extract_radius" value="${currentBuilding.extract_radius ?? 1}" min="1" max="4" required />
          </label>
        </div>
      `;
    } else if (kind === "extractor") {
      html += `
        <div class="form-row form-row-2">
          <label>
            <span>Extract Radius (1-4) *</span>
            <input type="number" name="extract_radius" value="${currentBuilding.extract_radius ?? 1}" min="1" max="4" required />
          </label>
          <label>
            <span>Extract Speed (%)</span>
            <input type="number" name="extract_speed" value="${currentBuilding.extract_speed ?? 100}" min="10" max="1000" placeholder="100 = 2x hand, 200 = hand" />
          </label>
        </div>
      `;
    } else if (kind === "pole") {
      html += `
        <div class="form-row form-row-2">
          <label>
            <span>Pole Supply Radius (Hexes) *</span>
            <input type="number" name="supply_radius" value="${currentBuilding.supply_radius ?? 3}" min="1" max="20" required />
          </label>
          <label>
            <span>Pole Link Reach (Hexes) *</span>
            <input type="number" name="pole_reach" value="${currentBuilding.pole_reach ?? 6}" min="1" max="30" required />
          </label>
        </div>
      `;
    } else if (kind === "belt") {
      html += `
        <div class="form-row form-row-3">
          <label>
            <span>Orientation Axis</span>
            <select name="orientation_axis">
              <option value="edge" ${currentBuilding.orientation_axis === "edge" || !currentBuilding.orientation_axis ? "selected" : ""}>Edge (6 directions)</option>
              <option value="corner" ${currentBuilding.orientation_axis === "corner" ? "selected" : ""}>Corner (6 vertex)</option>
              <option value="any" ${currentBuilding.orientation_axis === "any" ? "selected" : ""}>Any (12 headings)</option>
            </select>
          </label>
          <label>
            <span>Underpass Span (Hexes)</span>
            <input type="number" name="underpass_span" value="${currentBuilding.underpass_span ?? ""}" min="1" max="4" placeholder="e.g. 4 (max 4)" />
          </label>
          <label>
            <span>Corner Tech Unlock</span>
            <select name="corner_technology_id">
              <option value="">-- None --</option>
              ${store.technologies.technologies.map((t) => `<option value="${t.id}" ${t.id === currentBuilding.corner_technology_id ? "selected" : ""}>${t.name}</option>`).join("")}
            </select>
          </label>
        </div>
        <div class="form-row form-row-2">
          <label class="checkbox-label">
            <input type="checkbox" name="splits" ${currentBuilding.splits ? "checked" : ""} />
            <span>Splits cargo into multiple headings</span>
          </label>
          <label class="checkbox-label">
            <input type="checkbox" name="merges" ${currentBuilding.merges ? "checked" : ""} />
            <span>Merges inputs in rotation</span>
          </label>
        </div>
      `;
    } else {
      html += `<p class="field-hint">Standard ${kind} settings applied.</p>`;
    }

    kindFields.innerHTML = html;
  }

  const kindSelect = modal.querySelector<HTMLSelectElement>(
    "#building-kind-select",
  )!;
  kindSelect.addEventListener("change", () => {
    currentBuilding.kind = kindSelect.value as BuildingKind;
    updateKindSpecificFields();
  });

  modal.querySelector("#add-cost-btn")?.addEventListener("click", () => {
    const firstItemId = store.definitions.items[0]?.id ?? 1;
    currentBuilding.construction_cost.push({
      item_id: firstItemId,
      quantity: 2,
    });
    renderCostList();
  });

  const form = modal.querySelector<HTMLFormElement>("#building-form")!;
  form.onsubmit = (e) => {
    e.preventDefault();
    const formData = new FormData(form);

    const id = Number(formData.get("id"));
    const key = String(formData.get("key")).trim();
    const name = String(formData.get("name")).trim();
    const kind = String(formData.get("kind")) as BuildingKind;
    const description = String(formData.get("description")).trim();
    const icon = String(formData.get("icon")).trim();
    const placement_rule = String(
      formData.get("placement_rule"),
    ) as PlacementRule;
    const buildable = formData.get("buildable") !== null;
    const blocks_movement = formData.get("blocks_movement") !== null;

    const footprint: AxialCoordinate[] = footprintEditorInstance.getFootprint();
    const construction_cost: Ingredient[] =
      currentBuilding.construction_cost.map((c) => ({
        item_id: c.item_id,
        quantity: c.quantity,
      }));

    const updated: BuildingDefinition = {
      id,
      key,
      name,
      kind,
      description,
      icon,
      placement_rule,
      buildable,
      blocks_movement,
      footprint,
      construction_cost,
    };

    // Kind specific parsing
    if (kind === "composer") {
      updated.recipe_category = String(formData.get("recipe_category"));
      const cad = formData.get("cadence");
      if (cad && Number(cad) > 0) updated.cadence = Number(cad);
    } else if (kind === "generator") {
      updated.power_source = String(
        formData.get("power_source"),
      ) as PowerSource;
      updated.power_output = Number(formData.get("power_output"));
    } else if (kind === "pump") {
      updated.output_item_id = Number(formData.get("output_item_id"));
      const r = formData.get("extract_radius");
      if (r && Number(r) > 0) updated.extract_radius = Number(r);
    } else if (kind === "extractor") {
      const r = formData.get("extract_radius");
      if (r && Number(r) > 0) updated.extract_radius = Number(r);
      const sp = formData.get("extract_speed");
      if (sp && Number(sp) > 0) updated.extract_speed = Number(sp);
    } else if (kind === "pole") {
      updated.supply_radius = Number(formData.get("supply_radius"));
      updated.pole_reach = Number(formData.get("pole_reach"));
    } else if (kind === "belt") {
      const axis = formData.get("orientation_axis") as OrientationAxis;
      if (axis) updated.orientation_axis = axis;
      const span = formData.get("underpass_span");
      if (span && Number(span) > 0) updated.underpass_span = Number(span);
      const cornerTech = formData.get("corner_technology_id");
      if (cornerTech && Number(cornerTech) > 0)
        updated.corner_technology_id = Number(cornerTech);
      if (formData.get("splits") !== null) updated.splits = true;
      if (formData.get("merges") !== null) updated.merges = true;
    }

    const powerDraw = formData.get("power_draw");
    if (powerDraw && Number(powerDraw) > 0)
      updated.power_draw = Number(powerDraw);

    const cap = formData.get("capacity");
    if (cap && Number(cap) > 0) updated.capacity = Number(cap);

    const tier = formData.get("tier");
    if (tier !== null && tier !== "" && Number(tier) >= 0)
      updated.tier = Number(tier);

    const upgradesTo = formData.get("upgrades_to");
    if (upgradesTo && Number(upgradesTo) > 0)
      updated.upgrades_to = Number(upgradesTo);

    const unlockTech = formData.get("unlock_technology_id");
    if (unlockTech && Number(unlockTech) > 0)
      updated.unlock_technology_id = Number(unlockTech);

    store.saveBuilding(updated);
    showToast(`Saved building "${updated.name}"`, "success");
    modalOverlay.remove();
  };

  const closeModal = (): void => {
    store.setEditingTarget(null);
    modalOverlay.remove();
  };

  modal
    .querySelector(".modal-close-btn")
    ?.addEventListener("click", closeModal);
  modal
    .querySelector(".modal-cancel-btn")
    ?.addEventListener("click", closeModal);
  modalOverlay.addEventListener("click", (e) => {
    if (e.target === modalOverlay) closeModal();
  });

  modalOverlay.appendChild(modal);
  document.body.appendChild(modalOverlay);

  updateKindSpecificFields();
  renderCostList();
}
