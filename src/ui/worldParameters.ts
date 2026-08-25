/**
 * The custom world form: every generator scalar as a control a player can reason about.
 *
 * The numbers themselves are simulation truth — native validates them again on arrival and two
 * worlds differing here are different worlds — so nothing is rounded away for presentation. What
 * this module adds is the sentence beside each one. A raw `water_level` of 22000 tells a player
 * nothing; "38% of the height range is under water" tells them what they are about to generate.
 *
 * Two rules the generator doc states and this form enforces rather than restates:
 * feature scale is how big a landform is and a band cut is how much of the world it covers, and
 * the four band cuts must ascend. `orderBands` pushes its neighbours rather than letting a form
 * offer a set native will refuse.
 */

import type { Terrain, WorldParams } from "../core/types";
import { TERRAIN_INFO } from "../core/terrain";
import type { WorldPreviewPanel } from "./worldPreview";

export type WorldScalar = Exclude<keyof WorldParams, "site_rules">;

/** The noise ceiling every band cut, threshold, and river level is measured against. */
export const NOISE_MAX = 65535;
/** The four elevation cuts, in the ascending order native requires of them. */
export const BAND_KEYS: readonly WorldScalar[] = [
  "water_level",
  "shore_level",
  "hills_level",
  "highland_level",
];
/** The smallest band this form will leave: below this a band is not rare, it is unreachable. */
export const BAND_GAP = 512;

export interface WorldParameterField {
  key: WorldScalar;
  label: string;
  /** What the number does, in the player's terms, once. */
  hint: string;
  min: number;
  max: number;
  step: number;
  /** The live reading: what this value means for the world about to be generated. */
  read: (value: number, params: WorldParams) => string;
  /**
   * Present the parameter in a unit a player can hold. `river_width` is a noise half-width in the
   * save and a width in hexes on the slider; everything else is itself.
   */
  scale?: {
    toParam: (shown: number, params: WorldParams) => number;
    fromParam: (value: number, params: WorldParams) => number;
  };
}

export interface WorldParameterGroup {
  key: string;
  title: string;
  note: string;
  /** The group the pinned coverage strip is about. One group claims it; it is drawn above them all. */
  strip?: boolean;
  fields: WorldParameterField[];
}

function percent(value: number): string {
  return `${Math.round((Math.max(0, value) / NOISE_MAX) * 100)}%`;
}

function span(value: number): string {
  return `${value} hex${value === 1 ? "" : "es"}`;
}

/** The first entry whose ceiling the value is under. The last entry is the fallback. */
function pick(value: number, steps: readonly [number, string][]): string {
  for (const [ceiling, word] of steps) if (value < ceiling) return word;
  return steps[steps.length - 1]?.[1] ?? "";
}

/** How wide one river reads, in hexes, from the half-width the save actually carries. */
export function riverHexWidth(width: number, cell: number): number {
  return Math.round((2 * width * Math.max(1, cell)) / NOISE_MAX);
}

export function riverWidthFor(hexes: number, cell: number): number {
  return Math.round((hexes * NOISE_MAX) / (2 * Math.max(1, cell)));
}

