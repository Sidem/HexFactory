/**
 * The UI emblem library: one drawing vocabulary for every family the interface names.
 *
 * v0.30.0 gave technologies original SVG emblems and left everything else on whatever it had.
 * Buildings had a three-letter stamp — `EXT`, `BLT`, `COM` — which is a label, not an identity: at
 * dock size the player reads three glyphs of text and has to translate them, and two stations that
 * do completely different work look identical apart from the letters. Recipe categories and
 * research branches had a colour and nothing else, and colour alone is not an identity in a
 * catalogue that already contains three greys.
 *
 * So this is the icon pass, and it is a **library**, not a pile of drawings. Everything below obeys
 * one contract, which is what makes a family look like a family:
 *
 * - **Frame.** 32×32 viewBox, `fill="none"`, `stroke="currentColor"`, stroke width 1.7, round caps
 *   and joins. One {@link frame} function emits every emblem, so no glyph can drift.
 * - **Perspective and lighting.** Straight-on elevation. No perspective, no shading, no gradients.
 *   An emblem is a diagram of the thing; `docs/ART.md`'s generated mesh is the thing. This library
 *   never touches the world.
 * - **Framing and safe area.** Ink stays inside 3–29 on both axes, so a stroke never touches the
 *   box edge and a 16px rendering never clips. Anything that stands on the ground shares a ground
 *   line near y=28, so a row of building emblems reads against one horizon.
 * - **Palette.** `currentColor` only. Colour is the caller's — the family accent — and is never
 *   baked into the markup. {@link EMBLEM_ACCENTS} is where an accent comes from.
 * - **Transparent background.** No emblem paints a backdrop; the box around it is CSS.
 * - **No baked text.** An upgrade keeps its base shape and the rank is a UI overlay
 *   ({@link emblemRank}), so Extractor II is the extractor emblem with a badge rather than a second
 *   drawing that has to be kept in step with the first.
 * - **Fallback, never a blank.** An unknown key yields {@link GENERIC} plus the caller's text, which
 *   is why a definition the library has never heard of is a slightly plain button rather than an
 *   empty one or an invalid definition.
 *
 * Presentation only. No key here reaches a save, a checksum, a native definition, or the wire —
 * `emblems.test.ts` is where that is stated as a test rather than as a comment.
 */

const STROKE = 1.7;

/** The one frame every emblem is drawn in. Nothing else in this file emits an `<svg>`. */
function frame(glyph: string): string {
  return `<svg viewBox="0 0 32 32" fill="none" stroke="currentColor" stroke-width="${STROKE}" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${glyph}</svg>`;
}

/** What an unrecognised key draws. Never blank: a hex plate with a mark, plus the caller's text. */
const GENERIC =
  '<path d="m16 3 12 7v12l-12 7-12-7V10L16 3Z"/><path d="M16 12v8m-4-4h8"/>';

/**
 * Buildings, keyed by their **base** definition key.
 *
 * A tier is not a separate entry. `extractor-ii` resolves to `extractor` through
 * {@link emblemBaseKey}, because an upgrade is the same machine doing the same job and the player
 * should recognise it instantly; the rank rides on top as a badge.
 */
