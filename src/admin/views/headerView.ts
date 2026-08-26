import {
  downloadJsonFile,
  formatDefinitionsJson,
  formatScenariosJson,
  formatTechnologiesJson,
} from "../exporter";
import type { AdminStore } from "../state";
import { showToast } from "../toast";
import type { AdminTab } from "../types";

export function renderHeader(
  container: HTMLElement,
  store: AdminStore,
  onOpenDiff: () => void,
): void {
  container.innerHTML = "";

  const header = document.createElement("header");
  header.className = "admin-header";

  // Left brand
  const brandRow = document.createElement("div");
  brandRow.className = "header-brand-row";

  const brandLink = document.createElement("a");
  brandLink.className = "header-brand";
  brandLink.href = "./";
  brandLink.innerHTML = `
    <svg class="header-logo" viewBox="0 0 48 48" aria-hidden="true">
      <path d="M24 2.5 42.6 13.25 42.6 34.75 24 45.5 5.4 34.75 5.4 13.25Z" fill="#16241f" stroke="#f6c85f" stroke-width="2.6" stroke-linejoin="round"/>
      <path d="M24 15 14.5 31M24 15 33.5 31M14.5 31H33.5" stroke="#72e2b4" stroke-width="2.3" stroke-linecap="round"/>
      <path d="M24 8.6 30.06 12.1 30.06 19.1 24 22.6 17.94 19.1 17.94 12.1Z" fill="#f6c85f"/>
    </svg>
    <div class="brand-text">
      <span class="brand-title"><strong>HexFactory</strong> Studio</span>
      <span class="brand-subtitle">Definitions & Balance Workspace</span>
    </div>
  `;
  brandRow.appendChild(brandLink);

  // Quick stats summary
  const statsGroup = document.createElement("div");
  statsGroup.className = "header-stats";
  statsGroup.innerHTML = `
    <span class="stat-pill" title="Total Items">📦 ${store.definitions.items.length}</span>
    <span class="stat-pill" title="Total Recipes">⚙ ${store.definitions.recipes.length}</span>
    <span class="stat-pill" title="Total Buildings">🏗 ${store.definitions.buildings.length}</span>
    <span class="stat-pill" title="Total Requests">📋 ${store.definitions.requests.length}</span>
    <span class="stat-pill" title="Total Technologies">🔬 ${store.technologies.technologies.length}</span>
  `;
  brandRow.appendChild(statsGroup);

  // Health / Validation status badge
  const errorCount = store.diagnostics.filter(
    (d) => d.severity === "error",
  ).length;
  const warnCount = store.diagnostics.filter(
    (d) => d.severity === "warning",
  ).length;

  const healthBadge = document.createElement("button");
  healthBadge.type = "button";
  healthBadge.className = `health-badge ${errorCount > 0 ? "health-error" : warnCount > 0 ? "health-warning" : "health-ok"}`;
  healthBadge.title = "Click to view diagnostics report";
  healthBadge.onclick = () => store.setTab("diagnostics");
  if (errorCount > 0) {
    healthBadge.innerHTML = `<span class="health-dot">✕</span> ${errorCount} Error${errorCount > 1 ? "s" : ""}${warnCount > 0 ? ` (${warnCount} warn)` : ""}`;
  } else if (warnCount > 0) {
    healthBadge.innerHTML = `<span class="health-dot">⚠</span> ${warnCount} Warning${warnCount > 1 ? "s" : ""}`;
  } else {
    healthBadge.innerHTML = `<span class="health-dot">✓</span> Valid (0 issues)`;
  }
  brandRow.appendChild(healthBadge);

  // Action controls
  const actionsGroup = document.createElement("div");
  actionsGroup.className = "header-actions";

  // Undo button
  const undoBtn = document.createElement("button");
  undoBtn.type = "button";
  undoBtn.className = "btn btn-icon";
  undoBtn.disabled = !store.canUndo();
  undoBtn.title = "Undo (Ctrl+Z)";
  undoBtn.innerHTML = `<span>↶</span> Undo`;
  undoBtn.onclick = () => {
    if (store.undo()) showToast("Action undone", "info");
  };
  actionsGroup.appendChild(undoBtn);

  // Redo button
  const redoBtn = document.createElement("button");
  redoBtn.type = "button";
  redoBtn.className = "btn btn-icon";
  redoBtn.disabled = !store.canRedo();
  redoBtn.title = "Redo (Ctrl+Y)";
  redoBtn.innerHTML = `<span>↷</span> Redo`;
  redoBtn.onclick = () => {
    if (store.redo()) showToast("Action redone", "info");
  };
  actionsGroup.appendChild(redoBtn);

  // Diff button
  const dirtyCount = store.getDirtyCount();
  const diffBtn = document.createElement("button");
  diffBtn.type = "button";
  diffBtn.className = `btn ${dirtyCount > 0 ? "btn-dirty" : ""}`;
  diffBtn.title = "Review visual and JSON diff against baseline definitions";
  diffBtn.innerHTML = `<span>📝</span> Diff${dirtyCount > 0 ? ` (${dirtyCount})` : ""}`;
  diffBtn.onclick = onOpenDiff;
  actionsGroup.appendChild(diffBtn);

  // Import button & hidden file input
  const fileInput = document.createElement("input");
  fileInput.type = "file";
  fileInput.accept = ".json";
  fileInput.style.display = "none";
  fileInput.onchange = (e) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const content = String(reader.result);
      try {
        const parsed = JSON.parse(content);
        if (parsed.items && parsed.recipes && parsed.buildings) {
          store.importDefinitions(parsed);
          showToast(
            `Imported ${parsed.items.length} items, ${parsed.recipes.length} recipes from ${file.name}`,
            "success",
          );
        } else if (parsed.technologies) {
          store.importTechnologies(parsed);
          showToast(
            `Imported ${parsed.technologies.length} technologies from ${file.name}`,
            "success",
          );
        } else {
          showToast(
            "Unrecognized JSON schema. Expected definitions or technologies format.",
            "error",
          );
        }
      } catch (err) {
        showToast(
          `Failed to parse JSON: ${err instanceof Error ? err.message : String(err)}`,
          "error",
        );
      }
    };
    reader.readAsText(file);
    fileInput.value = "";
  };
  actionsGroup.appendChild(fileInput);

  const importBtn = document.createElement("button");
  importBtn.type = "button";
  importBtn.className = "btn";
  importBtn.title = "Import a JSON definition or technology file";
  importBtn.innerHTML = `<span>📥</span> Import JSON`;
  importBtn.onclick = () => fileInput.click();
  actionsGroup.appendChild(importBtn);

  // Revert button
  const revertBtn = document.createElement("button");
  revertBtn.type = "button";
  revertBtn.className = "btn btn-subtle";
  revertBtn.title = "Reset all changes back to original shipped data";
  revertBtn.textContent = "Reset to Default";
  revertBtn.onclick = () => {
    if (
      confirm(
        "Reset all modified definitions back to the shipped defaults? Any unsaved changes will be cleared.",
      )
    ) {
      store.revertToBaseline();
      showToast("Reset to default definitions", "info");
    }
  };
  actionsGroup.appendChild(revertBtn);

  // Export dropdown container
  const exportDropdown = document.createElement("div");
  exportDropdown.className = "dropdown export-dropdown";

  const exportMainBtn = document.createElement("button");
  exportMainBtn.type = "button";
  exportMainBtn.className = "btn btn-primary export-btn";
  exportMainBtn.title = "Export and download updated definitions.json (Ctrl+S)";
  exportMainBtn.innerHTML = `<span>💾</span> Export definitions.json`;
  exportMainBtn.onclick = () => {
    const json = formatDefinitionsJson(store.definitions);
    downloadJsonFile("definitions.json", json);
    showToast("Downloaded definitions.json", "success");
  };
  exportDropdown.appendChild(exportMainBtn);

  const exportToggle = document.createElement("button");
  exportToggle.type = "button";
  exportToggle.className = "btn btn-primary dropdown-toggle";
  exportToggle.innerHTML = "▾";
  exportToggle.title = "More export options";
  exportToggle.onclick = (e) => {
    e.stopPropagation();
    exportMenu.classList.toggle("open");
  };
  exportDropdown.appendChild(exportToggle);

  const exportMenu = document.createElement("div");
  exportMenu.className = "dropdown-menu";
  exportMenu.innerHTML = `
    <button type="button" data-export="definitions"><span>💾</span> definitions.json</button>
    <button type="button" data-export="technologies"><span>🔬</span> technologies.json</button>
    <button type="button" data-export="scenarios"><span>🗺</span> scenarios.json</button>
    <button type="button" data-export="all"><span>📦</span> Export All Files</button>
  `;
  exportMenu.addEventListener("click", (e) => {
    const target = (e.target as HTMLElement).closest("button");
    if (!target) return;
    const kind = target.dataset.export;
    if (kind === "definitions") {
      downloadJsonFile(
        "definitions.json",
        formatDefinitionsJson(store.definitions),
      );
      showToast("Downloaded definitions.json", "success");
    } else if (kind === "technologies") {
      downloadJsonFile(
        "technologies.json",
        formatTechnologiesJson(store.technologies),
      );
      showToast("Downloaded technologies.json", "success");
    } else if (kind === "scenarios") {
      downloadJsonFile("scenarios.json", formatScenariosJson(store.scenarios));
      showToast("Downloaded scenarios.json", "success");
    } else if (kind === "all") {
      downloadJsonFile(
        "definitions.json",
        formatDefinitionsJson(store.definitions),
      );
      setTimeout(
        () =>
          downloadJsonFile(
            "technologies.json",
            formatTechnologiesJson(store.technologies),
          ),
        200,
      );
      setTimeout(
        () =>
          downloadJsonFile(
            "scenarios.json",
            formatScenariosJson(store.scenarios),
          ),
        400,
      );
      showToast(
        "Downloaded definitions, technologies, and scenarios",
        "success",
      );
    }
    exportMenu.classList.remove("open");
  });
  exportDropdown.appendChild(exportMenu);
  actionsGroup.appendChild(exportDropdown);

  // Close dropdown on outside click
  document.addEventListener("click", (e) => {
    if (!exportDropdown.contains(e.target as Node)) {
      exportMenu.classList.remove("open");
    }
  });

  // Nav links to game, sheet, bench
  const navLinks = document.createElement("div");
  navLinks.className = "header-nav-links";
  navLinks.innerHTML = `
    <a href="./" class="nav-link" title="Launch game">Play Game ↗</a>
    <a href="./contact.html" class="nav-link" title="Open shape contact sheet">Contact Sheet ↗</a>
    <a href="./bench.html" class="nav-link" title="Open performance bench">Benchmark ↗</a>
  `;
  actionsGroup.appendChild(navLinks);

  header.appendChild(brandRow);
  header.appendChild(actionsGroup);

  // Navigation Tabs Row
  const navBar = document.createElement("nav");
  navBar.className = "admin-nav-bar";

  const tabs: Array<{
    id: AdminTab;
    label: string;
    icon: string;
    badge?: string | number;
  }> = [
    {
      id: "items",
      label: "Items",
      icon: "📦",
      badge: store.definitions.items.length,
    },
    {
      id: "recipes",
      label: "Recipes",
      icon: "⚙",
      badge: store.definitions.recipes.length,
    },
    {
      id: "buildings",
      label: "Buildings",
      icon: "🏗",
      badge: store.definitions.buildings.length,
    },
    {
      id: "requests",
      label: "Hub Requests",
      icon: "📋",
      badge: store.definitions.requests.length,
    },
    {
      id: "technologies",
      label: "Technologies",
      icon: "🔬",
      badge: store.technologies.technologies.length,
    },
    { id: "chains", label: "Production Chains", icon: "🔗" },
    {
      id: "diagnostics",
      label: "Diagnostics",
      icon: "🔍",
      badge:
        errorCount > 0
          ? `${errorCount}!`
          : warnCount > 0
            ? `${warnCount}`
            : "✓",
    },
    { id: "raw-json", label: "Raw JSON", icon: "📜" },
  ];

  for (const tab of tabs) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = `nav-tab ${store.activeTab === tab.id ? "active" : ""}`;
    btn.onclick = () => store.setTab(tab.id);

    const iconSpan = document.createElement("span");
    iconSpan.className = "nav-tab-icon";
    iconSpan.textContent = tab.icon;

    const labelSpan = document.createElement("span");
    labelSpan.className = "nav-tab-label";
    labelSpan.textContent = tab.label;

    btn.appendChild(iconSpan);
    btn.appendChild(labelSpan);

    if (tab.badge !== undefined) {
      const badgeSpan = document.createElement("span");
      badgeSpan.className = `nav-tab-badge ${
        tab.id === "diagnostics" && errorCount > 0
          ? "badge-error"
          : tab.id === "diagnostics" && warnCount > 0
            ? "badge-warn"
            : ""
      }`;
      badgeSpan.textContent = String(tab.badge);
      btn.appendChild(badgeSpan);
    }

    navBar.appendChild(btn);
  }

  container.appendChild(header);
  container.appendChild(navBar);
}
