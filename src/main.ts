import { rotateHexDirection, type HexDirection } from "@hexlife/embed/hex";

import { FactoryHost } from "./core/FactoryHost";
import type { FactorySnapshot } from "./core/types";
import { CanvasFactoryRenderer } from "./rendering/CanvasFactoryRenderer";
import "./styles.css";

type Tool = "inspect" | "erase" | "rotate" | number;

const canvas = required<HTMLCanvasElement>("factory-canvas");
const playButton = required<HTMLButtonElement>("play");
const stepButton = required<HTMLButtonElement>("step");
const resetButton = required<HTMLButtonElement>("reset");
const speedInput = required<HTMLSelectElement>("speed");
const toolShelf = required<HTMLDivElement>("tool-shelf");
const tickValue = required<HTMLElement>("tick-value");
const deliveredValue = required<HTMLElement>("delivered-value");
const checksumValue = required<HTMLElement>("checksum-value");
const selectionValue = required<HTMLElement>("selection-value");
const orientationValue = required<HTMLElement>("orientation-value");

const host = await FactoryHost.create();
const renderer = new CanvasFactoryRenderer(canvas, host.definitions);
let snapshot = host.snapshot();
let playing = true;
let tool: Tool = "inspect";
let orientation: HexDirection = 0;
let accumulator = 0;
let previousTime = performance.now();

for (const definition of host.definitions.buildings) {
  const button = document.createElement("button");
  button.type = "button";
  button.dataset.tool = String(definition.id);
  button.textContent = definition.name;
  toolShelf.append(button);
}

function update(next: FactorySnapshot): void {
  snapshot = next;
  renderer.setSnapshot(snapshot);
  tickValue.textContent = snapshot.tick.toLocaleString();
  deliveredValue.textContent = snapshot.delivered.toLocaleString();
  checksumValue.textContent = snapshot.checksum
    .toString(16)
    .padStart(8, "0")
    .toUpperCase();
}

function setPlaying(value: boolean): void {
  playing = value;
  playButton.textContent = playing ? "Pause" : "Play";
  playButton.setAttribute("aria-pressed", String(playing));
}

function selectTool(next: Tool): void {
  tool = next;
  for (const button of toolShelf.querySelectorAll("button")) {
    button.classList.toggle("active", button.dataset.tool === String(next));
  }
}

playButton.addEventListener("click", () => setPlaying(!playing));
stepButton.addEventListener("click", () => {
  setPlaying(false);
  update(host.tick(1));
});
resetButton.addEventListener("click", () => update(host.reset()));
required<HTMLButtonElement>("turn").addEventListener("click", () => {
  orientation = rotateHexDirection(orientation, 1);
  orientationValue.textContent = String(orientation);
});
toolShelf.addEventListener("click", (event) => {
  const button = (event.target as Element).closest<HTMLButtonElement>(
    "button[data-tool]",
  );
  if (!button) return;
  const value = button.dataset.tool ?? "inspect";
  selectTool(/^\d+$/.test(value) ? Number(value) : (value as Tool));
});
canvas.addEventListener("pointermove", (event) => {
  const coordinate = renderer.pick(event.clientX, event.clientY);
  renderer.setHover(coordinate);
  const entity = snapshot.buildings.find(
    ({ q, r }) => q === coordinate.q && r === coordinate.r,
  );
  selectionValue.textContent = entity
    ? `${entity.kind} (${coordinate.q}, ${coordinate.r}) · ${entity.status}`
    : `empty (${coordinate.q}, ${coordinate.r})`;
});
canvas.addEventListener("pointerleave", () => renderer.setHover(null));
canvas.addEventListener("click", (event) => {
  const { q, r } = renderer.pick(event.clientX, event.clientY);
  if (tool === "erase") update(host.erase(q, r));
  else if (tool === "rotate") update(host.rotate(q, r));
  else if (typeof tool === "number")
    update(host.place(q, r, tool, orientation));
});

function frame(now: number): void {
  const elapsed = Math.min(250, now - previousTime);
  previousTime = now;
  if (playing) {
    accumulator += elapsed * Number(speedInput.value);
    const ticks = Math.min(20, Math.floor(accumulator / 1000));
    if (ticks > 0) {
      accumulator -= ticks * 1000;
      update(host.tick(ticks));
    }
  }
  requestAnimationFrame(frame);
}

update(snapshot);
selectTool("inspect");
requestAnimationFrame(frame);

declare global {
  interface Window {
    __hexFactory?: {
      snapshot: () => FactorySnapshot;
      step: (count?: number) => FactorySnapshot;
      reset: () => FactorySnapshot;
    };
  }
}

window.__hexFactory = {
  snapshot: () => host.snapshot(),
  step: (count = 1) => {
    setPlaying(false);
    const next = host.tick(count);
    update(next);
    return next;
  },
  reset: () => {
    const next = host.reset();
    update(next);
    return next;
  },
};

function required<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (!element) throw new Error(`Missing #${id}`);
  return element as T;
}
