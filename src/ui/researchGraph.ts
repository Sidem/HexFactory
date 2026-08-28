import {
  technologyBoundaryUnlocks,
  technologyBuildingUnlocks,
  technologyBuildRangeBonus,
  technologyCarrySlotsBonus,
} from "../core/definitions";
import type {
  Definitions,
  Technologies,
  TechnologyDefinition,
} from "../core/types";
import { technologyContext } from "./research";

export const RESEARCH_NODE_WIDTH = 88;
export const RESEARCH_NODE_HEIGHT = 88;

/** Art-directed landmarks for the current atlas. Keys/coordinates are presentation, not progression. */
const LANDMARKS: Record<string, readonly [number, number]> = {
  "field-logistics": [170, 32],
  "shallow-crossings": [40, 180],
  "corner-transport": [180, 180],
  "belt-junctions": [320, 180],
  "grade-separation": [250, 355],
  "storage-planning": [465, 32],
  "expanded-pack": [465, 230],
  "automated-extraction": [740, 32],
  composition: [620, 170],
  "material-processing": [760, 170],
  "surveyed-construction": [940, 170],
  "mechanical-shaping": [680, 315],
  "machine-tiers": [825, 315],
  hydrology: [970, 315],
  "on-site-power": [1120, 32],
  "sited-generation": [1260, 180],
  transmission: [1200, 315],
  "grid-engineering": [915, 465],
  "steam-works": [1130, 465],
  "fired-masonry": [760, 480],
};

export interface ResearchNode {
  id: number;
  rank: number;
  x: number;
  y: number;
}

export interface ResearchEdge {
  from: number;
  to: number;
  path: string;
}

/** Geometry depends only on the validated catalog, never balances or completion state. */
export function layoutResearch(catalog: Technologies): {
  nodes: ResearchNode[];
  edges: ResearchEdge[];
  width: number;
  height: number;
} {
  const all = new Map(
    catalog.technologies.map((technology) => [technology.id, technology]),
  );
  const ranks = new Map<number, number>();
  const rank = (id: number, visiting = new Set<number>()): number => {
    if (ranks.has(id)) return ranks.get(id)!;
    if (visiting.has(id))
      throw new Error("Research layout requires an acyclic catalog");
    const technology = all.get(id);
    if (!technology) throw new Error(`Unknown prerequisite ${id}`);
    visiting.add(id);
    const value = technology.prerequisites.length
      ? 1 +
        Math.max(
          ...technology.prerequisites.map((parent) => rank(parent, visiting)),
        )
      : 0;
    visiting.delete(id);
    ranks.set(id, value);
    return value;
  };
  const ordered = [...all.values()].sort((a, b) => a.id - b.id);
  let extra = 0;
  const nodes = ordered.map((technology) => {
    const depth = rank(technology.id);
    // Future catalog nodes stay inspectable without borrowing another technology's identity.
    const position = LANDMARKS[technology.key] ?? [
      1450 + Math.floor(extra / 4) * 140,
      60 + (extra++ % 4) * 145,
    ];
    return {
      id: technology.id,
      rank: depth,
      x: position[0]! + 48,
      y: position[1]! + 48,
    };
  });
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const edges: ResearchEdge[] = [];
  for (const technology of ordered) {
    const target = byId.get(technology.id)!;
    for (const id of [...technology.prerequisites].sort((a, b) => a - b)) {
      const source = byId.get(id)!;
      const x1 = source.x + RESEARCH_NODE_WIDTH / 2;
      const y1 = source.y + RESEARCH_NODE_HEIGHT + 5;
      const x2 = target.x + RESEARCH_NODE_WIDTH / 2;
      const y2 = target.y - 5;
      // Separate downward trees, with shared knowledge shown as actual cross-branch links.
      const lane = y1 + (y2 - y1) * (id % 3 === 0 ? 0.65 : 0.45);
      edges.push({
        from: id,
        to: technology.id,
        path: `M ${x1} ${y1} V ${lane} H ${x2} V ${y2}`,
      });
    }
  }
  return {
    nodes,
    edges,
    width:
      Math.max(0, ...nodes.map((node) => node.x)) + RESEARCH_NODE_WIDTH + 48,
    height:
      Math.max(0, ...nodes.map((node) => node.y)) + RESEARCH_NODE_HEIGHT + 48,
  };
}

/** All dependencies of a selected node, for highlighting only; native still decides purchases. */
export function researchAncestors(
  id: number,
  catalog: Technologies,
): Set<number> {
  const visited = new Set<number>();
  const all = new Map(
    catalog.technologies.map((technology) => [technology.id, technology]),
  );
  const visit = (current: number): void => {
    if (visited.has(current)) return;
    visited.add(current);
    all.get(current)?.prerequisites.forEach(visit);
  };
  visit(id);
  return visited;
}

export function researchBenefits(
  technology: TechnologyDefinition,
  definitions: Definitions,
): string[] {
  const benefits = [
    ...technologyBuildingUnlocks(technology).map(
      (id) =>
        definitions.buildings.find((building) => building.id === id)?.name ??
        `Building ${id}`,
    ),
    ...technologyBoundaryUnlocks(technology).map(
      (id) =>
        definitions.boundaries.find((boundary) => boundary.id === id)?.name ??
        `Boundary ${id}`,
    ),
  ];
  const headings = definitions.buildings.filter(
    (building) => building.corner_technology_id === technology.id,
  );
  if (headings.length)
    benefits.push(
      `Six corner headings for ${headings.map((building) => building.name).join(", ")} (building research still required)`,
    );
  const carry = technologyCarrySlotsBonus(technology);
  if (carry) benefits.push(`+${carry} cargo slots`);
  const reach = technologyBuildRangeBonus(technology);
  if (reach) benefits.push(`+${reach} hex construction reach`);
  return benefits;
}

export function researchMatches(
  technology: TechnologyDefinition,
  query: string,
  catalog: Technologies,
  definitions: Definitions,
): boolean {
  const text = [
    technology.name,
    technology.key,
    technology.description,
    technologyContext(technology, catalog),
    ...researchBenefits(technology, definitions),
  ]
    .join(" ")
    .toLowerCase();
  return query
    .toLowerCase()
    .trim()
    .split(/\s+/)
    .every((word) => text.includes(word));
}

export function researchNeighbor(
  nodes: ResearchNode[],
  id: number,
  direction: string,
): number | undefined {
  const origin = nodes.find((node) => node.id === id);
  if (!origin) return undefined;
  const horizontal = direction === "ArrowLeft" || direction === "ArrowRight";
  const sign = direction === "ArrowLeft" || direction === "ArrowUp" ? -1 : 1;
  return nodes
    .filter(
      (node) =>
        node.id !== id &&
        (horizontal ? node.x - origin.x : node.y - origin.y) * sign > 0,
    )
    .sort((a, b) => {
      const score = (node: ResearchNode): number =>
        horizontal
          ? Math.abs(node.x - origin.x) * 4 + Math.abs(node.y - origin.y)
          : Math.abs(node.y - origin.y) + Math.abs(node.x - origin.x) * 4;
      return score(a) - score(b) || a.id - b.id;
    })[0]?.id;
}
