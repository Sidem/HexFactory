/**
 * What the inspected hex is holding, and what the hand can take back out of it.
 *
 * Take used to belong to containers alone, which made a burner a one-way slot: fuel went in and
 * only demolition got it back. Every kind that holds stock the player can see now grows the
 * control, because seeing stock you cannot touch is the part that read as a bug.
 *
 * A composer still shows its reserved inputs and still will not hand them back — but that costs no
 * rule here, because native keeps reserved inputs in a different map from `inventory`. What this
 * list shows is free stock, so what it offers is exactly what native will give.
 */
export interface MachineStockSlot {
  key: string;
  item_id?: number;
  quantity: number;
  ghost?: boolean;
  accepts: boolean;
}

/**
 * Slot layout is derived from the current recipe (and kind), not from a second building schema.
 * One expected field per named input, a fuel field when the machine burns, and an output field
 * when it produces — a kiln with clay in the bed still shows an empty coal slot.
 *
 * The `key` is the slot's identity for `syncChildren`, and it deliberately names the item rather
 * than the amount. A slot whose element is replaced when its count changes is a slot that cannot be
 * dragged onto: the press lands on one element and the release resolves against its replacement, so
 * the drop target vanishes from under the pointer while a machine ticks. Keys stable across
 * quantity are what make the grid patch in place, and `tests/ui.test.ts` pins that.
 *
 * The spare drop slot asks one question — would this compartment take an item it is not already
 * holding? — and `perItem` is what makes the answer match native. Native bounds ingredients and
 * fuel per item, so a per-item compartment has its whole capacity waiting for a new item however
 * full its named slots are; a container's store is one shared pool, so a full pool has nothing
 * left. An undefined `capacity` is unbounded. The arithmetic lives here rather than at the call
 * site because it is a drawing decision: native still clamps every transfer, and `tests/host.test.ts`
 * pins that the host never turns a displayed capacity into authority of its own.
 */
export function machineStockSlots(
  stored: { item_id: number; quantity: number }[],
  expected: number[],
  accepts: boolean,
  capacity?: number,
  perItem = false,
): MachineStockSlot[] {
  const byId = new Map(stored.map((entry) => [entry.item_id, entry.quantity]));
  const slots: MachineStockSlot[] = [];
  const seen = new Set<number>();
  for (const item_id of expected) {
    if (item_id <= 0 || seen.has(item_id)) continue;
    seen.add(item_id);
    const quantity = byId.get(item_id) ?? 0;
    slots.push({
      key: `expected-${item_id}`,
      item_id,
      quantity,
      ghost: quantity === 0,
      accepts,
    });
  }
  for (const entry of stored) {
    if (seen.has(entry.item_id)) continue;
    seen.add(entry.item_id);
    slots.push({
      key: `stored-${entry.item_id}`,
      item_id: entry.item_id,
      quantity: entry.quantity,
      accepts,
    });
  }
  const total = stored.reduce((sum, entry) => sum + entry.quantity, 0);
  const room =
    capacity === undefined ? Infinity : perItem ? capacity : capacity - total;
  if (
    accepts &&
    expected.length === 0 &&
    !slots.some((slot) => slot.quantity === 0) &&
    room > 0
  ) {
    slots.push({ key: "drop", quantity: 0, accepts: true });
  }
  if (slots.length === 0) {
    slots.push({ key: "empty", quantity: 0, accepts });
  }
  return slots;
}
