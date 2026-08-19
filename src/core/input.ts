import type { NativeInputCommand } from "./types";

export const MAX_INPUT_COMMANDS = 8;
export const MOVEMENT_KEYS = Object.freeze({
  KeyW: { x: 0, y: -1 },
  KeyA: { x: -1, y: 0 },
  KeyS: { x: 0, y: 1 },
  KeyD: { x: 1, y: 0 },
} satisfies Readonly<Record<string, { x: number; y: number }>>);

/**
 * How much of full speed a precision walk is worth.
 *
 * The recorded defect is that walking overshoots a single hex at hold-to-move speed, and the
 * arithmetic says why: at `PLAYER_SPEED` 242 over 30 steps a second the player crosses a hex
 * column in about a quarter of a second, which is a human reaction time. Slowing the walk outright
 * would fix the aim and ruin the travel, so precision is a modifier instead — and it is a modifier
 * native already accepts, because `move_intent` carries a magnitude in thousandths and always did.
 * No native rule moves for this: the host sends a smaller intent, not a smaller step.
 */
export const PRECISION_SCALE = 0.4;

export function movementIntent(
  pressed: ReadonlySet<string>,
  precise = false,
): {
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
  const magnitude = (diagonal ? 707 : 1000) * (precise ? PRECISION_SCALE : 1);
  return {
    type: "move_intent",
    x: Math.round(x * magnitude),
    y: Math.round(y * magnitude),
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
