import type { Runtime } from "./runtime";
import type { PasturePreview } from "../core/types";
import { NativePreview } from "./nativePreview";
import { required } from "../ui/dom";

const previews = new WeakMap<Runtime, NativePreview<PasturePreview>>();

export function renderPastureInspector(app: Runtime): void {
  let panel = document.getElementById("inspect-pasture");
  const cell = app.selected;
  if (!cell || app.selectedHerd !== null) {
    if (panel) panel.hidden = true;
    previews.get(app)?.clear();
    app.renderer.setPastureWork?.(null);
    return;
  }
  if (!panel) {
    panel = document.createElement("div");
    panel.id = "inspect-pasture";
    const note = document.createElement("p");
    note.id = "inspect-pasture-note";
    const cut = document.createElement("button");
    cut.id = "inspect-pasture-cut";
    cut.textContent = "Cut and pack feed";
    cut.onclick = () => {
      if (app.selected) app.enqueue({ type: "cut_feed", ...app.selected });
    };
    const feed = document.createElement("button");
    feed.id = "inspect-pasture-feed";
    feed.onclick = () => {
      if (app.selected) app.enqueue({ type: "place_feed", ...app.selected });
    };
    panel.append(note, cut, feed);
    required<HTMLElement>("inspect-sheet").after(panel);
  }
  const current = panel;
  const cellKey = `${cell.q},${cell.r}`;
  if (current.dataset.cell !== cellKey) {
    previews.get(app)?.clear();
    current.dataset.cell = cellKey;
    current.hidden = true;
    app.renderer.setPastureWork?.(null);
  }
  let preview = previews.get(app);
  if (!preview) {
    preview = new NativePreview();
    previews.set(app, preview);
  }
  // Include the selection so two clicks between snapshots cannot reuse an unrelated preview.
  preview.update(
    cellKey,
    () => app.host.pasturePreview(cell.q, cell.r),
    (value) => {
      if (
        app.selected?.q !== value.q ||
        app.selected?.r !== value.r ||
        app.selectedHerd !== null
      )
        return;
      current.hidden =
        value.limit === 0 && !value.can_feed && !value.station_note;
      required<HTMLElement>("inspect-pasture-note").textContent =
        value.station_note ??
        `Pasture ${value.grass}/${value.limit}. ${value.reason}`;
      const cut = required<HTMLButtonElement>("inspect-pasture-cut");
      cut.disabled = !value.can_cut;
      cut.hidden = Boolean(value.station_note);
      const feed = required<HTMLButtonElement>("inspect-pasture-feed");
      feed.disabled = !value.can_feed;
      feed.hidden = Boolean(value.station_note);
      feed.textContent = `Place one feed · ${value.feed_held} packed`;
      app.renderer.setPastureWork?.(value.work_point);
    },
    () => {
      current.hidden = true;
    },
    app.snapshot,
  );
}
