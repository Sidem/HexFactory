const PANEL_KEY = "hexfactory:panels:v1";
/**
 * The one panel that stands on the world rather than waiting behind a key. Its id lives here rather
 * than at the call site so the key table remains the only place in `main.ts` that names a panel.
 */
const INSPECTOR = "inspector-panel";

/** Owns presentation-only workspace state and its DOM/storage synchronization. */
export class PanelController {
  private readonly dialogOpeners = new Map<string, HTMLElement>();
  constructor(
    private readonly root: Document,
    private readonly storage: Storage,
    private readonly onDialogChange?: (id: string, open: boolean) => void,
  ) {}

  bind(): void {
    for (const dialog of this.root.querySelectorAll<HTMLDialogElement>(
      "dialog.glass-panel",
    )) {
      dialog.addEventListener("keydown", (event) => {
        if (event.key !== "Escape") return;
        event.preventDefault();
        event.stopPropagation();
        dialog.classList.remove("open");
        this.syncAndSave();
      });
      dialog.addEventListener("cancel", (event) => {
        event.preventDefault();
        dialog.classList.remove("open");
        this.syncAndSave();
      });
      dialog.addEventListener("click", (event) => {
        const bounds = dialog.getBoundingClientRect();
        if (
          event.target !== dialog ||
          (event.clientX >= bounds.left &&
            event.clientX <= bounds.right &&
            event.clientY >= bounds.top &&
            event.clientY <= bounds.bottom)
        )
          return;
        dialog.classList.remove("open");
        this.syncAndSave();
      });
    }
    for (const toggle of this.root.querySelectorAll<HTMLButtonElement>(
      ".panel-toggle",
    )) {
      toggle.addEventListener("click", () =>
        this.toggle(toggle.dataset.panelTarget ?? "", toggle),
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

  toggle(id: string, opener?: HTMLElement): void {
    const target = this.root.getElementById(id);
    if (!target) return;
    const opening = !target.classList.contains("open");
    if (opening && target.tagName === "DIALOG") {
      const control =
        opener ??
        this.root.querySelector<HTMLElement>(
          `.panel-toggle[data-panel-target="${id}"]`,
        );
      if (control) this.dialogOpeners.set(id, control);
    }
    if (opening) this.close(target);
    target.classList.toggle("open", opening);
    this.syncAndSave();
  }

  /**
   * Bring the inspector out for something the world did rather than something the player pressed —
   * walking up to a machine. On a wide screen it already stands beside the world and this changes
   * nothing; on a narrow one it is behind its button, and this is what opens it.
   *
   * A workspace the player opened is left alone. One panel at a time is the rule, and a deliberate
   * press outranks a footstep: a build list should not close itself because the player walked past
   * a belt on the way to the hex they are about to build on.
   */
  revealInspector(): void {
    const target = this.root.getElementById(INSPECTOR);
    if (!target || this.root.querySelector(".glass-panel.open")) return;
    target.classList.add("open");
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
        this.root.getElementById(id)?.tagName !== "DIALOG" &&
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
    for (const dialog of this.root.querySelectorAll<HTMLDialogElement>(
      "dialog.glass-panel",
    )) {
      const open = dialog.classList.contains("open");
      if (dialog.open === open) continue;
      if (open) dialog.showModal();
      else {
        dialog.close();
        const opener = this.dialogOpeners.get(dialog.id);
        // Cross-links can live inside the panel that opening this dialog just hid.
        const visibleOpener = opener?.getClientRects().length
          ? opener
          : this.root.querySelector<HTMLElement>(
              `.panel-toggle[data-panel-target="${dialog.id}"]`,
            );
        visibleOpener?.focus({ preventScroll: true });
      }
      this.onDialogChange?.(dialog.id, open);
    }
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
