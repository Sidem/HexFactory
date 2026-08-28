/**
 * The one prompt in the game that stops an edit to ask.
 *
 * Deliberately not a `PanelController` panel. A workspace is remembered and restored between
 * sessions; a question must never come back on its own, must never be one of several open at once,
 * and must never survive the thing it was asking about. So it owns its own `<dialog>`, opens with
 * `showModal`, and closes on the answer — nothing else in the game can leave it standing.
 *
 * Every exit resolves once. Native close events are asynchronous, so an old close event must never
 * cancel a newer question opened on the same element.
 */
export interface ConfirmRow {
  /** What the row says. Also its accessible text, so it must read on its own. */
  readonly text: string;
  /** Optional decoration — an item chip — painted into the row's leading slot. */
  readonly paint?: (host: HTMLElement) => void;
}

export interface ConfirmRequest {
  readonly title: string;
  /** What is at stake, itemised. Empty when there is nothing to itemise. */
  readonly rows?: readonly ConfirmRow[];
  /** One plain sentence naming the consequence, including anything on a timer. */
  readonly note: string;
  /** The label on the button that goes through with it. */
  readonly accept: string;
  /** The label on the button that does not. */
  readonly cancel: string;
}

export class ConfirmDialog {
  private readonly dialog: HTMLDialogElement;
  private readonly title: HTMLElement;
  private readonly contents: HTMLElement;
  private readonly note: HTMLElement;
  private readonly acceptButton: HTMLButtonElement;
  private readonly cancelButton: HTMLButtonElement;
  private pending: (() => void) | null = null;

  constructor(
    dialog: HTMLDialogElement,
    private readonly onOpen?: () => void,
  ) {
    this.dialog = dialog;
    this.title = this.part("#confirm-title");
    this.contents = this.part("#confirm-contents");
    this.note = this.part("#confirm-note");
    this.acceptButton = this.part<HTMLButtonElement>("#confirm-accept");
    this.cancelButton = this.part<HTMLButtonElement>("#confirm-cancel");
    this.acceptButton.addEventListener("click", () => {
      this.finish(true);
    });
    this.cancelButton.addEventListener("click", () => this.finish(false));
    this.dialog.addEventListener("cancel", (event) => {
      event.preventDefault();
      this.finish(false);
    });
    // Clicking the backdrop is a dismissal, exactly like `Escape`. The hit test is against the
    // dialog's own box because a modal `<dialog>`'s backdrop reports the dialog as its target.
    this.dialog.addEventListener("click", (event) => {
      if (event.target !== this.dialog) return;
      const bounds = this.dialog.getBoundingClientRect();
      const inside =
        event.clientX >= bounds.left &&
        event.clientX <= bounds.right &&
        event.clientY >= bounds.top &&
        event.clientY <= bounds.bottom;
      if (!inside) this.finish(false);
    });
    this.dialog.addEventListener("close", () => {
      if (!this.dialog.open) this.finish(false);
    });
  }

  get open(): boolean {
    return this.dialog.open;
  }

  /**
   * Ask, and run `onAccept` only if the player says yes.
   *
   * Focus lands on the accept button. This is a question the player asked for by pressing delete on
   * a building they chose, so the fast path — press, read, press again — stays two keys, and every
   * other key out of the dialog is a no.
   */
  ask(request: ConfirmRequest, onAccept: () => void): void {
    this.dismiss();
    this.pending = onAccept;
    this.title.textContent = request.title;
    this.note.textContent = request.note;
    const rows = request.rows ?? [];
    this.contents.hidden = rows.length === 0;
    this.contents.replaceChildren(
      ...rows.map((row) => {
        const item = document.createElement("li");
        const chip = document.createElement("span");
        chip.className = "confirm-chip chip-host";
        chip.setAttribute("aria-hidden", "true");
        row.paint?.(chip);
        const label = document.createElement("span");
        label.textContent = row.text;
        item.append(chip, label);
        return item;
      }),
    );
    this.acceptButton.textContent = request.accept;
    this.cancelButton.textContent = request.cancel;
    this.dialog.showModal();
    this.onOpen?.();
    this.acceptButton.focus({ preventScroll: true });
  }

  /** Take the question away because what it was asking about is gone. Counts as a no. */
  dismiss(): void {
    this.finish(false);
  }

  private finish(accepted: boolean): void {
    const run = accepted ? this.pending : null;
    this.pending = null;
    if (this.dialog.open) this.dialog.close();
    run?.();
  }

  private part<T extends HTMLElement = HTMLElement>(selector: string): T {
    const found = this.dialog.querySelector<T>(selector);
    if (!found) throw new Error(`confirmation dialog is missing ${selector}`);
    return found;
  }
}