export const WORLD_PARAMETER_GROUPS: readonly WorldParameterGroup[] = [
  {
    key: "landmass",
    title: "Landmass",
    note: "How big each shape is. None of these change how much of the world is water — that is the band cuts below.",
    fields: [
      {
        key: "elevation_coarse_cell",
        label: "Landform scale",
        hint: "How far apart hills and basins sit. This is the difference between a world of ponds and a world of oceans.",
        min: 1,
        max: 1024,
        step: 1,
        read: (value) =>
          `${span(value)} across · ${pick(value, [
            [48, "ponds and hillocks"],
            [160, "lakes and ridges"],
            [448, "seas and long ranges"],
            [Infinity, "oceans and continents"],
          ])}`,
      },
      {
        key: "elevation_fine_cell",
        label: "Coastline detail",
        hint: "The small octave that breaks up an edge. It roughens coastlines and slopes without moving the landform under them.",
        min: 1,
        max: 1024,
        step: 1,
        read: (value) =>
          `${span(value)} across · ${pick(value, [
            [16, "ragged, fjord-cut edges"],
            [64, "broken, natural edges"],
            [Infinity, "smooth, simple edges"],
          ])}`,
      },
      {
        key: "elevation_coarse_weight",
        label: "Landform vs detail",
        hint: "How much of a hex's height comes from the big shape rather than the small one.",
        min: 0,
        max: 100,
        step: 1,
        read: (value) => `${value}% landform · ${100 - value}% detail`,
      },
    ],
  },
  {
    key: "bands",
    title: "Terrain bands",
    note: "Where the cuts sit is how much of the world each band covers. Raising the sea level makes more water, not bigger water. The four cuts are kept in order.",
    strip: true,
    fields: [
      {
        key: "water_level",
        label: "Sea level",
        hint: "Everything below this is water. Higher floods more of the map.",
        min: 0,
        max: NOISE_MAX,
        step: 128,
        read: (value) => `${percent(value)} of the world is under water`,
      },
      {
        key: "shore_level",
        label: "Shore line",
        hint: "The sand and clay ring above the water. Shore is where clay is dug.",
        min: 0,
        max: NOISE_MAX,
        step: 128,
        read: (value, params) =>
          `${percent(value - params.water_level)} of the world is shore`,
      },
      {
        key: "hills_level",
        label: "Hills begin",
        hint: "Above this the ground reads as hills, which is where copper and coal sit.",
        min: 0,
        max: NOISE_MAX,
        step: 128,
        read: (value, params) =>
          `${percent(value - params.shore_level)} of the world is lowland`,
      },
      {
        key: "highland_level",
        label: "Highland begins",
        hint: "The top band, and the only ground iron and crystal are guaranteed on.",
        min: 0,
        max: NOISE_MAX,
        step: 128,
        read: (value, params) =>
          `${percent(value - params.hills_level)} hills, ${percent(NOISE_MAX - value)} highland`,
      },
      {
        key: "cliff_step",
        label: "Cliff steepness",
        hint: "A hex that drops more than this onto a neighbour becomes an impassable cliff. Lower makes more cliffs.",
        min: 1,
        max: NOISE_MAX,
        step: 64,
        read: (value) =>
          `${percent(value)} drop in one hex · ${pick(value, [
            [1200, "cliffs almost everywhere"],
            [3200, "cliffs along the steep edges"],
            [Infinity, "cliffs are rare"],
          ])}`,
      },
      {
        key: "deep_water_moisture",
        label: "Deep water cut",
        hint: "Water wetter than this is deep water, which cannot be forded or built on. Shallow water can be walked.",
        min: -1,
        max: NOISE_MAX,
        step: 128,
        read: (value) =>
          value < 0
            ? "every water hex is deep"
            : `${percent(NOISE_MAX - value)} of water is deep`,
      },
      {
        key: "ocean_level",
        label: "Ocean cut",
        hint: "Deposits that ask for an ocean coast read the landform octave alone against this, so a pond never counts as a sea.",
        min: 0,
        max: NOISE_MAX,
        step: 128,
        read: (value, params) =>
          value >= params.water_level
            ? "any coast can host an ocean deposit"
            : `only coasts on the deepest ${percent(value)} count as ocean`,
      },
    ],
  },
  {
    key: "climate",
    title: "Climate and ore",
    note: "Two more noise channels. Neither moves any land; they decide what is where.",
    fields: [
      {
        key: "moisture_cell",
        label: "Moisture scale",
        hint: "How wide one wet or dry region is. It sorts deep from shallow water and feeds the deposit table.",
        min: 1,
        max: 1024,
        step: 1,
        read: (value) =>
          `${span(value)} across · ${pick(value, [
            [48, "damp and dry patches interleave"],
            [256, "regional wet and dry belts"],
            [Infinity, "whole climates at a time"],
          ])}`,
      },
      {
        key: "richness_cell",
        label: "Ore richness scale",
        hint: "How wide one rich or poor region is. Deposit rules read it to decide which material a site carries.",
        min: 1,
        max: 1024,
        step: 1,
        read: (value) =>
          `${span(value)} across · ${pick(value, [
            [48, "rich ground changes every walk"],
            [256, "rich districts worth settling"],
            [Infinity, "one material dominates a region"],
          ])}`,
      },
    ],
  },
  {
    key: "rivers",
    title: "Rivers",
    note: "Rivers are channels cut from a lattice, not simulated flow. They are shallow water inland, so they can be forded and pumped.",
    fields: [
      {
        key: "river_cell",
        label: "River spacing",
        hint: "How far apart channels run.",
        min: 1,
        max: 1024,
        step: 1,
        read: (value) =>
          `one channel every ${span(value)} · ${pick(value, [
            [128, "a braided, wet world"],
            [384, "a river within a short walk"],
            [Infinity, "rivers are a landmark"],
          ])}`,
      },
      {
        key: "river_width",
        label: "River width",
        hint: "How wide one river runs. Zero is a world without rivers at all.",
        min: 0,
        max: 24,
        step: 1,
        scale: {
          toParam: (shown, params) => riverWidthFor(shown, params.river_cell),
          fromParam: (value, params) => riverHexWidth(value, params.river_cell),
        },
        read: (shown) =>
          shown === 0
            ? "no rivers anywhere"
            : `${span(shown)} wide · ${pick(shown, [
                [2, "a stream to step over"],
                [6, "a river to ford"],
                [Infinity, "a channel wide enough to need a crossing"],
              ])}`,
      },
      {
        key: "river_max_elevation",
        label: "River ceiling",
        hint: "Rivers stop at this height, so none of them runs over a summit.",
        min: 0,
        max: NOISE_MAX,
        step: 128,
        read: (value, params) =>
          value <= params.shore_level
            ? "rivers never reach inland"
            : `rivers run up to ${percent(value)} of the height range`,
      },
    ],
  },
  {
    key: "deposits",
    title: "Deposits",
    note: "One site holds one material. Which materials exist and what ground they may stand on comes from the preset's resource table and is not edited here.",
    fields: [
      {
        key: "site_cell",
        label: "Deposit spacing",
        hint: "One deposit at most per cell this wide, so this is how far apart patches stand.",
        min: 1,
        max: 1024,
        step: 1,
        read: (value) =>
          `one deposit per ${span(value)} · ${pick(value, [
            [24, "ore underfoot"],
            [80, "a patch within a short walk"],
            [Infinity, "an expedition per material"],
          ])}`,
      },
      {
        key: "site_jitter",
        label: "Deposit wander",
        hint: "How far a deposit may drift inside its own cell.",
        min: 0,
        max: 16,
        step: 1,
        read: (value) =>
          value === 0
            ? "every deposit sits on a visible grid"
            : `up to ${span(value)} off the lattice · ${pick(value, [
                [4, "the grid still shows"],
                [Infinity, "scattered naturally"],
              ])}`,
      },
    ],
  },
];

