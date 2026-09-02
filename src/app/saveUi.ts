import {
  formatVersions,
  importLegacySlots,
  latestCompatible,
  readCatalog,
  removeSlot,
  writeCatalog,
  type CurrentBuild,
  type SaveSlot,
} from "../core/saveSlots";
import { required } from "../ui/dom";
import { paintSaveSlotList } from "../ui/saveList";

interface SaveUiActions {
  load(slot: SaveSlot): void;
  export(slot: SaveSlot): void;
  refresh(message?: string): void;
}

/** Owns save selection, both keyed lists, and their shared status controls. */
export class SaveUi {
  readonly #nameInput = required<HTMLInputElement>("save-name");
  readonly #sessionList = required<HTMLElement>("save-slots");
  readonly #titleList = required<HTMLElement>("title-save-slots");
  readonly #continue = required<HTMLButtonElement>("continue");
  readonly #titleContinue = required<HTMLButtonElement>("title-continue");
  readonly #titleContinueSub = required<HTMLElement>("title-continue-sub");
  readonly #titleSavesBadge = required<HTMLElement>("title-saves-badge");
  #selectedId: string | null = null;
  #actions: SaveUiActions | null = null;

  constructor() {
    this.#sessionList.addEventListener("click", (event) => this.#click(event));
    this.#titleList.addEventListener("click", (event) => this.#click(event));
  }

  bind(actions: SaveUiActions): void {
    this.#actions = actions;
  }

  get selectedId(): string | null {
    return this.#selectedId;
  }

  get name(): string {
    return this.#nameInput.value.trim();
  }

  setName(name: string): void {
    this.#nameInput.value = name;
  }

  select(slot: SaveSlot): void {
    this.#selectedId = slot.id;
    this.#nameInput.value = slot.name;
  }

  clearSelection(): void {
    this.#selectedId = null;
  }

  update(build: CurrentBuild, scenarioVersion: number, message?: string): void {
    let slots: SaveSlot[] = [];
    let imported = 0;
    let error: string | undefined;
    try {
      const pulled = importLegacySlots(localStorage, build);
      imported = pulled.imported;
      const read =
        imported > 0 ? { slots: pulled.slots } : readCatalog(localStorage);
      slots = read.slots;
      error = "error" in read ? read.error : undefined;
    } catch (caught) {
      error = `Save list failed: ${String(caught)}`;
    }
    const compatible = latestCompatible(slots, build);
    this.#continue.disabled = !compatible;
    this.#titleContinue.disabled = !compatible;
    this.#titleContinueSub.textContent = compatible
      ? `Restore “${compatible.name}”`
      : "No saved factory found";
    this.#titleSavesBadge.textContent = String(slots.length);
    paintSaveSlotList(
      this.#sessionList,
      slots,
      build,
      "save-slot",
      this.#selectedId,
    );
    paintSaveSlotList(
      this.#titleList,
      slots,
      build,
      "save-slot title-save-slot",
      this.#selectedId,
    );
    required<HTMLElement>("title-envelope-info").textContent =
      `Save ${build.versions.save} · Definitions ${build.versions.definitions} · World ${build.versions.world}`;
    const importedNote =
      imported > 0
        ? `Imported ${imported} previous run${imported === 1 ? "" : "s"} from an older slot. `
        : "";
    const titleStatus = required("title-save-status");
    titleStatus.textContent = message ?? error ?? "";
    titleStatus.hidden = !titleStatus.textContent;
    required<HTMLElement>("save-status").textContent =
      message ??
      importedNote +
        (error
          ? error
          : compatible
            ? `Continue loads “${compatible.name}”. This build is ${formatVersions({ ...build.versions, scenario: scenarioVersion })}.`
            : slots.length > 0
              ? "Saved runs are listed below. None of them can load in this build."
              : "No local save yet.");
  }

  #click(event: Event): void {
    const target = event.target as HTMLElement;
    const load = target.closest<HTMLButtonElement>(".save-slot-load");
    const exported = target.closest<HTMLButtonElement>(".save-slot-export");
    const remove = target.closest<HTMLButtonElement>(".save-slot-delete");
    const select = target.closest<HTMLButtonElement>(".save-slot-select");
    const id = (load ?? exported ?? remove ?? select)?.dataset.slotId;
    if (!id) return;
    const { slots, error } = readCatalog(localStorage);
    if (error) {
      this.#actions?.refresh(error);
      return;
    }
    const slot = slots.find((entry) => entry.id === id);
    if (!slot) return;
    if (load) return this.#actions?.load(slot);
    if (exported) return this.#actions?.export(slot);
    if (remove) {
      if (!window.confirm(`Delete “${slot.name}”? This cannot be undone.`))
        return;
      if (slot.sourceKey) localStorage.removeItem(slot.sourceKey);
      writeCatalog(localStorage, removeSlot(slots, slot.id));
      if (this.#selectedId === slot.id) {
        this.#selectedId = null;
        if (this.#nameInput.value === slot.name) this.#nameInput.value = "";
      }
      this.#actions?.refresh(`Deleted “${slot.name}”.`);
      return;
    }
    this.select(slot);
    this.#actions?.refresh();
  }
}
