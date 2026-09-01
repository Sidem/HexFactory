import { describe, expect, it } from "vitest";
import definitionsJson from "../src/data/definitions.json";
import technologiesJson from "../src/data/technologies.json";
import type { Definitions, Technologies } from "../src/core/types";
import {
  BRANCH_EMBLEM_KEYS,
  BUILDING_EMBLEM_KEYS,
  RECIPE_CATEGORY_EMBLEM_KEYS,
  branchEmblemSvg,
  buildingEmblemSvg,
  emblemBaseKey,
  emblemRank,
  genericEmblemSvg,
  hasBuildingEmblem,
  recipeCategoryEmblemSvg,
} from "../src/rendering/emblems";

const definitions = definitionsJson as unknown as Definitions;
const technologies = technologiesJson as unknown as Technologies;
const GENERIC = genericEmblemSvg();

/**
 * Everything the interface draws an emblem for, in one list, so the contract can be stated once —
 * and named, so a failure says which drawing broke it rather than only that one did.
 */
const EVERY_EMBLEM: [string, string][] = [
  ...BUILDING_EMBLEM_KEYS.map(
    (key) => [`building/${key}`, buildingEmblemSvg(key)] as [string, string],
  ),
  ...RECIPE_CATEGORY_EMBLEM_KEYS.map(
    (key) =>
      [`recipe/${key}`, recipeCategoryEmblemSvg(key)] as [string, string],
  ),
  ...BRANCH_EMBLEM_KEYS.map(
    (key) => [`branch/${key}`, branchEmblemSvg(key)] as [string, string],
  ),
];

describe("the emblem library is a family, not a pile of drawings", () => {
  /**
   * The frame is what makes twelve unrelated drawings read as one set. If a glyph can set its own
   * size, stroke or cap, the family dissolves one careless emblem at a time — so this is checked on
   * every emblem the library can emit rather than on a sample.
   */
  it("draws every emblem in the one frame, leaving colour to the caller", () => {
    for (const svg of [...EVERY_EMBLEM.map(([, value]) => value), GENERIC]) {
      expect(svg.startsWith('<svg viewBox="0 0 32 32" fill="none"')).toBe(true);
      expect(svg).toContain('stroke="currentColor"');
      expect(svg).toContain('stroke-width="1.7"');
      expect(svg).toContain('stroke-linecap="round"');
      expect(svg).toContain('stroke-linejoin="round"');
      expect(svg).toContain('aria-hidden="true"');
      expect(svg.endsWith("</svg>")).toBe(true);
      // Exactly one frame: a glyph that smuggled in its own <svg> would nest and escape the rules.
      expect(svg.split("<svg").length).toBe(2);

      // Colour belongs to the caller, so the same drawing can carry a building accent in the
      // catalogue and a branch accent in the research pane. A baked fill or an inline style would
      // freeze it, and baked text would not translate.
      const glyph = svg.slice(svg.indexOf(">") + 1);
      expect(glyph).not.toContain("fill=");
      expect(glyph).not.toContain("style=");
      expect(glyph).not.toContain("stroke=");
      expect(glyph).not.toContain("<text");
      expect(glyph).not.toContain("url(");
      expect(glyph).not.toContain("Gradient");
    }
  });

  /**
   * A stroke that touches the box clips at 16px, so the ink stays inside 3–29 on both axes.
   *
   * This walks the pen rather than scanning the numbers: half the path data is relative, where a
   * lone `-12` is an ordinary leftward move and means nothing on its own. Curve and arc control
   * points are not visited — only the points the pen actually lands on — so a wild bulge between
   * two legal endpoints would slip through. Every glyph here is straight lines and small radii, and
   * the endpoints are what pin the drawing to the frame.
   */
  it("keeps every stroke inside the safe area", () => {
    const ARGS: Record<string, number> = {
      M: 2,
      L: 2,
      H: 1,
      V: 1,
      C: 6,
      S: 4,
      A: 7,
      Z: 0,
    };
    // Collected rather than asserted per emblem, so one run names every drawing that strays.
    const outside: string[] = [];
    for (const [name, svg] of EVERY_EMBLEM) {
      const points: [number, number][] = [];
      for (const [, cx, cy, r] of svg.matchAll(
        /<circle cx="([\d.]+)" cy="([\d.]+)" r="([\d.]+)"/g,
      )) {
        const [x, y, radius] = [Number(cx), Number(cy), Number(r)];
        points.push([x - radius, y - radius], [x + radius, y + radius]);
      }
      for (const [, data] of svg.matchAll(/ d="([^"]+)"/g)) {
        let [x, y, startX, startY] = [0, 0, 0, 0];
        const tokens = data?.match(/[A-Za-z]|-?\d*\.?\d+/g) ?? [];
        let command = "M";
        for (let at = 0; at < tokens.length; ) {
          if (/[A-Za-z]/.test(tokens[at]!)) command = tokens[at++]!;
          const upper = command.toUpperCase();
          const relative = command !== upper;
          const take = ARGS[upper] ?? 0;
          const args = tokens.slice(at, at + take).map(Number);
          at += take;
          if (upper === "Z") [x, y] = [startX, startY];
          else if (upper === "H") x = relative ? x + args[0]! : args[0]!;
          else if (upper === "V") y = relative ? y + args[0]! : args[0]!;
          else {
            const [dx, dy] = [args[take - 2]!, args[take - 1]!];
            x = relative ? x + dx : dx;
            y = relative ? y + dy : dy;
          }
          if (upper === "M") [startX, startY] = [x, y];
          points.push([x, y]);
          // After the first pair, `M` repeats as `L`, which is what the data relies on.
          if (upper === "M") command = relative ? "l" : "L";
        }
      }
      expect(points.length).toBeGreaterThan(0);
      const values = points.flat();
      const [low, high] = [Math.min(...values), Math.max(...values)];
      if (low < 3 || high > 29)
        outside.push(`${name} ${low.toFixed(1)}..${high.toFixed(1)}`);
    }
    expect(outside).toEqual([]);
  });
});

