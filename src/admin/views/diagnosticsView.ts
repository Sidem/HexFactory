import type { AdminStore } from "../state";
import { showToast } from "../toast";

export function renderDiagnosticsView(
  container: HTMLElement,
  store: AdminStore,
): void {
  container.innerHTML = "";

  const view = document.createElement("div");
  view.className = "admin-view diagnostics-view";

  const issues = store.diagnostics;
  const errorCount = issues.filter((i) => i.severity === "error").length;
  const warnCount = issues.filter((i) => i.severity === "warning").length;

  // Hero Status Card
  const hero = document.createElement("div");
  hero.className = `diag-hero-card ${errorCount > 0 ? "hero-error" : warnCount > 0 ? "hero-warning" : "hero-healthy"}`;

  if (errorCount === 0 && warnCount === 0) {
    hero.innerHTML = `
      <div class="hero-icon">✓</div>
      <div class="hero-content">
        <h2>All Systems Nominal &amp; Valid</h2>
        <p>Definitions, recipe balances, building footprint rules, and technology graphs strictly adhere to HexFactory native and browser core requirements.</p>
      </div>
      <button type="button" class="btn btn-sm" id="rerun-btn">Re-verify</button>
    `;
  } else {
    hero.innerHTML = `
      <div class="hero-icon">${errorCount > 0 ? "✕" : "⚠"}</div>
      <div class="hero-content">
        <h2>${errorCount > 0 ? `${errorCount} Critical Error${errorCount > 1 ? "s" : ""}` : "Validation Warnings"} Detected</h2>
        <p>${
          errorCount > 0
            ? "These errors will cause the native Rust game simulation or TypeScript loader to refuse initialization. Review and resolve each issue below."
            : "The game will load, but these balance or balance completeness warnings may affect progression."
        }</p>
      </div>
      <button type="button" class="btn btn-sm" id="rerun-btn">Re-verify</button>
    `;
  }

  hero.querySelector("#rerun-btn")?.addEventListener("click", () => {
    store.recomputeDiagnostics();
    showToast("Diagnostics re-computed", "info");
    renderDiagnosticsView(container, store);
  });

  view.appendChild(hero);

  // Issues List
  if (issues.length > 0) {
    const listSection = document.createElement("div");
    listSection.className = "diag-issues-section";
    listSection.innerHTML = `<h3>Reported Diagnostics (${issues.length})</h3>`;

    const list = document.createElement("div");
    list.className = "diag-issues-list";

    for (const issue of issues) {
      const row = document.createElement("div");
      row.className = `diag-issue-row issue-${issue.severity}`;

      row.innerHTML = `
        <div class="issue-badge">${issue.severity === "error" ? "✕ ERROR" : "⚠ WARN"}</div>
        <div class="issue-category">${issue.category}</div>
        <div class="issue-message">${issue.message}</div>
        <div class="issue-action">
          ${
            issue.entity !== "general" && issue.entityId !== undefined
              ? `<button type="button" class="btn btn-sm btn-subtle fix-btn">Fix ${issue.entity} #${issue.entityId}</button>`
              : ""
          }
        </div>
      `;

      row.querySelector(".fix-btn")?.addEventListener("click", () => {
        if (issue.entity === "item" && issue.entityId !== undefined) {
          const item = store.definitions.items.find(
            (i) => i.id === issue.entityId,
          );
          if (item) {
            store.activeTab = "items";
            store.setEditingTarget({
              type: "item",
              data: structuredClone(item),
              isNew: false,
            });
          }
        } else if (issue.entity === "recipe" && issue.entityId !== undefined) {
          const recipe = store.definitions.recipes.find(
            (r) => r.id === issue.entityId,
          );
          if (recipe) {
            store.activeTab = "recipes";
            store.setEditingTarget({
              type: "recipe",
              data: structuredClone(recipe),
              isNew: false,
            });
          }
        } else if (
          issue.entity === "building" &&
          issue.entityId !== undefined
        ) {
          const bld = store.definitions.buildings.find(
            (b) => b.id === issue.entityId,
          );
          if (bld) {
            store.activeTab = "buildings";
            store.setEditingTarget({
              type: "building",
              data: structuredClone(bld),
              isNew: false,
            });
          }
        } else if (issue.entity === "request" && issue.entityId !== undefined) {
          const req = store.definitions.requests.find(
            (r) => r.id === issue.entityId,
          );
          if (req) {
            store.activeTab = "requests";
            store.setEditingTarget({
              type: "request",
              data: structuredClone(req),
              isNew: false,
            });
          }
        } else if (
          issue.entity === "technology" &&
          issue.entityId !== undefined
        ) {
          const tech = store.technologies.technologies.find(
            (t) => t.id === issue.entityId,
          );
          if (tech) {
            store.activeTab = "technologies";
            store.setEditingTarget({
              type: "technology",
              data: structuredClone(tech),
              isNew: false,
            });
          }
        }
      });

      list.appendChild(row);
    }

    listSection.appendChild(list);
    view.appendChild(listSection);
  }

  container.appendChild(view);
}
