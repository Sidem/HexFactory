import {
  compatibility,
  describeMismatches,
  formatConfig,
  formatSavedAt,
  formatVersions,
  parseHxf1,
  slotsNewestFirst,
  type CurrentBuild,
  type SaveSlot,
} from "../core/saveSlots";
import { part, syncChildren } from "./dom";

/** Key and refresh one save list without rebuilding its controls. */
export function paintSaveSlotList(
  board: HTMLElement,
  slots: SaveSlot[],
  build: CurrentBuild,
  rowClass: string,
  selectedId: string | null,
): void {
  const ordered = slotsNewestFirst(slots);
  const rows = syncChildren(
    board,
    ordered.map((slot) => slot.id),
    () => {
      const row = document.createElement("li");
      row.className = rowClass;
      row.innerHTML = `<button type="button" class="save-slot-select"><strong></strong><span class="save-slot-when"></span><span class="save-slot-config"></span><span class="save-slot-versions"></span><span class="save-slot-issue"></span></button><div class="save-slot-actions"><button type="button" class="save-slot-load">Load</button><button type="button" class="save-slot-export">Export</button><button type="button" class="save-slot-delete">Delete</button></div>`;
      return row;
    },
  );
  ordered.forEach((slot, index) => {
    const row = rows[index];
    if (!row) return;
    const envelope = parseHxf1(slot.payload);
    const check = envelope
      ? compatibility(envelope, build)
      : {
          compatible: false,
          mismatches: [
            {
              field: "save" as const,
              expected: "a readable HXF1 file",
              found: "unreadable",
            },
          ],
        };
    row.classList.toggle("selected", slot.id === selectedId);
    row.classList.toggle("incompatible", !check.compatible);
    part(row, "strong").textContent = slot.name;
    part(row, ".save-slot-when").textContent = formatSavedAt(slot.savedAt);
    part(row, ".save-slot-config").textContent = formatConfig(slot.config);
    part(row, ".save-slot-versions").textContent = formatVersions(
      slot.versions,
    );
    part(row, ".save-slot-issue").textContent = check.compatible
      ? ""
      : describeMismatches(check.mismatches);
    const select = part<HTMLButtonElement>(row, ".save-slot-select");
    select.dataset.slotId = slot.id;
    select.setAttribute("aria-pressed", String(slot.id === selectedId));
    select.setAttribute("aria-label", `Select save ${slot.name}`);
    const load = part<HTMLButtonElement>(row, ".save-slot-load");
    load.dataset.slotId = slot.id;
    load.disabled = !check.compatible;
    load.setAttribute("aria-label", `Load ${slot.name}`);
    const exported = part<HTMLButtonElement>(row, ".save-slot-export");
    exported.dataset.slotId = slot.id;
    exported.setAttribute("aria-label", `Export ${slot.name}`);
    const remove = part<HTMLButtonElement>(row, ".save-slot-delete");
    remove.dataset.slotId = slot.id;
    remove.setAttribute("aria-label", `Delete ${slot.name}`);
  });
}