export const WORLD_PARAMETER_FIELDS: readonly WorldParameterField[] =
  WORLD_PARAMETER_GROUPS.flatMap((group) => group.fields);

function clamp(value: number, low: number, high: number): number {
  return Math.min(high, Math.max(low, value));
}

/**
 * Keep the four elevation cuts ascending after `changed` moved. Out of order, a band is not rare —
 * native refuses the whole set, so the form pushes the neighbours instead of offering it.
 */
export function orderBands(
  params: WorldParams,
  changed: WorldScalar,
): WorldParams {
  const index = BAND_KEYS.indexOf(changed);
  if (index < 0) return params;
  const next = { ...params };
  // The moved cut has to leave one gap for every cut that must stay below and above it.
  next[changed] = clamp(
    next[changed],
    index * BAND_GAP,
    NOISE_MAX - (BAND_KEYS.length - 1 - index) * BAND_GAP,
  );
  for (let below = index - 1; below >= 0; below -= 1) {
    const key = BAND_KEYS[below] as WorldScalar;
    const above = BAND_KEYS[below + 1] as WorldScalar;
    next[key] = Math.min(next[key], next[above] - BAND_GAP);
  }
  for (let above = index + 1; above < BAND_KEYS.length; above += 1) {
    const key = BAND_KEYS[above] as WorldScalar;
    const below = BAND_KEYS[above - 1] as WorldScalar;
    next[key] = Math.max(next[key], next[below] + BAND_GAP);
  }
  return next;
}

