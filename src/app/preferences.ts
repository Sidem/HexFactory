import type { FeedbackAudio } from "../audio/feedback";
import type {
  FactoryRenderer,
  GraphicsProfile,
} from "../rendering/FactoryRenderer";
import {
  GRAPHICS_STORAGE_KEY,
  parseGraphicsProfile,
} from "../rendering/three/quality";
import { part, required } from "../ui/dom";

const MOTION_KEY = "hexfactory:reduced-motion:v1";

/** Owns the duplicated title/session preference controls and their persistence. */
export class PreferencesController {
  readonly #soundButton = required<HTMLButtonElement>("sound");
  readonly #muteInput = required<HTMLInputElement>("mute");
  readonly #reduceMotionInput = required<HTMLInputElement>("reduce-motion");
  readonly #graphicsProfileInput =
    required<HTMLSelectElement>("graphics-profile");
  readonly #titleMuteInput = required<HTMLInputElement>("title-mute");
  readonly #titleReduceMotionInput = required<HTMLInputElement>(
    "title-reduce-motion",
  );
  readonly #titleGraphicsProfileInput = required<HTMLSelectElement>(
    "title-graphics-profile",
  );

  constructor(
    private readonly audio: FeedbackAudio,
    private readonly renderer: FactoryRenderer,
  ) {
    this.#soundButton.addEventListener("click", () => this.toggleMuted());
    this.#muteInput.addEventListener("change", () =>
      this.setMuted(this.#muteInput.checked),
    );
    this.#titleMuteInput.addEventListener("change", () =>
      this.setMuted(this.#titleMuteInput.checked),
    );
    this.#reduceMotionInput.addEventListener("change", () =>
      this.setReducedMotion(this.#reduceMotionInput.checked),
    );
    this.#titleReduceMotionInput.addEventListener("change", () =>
      this.setReducedMotion(this.#titleReduceMotionInput.checked),
    );
    this.#graphicsProfileInput.addEventListener("change", () =>
      this.#readProfile(this.#graphicsProfileInput),
    );
    this.#titleGraphicsProfileInput.addEventListener("change", () =>
      this.#readProfile(this.#titleGraphicsProfileInput),
    );
  }

  applyInitial(profile: GraphicsProfile): void {
    this.setMuted(this.audio.isMuted);
    this.setReducedMotion(this.#loadReducedMotion());
    this.setGraphicsProfile(profile);
  }

  toggleMuted(): void {
    this.setMuted(!this.audio.isMuted);
  }

  setMuted(value: boolean): void {
    this.audio.setMuted(value);
    this.#muteInput.checked = value;
    this.#titleMuteInput.checked = value;
    part(this.#soundButton, ".utility-icon").textContent = value ? "♪̸" : "♪";
    part(this.#soundButton, ".utility-label").textContent = value
      ? "Muted"
      : "Sound";
    this.#soundButton.setAttribute("aria-pressed", String(!value));
    this.#soundButton.setAttribute(
      "aria-label",
      value ? "Unmute feedback sounds" : "Mute feedback sounds",
    );
    this.#soundButton.title = value
      ? "Unmute feedback sounds (M)"
      : "Mute feedback sounds (M)";
  }

  setReducedMotion(value: boolean): void {
    this.#reduceMotionInput.checked = value;
    this.#titleReduceMotionInput.checked = value;
    this.renderer.setReducedMotion(value);
    this.#store(MOTION_KEY, value ? "1" : "0");
  }

  setGraphicsProfile(value: GraphicsProfile): void {
    this.#graphicsProfileInput.value = value;
    this.#titleGraphicsProfileInput.value = value;
    this.renderer.setGraphicsProfile(value);
    this.#store(GRAPHICS_STORAGE_KEY, value);
  }

  #readProfile(input: HTMLSelectElement): void {
    const profile = parseGraphicsProfile(input.value);
    if (profile) this.setGraphicsProfile(profile);
  }

  #loadReducedMotion(): boolean {
    try {
      return window.localStorage.getItem(MOTION_KEY) === "1";
    } catch {
      return false;
    }
  }

  #store(key: string, value: string): void {
    try {
      window.localStorage.setItem(key, value);
    } catch {
      // A blocked preference does not interrupt the running factory.
    }
  }
}
