export interface FrameClockState {
  playing: boolean;
  speed: number;
  playerActive: boolean;
  playerTicksPerSecond: number;
}

export interface FrameAdvance {
  elapsed: number;
  ticks: number;
  playerSteps: number;
}

/**
 * Converts elapsed real time into the two bounded native clocks without inventing game state.
 * Factory time scales and pauses; player time does neither and accrues only while it has work.
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
    if (state.playing) this.factoryAccumulator += elapsed * state.speed;
    if (state.playerActive)
      this.playerAccumulator += elapsed * state.playerTicksPerSecond;
    else this.playerAccumulator = 0;
    return {
      elapsed,
      ticks: state.playing
        ? Math.min(20, Math.floor(this.factoryAccumulator / 1000))
        : 0,
      playerSteps: Math.min(20, Math.floor(this.playerAccumulator / 1000)),
    };
  }

  consume(ticks: number, playerSteps: number): void {
    this.factoryAccumulator -= ticks * 1000;
    this.playerAccumulator -= playerSteps * 1000;
  }
}
