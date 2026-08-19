/**
 * The feedback vocabulary: what the game sounds like when something actually happened.
 *
 * A factory whose gather, delivery, placement, refusal, and completion all feel identical is a
 * factory the player reads entirely through text. The answer here is deliberately *not* a folder of
 * one-off assets — that is a maintenance liability and a download, and it goes stale the moment a
 * new action exists. Every cue is synthesised from the same three-line envelope over a table of
 * specs, the same way a building's drawing is a part list over one walker: a new cue is a data row,
 * not a new file and not a new code path.
 *
 * The context is created on the first cue rather than at boot, because a browser refuses to start
 * one before a gesture and a suspended context that nobody resumes is silence with a warning in the
 * console. Mute is a preference about a room, so it lives in `localStorage` beside the hotbar and
 * is never saved with the game, hashed, or sent.
 */

export type FeedbackCue =
  | "gather"
  | "place"
  | "reject"
  | "deliver"
  | "reward"
  | "unlock"
  | "project";

interface CueSpec {
  /** Notes in Hz, played in order. One note is a blip; three are an arrival. */
  readonly notes: readonly number[];
  readonly type: OscillatorType;
  /** Seconds per note, including its own decay. */
  readonly length: number;
  /** Peak gain before the decay. Kept low: this plays under a game, not over it. */
  readonly gain: number;
  /** Where each note ends, as a multiple of where it started. 1 is a flat tone. */
  readonly bend: number;
}

/**
 * The table. Pitch direction carries the meaning — up is progress, down is refusal — so the cues
 * stay distinguishable to someone who is not listening for them, and the three that fire most
 * often are the three shortest.
 */
const CUES: Record<FeedbackCue, CueSpec> = {
  gather: {
    notes: [620],
    type: "triangle",
    length: 0.09,
    gain: 0.1,
    bend: 0.7,
  },
  place: { notes: [180], type: "square", length: 0.08, gain: 0.07, bend: 0.82 },
  reject: {
    notes: [200, 150],
    type: "sawtooth",
    length: 0.09,
    gain: 0.06,
    bend: 0.9,
  },
  deliver: {
    notes: [523, 784],
    type: "sine",
    length: 0.11,
    gain: 0.11,
    bend: 1,
  },
  // A filled request is paid work, so it rises further than a delivery and stops short of the
  // fanfare a founding stage gets. One more data row, which is the whole claim this table makes.
  reward: {
    notes: [523, 659, 988],
    type: "triangle",
    length: 0.1,
    gain: 0.1,
    bend: 1,
  },
  unlock: {
    notes: [659, 880],
    type: "triangle",
    length: 0.12,
    gain: 0.09,
    bend: 1,
  },
  project: {
    notes: [523, 659, 784, 1047],
    type: "sine",
    length: 0.14,
    gain: 0.12,
    bend: 1,
  },
};

const MUTE_KEY = "hexfactory:muted:v1";

/** One cue may not retrigger faster than this, so a burst of deliveries is a run and not a buzz. */
const REPEAT_GUARD_MS = 70;

export class FeedbackAudio {
  private context: AudioContext | null = null;
  private muted = false;
  private readonly lastPlayed = new Map<FeedbackCue, number>();

  constructor() {
    try {
      this.muted = window.localStorage.getItem(MUTE_KEY) === "1";
    } catch {
      // A storage refusal is not worth failing a boot over; the game starts audible.
    }
  }

  get isMuted(): boolean {
    return this.muted;
  }

  setMuted(value: boolean): void {
    this.muted = value;
    try {
      window.localStorage.setItem(MUTE_KEY, value ? "1" : "0");
    } catch {
      // Same as above: the preference is lost, the session is not.
    }
    if (value) void this.context?.suspend();
    else void this.context?.resume();
  }

  /**
   * Play one cue. Never throws: audio is feedback, and a browser that refuses to make a sound must
   * not take an action down with it.
   */
  play(cue: FeedbackCue): void {
    if (this.muted) return;
    const now = performance.now();
    const last = this.lastPlayed.get(cue) ?? -Infinity;
    if (now - last < REPEAT_GUARD_MS) return;
    this.lastPlayed.set(cue, now);
    const context = this.ensureContext();
    if (!context) return;
    if (context.state === "suspended") void context.resume();
    const spec = CUES[cue];
    spec.notes.forEach((frequency, index) => {
      this.note(
        context,
        spec,
        frequency,
        context.currentTime + index * spec.length,
      );
    });
  }

  private note(
    context: AudioContext,
    spec: CueSpec,
    frequency: number,
    at: number,
  ): void {
    const oscillator = context.createOscillator();
    const gain = context.createGain();
    oscillator.type = spec.type;
    oscillator.frequency.setValueAtTime(frequency, at);
    if (spec.bend !== 1)
      oscillator.frequency.exponentialRampToValueAtTime(
        frequency * spec.bend,
        at + spec.length,
      );
    // An attack of a few milliseconds rather than none: a square wave that starts at full gain
    // clicks, and the click is louder than the note.
    gain.gain.setValueAtTime(0.0001, at);
    gain.gain.exponentialRampToValueAtTime(spec.gain, at + 0.008);
    gain.gain.exponentialRampToValueAtTime(0.0001, at + spec.length);
    oscillator.connect(gain).connect(context.destination);
    oscillator.start(at);
    oscillator.stop(at + spec.length + 0.02);
  }

  private ensureContext(): AudioContext | null {
    if (this.context) return this.context;
    try {
      this.context = new AudioContext();
    } catch {
      this.context = null;
    }
    return this.context;
  }
}

/**
 * Which cue a native event deserves, if any.
 *
 * Native events are the game's own account of what happened, so the sound is chosen from them
 * rather than from wherever the host happened to send a command. That is what keeps a delivery made
 * by a belt and a delivery made by hand sounding the same: they are the same event.
 *
 * Unmatched events are silent on purpose. Every string that reaches the player does not deserve a
 * noise, and a vocabulary that fires on everything says nothing.
 */
export function cueForEvent(event: string): FeedbackCue | null {
  if (!event) return null;
  // Native writes what happened in sentence case and what was refused in lower case, so the
  // refusal cue is chosen by the same rule the player already reads rather than by a list of
  // error strings this file would have to keep in step with the core.
  const first = event[0] ?? "";
  if (first !== first.toUpperCase()) return "reject";
  if (event.startsWith("Gathered") || event.startsWith("Recovered"))
    return "gather";
  if (event.startsWith("Delivered")) return "deliver";
  // The hub paying for a filled request, whether a belt or a hand completed it.
  if (event.includes("the hub pays")) return "reward";
  if (event.startsWith("Researched")) return "unlock";
  if (
    event.includes("landing hub grows") ||
    event.includes("free play continues")
  )
    return "project";
  if (
    event.startsWith("Placed") ||
    event.startsWith("Upgraded") ||
    event.startsWith("Rotated")
  )
    return "place";
  return null;
}
