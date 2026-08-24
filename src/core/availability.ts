import type {
  BuildingDefinition,
  FactorySnapshot,
  ItemDefinition,
  TechnologyDefinition,
} from "./types";

/**
 * One line of a cost, with what the player holds against it.
 *
 * A card used to be handed `affordable: false` and nothing else, so it knew the answer and threw
 * away the reason: a player told "no" had to open the cargo pack to find out which of three lines
 * was short and by how much. The shortfall is the value here, because "you need 2 more Iron plate"
 * is a different fact from "no". The boolean below is only its summary.
 */
export interface CostLine {
  item_id: number;
  required: number;
  held: number;
  /** `required - held`, floored at zero. Zero means this line is covered. */
  shortfall: number;
}

export interface BuildAvailability {
  locked: boolean;
  /** Derived from {@link BuildAvailability.cost}: true when no line is short. */
  affordable: boolean;
  costLabel: string;
  cost: CostLine[];
}

/** What the player is carrying of one item, from the inventory native publishes. */
export function heldQuantity(
  snapshot: FactorySnapshot,
  itemId: number,
): number {
  return snapshot.player.inventory[String(itemId)] ?? 0;
}

/** Every line of a bill, priced against the pack. The same shape a recipe or a contract uses. */
export function costLines(
  ingredients: { item_id: number; quantity: number }[],
  snapshot: FactorySnapshot,
): CostLine[] {
  return ingredients.map(({ item_id, quantity }) => {
    const held = heldQuantity(snapshot, item_id);
    return {
      item_id,
      required: quantity,
      held,
      shortfall: Math.max(0, quantity - held),
    };
  });
}

/** Matches `CORNER_START` in the direction table: index 6 begins the six vertex headings. */
const NORTH = 6;

/**
 * What one of this building costs at that heading. Mirrors `BuildingDefinition::cost_at` in the
 * core: a corner step covers twice the ground, so it is priced at what it covers rather than at
 * the edge price, which is the whole reason one belt definition can carry both periods.
 */
export function costAt(
  building: BuildingDefinition,
  orientation: number,
): BuildingDefinition["construction_cost"] {
  return orientation >= NORTH && building.corner_construction_cost
    ? building.corner_construction_cost
    : building.construction_cost;
}

export function buildingAvailability(
  building: BuildingDefinition,
  snapshot: FactorySnapshot,
  items: ItemDefinition[],
  /** The heading the player is holding. An edge heading is what every non-transport tool takes. */
  orientation = 0,
): BuildAvailability {
  // Both gates, because the two-row reach is its own research step: a belt whose corner heading is
  // still locked is buildable, and the card has to say so at the heading actually held.
  const locked =
    (building.unlock_technology_id !== undefined &&
      !snapshot.researched.includes(building.unlock_technology_id)) ||
    (orientation >= NORTH &&
      building.corner_technology_id !== undefined &&
      !snapshot.researched.includes(building.corner_technology_id));
  const bill = costAt(building, orientation);
  const cost = costLines(bill, snapshot);
  // A creative run is charged nothing, so nothing is ever short. The bill is still listed — it is
  // what the building would cost in a priced run, which is the number somebody testing a layout
  // wants to see — but reading it as a refusal would contradict the placement native will allow.
  const affordable =
    snapshot.player.creative || cost.every(({ shortfall }) => shortfall === 0);
  const costLabel = bill.length
    ? bill
        .map(({ item_id, quantity }) => {
          const item = items.find(({ id }) => id === item_id);
          return `${quantity} ${item?.name ?? `#${item_id}`}`;
        })
        .join(" · ")
    : "Scenario only";
  return { locked, affordable, costLabel, cost };
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
