import type { NativeInputCommand } from "./types";

export const MAX_INPUT_COMMANDS = 8;
export const MOVEMENT_KEYS = Object.freeze({
  KeyW: { x: 0, y: -1 },
  KeyA: { x: -1, y: 0 },
  KeyS: { x: 0, y: 1 },
  KeyD: { x: 1, y: 0 },
} satisfies Readonly<Record<string, { x: number; y: number }>>);

export function movementIntent(pressed: ReadonlySet<string>): {
  type: "move_intent";
  x: number;
  y: number;
} {
  let x = 0;
  let y = 0;
  for (const code of pressed) {
    const direction = MOVEMENT_KEYS[code as keyof typeof MOVEMENT_KEYS];
    if (direction) {
      x += direction.x;
      y += direction.y;
    }
  }
  x = Math.max(-1, Math.min(1, x));
  y = Math.max(-1, Math.min(1, y));
  const diagonal = x !== 0 && y !== 0;
  return {
    type: "move_intent",
    x: x * (diagonal ? 707 : 1000),
    y: y * (diagonal ? 707 : 1000),
  };
}

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