const BUILDINGS: Record<string, string> = {
  extractor:
    '<path d="M7 28V14h18v14M7 14l9-6 9 6M16 18v4m-3 0h6l-3 6-3-6ZM4 28h24"/>',
  belt: '<path d="M8 11h16a6 6 0 0 1 0 12H8a6 6 0 0 1 0-12ZM11 17h9m-3-3 3 3-3 3"/>',
  composer:
    '<path d="M9 8h14v16H9zM3 12h6m-6 8h6m14-4h6m-3-3 3 3-3 3M13 13h6m-6 6h6"/>',
  container: '<path d="M5 10h22v17H5zM5 15h22M11 20h10v7H11zM3 27h26"/>',
  consumer: '<path d="M6 14h20v13H6zM16 3v9m-4-4 4 4 4-4M11 20h10M4 27h24"/>',
  "landing-hub":
    '<path d="m16 8 9 5v9l-9 5-9-5v-9l9-5ZM7 13l9 5 9-5M16 18v9M16 8V4m-3 2 3-3 3 3"/>',
  smelter:
    '<path d="M9 28V14h14v14M6 28h20M13 19h6v5h-6zM20 14V7h4v7M22 7V4M14 11c-2-2 1-3 1-5 2 2 3 4 1 6"/>',
  kiln: '<path d="M7 28V18a9 9 0 0 1 18 0v10M4 28h24M13 28v-6h6v6M15 9c-2-3 2-4 1-6 3 2 4 5 2 7"/>',
  cutter:
    '<circle cx="16" cy="12" r="6"/><circle cx="16" cy="12" r="1.7"/><path d="M16 4V3m6 9h1M16 20v1M9 12H8m8.6-5.4 1.4-1.4m-5.4 1.4L11.5 5m5.1 12.4 1.4 1.4m-5.4-1.4L11.5 19M5 22h22v5H5z"/>',
  crusher:
    '<path d="M5 6v10l6 4M27 6v10l-6 4M4 28h24M9 27l3.5-4 3.5 4M17 27l3.5-4 3.5 4"/>',
  pump: '<circle cx="13" cy="18" r="8"/><circle cx="13" cy="18" r="2.4"/><path d="M13 8V4h10v6M4 28h22M8 26h12M21 18h6"/>',
  pole: '<path d="M16 3v25M5 28h22M8 9h16M8 9v3m16-3v3M11 15h10m-10 0v2m10-2v2"/>',
  "burner-generator":
    '<circle cx="23" cy="19" r="5"/><path d="M6 28V15h10v13M4 28h24M9 24h4v4H9zM11 12c-2-3 2-4 1-7 3 3 4 6 2 8M24 16l-3 4h3l-2 4"/>',
  "wind-turbine":
    '<circle cx="16" cy="12" r="1.8"/><path d="M16 28V14M12 28h8M4 28h24M16 10V3M17.6 13l6.5 3.8M14.4 13 7.9 16.8"/>',
  "hydro-generator":
    '<circle cx="19" cy="19" r="8"/><circle cx="19" cy="19" r="2"/><path d="M4 4v9M4 8h8M4 13c4 0 6 3 7 5M19 11v16m-8-8h16m-5.7-5.7L13.3 24.7m0-11.4 11.4 11.4"/>',
  boiler:
    '<path d="M5 13h22v11H5zM8 24v3m16-3v3M4 27h24M10 18h12M11 10c-3-3 3-4 0-7m6 7c-3-3 3-4 0-7"/>',
  "steam-turbine":
    '<path d="M7 11h18l-2 12H9L7 11ZM4 17h3m18 0h3M16 11v12M11 8c-3-2 2-3 0-5M10 26h12M4 28h24"/>',
  bridge:
    '<path d="M3 12h26M8 12v3m16-3v3M6 12c0 6 4 9 10 9s10-3 10-9M3 26c3-3 5 3 8 0s5 3 8 0 5 3 8 0"/>',
  splitter:
    '<path d="M3 16h11M14 16l6-6h9M14 16l6 6h9M26 7l3 3-3 3M26 19l3 3-3 3"/>',
  // Two lanes in, one lane out, and the arrowheads have to agree with that: they sit at the head of
  // each input and point along the flow, into the junction. Drawn the other way round the glyph is a
  // splitter running backwards, which is the one thing a merger must not be mistaken for.
  merger: '<path d="M6 10h6l6 6h11M6 22h6l6-6M3 7l3 3-3 3M3 19l3 3-3 3"/>',
  underpass:
    '<path d="M16 3v26M4 16h8m8 0h8M12 16l-2.5-3M12 16l-2.5 3M20 16l2.5-3M20 16l2.5 3"/>',
  "primitive-furnace":
    '<path d="M8 28V19a8 8 0 0 1 16 0v9M5 28h22M12 28v-5h8v5M8 22h16M16 26c-1.5-1.5.5-2.5.5-4 1.5 1.5 2 3 .5 4"/>',
  "manual-workshop":
    '<path d="M4 16h24v4H4zM7 20v8m18-8v8M3 28h26M13 12l7-7m-2 0h4v4"/>',
  "oil-well":
    '<path d="M8 28 12 7h8l4 21M10 21h12M11 15h10M4 28h24M13 7h6M16 7V3"/>',
  refinery:
    '<path d="M6 28V10a4 4 0 0 1 8 0v18M6 15h8M6 21h8M18 28V13a4 4 0 0 1 8 0v15M18 19h8M14 12h4M3 28h26M10 6V3"/>',
  "asphalt-mixer":
    '<path d="M8 8h16l-3 11H11L8 8ZM11 3h10l3 5M16 19v3M10 22h12l2 5H8l2-5M3 28h26"/>',
  pipe: '<path d="M3 13h17v6H3zM20 11h5v10h-5zM25 14h4v6h-4M6 13V9m-2 0h4"/>',
  "pipe-underpass":
    '<path d="M3 16h7l3 7h6l3-7h7M10 16l2-5h8l2 5M13 23v4m6-4v4M4 28h24"/>',
  "water-tank":
    '<path d="M7 9h18v18H7zM7 14h18M10 9V5h12v4M4 28h24M16 17c2 2.5 3 4 3 5.2a3 3 0 0 1-6 0c0-1.2 1-2.7 3-5.2Z"/>',
  "oil-tank":
    '<path d="M7 9h18v18H7zM7 14h18M10 9V5h12v4M4 28h24M16 17c2 2.5 3 4 3 5.2a3 3 0 0 1-6 0c0-1.2 1-2.7 3-5.2Z"/>',
  "barrel-station":
    '<path d="M5 11h13v16H5zM7 15h9m-9 7h9M21 8h6v15h-6zM18 13h3m-3 6h3M4 28h24M24 8V4"/>',
};

