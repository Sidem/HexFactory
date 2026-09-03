import type { NativeInputCommand } from "./types";

export const MAX_INPUT_COMMANDS = 8;
export const MOVEMENT_KEYS = Object.freeze({
  KeyW: { x: 0, y: -1 },
  KeyA: { x: -1, y: 0 },
  KeyS: { x: 0, y: 1 },
  KeyD: { x: 1, y: 0 },
} satisfies Readonly<Record<string, { x: number; y: number }>>);

/**
 * How much of full native speed an ordinary walk is worth.
 *
 * A hexagon is 25 m², so a neighbour step is 5.373 m. Native `PLAYER_SPEED` 275 over 30 steps a
 * second is 25 m/s at intent 1000 — the run, held on Shift. The walk is 20 m/s, which is 4/5 of
 * that, and it is a smaller intent rather than a smaller step: native already accepts a magnitude
 * in thousandths and always did. No native rule moves for the gait; the host sends 800 or 1000.
 *
 * Only the ratio is this file's business. The speeds are five times what they were before the
 * ground was rescaled because `PLAYER_SPEED` did not move with it; that decision and its reasons
 * live on the constant in `factory-wasm/src/lib.rs`.
 */
export const WALK_SCALE = 0.8;

export function movementIntent(
  pressed: ReadonlySet<string>,
  running = false,
  screenToWorld: (x: number, y: number) => { x: number; y: number } = (
    x,
    y,
  ) => ({ x, y }),
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
  const screenLength = Math.hypot(x, y);
  if (screenLength === 0) return { type: "move_intent", x: 0, y: 0 };
  const world = screenToWorld(x / screenLength, y / screenLength);
  const worldLength = Math.hypot(world.x, world.y);
  if (worldLength === 0) return { type: "move_intent", x: 0, y: 0 };
  const magnitude = 1000 * (running ? 1 : WALK_SCALE);
  return {
    type: "move_intent",
    x: Math.round((world.x / worldLength) * magnitude),
    y: Math.round((world.y / worldLength) * magnitude),
  };
}

export class BoundedInputQueue {
  readonly #commands: NativeInputCommand[] = [];

  enqueue(command: NativeInputCommand): boolean {
    if (this.#commands.length >= MAX_INPUT_COMMANDS) return false;
    this.#commands.push(command);
    return true;
  }

  /** A compound gesture must fit entirely, or leave the queue unchanged. */
  enqueueBatch(commands: readonly NativeInputCommand[]): boolean {
    if (this.#commands.length + commands.length > MAX_INPUT_COMMANDS)
      return false;
    this.#commands.push(...commands);
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
