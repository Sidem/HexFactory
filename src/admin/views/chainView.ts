import { supportsRecipe } from "../../core/definitions";
import { recipeOutputs, recipeYield } from "../../core/recipes";
import { itemIconSvg } from "../../rendering/icons";
import type { AdminStore } from "../state";

export function renderChainView(
  container: HTMLElement,
  store: AdminStore,
): void {
  container.innerHTML = "";

  const view = document.createElement("div");
  view.className = "admin-view chain-view";

  const itemMap = new Map(store.definitions.items.map((i) => [i.id, i]));
  const techMap = new Map(
    store.technologies.technologies.map((t) => [t.id, t]),
  );

  // Choose focused item (default to Component #2 or first item)
  let selectedItemId =
    store.definitions.items.find((i) => i.key === "component")?.id ??
    store.definitions.items[0]?.id ??
    1;

  // Header toolbar
  const toolbar = document.createElement("div");
  toolbar.className = "view-toolbar";

  const pickerLabel = document.createElement("label");
  pickerLabel.className = "chain-picker-label";
  pickerLabel.innerHTML = `<span>Focus Material:</span>`;

  const itemSelect = document.createElement("select");
  itemSelect.className = "chain-item-select";
  for (const it of store.definitions.items) {
    const opt = document.createElement("option");
    opt.value = String(it.id);
    opt.textContent = `${it.name} (#${it.id}) - ${it.key}`;
    if (it.id === selectedItemId) opt.selected = true;
    itemSelect.appendChild(opt);
  }
  itemSelect.onchange = () => {
    selectedItemId = Number(itemSelect.value);
    renderChainTree();
  };
  pickerLabel.appendChild(itemSelect);
  toolbar.appendChild(pickerLabel);

  view.appendChild(toolbar);

  // Container for tree
  const treeContainer = document.createElement("div");
  treeContainer.className = "chain-tree-container";
  view.appendChild(treeContainer);

  function renderChainTree(): void {
    treeContainer.innerHTML = "";

    const item = itemMap.get(selectedItemId);
    if (!item) {
      treeContainer.innerHTML = `<p class="dim-text">No item selected</p>`;
      return;
    }

    const itemColor = item.color;
    const itemSvg = itemIconSvg(item.icon, itemColor);

    // Recipes that produce this item
    const producerRecipes = store.definitions.recipes.filter(
      (r) => recipeYield(r, item.id) > 0,
    );

    // Recipes that consume this item
    const consumerRecipes = store.definitions.recipes.filter((r) =>
      r.inputs.some((i) => i.item_id === item.id),
    );

    // Buildings that require this item for construction
    const buildingsCosting = store.definitions.buildings.filter(
      (b) =>
        b.construction_cost.some((c) => c.item_id === item.id) ||
        b.corner_construction_cost?.some((c) => c.item_id === item.id),
    );

    // Hub requests
    const hubRequests = store.definitions.requests.filter(
      (r) => r.item_id === item.id,
    );

    // Is this a natural harvestable / pumpable resource?
    const isHarvestable =
      item.hand_gather_steps !== undefined || item.extract_steps !== undefined;
    const pumpSource = store.definitions.buildings.find(
      (b) => b.kind === "pump" && b.output_item_id === item.id,
    );

    const layout = document.createElement("div");
    layout.className = "chain-layout-grid";

    // 1. Upstream Producers Column
    const upstreamCol = document.createElement("div");
    upstreamCol.className = "chain-column upstream-column";
    upstreamCol.innerHTML = `<h3><span>⬆</span> Upstream Production</h3>`;

    if (isHarvestable) {
      const harvestBox = document.createElement("div");
      harvestBox.className = "chain-node-box node-nature";
      harvestBox.innerHTML = `
        <div class="node-badge">🌍 Natural World Deposit</div>
        <p>Harvested from field nodes in the world.</p>
        <small>${item.hand_gather_steps ? `Hand gather: ${item.hand_gather_steps} steps` : "No hand gather"} · ${item.extract_steps ? `Extractor: ${item.extract_steps} steps` : "Standard cadence"}</small>
      `;
      upstreamCol.appendChild(harvestBox);
    }

    if (pumpSource) {
      const pumpBox = document.createElement("div");
      pumpBox.className = "chain-node-box node-machine";
      pumpBox.innerHTML = `
        <div class="node-badge">💧 Pump Extraction</div>
        <p>Pumped continuously from water basins by <strong>${pumpSource.name}</strong>.</p>
      `;
      upstreamCol.appendChild(pumpBox);
    }

    if (producerRecipes.length === 0 && !isHarvestable && !pumpSource) {
      upstreamCol.innerHTML += `
        <div class="chain-node-box node-warning">
          <div class="node-badge">⚠ Unobtainable</div>
          <p>No recipe crafts this item, and it does not occur naturally in deposits.</p>
        </div>
      `;
    }

    for (const recipe of producerRecipes) {
      const rBox = document.createElement("div");
      rBox.className = "chain-node-box node-recipe";

      const machines = store.definitions.buildings.filter((b) =>
        supportsRecipe(b, recipe),
      );

      rBox.innerHTML = `
        <div class="node-badge">⚙ Recipe: ${recipe.name}</div>
        <div class="recipe-req-list">
          ${recipe.inputs
            .map((inp) => {
              const it = itemMap.get(inp.item_id);
              const c = it?.color ?? "#888";
              const s = itemIconSvg(it?.icon ?? "ore", c);
              return `
                <button type="button" class="chain-item-link" data-id="${inp.item_id}" style="border-color: ${c}50; background: ${c}15;">
                  ${s} ${inp.quantity}× ${it?.name ?? `#${inp.item_id}`}
                </button>
              `;
            })
            .join("")}
        </div>
        <div class="recipe-mid-info">
          <span>⏱ ${(recipe.duration / 60).toFixed(1)}s</span>
          ${recipe.fuel ? `<span>🔥 ${recipe.fuel}MJ</span>` : ""}
          <span class="gold-text">(${recipe.category})</span>
        </div>
        <small class="machines-list">Run by: ${machines.map((m) => m.name).join(", ") || "No machine"}</small>
      `;

      rBox.querySelectorAll(".chain-item-link").forEach((btn) => {
        btn.addEventListener("click", () => {
          selectedItemId = Number((btn as HTMLElement).dataset.id);
          itemSelect.value = String(selectedItemId);
          renderChainTree();
        });
      });

      upstreamCol.appendChild(rBox);
    }
    layout.appendChild(upstreamCol);

    // 2. Focused Center Item Column
    const centerCol = document.createElement("div");
    centerCol.className = "chain-column center-column";
    centerCol.innerHTML = `<h3><span>●</span> Selected Material</h3>`;

    const focusCard = document.createElement("div");
    focusCard.className = "chain-focus-card";
    focusCard.style.borderColor = `${itemColor}80`;
    focusCard.style.boxShadow = `0 0 25px ${itemColor}25`;

    focusCard.innerHTML = `
      <div class="focus-glyph" style="background: ${itemColor}20; border-color: ${itemColor};">
        ${itemSvg}
      </div>
      <h2 style="color: ${itemColor}">${item.name}</h2>
      <code>${item.key} (#${item.id})</code>
      <p class="focus-desc">${item.description}</p>
      <div class="focus-stats">
        <span class="badge">📦 Stack: ${item.stack_size}</span>
        ${item.fuel_value ? `<span class="badge badge-fuel">🔥 ${item.fuel_value} MJ Energy</span>` : ""}
      </div>
      <button type="button" class="btn btn-sm btn-primary" id="edit-focused-btn">Edit Definition</button>
    `;

    focusCard
      .querySelector("#edit-focused-btn")
      ?.addEventListener("click", () => {
        store.setEditingTarget({
          type: "item",
          data: structuredClone(item),
          isNew: false,
        });
      });

    centerCol.appendChild(focusCard);
    layout.appendChild(centerCol);

    // 3. Downstream Consumers Column
    const downstreamCol = document.createElement("div");
    downstreamCol.className = "chain-column downstream-column";
    downstreamCol.innerHTML = `<h3><span>⬇</span> Downstream Consumers</h3>`;

    if (consumerRecipes.length > 0) {
      const group = document.createElement("div");
      group.className = "chain-group-box";
      group.innerHTML = `<h4>Recipes Consuming This (${consumerRecipes.length})</h4>`;

      for (const cr of consumerRecipes) {
        for (const output of recipeOutputs(cr)) {
          const outIt = itemMap.get(output.item_id);
          const c = outIt?.color ?? "#888";
          const s = itemIconSvg(outIt?.icon ?? "ore", c);

          const btn = document.createElement("button");
          btn.type = "button";
          btn.className = "chain-item-link full-width-link";
          btn.style.borderColor = `${c}50`;
          btn.style.background = `${c}15`;
          btn.innerHTML = `
          ${s}
          <span>Creates <strong>${output.quantity}× ${outIt?.name ?? `#${output.item_id}`}</strong> (${cr.name})</span>
        `;
          btn.onclick = () => {
            selectedItemId = output.item_id;
            itemSelect.value = String(selectedItemId);
            renderChainTree();
          };
          group.appendChild(btn);
        }
      }
      downstreamCol.appendChild(group);
    }

    if (buildingsCosting.length > 0) {
      const group = document.createElement("div");
      group.className = "chain-group-box";
      group.innerHTML = `<h4>Construction Costs (${buildingsCosting.length})</h4>`;

      const list = document.createElement("div");
      list.className = "buildings-cost-pills-list";
      for (const b of buildingsCosting) {
        const pill = document.createElement("span");
        pill.className = "chain-bld-pill";
        const tech = b.unlock_technology_id
          ? techMap.get(b.unlock_technology_id)
          : undefined;
        pill.innerHTML = `🏗 <strong>${b.name}</strong> <small>(${tech ? tech.name : "Start"})</small>`;
        list.appendChild(pill);
      }
      group.appendChild(list);
      downstreamCol.appendChild(group);
    }

    if (hubRequests.length > 0) {
      const group = document.createElement("div");
      group.className = "chain-group-box";
      group.innerHTML = `<h4>Hub Insight Requests (${hubRequests.length})</h4>`;

      for (const req of hubRequests) {
        const row = document.createElement("div");
        row.className = "req-summary-pill";
        row.innerHTML = `
          <span>📋 <strong>${req.name}</strong></span>
          <span class="gold-text">◆ +${req.insight} Insight / ${req.quantity} units</span>
        `;
        group.appendChild(row);
      }
      downstreamCol.appendChild(group);
    }

    if (
      consumerRecipes.length === 0 &&
      buildingsCosting.length === 0 &&
      hubRequests.length === 0
    ) {
      downstreamCol.innerHTML += `
        <div class="chain-node-box node-info">
          <div class="node-badge">End Product / Leaf</div>
          <p>This item is currently not consumed by any recipes, building costs, or hub requests.</p>
        </div>
      `;
    }

    layout.appendChild(downstreamCol);
    treeContainer.appendChild(layout);
  }

  renderChainTree();
  container.appendChild(view);
}
