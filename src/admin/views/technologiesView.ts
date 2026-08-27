import type { TechnologyDefinition } from "../../core/types";
import type { AdminStore } from "../state";
import { showToast } from "../toast";

export function renderTechnologiesView(
  container: HTMLElement,
  store: AdminStore,
): void {
  container.innerHTML = "";

  const view = document.createElement("div");
  view.className = "admin-view technologies-view";

  const techMap = new Map(
    store.technologies.technologies.map((t) => [t.id, t]),
  );
  const buildingMap = new Map(
    store.definitions.buildings.map((b) => [b.id, b]),
  );

  // Toolbar
  const toolbar = document.createElement("div");
  toolbar.className = "view-toolbar";

  // Search input
  const searchInput = document.createElement("input");
  searchInput.type = "search";
  searchInput.placeholder =
    "Search technologies by name, key, unlocks, description...";
  searchInput.className = "search-input";
  searchInput.value = store.searchQuery;
  searchInput.oninput = () => store.setSearchQuery(searchInput.value.trim());
  toolbar.appendChild(searchInput);

  // Add Tech button
  const addBtn = document.createElement("button");
  addBtn.type = "button";
  addBtn.className = "btn btn-primary";
  addBtn.innerHTML = "<span>+</span> Add Technology";
  addBtn.onclick = () => {
    const nextId = store.getNextTechnologyId();
    const newTech: TechnologyDefinition = {
      id: nextId,
      key: `tech-${nextId}`,
      name: `New Technology ${nextId}`,
      description: "Description of what this breakthrough unlocks.",
      branch: store.technologies.branches[0]!.key,
      stage: store.technologies.stages[0]!.key,
      prerequisites: [],
      cost: 5,
      unlocks: [],
    };
    store.setEditingTarget({ type: "technology", data: newTech, isNew: true });
  };
  toolbar.appendChild(addBtn);

  view.appendChild(toolbar);

  // Filter techs
  const query = store.searchQuery.toLowerCase();
  const filteredTechs = store.technologies.technologies.filter((t) => {
    if (query) {
      const unlockNames = t.unlocks
        .map((uid) => buildingMap.get(uid)?.name ?? "")
        .join(" ");
      const matchText =
        `${t.id} ${t.key} ${t.name} ${t.description} ${unlockNames}`.toLowerCase();
      if (!matchText.includes(query)) return false;
    }
    return true;
  });

  // Calculate total insight cost of tree
  const totalCost = store.technologies.technologies.reduce(
    (sum, t) => sum + t.cost,
    0,
  );

  // Count bar
  const countBar = document.createElement("div");
  countBar.className = "view-count-bar";
  countBar.innerHTML = `Showing <strong>${filteredTechs.length}</strong> of ${store.technologies.technologies.length} technologies · Total Tree Cost: <strong class="gold-text">◆ ${totalCost} Insight</strong>`;
  view.appendChild(countBar);

  // Grid
  const grid = document.createElement("div");
  grid.className = "technologies-grid";

  for (const tech of filteredTechs) {
    const card = document.createElement("div");
    card.className = "tech-card";

    card.innerHTML = `
      <div class="card-header">
        <div class="tech-title-group">
          <div class="card-title-row">
            <h3 class="card-title">${tech.name}</h3>
            <span class="id-badge">#${tech.id}</span>
          </div>
          <div class="tech-meta-row">
            <code class="item-key">${tech.key}</code>
            <span class="insight-cost-badge">◆ ${tech.cost} Insight</span>
          </div>
        </div>
      </div>

      <p class="card-description">${tech.description}</p>

      <div class="tech-links-section">
        <div class="tech-link-group">
          <span class="tech-link-label">Prerequisites:</span>
          <div class="tech-badge-list">
            ${
              tech.prerequisites.length > 0
                ? tech.prerequisites
                    .map((pid) => {
                      const parent = techMap.get(pid);
                      return `<span class="tech-ref-pill">↳ ${parent?.name ?? `#${pid}`}</span>`;
                    })
                    .join("")
                : `<span class="dim-text">Root node (None)</span>`
            }
          </div>
        </div>

        <div class="tech-link-group">
          <span class="tech-link-label">Unlocks Buildings:</span>
          <div class="tech-badge-list">
            ${
              tech.unlocks.length > 0
                ? tech.unlocks
                    .map((bid) => {
                      const bld = buildingMap.get(bid);
                      return `<span class="tech-unlock-pill">🏗 ${bld?.name ?? `#${bid}`}</span>`;
                    })
                    .join("")
                : `<span class="dim-text">No direct building unlocks</span>`
            }
          </div>
        </div>
        <div class="tech-link-group">
          <span class="tech-link-label">Player Bonuses:</span>
          <div class="tech-badge-list">
            ${
              tech.carry_slots_bonus || tech.build_range_bonus
                ? [
                    tech.carry_slots_bonus
                      ? `+${tech.carry_slots_bonus} cargo slots`
                      : "",
                    tech.build_range_bonus
                      ? `+${tech.build_range_bonus} hex build range`
                      : "",
                  ]
                    .filter(Boolean)
                    .map(
                      (bonus) =>
                        `<span class="tech-unlock-pill">${bonus}</span>`,
                    )
                    .join("")
                : `<span class="dim-text">No direct player bonuses</span>`
            }
          </div>
        </div>
      </div>

      <div class="card-actions">
        <button type="button" class="btn btn-sm btn-edit">Edit</button>
        <button type="button" class="btn btn-sm btn-danger btn-del">Delete</button>
      </div>
    `;

    card.querySelector(".btn-edit")?.addEventListener("click", () => {
      store.setEditingTarget({
        type: "technology",
        data: structuredClone(tech),
        isNew: false,
      });
    });

    card.querySelector(".btn-del")?.addEventListener("click", () => {
      if (confirm(`Delete technology "${tech.name}"?`)) {
        store.deleteTechnology(tech.id);
        showToast(`Deleted ${tech.name}`, "info");
      }
    });

    grid.appendChild(card);
  }

  view.appendChild(grid);
  container.appendChild(view);

  // Modal
  if (store.editingTarget?.type === "technology") {
    renderTechnologyModal(
      store.editingTarget.data,
      store.editingTarget.isNew,
      store,
    );
  }
}

