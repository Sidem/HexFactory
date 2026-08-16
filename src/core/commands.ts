import type { AxialCoordinate, HexDirection } from "@hexlife/embed/hex";

export type FactoryCommand =
  | {
      type: "place";
      coordinate: AxialCoordinate;
      definitionId: number;
      orientation: HexDirection;
      recipeId?: number;
    }
  | { type: "erase"; coordinate: AxialCoordinate }
  | { type: "rotate"; coordinate: AxialCoordinate }
  | { type: "tick"; count: number }
  | { type: "reset" };

export interface EncodedCommand {
  opcode: 0 | 1 | 2 | 3 | 4;
  args: number[];
}

export function encodeCommand(command: FactoryCommand): EncodedCommand {
  switch (command.type) {
    case "place":
      return {
        opcode: 0,
        args: [
          command.coordinate.q,
          command.coordinate.r,
          command.definitionId,
          command.orientation,
          command.recipeId ?? 0,
        ],
      };
    case "erase":
      return { opcode: 1, args: [command.coordinate.q, command.coordinate.r] };
    case "rotate":
      return { opcode: 2, args: [command.coordinate.q, command.coordinate.r] };
    case "tick":
      if (!Number.isSafeInteger(command.count) || command.count < 1) {
        throw new RangeError("tick count must be a positive integer");
      }
      return { opcode: 3, args: [command.count] };
    case "reset":
      return { opcode: 4, args: [] };
  }
}
