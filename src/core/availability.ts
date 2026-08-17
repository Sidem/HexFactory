import type {
  BuildingDefinition,
  FactorySnapshot,
  ItemDefinition,
  TechnologyDefinition,
} from "./types";

export interface BuildAvailability {
  locked: boolean;
  affordable: boolean;
  costLabel: string;
}

export function buildingAvailability(
  building: BuildingDefinition,
  snapshot: FactorySnapshot,
  items: ItemDefinition[],
): BuildAvailability {
  const locked =
    building.unlock_technology_id !== undefined &&
    !snapshot.researched.includes(building.unlock_technology_id);
  const affordable = building.construction_cost.every(
    ({ item_id, quantity }) =>
      (snapshot.player.inventory[String(item_id)] ?? 0) >= quantity,
  );
  const costLabel = building.construction_cost.length
    ? building.construction_cost
        .map(({ item_id, quantity }) => {
          const item = items.find(({ id }) => id === item_id);
          return `${quantity} ${item?.name ?? `#${item_id}`}`;
        })
        .join(" · ")
    : "Scenario only";
  return { locked, affordable, costLabel };
}

export function technologyAvailability(
  technology: TechnologyDefinition,
  snapshot: FactorySnapshot,
): { complete: boolean; prerequisitesMet: boolean; affordable: boolean } {
  return {
    complete: snapshot.researched.includes(technology.id),
    prerequisitesMet: technology.prerequisites.every((id) =>
      snapshot.researched.includes(id),
    ),
    affordable: snapshot.insight >= technology.cost,
  };
}
