import type { FactoryHost } from "../core/FactoryHost";
import type { FactorySnapshot, WorldParams } from "../core/types";
import { required } from "../ui/dom";
import { WorldParameterForm } from "../ui/worldParameters";
import {
  applyChanges,
  PREVIEW_HEIGHT,
  PREVIEW_WIDTH,
  WorldPreviewPanel,
  type PreviewItemLook,
  type RepairChoice,
} from "../ui/worldPreview";

interface WorldSetupActions {
  /** A worker fault, reported wherever the rest of the shell reports one. */
  reportWorkerError(error: unknown): void;
  /** Refresh the save catalogue's line under Continue, which the title screen shows on open. */
  refreshContinue(): void;
}

/**
 * Owns choosing a world: both mountings of the parameter form, both preview panels, the seed and
 * scenario controls the title screen and the session rail share, and the title screen itself.
 *
 * Nothing here starts a run. The screen reports what the player picked and the composition root
 * decides what to do with it, so the one place that begins, loads, and names a session stays one
 * place.
 */
export class WorldSetup {
  readonly #host: FactoryHost;
  readonly #canvas: HTMLElement;
  #actions: WorldSetupActions | null = null;

  readonly #scenarioInput = required<HTMLSelectElement>("scenario");
  readonly #seedInput = required<HTMLInputElement>("seed");
  readonly #presetInput = required<HTMLSelectElement>("world-preset");
  readonly #presetDescription = required<HTMLParagraphElement>(
    "world-preset-description",
  );
  readonly #screen = required<HTMLElement>("title-screen");
  readonly #tabSaves = required<HTMLButtonElement>("title-tab-saves");
  readonly #tabNew = required<HTMLButtonElement>("title-tab-new");
  readonly #resume = required<HTMLButtonElement>("title-resume");
  readonly #savesView = required<HTMLElement>("title-saves-view");
  readonly #newGameView = required<HTMLElement>("title-new-game-view");
  readonly #saveNameInput = required<HTMLInputElement>("title-save-name");
  readonly #creativeInput = required<HTMLInputElement>("title-creative");
  readonly #creativeNote = required<HTMLParagraphElement>(
    "title-creative-note",
  );
  readonly #titleSeedInput = required<HTMLInputElement>("title-seed");
  readonly #titlePresetInput =
    required<HTMLSelectElement>("title-world-preset");
  readonly #titlePresetDescription = required<HTMLParagraphElement>(
    "title-world-preset-description",
  );

  /** What Start scenario will generate. Native validates it again on arrival. */
  #pending: WorldParams | null = null;
  #previewTimer: number | undefined;
  #previewTicket = 0;

  readonly #panels: WorldPreviewPanel[];
  readonly #forms: WorldParameterForm[];
  readonly #customOptions: HTMLOptionElement[];
  readonly #scenarioChoices = new Map<string, HTMLInputElement>();

  constructor(host: FactoryHost, canvas: HTMLElement) {
    this.#host = host;
    this.#canvas = canvas;

    for (const preset of host.worldPresets) {
      for (const select of [this.#presetInput, this.#titlePresetInput]) {
        const option = document.createElement("option");
        option.value = preset.key;
        option.textContent = preset.name;
        select.append(option);
      }
    }
    // A hand-edited parameter set is no preset, and saying so is what keeps the picker honest about
    // what is about to be generated.
    this.#customOptions = [this.#presetInput, this.#titlePresetInput].map(
      (select) => {
        const option = document.createElement("option");
        option.value = "custom";
        option.textContent = "Custom";
        option.hidden = true;
        select.append(option);
        return option;
      },
    );

    // Built once and only ever written to. A form rebuilt under a pointer loses the control it was
    // rebuilt for, which is the same rule the catalogue and the research list live under. The two
    // mountings are the same form: neither owns the values, both report a whole set, and both are
    // shown whatever the other reported.
    const look = (itemId: number): PreviewItemLook | undefined =>
      this.#itemLook(itemId);
    const request = (): void => this.requestPreview();
    const repair = (choice: RepairChoice): void => this.#applyRepair(choice);
    this.#panels = [
      new WorldPreviewPanel("world-preview", look, request, repair),
      new WorldPreviewPanel("title-world-preview", look, request, repair),
    ];
    this.#forms = [
      new WorldParameterForm(
        required<HTMLDivElement>("world-parameter-fields"),
        "world-param",
        (next) => this.showParams(next),
        this.#panels[0],
      ),
      new WorldParameterForm(
        required<HTMLDivElement>("title-world-parameter-fields"),
        "title-world-param",
        (next) => this.showParams(next),
        this.#panels[1],
      ),
    ];

    this.#buildScenarioCards();
    this.#listen();
  }

  bind(actions: WorldSetupActions): void {
    this.#actions = actions;
  }

  /** The parameters Start will hand to native, or nothing while the first read is still in flight. */
  get params(): WorldParams | null {
    return this.#pending;
  }

  /** The scenario the title screen is offering, falling back to the session rail's own pick. */
  get scenario(): string {
    for (const [key, choice] of this.#scenarioChoices) {
      if (choice.checked) return key;
    }
    return this.#scenarioInput.value;
  }

  /** The scenario the session rail is offering. A restart from inside a run uses this one. */
  get sessionScenario(): string {
    return this.#scenarioInput.value;
  }

  get creative(): boolean {
    return this.#creativeInput.checked;
  }

  /** A typed save name, or the empty string when the player left the naming to the shell. */
  get saveName(): string {
    return this.#saveNameInput.value.trim();
  }

  /** The title screen's seed, or nothing when the field does not name one native could take. */
  get seed(): number | undefined {
    return exactSeed(this.#titleSeedInput.value);
  }

  /** The session rail's seed, read the same way. */
  get sessionSeed(): number | undefined {
    return exactSeed(this.#seedInput.value);
  }

  isOpen(): boolean {
    return this.#screen.classList.contains("open");
  }

  open(): void {
    this.#screen.classList.add("open");
    this.#resume.hidden = false;
    this.#setTitleOpen(true);
    // A blank field means "name this one for me". Carrying the running factory's name over would
    // make the obvious next click overwrite the save the player just walked away from.
    this.#saveNameInput.value = "";
    this.#showCreativeNote();
    this.#actions?.refreshContinue();
    // The panels are built at boot but only raster while they are on screen, so opening the screen is
    // the moment the first picture can be drawn.
    this.requestPreview();
  }

  close(): void {
    this.#screen.classList.remove("open");
    this.#resume.hidden = false;
    this.#setTitleOpen(false);
    this.#canvas.focus();
  }

  /** Escape leaves a screen the player can leave. It says whether it took the key. */
  dismiss(): boolean {
    if (!this.isOpen() || this.#resume.hidden) return false;
    this.close();
    return true;
  }

  showTab(tab: "saves" | "new"): void {
    const showSaves = tab === "saves";
    this.#tabSaves.classList.toggle("active", showSaves);
    this.#tabSaves.setAttribute("aria-selected", String(showSaves));
    this.#tabNew.classList.toggle("active", !showSaves);
    this.#tabNew.setAttribute("aria-selected", String(!showSaves));
    this.#savesView.hidden = !showSaves;
    this.#savesView.classList.toggle("active", showSaves);
    this.#newGameView.hidden = showSaves;
    this.#newGameView.classList.toggle("active", !showSaves);
    if (!showSaves) this.requestPreview();
  }

  /** Show the world a snapshot came from, read back from native rather than remembered. */
  showSession(next: FactorySnapshot): void {
    this.#scenarioInput.value = next.scenario;
    this.#showScenario(next.scenario);
    this.#seedInput.value = String(next.seed);
    this.#titleSeedInput.value = String(next.seed);
    this.#creativeInput.checked = next.player.creative;
    this.#showCreativeNote();
    void this.#syncFromWorld();
  }

  showParams(params: WorldParams): void {
    this.#pending = params;
    for (const form of this.#forms) form.setValues(params);
    const preset = this.#host.presetKeyFor(params);
    for (const option of this.#customOptions)
      option.hidden = preset !== undefined;
    this.#presetInput.value = preset ?? "custom";
    this.#titlePresetInput.value = preset ?? "custom";
    const description =
      this.#host.worldPresets.find(
        (entry) => entry.key === (preset ?? "custom"),
      )?.description ?? "Hand-tuned parameters.";
    this.#presetDescription.textContent = description;
    this.#titlePresetDescription.textContent = description;
    this.requestPreview();
  }

  /**
   * Redraw the preview, at most once per idle moment.
   *
   * Debounced because a slider drag is a stream of edits and each one is a raster, and ticketed
   * because the worker answers in order but a drag can outrun it — a picture that arrives after the
   * parameters moved on is a picture of a world the player has already left.
   */
  requestPreview(): void {
    if (this.#previewTimer !== undefined) clearTimeout(this.#previewTimer);
    this.#previewTimer = window.setTimeout(() => {
      void this.#draw();
    }, 120);
  }

  /** What the preview panel needs from an item, looked up once per draw rather than kept in a copy. */
  #itemLook(itemId: number): PreviewItemLook | undefined {
    const item = this.#host.definitions.items.find(
      (entry) => entry.id === itemId,
    );
    return item ? { name: item.name, color: item.color } : undefined;
  }

  /**
   * The seed the preview draws, read from the same field the Start button reads. A world is its
   * parameters *and* its seed, so a preview of a different seed would be a picture of a world nobody
   * is about to generate.
   */
  #previewSeed(): number {
    const parsed = Number(this.#titleSeedInput.value);
    return Number.isFinite(parsed)
      ? Math.abs(Math.trunc(parsed)) % 4294967296
      : 0;
  }

  async #draw(): Promise<void> {
    const params = this.#pending;
    if (!params) return;
    // Both forms exist from boot and only one is ever shown, so rastering for the hidden one would be
    // asking the generator to draw a picture nobody is looking at.
    const panels = this.#panels.filter((panel) => panel.visible);
    if (panels.length === 0) return;
    this.#previewTicket += 1;
    const ticket = this.#previewTicket;
    const seed = this.#previewSeed();
    // Asked for together rather than one after the other. The worker runs a queue either way, so this
    // costs nothing extra — but it puts every panel of a request in front of the next request, where a
    // sequential loop would let a slider drag keep starving whichever panel came last.
    await Promise.all(
      panels.map(async (panel) => {
        try {
          const preview = await this.#host.worldPreview(
            params,
            seed,
            PREVIEW_WIDTH,
            PREVIEW_HEIGHT,
            panel.hexesAcross,
          );
          if (ticket === this.#previewTicket) panel.draw(preview, params);
        } catch (error) {
          if (ticket !== this.#previewTicket) return;
          // Native refuses a set the Start button would also refuse, so this is the panel saying what
          // is wrong with the parameters rather than the host reporting a worker fault.
          panel.showError(
            error instanceof Error ? error.message : String(error),
          );
        }
      }),
    );
  }

  /**
   * Scenario as a card each rather than a dropdown. The shipped list already carries a sentence
   * about every scenario, and a bare name does not tell a first-time player what they are choosing.
   * The session panel keeps its select — that one is a running game's control, not a first
   * impression, and the two stay in step through the handlers below.
   */
  #buildScenarioCards(): void {
    const host = required<HTMLDivElement>("title-scenario-choices");
    for (const scenario of this.#host.scenarios.scenarios) {
      const card = document.createElement("label");
      card.className = "choice-card";
      const choice = document.createElement("input");
      choice.type = "radio";
      choice.name = "title-scenario";
      choice.value = scenario.key;
      choice.checked = scenario.key === this.#scenarioInput.value;
      choice.addEventListener("change", () => {
        if (choice.checked) this.#scenarioInput.value = scenario.key;
      });
      const body = document.createElement("span");
      body.className = "choice-card-body";
      const name = document.createElement("strong");
      name.textContent = scenario.name;
      const note = document.createElement("small");
      note.textContent = scenario.description;
      body.append(name, note);
      card.append(choice, body);
      host.append(card);
      this.#scenarioChoices.set(scenario.key, choice);
    }
  }

  #showScenario(key: string): void {
    for (const [candidate, choice] of this.#scenarioChoices) {
      choice.checked = candidate === key;
    }
  }

  /**
   * Apply a repair the preview offered. Both halves are already verified against a real bootstrap
   * pass; this is only how they land on the same fields the player already has.
   */
  #applyRepair(choice: RepairChoice): void {
    if (choice.kind === "seed") {
      this.#seedInput.value = String(choice.seed);
      this.#titleSeedInput.value = String(choice.seed);
      this.requestPreview();
      return;
    }
    if (!this.#pending) return;
    this.showParams(applyChanges(this.#pending, choice.changes));
  }

  async #syncFromWorld(): Promise<void> {
    try {
      this.showParams(structuredClone(await this.#host.worldParams()));
    } catch (error) {
      this.#actions?.reportWorkerError(error);
    }
  }

  /**
   * What the mode switch is promising, in the present tense. The card's copy explains what creative
   * does; this line says what the player is currently choosing, which is the part that changes.
   */
  #showCreativeNote(): void {
    this.#creativeNote.textContent = this.#creativeInput.checked
      ? "Creative run: the clock still counts, but the run is marked as not comparable and earns no achievements."
      : "Standard run: everything is built and earned, and the run time counts.";
  }

  // The top bar belongs to a running factory. Behind the title screen it is a strip of controls for a
  // game the player has not chosen yet, so the shell drops the row entirely rather than dimming it —
  // the renderer watches the canvas for resizes, so the reclaimed height is picked up on its own.
  #setTitleOpen(open: boolean): void {
    document.body.classList.toggle("title-open", open);
  }

  /** Reseat the form on whichever preset the given select names, or the first one shipped. */
  #resetToPreset(select: HTMLSelectElement): void {
    const preset =
      this.#host.worldPresets.find((entry) => entry.key === select.value) ??
      this.#host.worldPresets[0];
    if (preset) this.showParams(structuredClone(preset.params));
  }

  #listen(): void {
    for (const select of [this.#presetInput, this.#titlePresetInput]) {
      select.addEventListener("change", () => {
        const preset = this.#host.worldPresets.find(
          (entry) => entry.key === select.value,
        );
        if (preset) this.showParams(structuredClone(preset.params));
      });
    }
    required<HTMLButtonElement>("world-parameters-reset").addEventListener(
      "click",
      () => this.#resetToPreset(this.#presetInput),
    );
    required<HTMLButtonElement>(
      "title-world-parameters-reset",
    ).addEventListener("click", () =>
      this.#resetToPreset(this.#titlePresetInput),
    );

    this.#scenarioInput.addEventListener("input", () => {
      this.#showScenario(this.#scenarioInput.value);
    });
    this.#seedInput.addEventListener("input", () => {
      this.#titleSeedInput.value = this.#seedInput.value;
      this.requestPreview();
    });
    this.#titleSeedInput.addEventListener("input", () => {
      this.#seedInput.value = this.#titleSeedInput.value;
      this.requestPreview();
    });
    required<HTMLButtonElement>("title-seed-random").addEventListener(
      "click",
      () => {
        const randomized = Math.floor(Math.random() * 4294967295);
        this.#seedInput.value = String(randomized);
        this.#titleSeedInput.value = String(randomized);
        this.requestPreview();
      },
    );
    this.#creativeInput.addEventListener("change", () =>
      this.#showCreativeNote(),
    );
    this.#tabSaves.addEventListener("click", () => this.showTab("saves"));
    this.#tabNew.addEventListener("click", () => this.showTab("new"));
    this.#resume.addEventListener("click", () => this.close());
  }
}

/** A seed native will take verbatim, or nothing — a rejected field means "choose one for me". */
function exactSeed(value: string): number | undefined {
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed >= 0 && parsed <= 0xffffffff
    ? parsed
    : undefined;
}
