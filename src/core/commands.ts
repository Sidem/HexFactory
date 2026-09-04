import type { NativeInputCommand } from "./types";

export interface EncodedCommand {
  opcode: number;
  args: number[];
}

/**
 * How far from the origin an aim target may sit, matching `MAX_AIM_DISTANCE` in the core. Native
 * measures it from the player and this measures it from the origin, which is the stricter of the
 * two and needs no snapshot to check.
 */
export const MAX_AIM_COORDINATE = 2 ** 30;

/**
 * Half a stack, for the fractional half of a hand transfer.
 *
 * Rounded up and floored at one, so the control is never a no-op: half of one unit has to be one,
 * or a row holding a single item would grow a button that does nothing. The result is still only a
 * ceiling — native clamps it to the stock, the carrying room, and the container's capacity, and
 * reports how much actually moved.
 */
export function halfTransfer(quantity: number): number {
  return Math.max(1, Math.ceil(quantity / 2));
}

const STOCK_CODE = {
  auto: 0,
  inventory: 1,
  input: 2,
  fuel: 3,
  output: 4,
} as const;

export function encodeCommand(command: NativeInputCommand): EncodedCommand {
  switch (command.type) {
    // Two lattice vertices, a shape and one verb. A vertex is a hex and one of its six corners, so
    // the same point can be named three ways and native folds them together — the host never has to
    // decide which hex "owns" the wall it just drew.
    case "boundary_edit":
      if (
        ![command.q, command.r, command.to_q, command.to_r].every(
          (n) => Number.isInteger(n) && Math.abs(n) <= 100000,
        ) ||
        ![command.corner, command.to_corner].every(
          (n) => Number.isInteger(n) && n >= 0 && n <= 5,
        )
      )
        throw new RangeError("Invalid boundary target");
      return {
        opcode: 30,
        args: [
          command.q,
          command.r,
          command.corner,
          command.to_q,
          command.to_r,
          command.to_corner,
          { line: 0, yard: 1 }[command.shape],
          command.definition_id,
          { build: 0, remove: 1, open: 2, close: 3 }[command.action],
        ],
      };
    case "undo_boundary":
      return { opcode: 31, args: [] };

    // Two anchors, a shape, a surface and one verb. What the grade becomes, what it costs, what it
    // buries and whether anything is left stranded are all native's — the host names a selection.
    // Every shape rides the same two anchors, so switching between a floor and the kerb around it,
    // or between a disc and its rim, changes one field and never the gesture that produced them.
    // `cover` is carried rather than defaulted because sealing a deposit is the one change here the
    // player cannot walk back by looking at it, so it only ever travels as a deliberate yes.
    // A live brush adds one more coordinate: the hex whose height the stroke sampled. It rides at
    // the end, present only when a stroke supplied one, so a precise edit encodes as it always did.
    case "ground_edit":
      if (
        ![
          command.q,
          command.r,
          command.to_q,
          command.to_r,
          ...(command.datum ?? []),
        ].every((n) => Number.isInteger(n) && Math.abs(n) <= 100000) ||
        ![command.corner, command.to_corner].every(
          (n) => Number.isInteger(n) && n >= 0 && n <= 5,
        ) ||
        !Number.isInteger(command.steps) ||
        command.steps < 1 ||
        command.steps > 8
      )
        throw new RangeError("Invalid ground target");
      return {
        opcode: 32,
        args: [
          command.q,
          command.r,
          command.corner,
          command.to_q,
          command.to_r,
          command.to_corner,
          { cell: 0, path: 1, rect: 2, frame: 3, disc: 4, ring: 5 }[
            command.shape
          ],
          command.definition_id,
          { pave: 0, clear: 1, raise: 2, lower: 3, level: 4, smooth: 5 }[
            command.action
          ],
          command.cover ? 1 : 0,
          command.steps,
          { first: 0, lowest: 1, highest: 2 }[command.reference],
          ...(command.datum ?? []),
        ],
      };
    case "undo_ground":
      return { opcode: 33, args: [] };
    case "water_edit":
      if (
        ![command.q, command.r].every(
          (n) => Number.isInteger(n) && Math.abs(n) <= 100000,
        ) ||
        !Number.isInteger(command.quanta) ||
        command.quanta < 1 ||
        command.quanta > 32
      )
        throw new RangeError("Invalid water target");
      return {
        opcode: 35,
        args: [
          command.q,
          command.r,
          { flood: 0, drain: 1 }[command.action],
          command.quanta,
        ],
      };

    case "move_intent":
      if (
        !Number.isInteger(command.x) ||
        !Number.isInteger(command.y) ||
        Math.abs(command.x) > 1000 ||
        Math.abs(command.y) > 1000
      )
        throw new RangeError("movement intent must be in -1000..1000");
      return { opcode: 0, args: [command.x, command.y] };
    case "aim":
      // World coordinates, bounded the way native bounds them. The magnitude carries no meaning —
      // native normalizes the delta to the player — so this only refuses what native would refuse.
      if (
        !Number.isInteger(command.x) ||
        !Number.isInteger(command.y) ||
        Math.abs(command.x) > MAX_AIM_COORDINATE ||
        Math.abs(command.y) > MAX_AIM_COORDINATE
      )
        throw new RangeError("aim target must be an integer world position");
      return { opcode: 12, args: [command.x, command.y] };
    case "gather":
      return { opcode: 1, args: [] };
    case "deposit":
      return {
        opcode: 2,
        args: command.item_id !== undefined ? [command.item_id] : [],
      };
    case "place":
      return {
        opcode: 3,
        args: [
          command.q,
          command.r,
          command.definition_id,
          command.orientation,
          command.recipe_id ?? 0,
        ],
      };
    case "erase":
      return { opcode: 4, args: [command.q, command.r] };
    case "rotate":
      return {
        opcode: 5,
        args: [command.q, command.r, command.reverse ? 1 : 0],
      };
    case "set_output_route":
      if (
        ![command.q, command.r, command.output_q, command.output_r].every(
          (value) => Number.isInteger(value) && Math.abs(value) <= 100000,
        ) ||
        !Number.isInteger(command.item_id) ||
        command.item_id <= 0 ||
        command.item_id > 65535 ||
        !Number.isInteger(command.direction) ||
        command.direction < 0 ||
        command.direction > 5
      )
        throw new RangeError("Invalid output route");
      return {
        opcode: 34,
        args: [
          command.q,
          command.r,
          command.item_id,
          command.output_q,
          command.output_r,
          command.direction,
        ],
      };
    case "research":
      return { opcode: 6, args: [command.technology_id] };
    case "place_line":
      return {
        opcode: 7,
        args: [
          command.q,
          command.r,
          command.to_q,
          command.to_r,
          command.definition_id,
          command.orientation,
          command.recipe_id ?? 0,
        ],
      };
    case "erase_line":
      return {
        opcode: 8,
        args: [command.q, command.r, command.to_q, command.to_r],
      };
    case "undo":
      return { opcode: 9, args: [] };
    case "withdraw":
      return {
        opcode: 10,
        args: [
          command.q,
          command.r,
          command.item_id,
          command.quantity,
          STOCK_CODE[command.stock ?? "auto"],
        ],
      };
    case "set_recipe":
      return {
        opcode: 11,
        args: [command.q, command.r, command.recipe_id],
      };
    // No recipe id: the machine already knows which job it is abandoning, and naming a second one
    // here would let the host describe a cancel of something native is not running.
    case "cancel_craft":
      return { opcode: 36, args: [command.q, command.r] };
    case "upgrade":
      return { opcode: 13, args: [command.q, command.r] };
    case "gather_at":
      return { opcode: 14, args: [command.q, command.r] };
    case "store":
      return {
        opcode: 15,
        args: [
          command.q,
          command.r,
          command.item_id,
          command.quantity,
          STOCK_CODE[command.stock ?? "auto"],
        ],
      };
    case "pickup_player_stack":
      return { opcode: 23, args: [command.item_id, command.quantity] };
    case "pickup_building_stack":
      return {
        opcode: 24,
        args: [
          command.q,
          command.r,
          STOCK_CODE[command.stock],
          command.item_id,
          command.quantity,
        ],
      };
    case "place_player_stack":
      return { opcode: 25, args: [command.quantity] };
    case "place_building_stack":
      return {
        opcode: 26,
        args: [
          command.q,
          command.r,
          STOCK_CODE[command.stock],
          command.quantity,
        ],
      };
    case "drop_player_stack":
      return {
        opcode: 27,
        args: [command.q, command.r, command.quantity],
      };
    // A pass names a slot and nothing else. Which row replaces it, and what passing costs, are
    // native's — the host never re-derives the draw order.
    case "skip_request":
      return { opcode: 16, args: [command.slot] };
    // A pull names a project and nothing else. Which slot it displaces is native's, for the same
    // reason the draw order is: the host does not know what the player has committed where.
    case "purchase_skill":
      if (
        !Number.isInteger(command.skill_id) ||
        command.skill_id <= 0 ||
        command.skill_id > 65535
      )
        throw new RangeError("Invalid skill id");
      return { opcode: 29, args: [command.skill_id] };
    case "post_request":
      return { opcode: 28, args: [command.request_id] };
    // The state, not a flip: encoding a toggle would make the opcode stream order-dependent, and
    // two presses of the same button would cancel instead of settling.
    case "set_enabled":
      return {
        opcode: 17,
        args: [command.q, command.r, command.enabled ? 1 : 0],
      };
    // Carried rather than toggled, for the same reason `set_enabled` is.
    case "set_creative":
      return { opcode: 18, args: [command.enabled ? 1 : 0] };
    case "grant":
      return { opcode: 19, args: [command.item_id, command.quantity] };
    // Both arguments are optional and both defaults are "all of it": no item means the whole pack,
    // no quantity means the whole stack. Zero is the wire spelling of an absent item id, which is
    // safe because item ids start at one.
    case "discard":
      return {
        opcode: 20,
        args: [command.item_id ?? 0, command.quantity ?? 0],
      };
    case "set_carry_slots":
      return { opcode: 21, args: [command.slots] };
    // A destination, not a route. Native bounds how far a click may mean and answers with the way
    // there, so there is nothing to validate here that native would not have to validate anyway.
    case "walk_to":
      return { opcode: 22, args: [command.q, command.r] };
  }
}
