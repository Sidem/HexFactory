import type { Ingredient, RecipeDefinition } from "../../core/types";
import { itemIconSvg } from "../../rendering/icons";
import type { AdminStore } from "../state";
import { showToast } from "../toast";

export function renderRecipesView(
  container: HTMLElement,
  store: AdminStore,
): void {
  container.innerHTML = "";

  const view = document.createElement("div");
  view.className = "admin-view recipes-view";

  // Index items by ID for fast lookup
  const itemMap = new Map(store.definitions.items.map((i) => [i.id, i]));

  // Find all distinct categories
  const categories = Array.from(
    new Set(store.definitions.recipes.map((r) => r.category)),
  ).sort();

  // Toolbar
  const toolbar = document.createElement("div");
  toolbar.className = "view-toolbar";

  // Search input
  const searchInput = document.createElement("input");
  searchInput.type = "search";
  searchInput.placeholder =
    "Search recipes by name, key, category, inputs, output...";
  searchInput.className = "search-input";
  searchInput.value = store.searchQuery;
  searchInput.oninput = () => store.setSearchQuery(searchInput.value.trim());
  toolbar.appendChild(searchInput);

  // Category filter chips
  const filterChips = document.createElement("div");
  filterChips.className = "filter-chips";

  const allChip = document.createElement("button");
  allChip.type = "button";
  allChip.className = `chip-btn ${store.selectedFilter === "all" ? "active" : ""}`;
  allChip.textContent = "All Categories";
  allChip.onclick = () => store.setSelectedFilter("all");
  filterChips.appendChild(allChip);

  for (const cat of categories) {
    const chip = document.createElement("button");
    chip.type = "button";
    chip.className = `chip-btn ${store.selectedFilter === cat ? "active" : ""}`;
    chip.textContent = cat.charAt(0).toUpperCase() + cat.slice(1);
    chip.onclick = () => store.setSelectedFilter(cat);
    filterChips.appendChild(chip);
  }
  toolbar.appendChild(filterChips);

  // Add Recipe button
  const addBtn = document.createElement("button");
  addBtn.type = "button";
  addBtn.className = "btn btn-primary";
  addBtn.innerHTML = "<span>+</span> Add Recipe";
  addBtn.onclick = () => {
    const nextId = store.getNextRecipeId();
    const firstItemId = store.definitions.items[0]?.id ?? 1;
    const outputItemId = store.definitions.items[1]?.id ?? firstItemId;
    const newRecipe: RecipeDefinition = {
      id: nextId,
      key: `recipe-${nextId}`,
      name: `New Recipe ${nextId}`,
      description: "Description of what this recipe crafts.",
      category: categories[0] ?? "assembly",
      inputs: [{ item_id: firstItemId, quantity: 1 }],
      output: { item_id: outputItemId, quantity: 1 },
      duration: 60,
    };
    store.setEditingTarget({ type: "recipe", data: newRecipe, isNew: true });
  };
  toolbar.appendChild(addBtn);

  view.appendChild(toolbar);

  // Filter recipes
  const query = store.searchQuery.toLowerCase();
  const filterCat = store.selectedFilter;

  const filteredRecipes = store.definitions.recipes.filter((r) => {
    if (filterCat !== "all" && r.category !== filterCat) return false;
    if (query) {
      const outputItem = itemMap.get(r.output.item_id);
      const inputItems = r.inputs
        .map((i) => itemMap.get(i.item_id)?.name ?? "")
        .join(" ");
      const matchText =
        `${r.id} ${r.key} ${r.name} ${r.category} ${r.description} ${outputItem?.name ?? ""} ${inputItems}`.toLowerCase();
      if (!matchText.includes(query)) return false;
    }
    return true;
  });

  // Count bar
  const countBar = document.createElement("div");
  countBar.className = "view-count-bar";
  countBar.innerHTML = `Showing <strong>${filteredRecipes.length}</strong> of ${store.definitions.recipes.length} recipes`;
  view.appendChild(countBar);

  // Recipes Grid
  const grid = document.createElement("div");
  grid.className = "recipes-grid";

  for (const recipe of filteredRecipes) {
    const card = document.createElement("div");
    card.className = "recipe-card";

    const outputItem = itemMap.get(recipe.output.item_id);
    const machinesRunning = store.definitions.buildings.filter(
      (b) => b.recipe_category === recipe.category,
    );

    // Calculate production rate (per minute at 60 ticks/s)
    const craftsPerMinute = (60 * 60) / Math.max(1, recipe.duration);
    const itemsPerMinute = (craftsPerMinute * recipe.output.quantity).toFixed(
      1,
    );
    const durationSec = (recipe.duration / 60).toFixed(1);

    card.innerHTML = `
      <div class="card-header">
        <div class="recipe-title-group">
          <div class="card-title-row">
            <h3 class="card-title">${recipe.name}</h3>
            <span class="id-badge">#${recipe.id}</span>
          </div>
          <div class="recipe-meta-row">
            <code class="item-key">${recipe.key}</code>
            <span class="badge badge-category">${recipe.category}</span>
          </div>
        </div>
      </div>
      <p class="card-description">${recipe.description}</p>

      <!-- Visual Flowchart -->
      <div class="recipe-flow-diagram">
        <div class="flow-inputs">
          ${recipe.inputs
            .map((inp) => {
              const it = itemMap.get(inp.item_id);
              const color = it?.color ?? "#888";
              const svg = itemIconSvg(it?.icon ?? "ore", color);
              return `
                <div class="flow-item-pill" style="border-color: ${color}50; background: ${color}12;">
                  <span class="flow-icon">${svg}</span>
                  <span class="flow-qty" style="color: ${color}">${inp.quantity}×</span>
                  <span class="flow-name">${it?.name ?? `#${inp.item_id}`}</span>
                </div>
              `;
            })
            .join("")}
        </div>

        <div class="flow-arrow-col">
          <div class="flow-timing">⏱ ${durationSec}s <small>(${recipe.duration}t)</small></div>
          <div class="flow-arrow">➔</div>
          ${
            recipe.fuel
              ? `<div class="flow-fuel" title="Fuel required per craft">🔥 ${recipe.fuel} MJ</div>`
              : ""
          }
        </div>

        <div class="flow-output">
          ${(() => {
            const it = outputItem;
            const color = it?.color ?? "#888";
            const svg = itemIconSvg(it?.icon ?? "ore", color);
            return `
              <div class="flow-item-pill flow-output-pill" style="border-color: ${color}60; background: ${color}20;">
                <span class="flow-icon">${svg}</span>
                <span class="flow-qty" style="color: ${color}">${recipe.output.quantity}×</span>
                <span class="flow-name"><strong>${it?.name ?? `#${recipe.output.item_id}`}</strong></span>
              </div>
            `;
          })()}
        </div>
      </div>

      <div class="recipe-footer-info">
        <span class="rate-metric">Throughput: <strong>${itemsPerMinute}</strong>/min</span>
        <span class="machines-metric" title="Compatible Machines">
          ${
            machinesRunning.length > 0
              ? `⚙ Run by: ${machinesRunning.map((m) => m.name).join(", ")}`
              : `<span class="warn-text">⚠ No machine has category "${recipe.category}"</span>`
          }
        </span>
      </div>

      <div class="card-actions">
        <button type="button" class="btn btn-sm btn-edit">Edit</button>
        <button type="button" class="btn btn-sm btn-subtle btn-dup">Duplicate</button>
        <button type="button" class="btn btn-sm btn-danger btn-del">Delete</button>
      </div>
    `;

    card.querySelector(".btn-edit")?.addEventListener("click", () => {
      store.setEditingTarget({
        type: "recipe",
        data: structuredClone(recipe),
        isNew: false,
      });
    });

    card.querySelector(".btn-dup")?.addEventListener("click", () => {
      const cloned = store.duplicateRecipe(recipe.id);
      if (cloned)
        showToast(`Duplicated ${recipe.name} as #${cloned.id}`, "success");
    });

    card.querySelector(".btn-del")?.addEventListener("click", () => {
      if (confirm(`Delete recipe "${recipe.name}"?`)) {
        store.deleteRecipe(recipe.id);
        showToast(`Deleted ${recipe.name}`, "info");
      }
    });

    grid.appendChild(card);
  }

  view.appendChild(grid);
  container.appendChild(view);

  // If currently editing a recipe, render modal
  if (store.editingTarget?.type === "recipe") {
    renderRecipeModal(
      store.editingTarget.data,
      store.editingTarget.isNew,
      store,
    );
  }
}

