import type { RequestDefinition } from "../../core/types";
import { itemIconSvg } from "../../rendering/icons";
import type { AdminStore } from "../state";
import { showToast } from "../toast";

export function renderRequestsView(
  container: HTMLElement,
  store: AdminStore,
): void {
  container.innerHTML = "";

  const view = document.createElement("div");
  view.className = "admin-view requests-view";

  const itemMap = new Map(store.definitions.items.map((i) => [i.id, i]));

  // Toolbar
  const toolbar = document.createElement("div");
  toolbar.className = "view-toolbar";

  // Search input
  const searchInput = document.createElement("input");
  searchInput.type = "search";
  searchInput.placeholder = "Search hub requests by name, key, brief, item...";
  searchInput.className = "search-input";
  searchInput.value = store.searchQuery;
  searchInput.oninput = () => store.setSearchQuery(searchInput.value.trim());
  toolbar.appendChild(searchInput);

  // Add button
  const addBtn = document.createElement("button");
  addBtn.type = "button";
  addBtn.className = "btn btn-primary";
  addBtn.innerHTML = "<span>+</span> Add Request";
  addBtn.onclick = () => {
    const nextId = store.getNextRequestId();
    const firstItemId = store.definitions.items[0]?.id ?? 1;
    const newRequest: RequestDefinition = {
      id: nextId,
      key: `request-${nextId}`,
      name: `New Hub Request ${nextId}`,
      brief: "Brief description of the research contract requirement.",
      item_id: firstItemId,
      quantity: 10,
      insight: 10,
      repeat_insight: 2,
    };
    store.setEditingTarget({ type: "request", data: newRequest, isNew: true });
  };
  toolbar.appendChild(addBtn);

  view.appendChild(toolbar);

  // Filter requests
  const query = store.searchQuery.toLowerCase();
  const filteredRequests = store.definitions.requests.filter((r) => {
    if (query) {
      const it = itemMap.get(r.item_id);
      const matchText =
        `${r.id} ${r.key} ${r.name} ${r.brief} ${it?.name ?? ""}`.toLowerCase();
      if (!matchText.includes(query)) return false;
    }
    return true;
  });

  // Count bar
  const countBar = document.createElement("div");
  countBar.className = "view-count-bar";
  countBar.innerHTML = `Showing <strong>${filteredRequests.length}</strong> of ${store.definitions.requests.length} standing hub requests`;
  view.appendChild(countBar);

  // Grid
  const grid = document.createElement("div");
  grid.className = "requests-grid";

  for (const req of filteredRequests) {
    const card = document.createElement("div");
    card.className = "request-card";

    const item = itemMap.get(req.item_id);
    const color = item?.color ?? "#b78cff";
    const svg = itemIconSvg(item?.icon ?? "ore", color);

    const ratio = (req.insight / Math.max(1, req.quantity)).toFixed(2);
    const repeatRatio = req.repeat_insight
      ? (req.repeat_insight / Math.max(1, req.quantity)).toFixed(2)
      : ratio;

    card.innerHTML = `
      <div class="card-header">
        <div class="request-item-badge" style="border-color: ${color}50; background: ${color}15;">
          ${svg}
          <span style="color: ${color}">${req.quantity}×</span>
        </div>
        <div class="card-title-group">
          <div class="card-title-row">
            <h3 class="card-title">${req.name}</h3>
            <span class="id-badge">#${req.id}</span>
          </div>
          <code class="item-key">${req.key}</code>
        </div>
      </div>

      <p class="card-description">${req.brief}</p>

      <div class="request-rewards-box">
        <div class="reward-row">
          <span>First Delivery Reward:</span>
          <strong class="insight-payout">◆ ${req.insight} Insight <small>(${ratio} / item)</small></strong>
        </div>
        <div class="reward-row">
          <span>Subsequent Deliveries:</span>
          <strong>◆ ${req.repeat_insight ?? req.insight} Insight <small>(${repeatRatio} / item)</small></strong>
        </div>
      </div>

      <div class="card-actions">
        <button type="button" class="btn btn-sm btn-edit">Edit</button>
        <button type="button" class="btn btn-sm btn-subtle btn-dup">Duplicate</button>
        <button type="button" class="btn btn-sm btn-danger btn-del">Delete</button>
      </div>
    `;

    card.querySelector(".btn-edit")?.addEventListener("click", () => {
      store.setEditingTarget({
        type: "request",
        data: structuredClone(req),
        isNew: false,
      });
    });

    card.querySelector(".btn-dup")?.addEventListener("click", () => {
      const cloned = store.duplicateRequest(req.id);
      if (cloned)
        showToast(`Duplicated ${req.name} as #${cloned.id}`, "success");
    });

    card.querySelector(".btn-del")?.addEventListener("click", () => {
      if (confirm(`Delete request "${req.name}"?`)) {
        store.deleteRequest(req.id);
        showToast(`Deleted ${req.name}`, "info");
      }
    });

    grid.appendChild(card);
  }

  view.appendChild(grid);
  container.appendChild(view);

  // Modal
  if (store.editingTarget?.type === "request") {
    renderRequestModal(
      store.editingTarget.data,
      store.editingTarget.isNew,
      store,
    );
  }
}

