import type { NativeInputCommand } from "./types";

export interface EncodedCommand {
  opcode: number;
  args: number[];
}

export function encodeCommand(command: NativeInputCommand): EncodedCommand {
  switch (command.type) {
    case "move":
      if (
        !Number.isInteger(command.direction) ||
        command.direction < 0 ||
        command.direction > 5
      )
        throw new RangeError("direction must be in 0..6");
      return { opcode: 0, args: [command.direction] };
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
  }
}