export interface BandSegment {
  terrain: Terrain;
  /** Share of the full height range, 0..1. */
  share: number;
}

/**
 * The band cuts as the coverage strip draws them: how much of the height range each band owns.
 * Cliffs and rivers are steepness and lattice rather than height, so neither appears here.
 */
export function bandSegments(params: WorldParams): BandSegment[] {
  const cuts: [Terrain, number, number][] = [
    ["shallow_water", 0, params.water_level],
    ["shore", params.water_level, params.shore_level],
    ["lowland", params.shore_level, params.hills_level],
    ["hills", params.hills_level, params.highland_level],
    ["highland", params.highland_level, NOISE_MAX],
  ];
  return cuts.map(([terrain, low, high]) => ({
    terrain,
    share: clamp(high - low, 0, NOISE_MAX) / NOISE_MAX,
  }));
}

/**
 * One instance of the form. The controls are built once and only ever written to: a form rebuilt
 * under a pointer loses the slider it was rebuilt for, the same rule the catalogue lives under.
 *
 * The form never holds the authoritative parameters. It reports an edited set and waits to be told
 * what to show, so the two copies of this form and the preset picker cannot disagree.
 */
export class WorldParameterForm {
  private readonly sliders = new Map<WorldScalar, HTMLInputElement>();
  private readonly numbers = new Map<WorldScalar, HTMLInputElement>();
  private readonly readings = new Map<WorldScalar, HTMLElement>();
  private readonly strips: HTMLElement[] = [];
  private params: WorldParams | null = null;

  constructor(
    container: HTMLElement,
    idPrefix: string,
    private readonly onChange: (next: WorldParams) => void,
    /**
     * The live raster, when the caller has one to mount. Optional because the panel needs an item
     * table and a worker and this form needs neither — it draws the cuts, not the world.
     */
    preview: WorldPreviewPanel | null = null,
  ) {
    // The strip is the one control here that is about the world rather than about a number, so it
    // is pinned above the whole form instead of living inside the band group. Scrolling down to
    // the rivers is exactly when a player wants to still see the coastline they just moved.
    const banded = WORLD_PARAMETER_GROUPS.find((group) => group.strip);
    if (banded) container.append(this.buildPinnedStrip(banded, preview));
    for (const group of WORLD_PARAMETER_GROUPS) {
      container.append(this.buildGroup(group, idPrefix));
    }
  }

  /** Show a parameter set. Called after every edit, so a clamped cut is visible immediately. */
  setValues(params: WorldParams): void {
    this.params = params;
    for (const field of WORLD_PARAMETER_FIELDS) {
      const shown = field.scale
        ? field.scale.fromParam(params[field.key], params)
        : params[field.key];
      const slider = this.sliders.get(field.key);
      const number = this.numbers.get(field.key);
      const reading = this.readings.get(field.key);
      if (slider) slider.value = String(clamp(shown, field.min, field.max));
      if (number) number.value = String(shown);
      if (reading) reading.textContent = field.read(shown, params);
    }
    for (const strip of this.strips) this.paintStrip(strip, params);
  }

  /** The coverage strip, captioned and pinned, above every group rather than inside one. */
  private buildPinnedStrip(
    group: WorldParameterGroup,
    preview: WorldPreviewPanel | null,
  ): HTMLElement {
    const panel = document.createElement("div");
    panel.className = "world-param-pinned";
    const caption = document.createElement("p");
    caption.className = "world-param-pinned-caption";
    caption.textContent = preview ? "This world" : group.title;
    const strip = document.createElement("div");
    strip.className = "world-param-strip";
    strip.setAttribute("role", "img");
    this.strips.push(strip);
    panel.append(caption, strip);
    // Under the strip rather than over it: the strip is the legend the picture is read with, and a
    // legend below the thing it labels is a legend the eye has to come back up for.
    if (preview) panel.append(preview.element);
    return panel;
  }

