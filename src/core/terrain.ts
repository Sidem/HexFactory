import type { Terrain } from "./types";

/**
 * One terrain band as the host needs it: what to call it, what it is good for, whether the player
 * can stand or build on it, and what it is painted with.
 *
 * `passable` and `buildable` are native's rule, not the renderer's. They are pinned against
 * `Terrain::blocks_movement` and `Terrain::blocks_construction` by
 * `fixtures/terrain-passability.json`, which Rust asserts it agrees with and `tests/host.test.ts`
 * asserts this table agrees with — because the alternative to pinning a copied rule is discovering
 * it has drifted by walking into a hex that looked walkable.
 */
export interface TerrainInfo {
  name: string;
  /** What the band is for in play, with no passability wording in it — that comes from the flags. */
  note: string;
  passable: boolean;
  buildable: boolean;
  fill: string;
  stroke: string;
}

export const TERRAIN_INFO: Record<Terrain, TerrainInfo> = {
  deep_water: {
    name: "Deep water",
    note: "pump it from the shore",
    passable: false,
    buildable: false,
    fill: "#0f3550ee",
    stroke: "#3f9ad0",
  },
  shallow_water: {
    name: "Shallow water",
    note: "ford at 1 m/s",
    passable: true,
    buildable: false,
    fill: "#1a5474dd",
    stroke: "#5cb6d8",
  },
  shore: {
    name: "Shore",
    note: "sand and clay",
    passable: true,
    buildable: true,
    fill: "#c4a56add",
    stroke: "#e0c88a",
  },
  lowland: {
    name: "Lowland",
    note: "wood, clay, crystal",
    passable: true,
    buildable: true,
    fill: "#2a4a3ccc",
    stroke: "#4d7a62",
  },
  hills: {
    name: "Hills",
    note: "copper ore and coal",
    passable: true,
    buildable: true,
    fill: "#48604ddd",
    stroke: "#6f8a6c",
  },
  highland: {
    name: "Highland",
    note: "iron ore, coal, crystal",
    passable: true,
    buildable: true,
    fill: "#5c6b58dd",
    stroke: "#8a9a84",
  },
  cliff: {
    // Warm brown against highland's green-grey, because two greys a step apart were only told
    // apart by walking into one of them. The hatch below carries the category; this carries the
    // material.
    name: "Cliff",
    note: "stone, quarried from the hex beside it",
    passable: false,
    buildable: false,
    fill: "#57493eee",
    stroke: "#c19a72",
  },
};

/**
 * The band names in the order the fixture and the native enum declare them, so the legend reads as
 * a landform rising out of water rather than as an alphabetised list.
 */
export const TERRAIN_ORDER: Terrain[] = [
  "deep_water",
  "shallow_water",
  "shore",
  "lowland",
  "hills",
  "highland",
  "cliff",
];

/** What a band lets the player do, in one word. */
export function terrainAccess(info: TerrainInfo): string {
  if (!info.passable) return "Impassable";
  return info.buildable ? "Buildable" : "Walkable";
}
