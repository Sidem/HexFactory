import type { Runtime } from "./runtime";
import { required } from "../ui/dom";

/** A herd selection follows its stable id, even when its centre leaves the selected hex. */
export function renderHerdInspector(app: Runtime): boolean {
  let panel = document.getElementById("inspect-herd");
  const herd = app.snapshot?.herds.find((h) => h.id === app.selectedHerd);
  if (!herd) {
    if (panel) panel.hidden = true;
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
    hunt.onclick = () => {
      if (app.selectedHerd !== null)
        app.enqueue({ type: "hunt_herd", herd_id: app.selectedHerd });
    };
    cancel.onclick = () => app.enqueue({ type: "cancel_hunt" });
    panel.append(note, hunt, cancel);
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
      ? " No breeding pair; recovery needs migrants from a nearby refuge."
      : "");
  const hunt = required<HTMLButtonElement>("inspect-herd-hunt");
  hunt.disabled = herd.hunt_tick > 0;
  hunt.textContent =
    herd.hunt_tick > 0 ? "Hunting — hold still" : "Hunt herd (2 s)";
  required<HTMLButtonElement>("inspect-herd-cancel").hidden =
    herd.hunt_tick === 0;
  return true;
}