  private buildGroup(
    group: WorldParameterGroup,
    idPrefix: string,
  ): HTMLElement {
    const section = document.createElement("section");
    section.className = "world-param-group";
    const heading = document.createElement("h4");
    heading.textContent = group.title;
    const note = document.createElement("p");
    note.className = "world-param-group-note";
    note.textContent = group.note;
    section.append(heading, note);
    for (const field of group.fields) {
      section.append(this.buildField(field, idPrefix));
    }
    return section;
  }

  private buildField(
    field: WorldParameterField,
    idPrefix: string,
  ): HTMLElement {
    const row = document.createElement("div");
    row.className = "world-param";
    const id = `${idPrefix}-${field.key}`;

    const head = document.createElement("div");
    head.className = "world-param-head";
    const label = document.createElement("label");
    label.htmlFor = id;
    label.textContent = field.label;
    const number = document.createElement("input");
    number.type = "number";
    number.className = "world-param-number";
    number.min = String(field.min);
    number.max = String(field.max);
    number.step = String(field.step);
    number.setAttribute("aria-label", `${field.label} value`);
    head.append(label, number);

    const slider = document.createElement("input");
    slider.type = "range";
    slider.id = id;
    slider.className = "world-param-slider";
    slider.min = String(field.min);
    slider.max = String(field.max);
    slider.step = String(field.step);

    const hint = document.createElement("p");
    hint.className = "world-param-hint";
    hint.textContent = field.hint;
    const reading = document.createElement("p");
    reading.className = "world-param-reading";
    reading.setAttribute("aria-live", "off");
    // The hint says what the number does; the reading says what this value of it means. Tying the
    // slider to both is what makes a screen reader announce the consequence and not just a digit.
    hint.id = `${id}-hint`;
    reading.id = `${id}-reading`;
    slider.setAttribute("aria-describedby", `${hint.id} ${reading.id}`);

    for (const control of [slider, number]) {
      control.addEventListener("input", () => this.commit(field, control));
    }

    row.append(head, slider, hint, reading);
    this.sliders.set(field.key, slider);
    this.numbers.set(field.key, number);
    this.readings.set(field.key, reading);
    return row;
  }

  private commit(field: WorldParameterField, control: HTMLInputElement): void {
    if (!this.params) return;
    const shown = Number(control.value);
    // A half-typed number box is not an edit. Leaving the field empty or mid-word must not push a
    // NaN into the parameters the Start button is about to send.
    if (!Number.isSafeInteger(shown)) return;
    const bounded = clamp(shown, field.min, field.max);
    const value = field.scale
      ? field.scale.toParam(bounded, this.params)
      : bounded;
    this.onChange(
      orderBands({ ...this.params, [field.key]: value }, field.key),
    );
  }

  private paintStrip(strip: HTMLElement, params: WorldParams): void {
    const segments = bandSegments(params);
    strip.textContent = "";
    strip.setAttribute(
      "aria-label",
      segments
        .map(
          ({ terrain, share }) =>
            `${TERRAIN_INFO[terrain].name} ${Math.round(share * 100)}%`,
        )
        .join(", "),
    );
    for (const { terrain, share } of segments) {
      const cell = document.createElement("span");
      cell.className = "world-param-strip-cell";
      cell.style.flexGrow = String(Math.max(share, 0.001));
      cell.style.background = TERRAIN_INFO[terrain].fill;
      cell.style.borderColor = TERRAIN_INFO[terrain].stroke;
      // Under about a twelfth of the range there is no room for a word, and a clipped one reads
      // worse than the colour alone; the strip's own label still carries every share.
      cell.textContent = share > 0.08 ? TERRAIN_INFO[terrain].name : "";
      strip.append(cell);
    }
  }
}
