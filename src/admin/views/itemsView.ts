import type { ItemDefinition } from "../../core/types";
import {
  ITEM_ICON_KEYS,
  type ItemIconKey,
  itemIconSvg,
} from "../../rendering/icons";
import type { AdminStore } from "../state";
import { showToast } from "../toast";

const PRESET_COLORS = [
  "#e2a85f", // Iron ore
  "#c9743f", // Copper ore
  "#000000", // Coal
  "#8b9098", // Stone
  "#e6d197", // Sand
  "#b0714c", // Clay
  "#7c5a34", // Wood
  "#4aa8d8", // Water
  "#b78cff", // Signal crystal
  "#c3ced6", // Iron plate
  "#e08e58", // Copper plate
  "#a8dbe6", // Glass
  "#b5563f", // Brick
  "#6fddd0", // Component
  "#72e2b4", // Mint
  "#f6c85f", // Gold
];

export function renderItemsView(
  container: HTMLElement,
  store: AdminStore,
): void {
  container.innerHTML = "";

  const view = document.createElement("div");
  view.className = "admin-view items-view";

  // Controls bar: Search, Filters, Add button
  const toolbar = document.createElement("div");
  toolbar.className = "view-toolbar";

  // Search box
  const searchInput = document.createElement("input");
  searchInput.type = "search";
  searchInput.placeholder = "Search items by name, key, id, or description...";
  searchInput.className = "search-input";
  searchInput.value = store.searchQuery;
  searchInput.oninput = () => store.setSearchQuery(searchInput.value.trim());
  toolbar.appendChild(searchInput);

  // Filter chips
  const filterChips = document.createElement("div");
  filterChips.className = "filter-chips";

  const filters = [
    { id: "all", label: "All Items" },
    { id: "raw", label: "Raw / Gatherable" },
    { id: "fuel", label: "Fuel" },
    { id: "crafted", label: "Crafted / Smelted" },
  ];

  for (const f of filters) {
    const chip = document.createElement("button");
    chip.type = "button";
    chip.className = `chip-btn ${store.selectedFilter === f.id ? "active" : ""}`;
    chip.textContent = f.label;
    chip.onclick = () => store.setSelectedFilter(f.id);
    filterChips.appendChild(chip);
  }
  toolbar.appendChild(filterChips);

  // Add Item button
  const addBtn = document.createElement("button");
  addBtn.type = "button";
  addBtn.className = "btn btn-primary";
  addBtn.innerHTML = "<span>+</span> Add Item";
  addBtn.onclick = () => {
    const nextId = store.getNextItemId();
    const newItem: ItemDefinition = {
      id: nextId,
      key: `item-${nextId}`,
      name: `New Item ${nextId}`,
      color: "#6fddd0",
      icon: "ore",
      description: "Description of the new material.",
      stack_size: 20,
    };
    store.setEditingTarget({ type: "item", data: newItem, isNew: true });
  };
  toolbar.appendChild(addBtn);

  view.appendChild(toolbar);

  // Filter items
  const query = store.searchQuery.toLowerCase();
  const filter = store.selectedFilter;

  const filteredItems = store.definitions.items.filter((item) => {
    if (query) {
      const matchText =
        `${item.id} ${item.key} ${item.name} ${item.description}`.toLowerCase();
      if (!matchText.includes(query)) return false;
    }

    if (filter === "raw") {
      return (
        item.hand_gather_steps !== undefined ||
        item.extract_steps !== undefined ||
        item.key === "water"
      );
    }
    if (filter === "fuel") {
      return (item.fuel_value ?? 0) > 0;
    }
    if (filter === "crafted") {
      return store.definitions.recipes.some(
        (r) => r.output.item_id === item.id,
      );
    }
    return true;
  });

  // Results count header
  const countBar = document.createElement("div");
  countBar.className = "view-count-bar";
  countBar.innerHTML = `Showing <strong>${filteredItems.length}</strong> of ${store.definitions.items.length} items`;
  view.appendChild(countBar);

  // Items Cards Grid
  const grid = document.createElement("div");
  grid.className = "items-grid";

  for (const item of filteredItems) {
    const card = document.createElement("div");
    card.className = "item-card";

    // Usage statistics
    const recipesUsingAsInput = store.definitions.recipes.filter((r) =>
      r.inputs.some((i) => i.item_id === item.id),
    );
    const recipesProducing = store.definitions.recipes.filter(
      (r) => r.output.item_id === item.id,
    );
    const buildingsCosting = store.definitions.buildings.filter((b) =>
      b.construction_cost.some((c) => c.item_id === item.id),
    );
    const requestsAsking = store.definitions.requests.filter(
      (r) => r.item_id === item.id,
    );

    const iconHtml = itemIconSvg(item.icon, item.color);

    card.innerHTML = `
      <div class="card-header">
        <div class="item-icon-swatch" style="border-color: ${item.color}40; background: ${item.color}15;">
          ${iconHtml}
        </div>
        <div class="card-title-group">
          <div class="card-title-row">
            <h3 class="card-title">${item.name}</h3>
            <span class="id-badge">#${item.id}</span>
          </div>
          <code class="item-key">${item.key}</code>
        </div>
      </div>
      <p class="card-description">${item.description}</p>
      <div class="item-badges-row">
        <span class="badge" title="Stack Size">📦 ${item.stack_size}/stack</span>
        ${
          item.fuel_value
            ? `<span class="badge badge-fuel" title="Fuel Value">🔥 ${item.fuel_value} MJ</span>`
            : ""
        }
        ${
          item.hand_gather_steps
            ? `<span class="badge" title="Hand gather steps">✋ ${item.hand_gather_steps} steps</span>`
            : ""
        }
        ${
          item.extract_steps
            ? `<span class="badge" title="Machine extraction steps">⛏ ${item.extract_steps} steps</span>`
            : ""
        }
        ${
          item.regrowth_ticks
            ? `<span class="badge badge-nature" title="Regrowth ticks">🌱 ${item.regrowth_ticks} ticks</span>`
            : ""
        }
      </div>
      <div class="item-usage-summary">
        <span title="Used as input in recipes">In: <strong>${recipesUsingAsInput.length}</strong></span> ·
        <span title="Produced by recipes">Out: <strong>${recipesProducing.length}</strong></span> ·
        <span title="Required for buildings">Bldgs: <strong>${buildingsCosting.length}</strong></span> ·
        <span title="Hub requests">Orders: <strong>${requestsAsking.length}</strong></span>
      </div>
      <div class="card-actions">
        <button type="button" class="btn btn-sm btn-edit">Edit</button>
        <button type="button" class="btn btn-sm btn-subtle btn-dup">Duplicate</button>
        <button type="button" class="btn btn-sm btn-danger btn-del">Delete</button>
      </div>
    `;

    card.querySelector(".btn-edit")?.addEventListener("click", () => {
      store.setEditingTarget({
        type: "item",
        data: structuredClone(item),
        isNew: false,
      });
    });

    card.querySelector(".btn-dup")?.addEventListener("click", () => {
      const cloned = store.duplicateItem(item.id);
      if (cloned)
        showToast(`Duplicated ${item.name} as #${cloned.id}`, "success");
    });

    card.querySelector(".btn-del")?.addEventListener("click", () => {
      const usages =
        recipesUsingAsInput.length +
        recipesProducing.length +
        buildingsCosting.length +
        requestsAsking.length;
      if (usages > 0) {
        if (
          !confirm(
            `Item "${item.name}" is referenced in ${usages} place(s) (recipes/buildings/requests). Deleting it will cause validation errors until those references are updated. Continue?`,
          )
        ) {
          return;
        }
      } else if (!confirm(`Delete item "${item.name}"?`)) {
        return;
      }
      store.deleteItem(item.id);
      showToast(`Deleted ${item.name}`, "info");
    });

    grid.appendChild(card);
  }

  view.appendChild(grid);
  container.appendChild(view);

  // If currently editing an item, render modal
  if (store.editingTarget?.type === "item") {
    renderItemModal(store.editingTarget.data, store.editingTarget.isNew, store);
  }
}