/**
 * Recipe categories — the *process*, not the machine that runs it.
 *
 * A smelter emblem is a building; the smelting emblem is what happens inside one, which is why a
 * recipe row and a station card can sit next to each other without saying the same thing twice.
 */
const RECIPE_CATEGORIES: Record<string, string> = {
  assembly: '<path d="M12 7 6 16l6 9M20 7l6 9-6 9M11 16h10m-3-3 3 3-3 3"/>',
  smelting:
    '<path d="M7 9h14l-2 9a5 5 0 0 1-10 0zM19 14c4 0 4 4 4 6m0 0-1.5-2M23 20l1.5-2M10 6V3m5 4V3m5 3V3M6 28h20"/>',
  firing:
    '<path d="M6 6h20v7H6zM6 9.5h20M12 6v7m8-7v7M10 27c-3-3 1-5 1-8 3 3 5 6 2 8m8 0c-3-3 1-5 1-8 3 3 5 6 2 8"/>',
  cutting:
    '<circle cx="16" cy="11" r="6"/><circle cx="16" cy="11" r="1.6"/><path d="M16 3v2m8 6h2M16 17v2M6 11h2m8.6-5.4 1.4-1.4m-5.4 1.4L11.5 4.2M16.6 16.4l1.4 1.4m-5.4-1.4-1.1 1.4M4 21h24v5H4z"/>',
  crushing: '<path d="M6 5h20v6H6zM16 11v4M8 26l4-6 4 6m2 0 3-5 3 5M4 27h24"/>',
  refining:
    '<path d="M11 28V10a5 5 0 0 1 10 0v18M11 15h10M11 20h10M4 28h24M21 12h6M21 17h6M21 22h6M16 5V3"/>',
  "asphalt-mixing":
    '<circle cx="20" cy="8" r="1.6"/><circle cx="24" cy="13" r="1.6"/><circle cx="19" cy="15" r="1.6"/><path d="M4 27h24M7 21h18l-2 6H9zM11 4c-2 3-3 4-3 6a3 3 0 0 0 6 0c0-2-1-3-3-6Z"/>',
  barreling:
    '<path d="M8 5h11l2 4v14l-2 4H8l-2-4V9l2-4Zm-2 6h15M6 21h15M24 8v16m-2-3 2 3 3-3"/>',
};

/**
 * Research and skill branches, in one table because they are the same kind of thing to the player:
 * the discipline a purchase belongs to. These ride as accents beside a name, so they are the
 * simplest drawings in the library — legible at 14px is the requirement, not detail.
 */
const BRANCHES: Record<string, string> = {
  woodwork:
    '<path d="M16 4c6 4 8 10 8 14a8 8 0 0 1-16 0c0-4 2-10 8-14ZM16 10v18"/>',
  masonry:
    '<path d="M5 9h22v6H5zM5 17h22v6H5zM12 9v6m8-6v6M9 17v6m8-6v6M4 27h24"/>',
  metallurgy:
    '<path d="M6 20h20l-3 6H9zM10 16c-2-3 2-4 1-7 3 3 4 6 2 7m6 0c-2-3 2-4 1-7 3 3 4 6 2 7"/>',
  manufacturing:
    '<circle cx="16" cy="16" r="6"/><circle cx="16" cy="16" r="2"/><path d="M16 6V3m0 26v-3m10-10h3M3 16h3m16.6-6.6 2.2-2.2M7.2 24.8l2.2-2.2m0-13.2L7.2 7.2m17.6 17.6-2.2-2.2"/>',
  logistics: '<path d="M3 16h16m-4-5 5 5-5 5M21 9h7v14h-7"/>',
  infrastructure: '<path d="M3 22h26M5 22V10h22v12M5 10l22 12M27 10 5 22"/>',
  plumbing: '<path d="M5 16h8m6 0h8M13 10h6v12h-6zM16 10V6m-3 0h6"/>',
  electricity: '<path d="m19 3-12 16h8l-2 10 12-17h-8l2-9Z"/>',
  chemistry:
    '<path d="M13 4v8L6 25a2 2 0 0 0 2 3h16a2 2 0 0 0 2-3l-7-13V4M11 4h10M11 18h10"/>',
  carrying: '<path d="M9 10h14v18H9zM13 10V6h6v4M9 16h14M14 20h4v4h-4z"/>',
  construction: '<path d="M6 26 26 6M6 26h20M6 26V8m6 12h5"/>',
  surveying:
    '<circle cx="16" cy="12" r="5"/><path d="M16 17v7m-5 4h10M16 5V3M9 12H6m20 0h-3M4 28h24"/>',
};