function renderRecipeModal(
  recipe: RecipeDefinition,
  isNew: boolean,
  store: AdminStore,
): void {
  const modalOverlay = document.createElement("div");
  modalOverlay.className = "modal-overlay";

  const modal = document.createElement("div");
  modal.className = "modal recipe-modal";

  const currentRecipe: RecipeDefinition = structuredClone(recipe);
  const itemMap = new Map(store.definitions.items.map((i) => [i.id, i]));
  const categories = Array.from(
    new Set(store.definitions.recipes.map((r) => r.category)),
  );

  function renderInputsList(): void {
    const listEl = modal.querySelector<HTMLElement>("#recipe-inputs-list");
    if (!listEl) return;
    listEl.innerHTML = "";

    currentRecipe.inputs.forEach((inp, index) => {
      const row = document.createElement("div");
      row.className = "ingredient-builder-row";

      // Select item
      const select = document.createElement("select");
      select.required = true;
      for (const it of store.definitions.items) {
        const opt = document.createElement("option");
        opt.value = String(it.id);
        opt.textContent = `${it.name} (#${it.id})`;
        if (it.id === inp.item_id) opt.selected = true;
        select.appendChild(opt);
      }
      select.onchange = () => {
        inp.item_id = Number(select.value);
        updateFlowPreview();
      };

      // Quantity
      const qtyInput = document.createElement("input");
      qtyInput.type = "number";
      qtyInput.min = "1";
      qtyInput.max = "100";
      qtyInput.value = String(inp.quantity);
      qtyInput.required = true;
      qtyInput.oninput = () => {
        inp.quantity = Math.max(1, Number(qtyInput.value));
        updateFlowPreview();
      };

      // Remove button
      const removeBtn = document.createElement("button");
      removeBtn.type = "button";
      removeBtn.className = "btn btn-sm btn-danger";
      removeBtn.textContent = "×";
      removeBtn.disabled = currentRecipe.inputs.length <= 1;
      removeBtn.onclick = () => {
        if (currentRecipe.inputs.length > 1) {
          currentRecipe.inputs.splice(index, 1);
          renderInputsList();
          updateFlowPreview();
        }
      };

      row.appendChild(select);
      row.appendChild(qtyInput);
      row.appendChild(removeBtn);
      listEl.appendChild(row);
    });
  }

  function updateFlowPreview(): void {
    const previewEl = modal.querySelector(".modal-recipe-preview");
    if (!previewEl) return;

    const outItem = itemMap.get(currentRecipe.output.item_id);
    const outColor = outItem?.color ?? "#6fddd0";
    const outSvg = itemIconSvg(outItem?.icon ?? "ore", outColor);
    const sec = (currentRecipe.duration / 60).toFixed(1);

    previewEl.innerHTML = `
      <div class="flow-preview-wrap">
        <div class="flow-inputs-preview">
          ${currentRecipe.inputs
            .map((inp) => {
              const it = itemMap.get(inp.item_id);
              const color = it?.color ?? "#888";
              const svg = itemIconSvg(it?.icon ?? "ore", color);
              return `<span class="preview-chip" style="border-color: ${color}50; background: ${color}15;">${svg} ${inp.quantity}× ${it?.name ?? `#${inp.item_id}`}</span>`;
            })
            .join("")}
        </div>
        <div class="flow-mid-preview">
          <span class="preview-cat-badge">${currentRecipe.category}</span>
          <span class="preview-arrow">➔ ${sec}s ${currentRecipe.fuel ? `(🔥 ${currentRecipe.fuel}MJ)` : ""}</span>
        </div>
        <div class="flow-output-preview">
          <span class="preview-chip preview-out" style="border-color: ${outColor}70; background: ${outColor}25;">${outSvg} <strong>${currentRecipe.output.quantity}× ${outItem?.name ?? `#${currentRecipe.output.item_id}`}</strong></span>
        </div>
      </div>
    `;
  }

  modal.innerHTML = `
    <div class="modal-header">
      <h2>${isNew ? "Create New Recipe" : `Edit Recipe #${recipe.id}`}</h2>
      <button type="button" class="modal-close-btn">&times;</button>
    </div>
    <div class="modal-body">
      <div class="modal-recipe-preview"></div>
      <form class="entity-form" id="recipe-form">
        <div class="form-row form-row-2">
          <label>
            <span>Recipe ID *</span>
            <input type="number" name="id" value="${currentRecipe.id}" min="1" required ${isNew ? "" : "readonly"} />
          </label>
          <label>
            <span>Key Identifier *</span>
            <input type="text" name="key" value="${currentRecipe.key}" required pattern="[a-z0-9-_]+" />
          </label>
        </div>

        <div class="form-row form-row-2">
          <label>
            <span>Recipe Name *</span>
            <input type="text" name="name" value="${currentRecipe.name}" required />
          </label>
          <label>
            <span>Recipe Category *</span>
            <select name="category" required>
              ${categories
                .map(
                  (c) =>
                    `<option value="${c}" ${c === currentRecipe.category ? "selected" : ""}>${c.charAt(0).toUpperCase() + c.slice(1)}</option>`,
                )
                .join("")}
            </select>
          </label>
        </div>

        <div class="form-row">
          <label>
            <span>Description *</span>
            <textarea name="description" rows="2" required>${currentRecipe.description}</textarea>
          </label>
        </div>

        <div class="form-section">
          <div class="section-title-row">
            <span class="section-title">Inputs (Ingredients) *</span>
            <button type="button" class="btn btn-sm btn-subtle" id="add-input-btn">+ Add Input</button>
          </div>
          <div id="recipe-inputs-list" class="ingredients-list"></div>
        </div>

        <div class="form-section">
          <span class="section-title">Output Product *</span>
          <div class="ingredient-builder-row">
            <select name="output_item_id" id="output-item-select" required>
              ${store.definitions.items
                .map(
                  (it) =>
                    `<option value="${it.id}" ${it.id === currentRecipe.output.item_id ? "selected" : ""}>${it.name} (#${it.id})</option>`,
                )
                .join("")}
            </select>
            <input type="number" name="output_quantity" id="output-qty-input" value="${currentRecipe.output.quantity}" min="1" max="100" required />
          </div>
        </div>

        <div class="form-row form-row-2">
          <label>
            <span>Craft Duration (Ticks) *</span>
            <input type="number" name="duration" id="duration-input" value="${currentRecipe.duration}" min="1" required />
            <small class="field-hint" id="duration-hint">${(currentRecipe.duration / 60).toFixed(2)} seconds (at 60 TPS)</small>
          </label>
          <label>
            <span>Fuel Required (MJ)</span>
            <input type="number" name="fuel" id="fuel-input" value="${currentRecipe.fuel ?? ""}" min="0" placeholder="e.g. 10 (0 / empty for free)" />
            <small class="field-hint">Burned per craft in smelters/kilns</small>
          </label>
        </div>

        <div class="modal-actions">
          <button type="button" class="btn btn-subtle modal-cancel-btn">Cancel</button>
          <button type="submit" class="btn btn-primary">Save Recipe</button>
        </div>
      </form>
    </div>
  `;

  const form = modal.querySelector<HTMLFormElement>("#recipe-form")!;
  const nameInput =
    modal.querySelector<HTMLInputElement>("input[name='name']")!;
  const keyInput = modal.querySelector<HTMLInputElement>("input[name='key']")!;
  const catSelect = modal.querySelector<HTMLSelectElement>(
    "select[name='category']",
  )!;
  const outSelect = modal.querySelector<HTMLSelectElement>(
    "#output-item-select",
  )!;
  const outQty = modal.querySelector<HTMLInputElement>("#output-qty-input")!;
  const durInput = modal.querySelector<HTMLInputElement>("#duration-input")!;
  const durHint = modal.querySelector<HTMLElement>("#duration-hint")!;
  const fuelInput = modal.querySelector<HTMLInputElement>("#fuel-input")!;

  nameInput.addEventListener("input", () => {
    currentRecipe.name = nameInput.value;
    if (isNew && keyInput) {
      keyInput.value = nameInput.value
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-|-$/g, "");
      currentRecipe.key = keyInput.value;
    }
  });

  catSelect.addEventListener("change", () => {
    currentRecipe.category = catSelect.value;
    updateFlowPreview();
  });

  outSelect.addEventListener("change", () => {
    currentRecipe.output.item_id = Number(outSelect.value);
    updateFlowPreview();
  });

  outQty.addEventListener("input", () => {
    currentRecipe.output.quantity = Math.max(1, Number(outQty.value));
    updateFlowPreview();
  });

  durInput.addEventListener("input", () => {
    const val = Math.max(1, Number(durInput.value));
    currentRecipe.duration = val;
    durHint.textContent = `${(val / 60).toFixed(2)} seconds (at 60 TPS)`;
    updateFlowPreview();
  });

  fuelInput.addEventListener("input", () => {
    const val = Number(fuelInput.value);
    currentRecipe.fuel = val > 0 ? val : undefined;
    updateFlowPreview();
  });

  modal.querySelector("#add-input-btn")?.addEventListener("click", () => {
    const firstItemId = store.definitions.items[0]?.id ?? 1;
    currentRecipe.inputs.push({ item_id: firstItemId, quantity: 1 });
    renderInputsList();
    updateFlowPreview();
  });

  form.onsubmit = (e) => {
    e.preventDefault();
    const formData = new FormData(form);
    const id = Number(formData.get("id"));
    const key = String(formData.get("key")).trim();
    const name = String(formData.get("name")).trim();
    const description = String(formData.get("description")).trim();
    const category = String(formData.get("category")).trim();
    const duration = Number(formData.get("duration"));
    const fuelRaw = formData.get("fuel");

    const inputs: Ingredient[] = currentRecipe.inputs.map((inp) => ({
      item_id: inp.item_id,
      quantity: inp.quantity,
    }));

    const output: Ingredient = {
      item_id: Number(formData.get("output_item_id")),
      quantity: Number(formData.get("output_quantity")),
    };

    const updatedRecipe: RecipeDefinition = {
      id,
      key,
      name,
      description,
      category,
      inputs,
      output,
      duration,
    };

    if (fuelRaw && Number(fuelRaw) > 0) {
      updatedRecipe.fuel = Number(fuelRaw);
    }

    store.saveRecipe(updatedRecipe);
    showToast(`Saved recipe "${updatedRecipe.name}"`, "success");
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

  renderInputsList();
  updateFlowPreview();
}
