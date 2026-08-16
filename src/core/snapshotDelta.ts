import type { FactorySnapshot, FactorySnapshotDelta } from "./types";

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
  const patch = { ...delta } as Partial<FactorySnapshot> & {
    base_revision?: number;
    revision?: number;
  };
  delete patch.base_revision;
  delete patch.revision;
  return { snapshot: { ...snapshot, ...patch }, revision: delta.revision };
}
