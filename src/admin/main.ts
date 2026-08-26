import { downloadJsonFile, formatDefinitionsJson } from "./exporter";
import { AdminStore } from "./state";
import { showToast } from "./toast";
import { renderBuildingsView } from "./views/buildingsView";
import { renderChainView } from "./views/chainView";
import { renderDiagnosticsView } from "./views/diagnosticsView";
import { showDiffModal } from "./views/diffModal";
import { renderHeader } from "./views/headerView";
import { renderItemsView } from "./views/itemsView";
import { renderRawJsonView } from "./views/rawJsonView";
import { renderRecipesView } from "./views/recipesView";
import { renderRequestsView } from "./views/requestsView";
import { renderTechnologiesView } from "./views/technologiesView";

function initAdmin(): void {
  const root = document.getElementById("admin-app");
  if (!root) return;

  const store = new AdminStore();

  function render(): void {
    if (!root) return;
    root.innerHTML = "";

    const headerContainer = document.createElement("div");
    headerContainer.className = "header-mount";
    renderHeader(headerContainer, store, () => showDiffModal(store));
    root.appendChild(headerContainer);

    const mainContainer = document.createElement("main");
    mainContainer.className = "main-content-mount";

    switch (store.activeTab) {
      case "items":
        renderItemsView(mainContainer, store);
        break;
      case "recipes":
        renderRecipesView(mainContainer, store);
        break;
      case "buildings":
        renderBuildingsView(mainContainer, store);
        break;
      case "requests":
        renderRequestsView(mainContainer, store);
        break;
      case "technologies":
        renderTechnologiesView(mainContainer, store);
        break;
      case "chains":
        renderChainView(mainContainer, store);
        break;
      case "diagnostics":
        renderDiagnosticsView(mainContainer, store);
        break;
      case "raw-json":
        renderRawJsonView(mainContainer, store);
        break;
    }

    root.appendChild(mainContainer);
  }

  // Subscribe to store updates
  store.subscribe(render);

  // Initial render
  render();

  // Keyboard shortcuts
  window.addEventListener("keydown", (e) => {
    const isMac = navigator.platform.toUpperCase().includes("MAC");
    const mod = isMac ? e.metaKey : e.ctrlKey;

    // Ctrl+S / Cmd+S: Quick export definitions.json
    if (mod && (e.key === "s" || e.key === "S")) {
      e.preventDefault();
      const json = formatDefinitionsJson(store.definitions);
      downloadJsonFile("definitions.json", json);
      showToast("Exported definitions.json", "success");
      return;
    }

    // Ctrl+Z / Cmd+Z: Undo
    if (mod && (e.key === "z" || e.key === "Z") && !e.shiftKey) {
      const activeEl = document.activeElement;
      if (activeEl?.tagName === "INPUT" || activeEl?.tagName === "TEXTAREA") {
        return; // Allow native text undo
      }
      e.preventDefault();
      if (store.undo()) {
        showToast("Undo", "info");
      }
      return;
    }

    // Ctrl+Y / Cmd+Shift+Z: Redo
    if (
      (mod && (e.key === "y" || e.key === "Y")) ||
      (mod && e.shiftKey && (e.key === "z" || e.key === "Z"))
    ) {
      const activeEl = document.activeElement;
      if (activeEl?.tagName === "INPUT" || activeEl?.tagName === "TEXTAREA") {
        return; // Allow native text redo
      }
      e.preventDefault();
      if (store.redo()) {
        showToast("Redo", "info");
      }
      return;
    }

    // Escape: Close modals
    if (e.key === "Escape") {
      const openModal = document.querySelector(".modal-overlay");
      if (openModal) {
        store.setEditingTarget(null);
        openModal.remove();
      }
    }
  });
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initAdmin);
} else {
  initAdmin();
}