function renderTechnologyModal(
  tech: TechnologyDefinition,
  isNew: boolean,
  store: AdminStore,
): void {
  const modalOverlay = document.createElement("div");
  modalOverlay.className = "modal-overlay";

  const modal = document.createElement("div");
  modal.className = "modal tech-modal";

  const currentTech: TechnologyDefinition = structuredClone(tech);

  modal.innerHTML = `
    <div class="modal-header">
      <h2>${isNew ? "Create Technology" : `Edit Technology #${tech.id}`}</h2>
      <button type="button" class="modal-close-btn">&times;</button>
    </div>
    <div class="modal-body">
      <form class="entity-form" id="tech-form">
        <div class="form-row form-row-2">
          <label>
            <span>Technology ID *</span>
            <input type="number" name="id" value="${currentTech.id}" min="1" required ${isNew ? "" : "readonly"} />
          </label>
          <label>
            <span>Key Identifier *</span>
            <input type="text" name="key" value="${currentTech.key}" required pattern="[a-z0-9-_]+" />
          </label>
        </div>

        <div class="form-row form-row-2">
          <label>
            <span>Technology Name *</span>
            <input type="text" name="name" value="${currentTech.name}" required />
          </label>
          <label>
            <span>Insight Cost *</span>
            <input type="number" name="cost" value="${currentTech.cost}" min="1" required />
          </label>
        </div>

        <div class="form-row">
          <label>
            <span>Description *</span>
            <textarea name="description" rows="2" required>${currentTech.description}</textarea>
          </label>
        </div>

        <div class="form-row form-row-2">
          <label>
            <span>Cargo Slot Bonus</span>
            <input type="number" name="carry_slots_bonus" value="${currentTech.carry_slots_bonus ?? 0}" min="0" max="240" />
          </label>
          <label>
            <span>Build Range Bonus (hexes)</span>
            <input type="number" name="build_range_bonus" value="${currentTech.build_range_bonus ?? 0}" min="0" max="96" />
          </label>
        </div>

        <div class="form-section">
          <span class="section-title">Prerequisites (Required Technologies)</span>
          <div class="multi-select-grid" id="prereq-select-grid">
            ${store.technologies.technologies
              .filter((t) => t.id !== currentTech.id)
              .map((t) => {
                const checked = currentTech.prerequisites.includes(t.id);
                return `
                  <label class="multi-select-item">
                    <input type="checkbox" name="prereq_${t.id}" value="${t.id}" ${checked ? "checked" : ""} />
                    <span>${t.name} (#${t.id})</span>
                  </label>
                `;
              })
              .join("")}
          </div>
        </div>

        <div class="form-section">
          <span class="section-title">Unlocked Buildings</span>
          <div class="multi-select-grid" id="unlocks-select-grid">
            ${store.definitions.buildings
              .map((b) => {
                const checked = currentTech.unlocks.includes(b.id);
                return `
                  <label class="multi-select-item">
                    <input type="checkbox" name="unlock_${b.id}" value="${b.id}" ${checked ? "checked" : ""} />
                    <span>🏗 ${b.name} (#${b.id}) [${b.kind}]</span>
                  </label>
                `;
              })
              .join("")}
          </div>
        </div>

        <div class="modal-actions">
          <button type="button" class="btn btn-subtle modal-cancel-btn">Cancel</button>
          <button type="submit" class="btn btn-primary">Save Technology</button>
        </div>
      </form>
    </div>
  `;

  const form = modal.querySelector<HTMLFormElement>("#tech-form")!;
  const classification = document.createElement("div");
  classification.className = "form-row form-row-2";
  for (const [name, groups] of [
    ["branch", store.technologies.branches],
    ["stage", store.technologies.stages],
  ] as const) {
    const label = document.createElement("label");
    const title = document.createElement("span");
    title.textContent =
      name === "branch" ? "Branch" : "Stage (presentation only)";
    const select = document.createElement("select");
    select.name = name;
    select.required = true;
    for (const group of groups) {
      const option = document.createElement("option");
      option.value = group.key;
      option.textContent = group.name;
      option.selected = currentTech[name] === group.key;
      select.appendChild(option);
    }
    label.append(title, select);
    classification.appendChild(label);
  }
  form.prepend(classification);
  const nameInput =
    modal.querySelector<HTMLInputElement>("input[name='name']")!;
  const keyInput = modal.querySelector<HTMLInputElement>("input[name='key']")!;

  nameInput.addEventListener("input", () => {
    currentTech.name = nameInput.value;
    if (isNew && keyInput) {
      keyInput.value = nameInput.value
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-|-$/g, "");
    }
  });

  form.onsubmit = (e) => {
    e.preventDefault();
    const formData = new FormData(form);

    const id = Number(formData.get("id"));
    const key = String(formData.get("key")).trim();
    const name = String(formData.get("name")).trim();
    const description = String(formData.get("description")).trim();
    const cost = Number(formData.get("cost"));
    const carrySlotsBonus = Number(formData.get("carry_slots_bonus"));
    const buildRangeBonus = Number(formData.get("build_range_bonus"));

    const prerequisites: number[] = [];
    store.technologies.technologies.forEach((t) => {
      if (formData.get(`prereq_${t.id}`) !== null) {
        prerequisites.push(t.id);
      }
    });

    const unlocks: number[] = [];
    store.definitions.buildings.forEach((b) => {
      if (formData.get(`unlock_${b.id}`) !== null) {
        unlocks.push(b.id);
      }
    });

    const updated: TechnologyDefinition = {
      id,
      key,
      name,
      description,
      branch: String(formData.get("branch")),
      stage: String(formData.get("stage")),
      prerequisites,
      cost,
      unlocks,
      ...(carrySlotsBonus > 0 ? { carry_slots_bonus: carrySlotsBonus } : {}),
      ...(buildRangeBonus > 0 ? { build_range_bonus: buildRangeBonus } : {}),
    };

    store.saveTechnology(updated);

    // Also sync building unlock_technology_id
    for (const b of store.definitions.buildings) {
      if (unlocks.includes(b.id)) {
        b.unlock_technology_id = id;
      } else if (b.unlock_technology_id === id) {
        delete b.unlock_technology_id;
      }
    }

    showToast(`Saved technology "${updated.name}"`, "success");
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
}