function renderItemModal(
  item: ItemDefinition,
  isNew: boolean,
  store: AdminStore,
): void {
  const modalOverlay = document.createElement("div");
  modalOverlay.className = "modal-overlay";

  const modal = document.createElement("div");
  modal.className = "modal item-modal";

  const currentItem: ItemDefinition = structuredClone(item);

  function updatePreview(): void {
    const previewEl = modal.querySelector(".modal-item-preview");
    if (!previewEl) return;
    const svg = itemIconSvg(currentItem.icon, currentItem.color);
    previewEl.innerHTML = `
      <div class="preview-swatch" style="border-color: ${currentItem.color}50; background: ${currentItem.color}18;">
        ${svg}
      </div>
      <div class="preview-info">
        <strong style="color: ${currentItem.color}">${currentItem.name || "Untitled"}</strong>
        <code>${currentItem.key || "key"}</code>
        <small>${currentItem.stack_size} per stack ${currentItem.fuel_value ? `· ${currentItem.fuel_value} MJ fuel` : ""}</small>
      </div>
    `;
  }

  modal.innerHTML = `
    <div class="modal-header">
      <h2>${isNew ? "Create New Item" : `Edit Item #${item.id}`}</h2>
      <button type="button" class="modal-close-btn">&times;</button>
    </div>
    <div class="modal-body">
      <div class="modal-item-preview"></div>
      <form class="entity-form" id="item-form">
        <div class="form-row form-row-2">
          <label>
            <span>Item ID *</span>
            <input type="number" name="id" value="${currentItem.id}" min="1" required ${isNew ? "" : "readonly"} />
            <small class="field-hint">Unique positive integer ID</small>
          </label>
          <label>
            <span>Key Identifier *</span>
            <input type="text" name="key" value="${currentItem.key}" required pattern="[a-z0-9-_]+" />
            <small class="field-hint">Unique slug (e.g. iron-plate)</small>
          </label>
        </div>

        <div class="form-row">
          <label>
            <span>Display Name *</span>
            <input type="text" name="name" value="${currentItem.name}" required />
          </label>
        </div>

        <div class="form-row">
          <label>
            <span>Description *</span>
            <textarea name="description" rows="2" required>${currentItem.description}</textarea>
          </label>
        </div>

        <div class="form-row form-row-2">
          <label>
            <span>Identity Color *</span>
            <div class="color-picker-group">
              <input type="color" name="color" value="${currentItem.color}" />
              <input type="text" name="color_hex" value="${currentItem.color}" class="color-hex-input" />
            </div>
            <div class="preset-swatches">
              ${PRESET_COLORS.map(
                (c) =>
                  `<button type="button" class="swatch-btn" style="background: ${c}" data-color="${c}" title="${c}"></button>`,
              ).join("")}
            </div>
          </label>

          <label>
            <span>Stack Size *</span>
            <input type="number" name="stack_size" value="${currentItem.stack_size}" min="1" max="500" required />
            <small class="field-hint">How many fill 1 carried slot (typically 10 or 20)</small>
          </label>
        </div>

        <div class="form-row">
          <label>
            <span>Icon Glyph *</span>
            <div class="icon-selector-grid">
              ${ITEM_ICON_KEYS.map((k) => {
                const svg = itemIconSvg(k, currentItem.color);
                const isSelected = currentItem.icon === k;
                return `
                  <button type="button" class="icon-choice-btn ${isSelected ? "selected" : ""}" data-icon="${k}">
                    ${svg}
                    <span>${k}</span>
                  </button>
                `;
              }).join("")}
            </div>
          </label>
        </div>

        <details class="advanced-section" open>
          <summary>Gathering, Extraction & Fuel Rates</summary>
          <div class="form-row form-row-2">
            <label>
              <span>Fuel Value (MJ)</span>
              <input type="number" name="fuel_value" value="${currentItem.fuel_value ?? ""}" min="0" placeholder="e.g. 160 (empty for non-fuel)" />
              <small class="field-hint">Burn energy for furnaces & boilers</small>
            </label>
            <label>
              <span>Regrowth Ticks</span>
              <input type="number" name="regrowth_ticks" value="${currentItem.regrowth_ticks ?? ""}" min="0" placeholder="e.g. 450 (flora only)" />
              <small class="field-hint">Ticks between unit regrowth</small>
            </label>
          </div>
          <div class="form-row form-row-2">
            <label>
              <span>Hand Gather Steps</span>
              <input type="number" name="hand_gather_steps" value="${currentItem.hand_gather_steps ?? ""}" min="1" placeholder="e.g. 45 (empty = no hand gather)" />
              <small class="field-hint">Player clicks/steps to harvest</small>
            </label>
            <label>
              <span>Extractor Steps</span>
              <input type="number" name="extract_steps" value="${currentItem.extract_steps ?? ""}" min="1" placeholder="e.g. 30 (empty = default)" />
              <small class="field-hint">Tier-1 extractor base ticks</small>
            </label>
          </div>
        </details>

        <div class="modal-actions">
          <button type="button" class="btn btn-subtle modal-cancel-btn">Cancel</button>
          <button type="submit" class="btn btn-primary">Save Item</button>
        </div>
      </form>
    </div>
  `;

  // Attach event handlers
  const form = modal.querySelector<HTMLFormElement>("#item-form")!;
  const colorInput = modal.querySelector<HTMLInputElement>(
    "input[name='color']",
  )!;
  const colorHexInput = modal.querySelector<HTMLInputElement>(
    "input[name='color_hex']",
  )!;
  const nameInput =
    modal.querySelector<HTMLInputElement>("input[name='name']")!;
  const keyInput = modal.querySelector<HTMLInputElement>("input[name='key']")!;

  nameInput.addEventListener("input", () => {
    currentItem.name = nameInput.value;
    if (isNew && keyInput) {
      keyInput.value = nameInput.value
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-|-$/g, "");
      currentItem.key = keyInput.value;
    }
    updatePreview();
  });

  keyInput.addEventListener("input", () => {
    currentItem.key = keyInput.value;
    updatePreview();
  });

  colorInput.addEventListener("input", () => {
    currentItem.color = colorInput.value;
    colorHexInput.value = colorInput.value;
    updatePreview();
    updateIconGridColors();
  });

  colorHexInput.addEventListener("input", () => {
    if (/^#[0-9a-fA-F]{6}$/.test(colorHexInput.value)) {
      colorInput.value = colorHexInput.value;
      currentItem.color = colorHexInput.value;
      updatePreview();
      updateIconGridColors();
    }
  });

  modal.querySelectorAll<HTMLButtonElement>(".swatch-btn").forEach((btn) => {
    btn.onclick = () => {
      const c = btn.dataset.color!;
      colorInput.value = c;
      colorHexInput.value = c;
      currentItem.color = c;
      updatePreview();
      updateIconGridColors();
    };
  });

  function updateIconGridColors(): void {
    modal
      .querySelectorAll<HTMLButtonElement>(".icon-choice-btn")
      .forEach((btn) => {
        const iconKey = btn.dataset.icon as ItemIconKey;
        const svg = itemIconSvg(iconKey, currentItem.color);
        const span = btn.querySelector("span")?.textContent ?? iconKey;
        btn.innerHTML = `${svg}<span>${span}</span>`;
      });
  }

  modal
    .querySelectorAll<HTMLButtonElement>(".icon-choice-btn")
    .forEach((btn) => {
      btn.onclick = () => {
        modal
          .querySelectorAll(".icon-choice-btn")
          .forEach((b) => b.classList.remove("selected"));
        btn.classList.add("selected");
        currentItem.icon = btn.dataset.icon as ItemIconKey;
        updatePreview();
      };
    });

  form.onsubmit = (e) => {
    e.preventDefault();
    const formData = new FormData(form);
    const id = Number(formData.get("id"));
    const key = String(formData.get("key")).trim();
    const name = String(formData.get("name")).trim();
    const description = String(formData.get("description")).trim();
    const color = String(formData.get("color")).trim();
    const stack_size = Number(formData.get("stack_size"));

    const fuel_value_raw = formData.get("fuel_value");
    const regrowth_raw = formData.get("regrowth_ticks");
    const hand_gather_raw = formData.get("hand_gather_steps");
    const extract_raw = formData.get("extract_steps");

    const updatedItem: ItemDefinition = {
      id,
      key,
      name,
      description,
      color,
      icon: currentItem.icon,
      stack_size,
    };

    if (fuel_value_raw && Number(fuel_value_raw) > 0) {
      updatedItem.fuel_value = Number(fuel_value_raw);
    }
    if (regrowth_raw && Number(regrowth_raw) > 0) {
      updatedItem.regrowth_ticks = Number(regrowth_raw);
    }
    if (hand_gather_raw && Number(hand_gather_raw) > 0) {
      updatedItem.hand_gather_steps = Number(hand_gather_raw);
    }
    if (extract_raw && Number(extract_raw) > 0) {
      updatedItem.extract_steps = Number(extract_raw);
    }

    store.saveItem(updatedItem);
    showToast(`Saved item "${updatedItem.name}"`, "success");
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
  updatePreview();
}
