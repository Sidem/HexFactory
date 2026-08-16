import type {
  BuildingsPatch,
  EntitySnapshot,
  FactorySnapshot,
  FactorySnapshotDelta,
  ResourceSnapshot,
  ResourcesPatch,
} from "./types";

export function applySnapshotDelta(
  snapshot: FactorySnapshot,
  currentRevision: number,
  delta: FactorySnapshotDelta,
): { snapshot: FactorySnapshot; revision: number } {
  if (delta.base_revision !== currentRevision)
    throw new Error(
      `Snapshot delta revision mismatch: expected ${currentRevision}, received ${delta.base_revision}`,
    );
  if (delta.revision !== currentRevision + 1)
    throw new Error(
      `Snapshot delta revision must advance by one: received ${delta.revision}`,
    );
  const groups = { ...delta } as Record<string, unknown>;
  delete groups.base_revision;
  delete groups.revision;
  delete groups.buildings;
  delete groups.resources;
  const next: FactorySnapshot = { ...snapshot, ...groups };
  if (delta.buildings)
    next.buildings = applyBuildingsPatch(snapshot.buildings, delta.buildings);
  if (delta.resources)
    next.resources = applyResourcesPatch(snapshot.resources, delta.resources);
  return { snapshot: next, revision: delta.revision };
}

/**
 * Merge a per-entity buildings patch into the previous list. The previous list and the patch's
 * `changed` entries both arrive in ascending native entity id order, so one linear pass preserves
 * that order without re-sorting or rebuilding untouched entities.
 */
export function applyBuildingsPatch(
  current: EntitySnapshot[],
  patch: BuildingsPatch,
): EntitySnapshot[] {
  if (patch.replace) return patch.changed ?? [];
  const removed = new Set(patch.removed ?? []);
  const changed = patch.changed ?? [];
  const next: EntitySnapshot[] = [];
  let index = 0;
  const carryBefore = (id: number): void => {
    while (index < current.length) {
      const existing = current[index]!;
      if (existing.id >= id) break;
      if (!removed.has(existing.id)) next.push(existing);
      index += 1;
    }
  };
  for (const entity of changed) {
    carryBefore(entity.id);
    if (index < current.length && current[index]!.id === entity.id) index += 1;
    next.push(entity);
  }
  carryBefore(Number.POSITIVE_INFINITY);
  return next;
}

/**
 * Merge a per-deposit resources patch. Native world generation is the only path that adds a
 * deposit, and it sends `replace` with the complete list, so an incremental patch only ever
 * addresses deposits already present — each is substituted in place and the native ordering the
 * host received survives untouched.
 */
export function applyResourcesPatch(
  current: ResourceSnapshot[],
  patch: ResourcesPatch,
): ResourceSnapshot[] {
  if (patch.replace) return patch.changed ?? [];
  const changed = patch.changed ?? [];
  if (changed.length === 0) return current;
  const byId = new Map(changed.map((resource) => [resource.id, resource]));
  return current.map((resource) => byId.get(resource.id) ?? resource);
}