function renderRequestModal(
  request: RequestDefinition,
  isNew: boolean,
  store: AdminStore,
): void {
  const modalOverlay = document.createElement("div");
  modalOverlay.className = "modal-overlay";

  const modal = document.createElement("div");
  modal.className = "modal request-modal";

  const currentReq: RequestDefinition = structuredClone(request);

  modal.innerHTML = `
    <div class="modal-header">
      <h2>${isNew ? "Create Hub Request" : `Edit Hub Request #${request.id}`}</h2>
      <button type="button" class="modal-close-btn">&times;</button>
    </div>
    <div class="modal-body">
      <form class="entity-form" id="request-form">
        <div class="form-row form-row-2">
          <label>
            <span>Request ID *</span>
            <input type="number" name="id" value="${currentReq.id}" min="1" required ${isNew ? "" : "readonly"} />
          </label>
          <label>
            <span>Key Identifier *</span>
            <input type="text" name="key" value="${currentReq.key}" required pattern="[a-z0-9-_]+" />
          </label>
        </div>

        <div class="form-row">
          <label>
            <span>Request Name *</span>
            <input type="text" name="name" value="${currentReq.name}" required />
          </label>
        </div>

        <div class="form-row">
          <label>
            <span>Brief / Mission Briefing *</span>
            <textarea name="brief" rows="2" required>${currentReq.brief}</textarea>
          </label>
        </div>

        <div class="form-row form-row-2">
          <label>
            <span>Demanded Item *</span>
            <select name="item_id" required>
              ${store.definitions.items
                .map(
                  (it) =>
                    `<option value="${it.id}" ${it.id === currentReq.item_id ? "selected" : ""}>${it.name} (#${it.id})</option>`,
                )
                .join("")}
            </select>
          </label>
          <label>
            <span>Quantity Required *</span>
            <input type="number" name="quantity" value="${currentReq.quantity}" min="1" max="1000" required />
          </label>
        </div>

        <div class="form-row form-row-2">
          <label>
            <span>First-time Insight Reward *</span>
            <input type="number" name="insight" value="${currentReq.insight}" min="1" required />
            <small class="field-hint">Initial research currency reward</small>
          </label>
          <label>
            <span>Repeat Insight Reward</span>
            <input type="number" name="repeat_insight" value="${currentReq.repeat_insight ?? ""}" min="1" placeholder="Leave empty for same as first" />
            <small class="field-hint">Payout for subsequent repeat completions</small>
          </label>
        </div>

        <div class="modal-actions">
          <button type="button" class="btn btn-subtle modal-cancel-btn">Cancel</button>
          <button type="submit" class="btn btn-primary">Save Request</button>
        </div>
      </form>
    </div>
  `;

  const form = modal.querySelector<HTMLFormElement>("#request-form")!;
  const nameInput =
    modal.querySelector<HTMLInputElement>("input[name='name']")!;
  const keyInput = modal.querySelector<HTMLInputElement>("input[name='key']")!;

  nameInput.addEventListener("input", () => {
    currentReq.name = nameInput.value;
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
    const brief = String(formData.get("brief")).trim();
    const item_id = Number(formData.get("item_id"));
    const quantity = Number(formData.get("quantity"));
    const insight = Number(formData.get("insight"));
    const repeatRaw = formData.get("repeat_insight");

    const updated: RequestDefinition = {
      id,
      key,
      name,
      brief,
      item_id,
      quantity,
      insight,
    };

    if (repeatRaw && Number(repeatRaw) > 0) {
      updated.repeat_insight = Number(repeatRaw);
    }

    store.saveRequest(updated);
    showToast(`Saved request "${updated.name}"`, "success");
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
