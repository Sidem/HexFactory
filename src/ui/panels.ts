const PANEL_KEY = "hexfactory:panels:v1";

/** Owns presentation-only workspace state and its DOM/storage synchronization. */
export class PanelController {
  constructor(
    private readonly root: Document,
    private readonly storage: Storage,
  ) {}

  bind(): void {
    for (const toggle of this.root.querySelectorAll<HTMLButtonElement>(
      ".panel-toggle",
    )) {
      toggle.addEventListener("click", () =>
        this.toggle(toggle.dataset.panelTarget ?? ""),
      );
    }
    for (const close of this.root.querySelectorAll<HTMLButtonElement>(
      ".panel-close",
    )) {
      close.addEventListener("click", () => {
        close.closest<HTMLElement>(".glass-panel")?.classList.remove("open");
        this.syncAndSave();
      });
    }
  }

  toggle(id: string): void {
    const target = this.root.getElementById(id);
    if (!target) return;
    const opening = !target.classList.contains("open");
    if (opening) this.close(target);
    target.classList.toggle("open", opening);
    this.syncAndSave();
  }

  close(except?: HTMLElement): void {
    let changed = false;
    for (const panel of this.root.querySelectorAll<HTMLElement>(
      ".glass-panel.open",
    )) {
      if (panel === except) continue;
      panel.classList.remove("open");
      changed = true;
    }
    if (changed) this.syncAndSave();
  }

  restore(): void {
    let stored: unknown;
    try {
      stored = JSON.parse(this.storage.getItem(PANEL_KEY) ?? "[]");
    } catch {
      return;
    }
    if (!Array.isArray(stored)) return;
    const ids = stored.filter(
      (id): id is string =>
        typeof id === "string" &&
        this.root.getElementById(id)?.classList.contains("glass-panel") ===
          true,
    );
    for (const id of ids.slice(-1))
      this.root.getElementById(id)?.classList.add("open");
    this.syncToggles();
  }

  private openIds(): string[] {
    return [
      ...this.root.querySelectorAll<HTMLElement>(".glass-panel.open"),
    ].map(({ id }) => id);
  }

  private syncAndSave(): void {
    this.syncToggles();
    try {
      this.storage.setItem(PANEL_KEY, JSON.stringify(this.openIds()));
    } catch {
      // A browser with storage refused opens panels fresh; the session still works.
    }
  }

  private syncToggles(): void {
    for (const toggle of this.root.querySelectorAll<HTMLButtonElement>(
      ".panel-toggle",
    )) {
      const target = this.root.getElementById(toggle.dataset.panelTarget ?? "");
      toggle.setAttribute(
        "aria-expanded",
        String(target?.classList.contains("open") ?? false),
      );
    }
  }
}
