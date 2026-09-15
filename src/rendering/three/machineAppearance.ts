import type { Color } from "three";
import type { BuildingDefinition, EntitySnapshot } from "../../core/types";
import type { MachinePartInstance } from "./machineMeshes";
import type { WorldMaterials } from "./materials";
import { trimOf } from "../buildingLook";
export function machineMaterialFor(
  materials: WorldMaterials,
  role: MachinePartInstance["material"],
) {
  switch (role) {
    case "ceramic":
      return materials.machineCeramic;
    case "brass":
      return materials.machineBrass;
    case "dark":
      return materials.machineDark;
    case "structure":
      return materials.machine;
  }
}

export function machinePartColor(
  target: Color,
  scratch: Color,
  instance: MachinePartInstance,
  tier: number,
): Color {
  if (instance.part.model) return target.set(instance.part.model.color);
  target.set(instance.color);
  switch (instance.material) {
    case "ceramic":
      return target.lerp(scratch.set("#d9d1b8"), 0.68);
    case "brass":
      return target.lerp(scratch.set("#bf8948"), 0.78);
    case "dark":
      return target.lerp(scratch.set("#142126"), 0.74);
    case "structure":
      return target.lerp(
        scratch.set(tier > 0 ? trimOf(tier).stroke : "#dcefe6"),
        tier > 0 ? 0.2 : 0.04,
      );
  }
}

type PlumeKind = "smoke" | "steam";

/** Published status decides whether a chimney is live; the effect never guesses simulation work. */
export function plumeFor(
  building: EntitySnapshot,
  definition?: BuildingDefinition,
): PlumeKind | null {
  if (building.status === "generating") {
    if (building.kind === "boiler" || definition?.power_source === "turbine")
      return "steam";
    if (definition?.power_source === "burner") return "smoke";
  }
  if (
    building.status === "composing" &&
    (definition?.recipe_category === "smelting" ||
      definition?.recipe_category === "firing")
  )
    return "smoke";
  return null;
}

/** Effect sockets follow the authored silhouette scale so smoke leaves a chimney, not its deck. */
export function plumeOriginHeight(
  building: EntitySnapshot,
  definition?: BuildingDefinition,
): number {
  if (building.kind === "boiler") return 1.9;
  if (definition?.power_source === "turbine") return 1.85;
  if (definition?.power_source === "burner") return 1.65;
  if (definition?.recipe_category === "smelting") return 1.75;
  if (definition?.recipe_category === "firing") return 1.55;
  return 1.35;
}
