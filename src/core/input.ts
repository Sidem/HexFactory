import type { NativeInputCommand } from "./types";

export const MAX_INPUT_COMMANDS = 8;
export const MOVEMENT_KEYS: Readonly<Record<string, number>> = Object.freeze({
  KeyD: 0,
  KeyS: 1,
  KeyQ: 2,
  KeyA: 3,
  KeyW: 4,
  KeyE: 5,
});

export class BoundedInputQueue {
  readonly #commands: NativeInputCommand[] = [];

  enqueue(command: NativeInputCommand): boolean {
    if (this.#commands.length >= MAX_INPUT_COMMANDS) return false;
    this.#commands.push(command);
    return true;
  }

  drain(): NativeInputCommand[] {
    return this.#commands.splice(0, MAX_INPUT_COMMANDS);
  }

  clear(): void {
    this.#commands.length = 0;
  }

  get size(): number {
    return this.#commands.length;
  }
}
