import definitionData from "../data/definitions.json";

import type { BuildingDefinition, Definitions } from "../core/types";
import {
  BUILDING_SHAPES,
  partsFor,
  silhouetteOf,
  trimOf,
  type SilhouetteKey,
} from "../rendering/buildingLook";
import {
  profileTop,
  silhouetteSignature,
  TIER_LADDER,
} from "../rendering/shapeGrammar";
import { ContactSheetRenderer } from "../rendering/three/ContactSheetRenderer";

/**
 * The contact sheet — every definition x every tier x every status on one grid.
 *
 * This is the half of "maintained systematically" that the grammar alone is not. A part list can
 * be a clean data row and still draw two machines that read alike, or carry a tier modifier that
 * changes nothing a player would notice; neither is visible from the table, and finding either by
 * playing means happening to build both. The sheet reuses the shipped renderer, so what it shows
 * is what the game draws, and it costs nothing but an entry point.
 */

const definitions = definitionData as unknown as Definitions;
const buildings = definitions.buildings;

/** The statuses that actually reach the drawing, which is what makes them worth a column. */
const STATUSES = [
  { label: "idle", status: "idle", cycle: 0 },
  { label: "working 25%", status: "composing", cycle: 0.25 },
  { label: "working 75%", status: "composing", cycle: 0.75 },
  { label: "no power", status: "no power", cycle: 0 },
] as const;

const CELL = 56;
const source = document.createElement("canvas");
source.id = "contact-render-source";
source.setAttribute("aria-hidden", "true");
document.body.append(source);
const contactRenderer = new ContactSheetRenderer(source, CELL);

const state = { colour: true, animate: false };

interface Cell {
  canvas: HTMLCanvasElement;
  definition: BuildingDefinition;
  tier: number;
  status: (typeof STATUSES)[number];
}

const cells: Cell[] = [];

function paint(cell: Cell, now: number): void {
  const cycle = state.animate ? (now / 900) % 1 : cell.status.cycle;
  contactRenderer.paintStrip(
    cell.canvas,
    cell.definition,
    cell.tier,
    cell.status.status,
    cycle,
    state.colour,
    now,
  );
}

function repaint(now = performance.now()): void {
  for (const cell of cells) paint(cell, now);
}

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

/**
 * Two definitions that resolve to the same silhouette key and tier draw the same machine. That is
 * correct — a smelter is a smelter — but it is also exactly the mistake the sheet exists to catch
 * when it is not intended, so it is named on the card rather than left to the eye.
 */
function sharedWith(definition: BuildingDefinition): string[] {
  const key = keyOf(definition);
  return buildings
    .filter(
      (other) =>
        other.id !== definition.id &&
        keyOf(other) === key &&
        (other.tier ?? 0) === (definition.tier ?? 0),
    )
    .map((other) => other.key);
}

function keyOf(definition: BuildingDefinition): SilhouetteKey {
  return silhouetteOf(
    definition.kind,
    definition.recipe_category,
    definition.power_source,
  );
}

function buildCard(definition: BuildingDefinition): HTMLElement {
  const key = keyOf(definition);
  const card = element("section", "card");
  const head = element("header");
  head.append(element("h2", undefined, definition.name));
  const meta = element(
    "p",
    "meta",
    `${definition.key} · kind ${definition.kind} · silhouette ${key} · ships at tier ${definition.tier ?? 0}`,
  );
  head.append(meta);
  if (BUILDING_SHAPES[key].length === 0)
    head.append(
      element(
        "p",
        "warn",
        "No base shape. A tier on this definition would not be legible as a silhouette.",
      ),
    );
  const shared = sharedWith(definition);
  if (shared.length > 0)
    head.append(
      element("p", "warn", `Draws identically to: ${shared.join(", ")}`),
    );
  card.append(head);

  const grid = element("div", "grid");
  grid.append(element("div", "corner"));
  for (const status of STATUSES)
    grid.append(element("div", "col", status.label));

  for (let tier = 0; tier <= TIER_LADDER.length; tier += 1) {
    const step = TIER_LADDER[tier - 1];
    const label = element("div", "row");
    label.append(
      element("strong", undefined, tier === 0 ? "base" : `tier ${tier}`),
    );
    if (step) label.append(element("span", undefined, step.name));
    if (tier === (definition.tier ?? 0)) label.classList.add("shipped");
    grid.append(label);
    for (const status of STATUSES) {
      const holder = element("div", "cell");
      const canvas = element("canvas");
      canvas.style.width = `${CELL * contactRenderer.orbitCount}px`;
      canvas.style.height = `${CELL}px`;
      canvas.title = "Camera orbits 0–5, left to right";
      holder.append(canvas);
      grid.append(holder);
      cells.push({ canvas, definition, tier, status });
    }
  }
  card.append(grid);

  // A tier that moved neither the outline nor the part list is a data row that changed nothing,
  // which is the defect v0.14 shipped and the one this milestone exists to make impossible to
  // miss. It is asserted in the tests as well; here it is visible.
  const notes = element("ul", "notes");
  for (let tier = 1; tier <= TIER_LADDER.length; tier += 1) {
    const below = partsFor(key, tier - 1);
    const at = partsFor(key, tier);
    const moved = silhouetteSignature(below) !== silhouetteSignature(at);
    const lifted = profileTop(at) < profileTop(below) - 1e-9;
    const note = element("li");
    note.className = moved && lifted ? "ok" : "warn";
    note.textContent =
      below.length === 0
        ? `tier ${tier}: no base shape to modify`
        : `tier ${tier}: ${moved ? "part list changes" : "PART LIST UNCHANGED"}, ${
            lifted ? "outline grows" : "OUTLINE UNCHANGED"
          } (${at.length} parts, trim ${trimOf(tier).stroke})`;
    notes.append(note);
  }
  card.append(notes);
  return card;
}

function main(): void {
  const sheet = document.querySelector<HTMLElement>("#sheet");
  if (!sheet) return;
  for (const definition of buildings) sheet.append(buildCard(definition));

  const colour = document.querySelector<HTMLInputElement>("#colour");
  const animate = document.querySelector<HTMLInputElement>("#animate");
  colour?.addEventListener("change", () => {
    state.colour = colour.checked;
    repaint();
  });
  animate?.addEventListener("change", () => {
    state.animate = animate.checked;
    repaint();
  });

  const summary = document.querySelector<HTMLElement>("#summary");
  if (summary)
    summary.textContent =
      `${buildings.length} definitions · ${Object.keys(BUILDING_SHAPES).length} silhouettes · ` +
      `${TIER_LADDER.length} tier steps · ${cells.length} status cells · ` +
      `${contactRenderer.orbitCount} orbits · one WebGL context`;

  let lastAnimatedPaint = 0;
  const frame = (now: number): void => {
    if (state.animate && now - lastAnimatedPaint >= 500) {
      lastAnimatedPaint = now;
      repaint(now);
    }
    requestAnimationFrame(frame);
  };
  repaint();
  requestAnimationFrame(frame);
}

window.addEventListener("pagehide", () => contactRenderer.dispose(), {
  once: true,
});

main();
