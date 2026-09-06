import type { Runtime } from "./runtime";
import { required } from "../ui/dom";
import { NativePreview } from "./nativePreview";
import type { HuntPreview } from "../core/types";

const previews = new WeakMap<Runtime, NativePreview<HuntPreview>>();

/** A herd selection follows its stable id, even when its centre leaves the selected hex. */
export function renderHerdInspector(app: Runtime): boolean {
  let panel = document.getElementById("inspect-herd");
  const herd = app.snapshot?.herds.find((h) => h.id === app.selectedHerd);
  if (!herd) {
    if (panel) panel.hidden = true;
    previews.get(app)?.clear();
    app.renderer.setHuntPreview?.(null);
    return false;
  }
  if (!panel) {
    panel = document.createElement("div");
    panel.id = "inspect-herd";
    const note = document.createElement("p");
    note.id = "inspect-herd-note";
    const hunt = document.createElement("button");
    hunt.id = "inspect-herd-hunt";
    const cancel = document.createElement("button");
    cancel.id = "inspect-herd-cancel";
    cancel.textContent = "Cancel hunt";
    const aim = document.createElement("p");
    aim.id = "inspect-herd-aim";
    const progress = document.createElement("progress");
    progress.id = "inspect-herd-progress";
    progress.setAttribute("aria-label", "Hunting aim");
    const drive = document.createElement("button");
    drive.textContent = "Drive herd away";
    drive.title =
      "Stand opposite the open gate, drive the herd, then step aside. Feed cannot override fear.";
    drive.onclick = () => {
      if (app.selectedHerd !== null)
        app.enqueue({ type: "drive_herd", herd_id: app.selectedHerd });
    };
    hunt.onclick = () => {
      if (app.selectedHerd !== null)
        app.enqueue({ type: "hunt_herd", herd_id: app.selectedHerd });
    };
    cancel.onclick = () => app.enqueue({ type: "cancel_hunt" });
    panel.append(note, aim, progress, hunt, cancel, drive);
    required<HTMLElement>("inspect-sheet").after(panel);
  }
  panel.hidden = false;
  required<HTMLElement>("inspect-sheet").hidden = true;
  required<HTMLElement>("inspect-empty").hidden = true;
  required<HTMLElement>("inspect-kicker").textContent = "Living herd";
  required<HTMLElement>("inspect-title").textContent = `Herd of ${herd.count}`;
  required<HTMLElement>("inspect-status").hidden = true;
  const moving = herd.from[0] !== herd.to[0] || herd.from[1] !== herd.to[1];
  const note =
    herd.drive === "Flee"
      ? "Alarmed — give the herd space. Feed cannot override fear."
      : herd.drive === "Thirst"
        ? moving
          ? "Thirsty — heading to a drinking bank."
          : "Water access blocked — open a gate or connect fresh water."
        : herd.drive === "Graze"
          ? moving
            ? "Hungry — moving to reachable grass or feed."
            : "Pasture recovering — bring cut grass or open fresh grazing."
          : "Resting — leave grass and a drinking route for the breeding pair.";
  required<HTMLElement>("inspect-herd-note").textContent =
    note +
    (herd.shortage_ticks >= 1000
      ? " Prolonged shortage: restore access before animals are lost."
      : "") +
    (herd.count < 2
      ? " No breeding pair; bring another herd before harvesting again."
      : "");
  const hunt = required<HTMLButtonElement>("inspect-herd-hunt");
  if (panel.dataset.herd !== String(herd.id)) {
    previews.get(app)?.clear();
    app.renderer.setHuntPreview?.(null);
    panel.dataset.herd = String(herd.id);
    hunt.disabled = true;
    required<HTMLElement>("inspect-herd-aim").textContent =
      "Checking hunting reach…";
  }
  hunt.textContent =
    herd.hunt_tick > 0 ? "Aiming — hold still" : "Aim and hunt one animal";
  required<HTMLButtonElement>("inspect-herd-cancel").hidden =
    herd.hunt_tick === 0;
  const remaining = Math.max(0, herd.hunt_tick - (app.snapshot?.tick ?? 0));
  const progress = required<HTMLProgressElement>("inspect-herd-progress");
  progress.hidden = remaining === 0;
  if (remaining > 0) {
    progress.max = 10;
    progress.value = 10 - remaining;
  }
  let preview = previews.get(app);
  if (!preview) {
    preview = new NativePreview();
    previews.set(app, preview);
  }
  // Snapshot identity includes player-only steps and boundary edits, not merely simulation ticks.
  preview.update(
    herd.id,
    () => app.host.huntPreview(herd.id),
    (value) => {
      if (app.selectedHerd !== value.herd_id) return;
      hunt.disabled = !value.ready;
      required<HTMLElement>("inspect-herd-aim").textContent =
        value.reason +
        (value.remaining_ticks
          ? ` · ${(value.remaining_ticks / 10).toFixed(1)} s left`
          : " · 1 s at normal simulation speed");
      const progress = required<HTMLProgressElement>("inspect-herd-progress");
      progress.hidden = value.remaining_ticks === 0;
      progress.max = value.duration_ticks;
      progress.value = value.duration_ticks - value.remaining_ticks;
      app.renderer.setHuntPreview?.(value);
    },
    (error) => {
      hunt.disabled = true;
      required<HTMLElement>("inspect-herd-aim").textContent = String(error);
    },
    app.snapshot,
  );
  return true;
}
