import type { NativeInputCommand } from "./types";

export interface EncodedCommand {
  opcode: number;
  args: number[];
}

export function encodeCommand(command: NativeInputCommand): EncodedCommand {
  switch (command.type) {
    case "move_intent":
      if (
        !Number.isInteger(command.x) ||
        !Number.isInteger(command.y) ||
        Math.abs(command.x) > 1000 ||
        Math.abs(command.y) > 1000
      )
        throw new RangeError("movement intent must be in -1000..1000");
      return { opcode: 0, args: [command.x, command.y] };
    case "gather":
      return { opcode: 1, args: [] };
    case "deposit":
      return { opcode: 2, args: [] };
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
      return { opcode: 5, args: [command.q, command.r] };
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
        args: [command.q, command.r, command.item_id, command.quantity],
      };
    case "set_recipe":
      return {
        opcode: 11,
        args: [command.q, command.r, command.recipe_id],
      };
  }
}
