export interface FrameClockState {
  playerActive: boolean;
  playerTicksPerSecond: number;
}

/** One real-time simulation rate. It is a rule of the game, not a player preference. */
export const SIMULATION_TICKS_PER_SECOND = 10;

export interface FrameAdvance {
  elapsed: number;
  ticks: number;
  playerSteps: number;
}

/**
 * Converts elapsed real time into the two bounded native clocks without inventing game state.
 * Factory time always advances at the game's fixed rate; player time accrues only while it has work.
 */
export class FrameClock {
  private previousTime: number;
  private factoryAccumulator = 0;
  private playerAccumulator = 0;

  constructor(now: number) {
    this.previousTime = now;
  }

  update(now: number, state: FrameClockState): FrameAdvance {
    const elapsed = Math.min(250, Math.max(0, now - this.previousTime));
    this.previousTime = now;
    this.factoryAccumulator += elapsed * SIMULATION_TICKS_PER_SECOND;
    if (state.playerActive)
      this.playerAccumulator += elapsed * state.playerTicksPerSecond;
    else this.playerAccumulator = 0;
    return {
      elapsed,
      ticks: Math.min(20, Math.floor(this.factoryAccumulator / 1000)),
      playerSteps: Math.min(20, Math.floor(this.playerAccumulator / 1000)),
    };
  }

  consume(ticks: number, playerSteps: number): void {
    this.factoryAccumulator -= ticks * 1000;
    this.playerAccumulator -= playerSteps * 1000;
  }
}
