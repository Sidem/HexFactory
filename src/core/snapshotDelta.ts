import type {
  BuildingsPatch,
  EntitySnapshot,
  FactorySnapshot,
  FactorySnapshotDelta,
  ResourceSnapshot,
  ResourcesPatch,
  TerrainPatch,
  TerrainSnapshot,
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
  delete groups.terrain;
  const next: FactorySnapshot = { ...snapshot, ...groups };
  if (delta.buildings)
    next.buildings = applyBuildingsPatch(snapshot.buildings, delta.buildings);
  if (delta.resources)
    next.resources = applyResourcesPatch(snapshot.resources, delta.resources);
  if (delta.terrain)
    next.terrain = applyTerrainPatch(snapshot.terrain, delta.terrain);
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
 *
 * Cells are addressed by their tile key. An earlier version keyed them by a `u64` native packed
 * from the same two coordinates, which JSON delivers as a double: past 2^53 those ids arrived
 * rounded, several cells shared one, and a single harvest overwrote each of them with a copy of
 * the cell that had actually changed.
 */
export function applyResourcesPatch(
  current: ResourceSnapshot[],
  patch: ResourcesPatch,
): ResourceSnapshot[] {
  if (patch.replace) return patch.changed ?? [];
  const changed = patch.changed ?? [];
  if (changed.length === 0) return current;
  const byKey = new Map(
    changed.map((resource) => [tileKey(resource), resource]),
  );
  return current.map((resource) => byKey.get(tileKey(resource)) ?? resource);
}

/**
 * Merge a per-cell terrain patch. Native world generation is the only path that adds a tile, and
 * nothing ever changes or removes one, so an incremental patch is exactly the chunks surveyed since
 * the host last heard: they append, and every tile already held stays where it is.
 *
 * A key match still substitutes rather than appending twice. That costs one map of the surveyed
 * world on a frame that surveys, which is the frame that was already going to rebuild the terrain
 * mesh, and it means a mark that turns out to repeat a chunk cannot leave the host holding the same
 * cell twice.
 */
export function applyTerrainPatch(
  current: TerrainSnapshot[],
  patch: TerrainPatch,
): TerrainSnapshot[] {
  if (patch.replace) return patch.changed ?? [];
  const changed = patch.changed ?? [];
  if (changed.length === 0) return current;
  const at = new Map(current.map((tile, index) => [tileKey(tile), index]));
  const next = [...current];
  for (const tile of changed) {
    const key = tileKey(tile);
    const index = at.get(key);
    if (index === undefined) {
      at.set(key, next.push(tile) - 1);
      continue;
    }
    next[index] = tile;
  }
  return next;
}

function tileKey(cell: { q: number; r: number }): string {
  return `${cell.q},${cell.r}`;
}
