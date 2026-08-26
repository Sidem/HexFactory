import {
  copyToClipboard,
  downloadJsonFile,
  formatDefinitionsJson,
  formatScenariosJson,
  formatTechnologiesJson,
  parseImportedJson,
} from "../exporter";
import type { AdminStore } from "../state";
import { showToast } from "../toast";

type JsonFileType = "definitions" | "technologies" | "scenarios";

export function renderRawJsonView(
  container: HTMLElement,
  store: AdminStore,
): void {
  container.innerHTML = "";

  const view = document.createElement("div");
  view.className = "admin-view raw-json-view";

  let activeFile: JsonFileType = "definitions";

  // Sub-tabs
  const tabsRow = document.createElement("div");
  tabsRow.className = "json-subtabs";

  const files: Array<{ id: JsonFileType; label: string; filename: string }> = [
    {
      id: "definitions",
      label: "definitions.json",
      filename: "definitions.json",
    },
    {
      id: "technologies",
      label: "technologies.json",
      filename: "technologies.json",
    },
    { id: "scenarios", label: "scenarios.json", filename: "scenarios.json" },
  ];

  for (const f of files) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = `subtab-btn ${activeFile === f.id ? "active" : ""}`;
    btn.textContent = f.label;
    btn.onclick = () => {
      activeFile = f.id;
      tabsRow
        .querySelectorAll(".subtab-btn")
        .forEach((b) => b.classList.remove("active"));
      btn.classList.add("active");
      updateEditorContent();
    };
    tabsRow.appendChild(btn);
  }

  view.appendChild(tabsRow);

  // Editor container
  const editorWrap = document.createElement("div");
  editorWrap.className = "json-editor-wrapper";

  const textarea = document.createElement("textarea");
  textarea.className = "json-textarea";
  textarea.spellcheck = false;

  editorWrap.appendChild(textarea);
  view.appendChild(editorWrap);

  // Actions row
  const actionsRow = document.createElement("div");
  actionsRow.className = "json-actions-row";

  const copyBtn = document.createElement("button");
  copyBtn.type = "button";
  copyBtn.className = "btn";
  copyBtn.innerHTML = `<span>📋</span> Copy to Clipboard`;
  copyBtn.onclick = async () => {
    const ok = await copyToClipboard(textarea.value);
    if (ok) showToast(`Copied ${activeFile}.json to clipboard`, "success");
  };
  actionsRow.appendChild(copyBtn);

  const formatBtn = document.createElement("button");
  formatBtn.type = "button";
  formatBtn.className = "btn";
  formatBtn.innerHTML = `<span>⚡</span> Format JSON`;
  formatBtn.onclick = () => {
    try {
      const parsed = JSON.parse(textarea.value);
      textarea.value = JSON.stringify(parsed, null, 2) + "\n";
      showToast("Formatted JSON", "info");
    } catch (err) {
      showToast(
        `Invalid JSON: ${err instanceof Error ? err.message : String(err)}`,
        "error",
      );
    }
  };
  actionsRow.appendChild(formatBtn);

  const downloadBtn = document.createElement("button");
  downloadBtn.type = "button";
  downloadBtn.className = "btn";
  downloadBtn.innerHTML = `<span>💾</span> Download File`;
  downloadBtn.onclick = () => {
    downloadJsonFile(`${activeFile}.json`, textarea.value);
    showToast(`Downloaded ${activeFile}.json`, "success");
  };
  actionsRow.appendChild(downloadBtn);

  const applyBtn = document.createElement("button");
  applyBtn.type = "button";
  applyBtn.className = "btn btn-primary";
  applyBtn.innerHTML = `<span>✓</span> Apply JSON Edits`;
  applyBtn.onclick = () => {
    const parsedRes = parseImportedJson(textarea.value);
    if (!parsedRes.success) {
      showToast(`Parse error: ${parsedRes.error}`, "error");
      return;
    }
    const data = parsedRes.data;

    try {
      if (activeFile === "definitions") {
        store.importDefinitions(
          data as Parameters<typeof store.importDefinitions>[0],
        );
        showToast("Applied edits to definitions.json", "success");
      } else if (activeFile === "technologies") {
        store.importTechnologies(
          data as Parameters<typeof store.importTechnologies>[0],
        );
        showToast("Applied edits to technologies.json", "success");
      } else if (activeFile === "scenarios") {
        store.scenarios = data as Parameters<
          typeof store.importDefinitions
        >[0] as unknown as typeof store.scenarios;
        showToast("Applied edits to scenarios.json", "success");
      }
    } catch (err) {
      showToast(
        `Application error: ${err instanceof Error ? err.message : String(err)}`,
        "error",
      );
    }
  };
  actionsRow.appendChild(applyBtn);

  view.appendChild(actionsRow);

  function updateEditorContent(): void {
    if (activeFile === "definitions") {
      textarea.value = formatDefinitionsJson(store.definitions);
    } else if (activeFile === "technologies") {
      textarea.value = formatTechnologiesJson(store.technologies);
    } else if (activeFile === "scenarios") {
      textarea.value = formatScenariosJson(store.scenarios);
    }
  }

  updateEditorContent();
  container.appendChild(view);
}
