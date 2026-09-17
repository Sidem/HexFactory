import { type InstancedMesh, type Matrix4, type Color } from "three";
import type { ItemDefinition } from "../../core/types";
import { machinePartMatrix, type MachinePartInstance } from "./machineMeshes";
import { elementaryCycle, extractorPose } from "./elementaryAnimation";

export interface PartBucket {
  readonly mesh: InstancedMesh;
  readonly instances: MachinePartInstance[];
  readonly animated: boolean;
}

/** Only visible animation writes upload ranges; returning to view catches up at current time. */
export function updateAnimatedParts(
  buckets: readonly PartBucket[],
  visible: ReadonlySet<number> | undefined,
  now: number,
  reducedMotion: boolean,
  items: ReadonlyMap<number, ItemDefinition>,
  matrix: Matrix4,
  color: Color,
): void {
  for (const bucket of buckets) {
    if (!bucket.animated) continue;
    bucket.mesh.instanceMatrix.clearUpdateRanges();
    bucket.mesh.instanceColor?.clearUpdateRanges();
    let changed = false;
    for (let index = 0; index < bucket.instances.length; index++) {
      const instance = bucket.instances[index]!;
      if (visible && !visible.has(instance.building.id)) continue;
      bucket.mesh.setMatrixAt(
        index,
        machinePartMatrix(instance, now, reducedMotion, matrix),
      );
      bucket.mesh.instanceMatrix.addUpdateRange(index * 16, 16);
      changed = true;
      if (
        instance.part.model?.motion === "load" ||
        instance.part.model?.motion === "stored-output"
      ) {
        const item =
          instance.part.model.motion === "load"
            ? extractorPose(
                instance,
                elementaryCycle(instance, now, reducedMotion),
              ).itemId
            : instance.building.output_inventory?.find(
                (item) => item.quantity > 0,
              )?.item_id;
        bucket.mesh.setColorAt(
          index,
          color.set(items.get(item ?? 0)?.color ?? "#dfb778"),
        );
        if (bucket.mesh.instanceColor) {
          bucket.mesh.instanceColor.addUpdateRange(index * 3, 3);
          bucket.mesh.instanceColor.needsUpdate = true;
        }
      }
    }
    if (changed) bucket.mesh.instanceMatrix.needsUpdate = true;
  }
}