export const BUILDING_EMBLEM_KEYS = Object.keys(BUILDINGS);
export const RECIPE_CATEGORY_EMBLEM_KEYS = Object.keys(RECIPE_CATEGORIES);
export const BRANCH_EMBLEM_KEYS = Object.keys(BRANCHES);

/**
 * The base key a definition draws as: its own key with a tier suffix removed.
 *
 * Derived rather than tabled, so `pole-iv` inherits the pole the day it is added instead of
 * silently falling back to the generic plate.
 */
export function emblemBaseKey(key: string): string {
  return key.replace(/-(i{2,3}|iv|v)$/, "");
}

/** Roman rank for a tier, or an empty string for the base tier. `0 → ""`, `1 → "II"`. */
export function emblemRank(tier: number | undefined): string {
  const rank = (tier ?? 0) + 1;
  if (rank <= 1) return "";
  return ["", "I", "II", "III", "IV", "V", "VI"][rank] ?? String(rank);
}

/** True when the library actually has a drawing for this building, rather than the fallback. */
export function hasBuildingEmblem(key: string): boolean {
  return emblemBaseKey(key) in BUILDINGS;
}

export function buildingEmblemSvg(key: string): string {
  return frame(BUILDINGS[emblemBaseKey(key)] ?? GENERIC);
}

export function recipeCategoryEmblemSvg(category: string): string {
  return frame(RECIPE_CATEGORIES[category] ?? GENERIC);
}

export function branchEmblemSvg(branch: string): string {
  return frame(BRANCHES[branch] ?? GENERIC);
}

export function genericEmblemSvg(): string {
  return frame(GENERIC);
}

/**
 * The accent a family is drawn in.
 *
 * Branch colours were already published by `researchBranchColor`; these are the two families that
 * had none. Kept beside the drawings because an accent is part of the emblem contract — a family
 * that is legible in one hue and muddy in another has not been designed, it has been guessed.
 */
export const EMBLEM_ACCENTS = {
  recipeCategory: {
    assembly: "#8fd4ff",
    smelting: "#f0a071",
    firing: "#e8875f",
    cutting: "#d9b67b",
    crushing: "#b2beca",
    refining: "#c5a5e8",
    "asphalt-mixing": "#9aa7b0",
  } as Record<string, string>,
  fallback: "#a9c4c0",
} as const;

export function recipeCategoryAccent(category: string): string {
  return EMBLEM_ACCENTS.recipeCategory[category] ?? EMBLEM_ACCENTS.fallback;
}

/**
 * Fill a fixed emblem box.
 *
 * The box is fixed so later artwork cannot shift a layout that was tuned around it, and the rank
 * badge is written here rather than into the drawing — one place decides that Extractor II is the
 * extractor plus a `II`, and the drawing stays a drawing.
 *
 * Rewriting identical markup is skipped for the same reason `itemChip` skips it: it restarts the
 * SVG's own layout for nothing, every frame, on every card in the catalogue.
 */
export function paintEmblem(
  box: HTMLElement,
  options: {
    /** The identity this box is drawing. Used to skip an unchanged repaint. */
    key: string;
    markup: string;
    accent?: string;
    rank?: string;
    /** Present only for the fallback, where the generic plate carries the caller's short code. */
    text?: string;
  },
): void {
  const rank = options.rank ?? "";
  const stamp = `${options.key}|${rank}`;
  if (box.dataset.emblem !== stamp) {
    box.innerHTML = `${options.markup}${rank ? `<b class="emblem-rank" aria-hidden="true">${rank}</b>` : ""}`;
    box.dataset.emblem = stamp;
  }
  box.classList.add("emblem");
  box.style.setProperty("--emblem-color", options.accent ?? "currentColor");
  // The fallback is a generic emblem *plus text*, which is the whole reason an unknown key is not
  // a blank button. The text is the caller's — a definition's three-letter stamp, say.
  box.classList.toggle("emblem-generic", options.text !== undefined);
  if (options.text !== undefined) box.dataset.text = options.text;
  else delete box.dataset.text;
}

/**
 * Return a box to plain content.
 *
 * A hotbar slot is reused: the machine pinned to it can be replaced by a tool, or by nothing. Left
 * alone, the emblem's markup and its fixed-box class would survive under whatever text the caller
 * writes next, which is how a slot ends up showing two identities at once.
 */
export function clearEmblem<T extends HTMLElement>(box: T): T {
  if (box.dataset.emblem === undefined) return box;
  box.textContent = "";
  delete box.dataset.emblem;
  delete box.dataset.text;
  box.classList.remove("emblem", "emblem-generic");
  box.style.removeProperty("--emblem-color");
  return box;
}