describe("the emblem library covers what the interface actually names", () => {
  /**
   * The point of the pass: no buildable machine falls back to a plate and a text code. Tiers are
   * deliberately not entries — they resolve through the base key — so this also proves the
   * derivation works against the real catalogue rather than against a hand-written list.
   */
  it("draws every machine, category and branch the data declares, or falls back", () => {
    const missing = definitions.buildings
      .filter((building) => building.buildable)
      .filter((building) => !hasBuildingEmblem(building.key))
      .map((building) => building.key);
    expect(missing).toEqual([]);

    const categories = new Set(
      definitions.recipes.map((recipe) => recipe.category),
    );
    for (const category of categories)
      expect(
        RECIPE_CATEGORY_EMBLEM_KEYS.includes(category) ||
          recipeCategoryEmblemSvg(category) !== GENERIC,
        `recipe category ${category} has no emblem`,
      ).toBe(true);

    const branches = [
      ...technologies.branches.map((branch) => branch.key),
      ...new Set(technologies.skills.map((skill) => skill.branch)),
    ];
    for (const branch of branches)
      expect(
        branchEmblemSvg(branch) !== GENERIC,
        `branch ${branch} has no emblem`,
      ).toBe(true);

    // An unknown key is a slightly plain button, never an empty one and never a thrown error.
    expect(hasBuildingEmblem("no-such-machine")).toBe(false);
    expect(buildingEmblemSvg("no-such-machine")).toBe(GENERIC);
    expect(recipeCategoryEmblemSvg("no-such-category")).toBe(GENERIC);
    expect(branchEmblemSvg("no-such-branch")).toBe(GENERIC);
  });

  /** An upgrade is the same machine. It must share the drawing and differ only by the badge. */
  it("gives a tier the base drawing and a rank badge rather than a second drawing", () => {
    const tiered = definitions.buildings.filter(
      (building) =>
        building.buildable && emblemBaseKey(building.key) !== building.key,
    );
    expect(tiered.length).toBeGreaterThan(0);
    for (const building of tiered) {
      const base = emblemBaseKey(building.key);
      expect(BUILDING_EMBLEM_KEYS).toContain(base);
      expect(buildingEmblemSvg(building.key)).toBe(buildingEmblemSvg(base));
      expect(emblemRank(building.tier)).not.toBe("");
    }
    // The first tier is the plain machine: a lone "I" on every base emblem would be noise.
    expect(emblemRank(0)).toBe("");
    expect(emblemRank(undefined)).toBe("");
    expect(emblemRank(1)).toBe("II");
  });
});

/**
 * The invariant the whole project rests on: presentation never becomes simulation truth. An emblem
 * key is a drawing's name, and a drawing's name has no business in a save, a checksum or the wire.
 */
describe("emblem keys stay out of the simulation", () => {
  it("names nothing the native definitions do not already name", () => {
    const known = new Set([
      ...definitions.buildings.map((building) => building.key),
      ...definitions.recipes.map((recipe) => recipe.category),
      ...technologies.branches.map((branch) => branch.key),
      ...technologies.skills.map((skill) => skill.branch),
    ]);
    // Every emblem key is a key the data already carries — the library invents no vocabulary the
    // simulation would then have to learn.
    for (const key of BUILDING_EMBLEM_KEYS)
      expect(
        [...known].some((value) => emblemBaseKey(value) === key),
        `building emblem ${key} names nothing in the catalogue`,
      ).toBe(true);
    for (const key of [...RECIPE_CATEGORY_EMBLEM_KEYS, ...BRANCH_EMBLEM_KEYS])
      expect(known.has(key), `emblem ${key} names nothing in the data`).toBe(
        true,
      );
  });
});
