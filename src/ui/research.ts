import type { Technologies, TechnologyDefinition } from "../core/types";

/** Stable authored grouping; it never determines purchase eligibility. */
export function orderTechnologies(
  technologies: TechnologyDefinition[],
  catalog: Technologies,
): TechnologyDefinition[] {
  const branchOrder = new Map(
    catalog.branches.map((group) => [group.key, group.order]),
  );
  const stageOrder = new Map(
    catalog.stages.map((group) => [group.key, group.order]),
  );
  const compareKey = (a: string, b: string): number =>
    a < b ? -1 : a > b ? 1 : 0;
  return [...technologies].sort(
    (a, b) =>
      (stageOrder.get(a.stage) ?? 0) - (stageOrder.get(b.stage) ?? 0) ||
      compareKey(a.stage, b.stage) ||
      (branchOrder.get(a.branch) ?? 0) - (branchOrder.get(b.branch) ?? 0) ||
      compareKey(a.branch, b.branch) ||
      a.id - b.id,
  );
}

export function technologyContext(
  technology: TechnologyDefinition,
  catalog: Technologies,
): string {
  const branch = catalog.branches.find(
    (group) => group.key === technology.branch,
  );
  const stage = catalog.stages.find((group) => group.key === technology.stage);
  return `${branch?.name ?? technology.branch} · ${stage?.name ?? technology.stage}`;
}
