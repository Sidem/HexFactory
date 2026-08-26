import { downloadJsonFile, formatDefinitionsJson } from "../exporter";
import type { AdminStore } from "../state";
import { showToast } from "../toast";

export function showDiffModal(store: AdminStore): void {
  const modalOverlay = document.createElement("div");
  modalOverlay.className = "modal-overlay";

  const modal = document.createElement("div");
  modal.className = "modal diff-modal";

  const changes = store.getDiffSummary();

  modal.innerHTML = `
    <div class="modal-header">
      <h2>Working Changes &amp; Diff (${changes.length})</h2>
      <button type="button" class="modal-close-btn">&times;</button>
    </div>
    <div class="modal-body">
      ${
        changes.length === 0
          ? `<div class="diff-empty-state"><p>No changes detected. Working state is identical to baseline shipped data.</p></div>`
          : `
          <div class="diff-changes-summary">
            <p>The following ${changes.length} entities have been modified, added, or removed compared to the original baseline:</p>
            <div class="diff-items-list">
              ${changes
                .map((ch) => {
                  const typeClass =
                    ch.changeType === "added"
                      ? "diff-added"
                      : ch.changeType === "deleted"
                        ? "diff-deleted"
                        : "diff-modified";
                  const badge = ch.changeType.toUpperCase();
                  return `
                    <div class="diff-item-card ${typeClass}">
                      <div class="diff-item-header">
                        <span class="diff-badge">${badge}</span>
                        <span class="diff-entity-type">${ch.entityType.toUpperCase()} #${ch.id}</span>
                        <strong>${ch.name}</strong>
                      </div>
                      <ul class="diff-details-list">
                        ${ch.details.map((d) => `<li>${d}</li>`).join("")}
                      </ul>
                    </div>
                  `;
                })
                .join("")}
            </div>
          </div>
        `
      }
      <div class="modal-actions">
        <button type="button" class="btn btn-subtle modal-close-btn">Close</button>
        ${
          changes.length > 0
            ? `
            <button type="button" class="btn btn-danger" id="diff-revert-btn">Revert All Changes</button>
            <button type="button" class="btn btn-primary" id="diff-export-btn">Export definitions.json</button>
          `
            : ""
        }
      </div>
    </div>
  `;

  modal.querySelectorAll(".modal-close-btn").forEach((btn) => {
    btn.addEventListener("click", () => modalOverlay.remove());
  });

  modalOverlay.addEventListener("click", (e) => {
    if (e.target === modalOverlay) modalOverlay.remove();
  });

  modal.querySelector("#diff-revert-btn")?.addEventListener("click", () => {
    if (
      confirm(
        "Are you sure you want to discard all changes and revert back to baseline?",
      )
    ) {
      store.revertToBaseline();
      showToast("Reverted all changes to baseline", "info");
      modalOverlay.remove();
    }
  });

  modal.querySelector("#diff-export-btn")?.addEventListener("click", () => {
    downloadJsonFile(
      "definitions.json",
      formatDefinitionsJson(store.definitions),
    );
    showToast("Downloaded definitions.json", "success");
    modalOverlay.remove();
  });

  modalOverlay.appendChild(modal);
  document.body.appendChild(modalOverlay);
}
