type SoundKind = "green" | "amber" | "red";
/** Frequency (Hz), start and duration (s), waveform, peak volume. */
type Note = [frequency: number, start: number, duration: number, waveform: OscillatorType, volume: number];

/**
 * The sounds the maintainer chose on the vision page (`Cortex-plano`), note
 * for note: change one only with him. `repeat` plays the figure `times`
 * times, `every` seconds apart.
 */
const repeat = (notes: Note[], times: number, every: number): Note[] =>
  Array.from({ length: times }, (_, i) => notes.map(([f, at, ...rest]): Note => [f, at + i * every, ...rest])).flat();

const SOUNDS: Record<SoundKind, { rank: number; notes: Note[] }> = {
  // "Alerta", twice.
  green: {
    rank: 1,
    notes: repeat([[587, 0, 0.16, "triangle", 0.32], [740, 0.16, 0.16, "triangle", 0.32], [988, 0.32, 0.34, "triangle", 0.32]], 2, 0.95),
  },
  // "Batida", twice.
  amber: {
    rank: 2,
    notes: repeat([[196, 0, 0.12, "triangle", 0.5], [196, 0.16, 0.12, "triangle", 0.5], [196, 0.32, 0.12, "triangle", 0.5], [1319, 0.5, 0.35, "sine", 0.22]], 2, 1.05),
  },
  // "Sirene", once (the pair four times).
  red: {
    rank: 3,
    notes: repeat([[740, 0, 0.23, "sawtooth", 0.11], [554, 0.25, 0.23, "sawtooth", 0.11]], 4, 0.5),
  },
};

let active: { context: AudioContext; rank: number; timer?: number } | undefined;

function stop(): void {
  if (!active) return;
  const { context, timer } = active;
  active = undefined;
  window.clearTimeout(timer);
  try {
    void context.close().catch(() => { /* Audio is optional. */ });
  } catch {
    // Some webviews cannot open or close an audio device.
  }
}

/** `force`: a sample the user asked for plays over anything. */
export function playSound(kind: SoundKind, force = false): void {
  const { rank, notes } = SOUNDS[kind];
  if (!force && active && active.rank >= rank) return;
  stop();

  try {
    const context = new AudioContext();
    // Some webviews start a context suspended; resuming is free where allowed.
    void context.resume().catch(() => { /* Audio is optional. */ });
    active = { context, rank };
    const done = (): void => { if (active?.context === context) stop(); };
    let last: OscillatorNode | undefined;
    let end = 0;
    for (const [frequency, at, duration, waveform, volume] of notes) {
      // A plucked note: up in 15 ms, then down exponentially to its end.
      const t = context.currentTime + at;
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      oscillator.type = waveform;
      oscillator.frequency.value = frequency;
      gain.gain.setValueAtTime(0.0001, t);
      gain.gain.exponentialRampToValueAtTime(volume, t + 0.015);
      gain.gain.exponentialRampToValueAtTime(0.0001, t + duration);
      oscillator.connect(gain).connect(context.destination);
      oscillator.start(t);
      oscillator.stop(t + duration + 0.05);
      if (at + duration >= end) {
        end = at + duration;
        last = oscillator;
      }
    }
    // One context per sound, closed after its last note: an open one keeps
    // an audio thread running, and Cortex must use no CPU at rest.
    if (last) last.onended = done;
    // A suspended context may never dispatch an oscillator's ended event. The
    // page clock is not the audio clock: a context that started late is
    // still playing, so wait for what is left of it.
    const endsAt = context.currentTime + end + 0.05;
    const fallback = (): void => {
      if (active?.context !== context) return;
      const left = endsAt - context.currentTime;
      if (context.state === "running" && left > 0) active.timer = window.setTimeout(fallback, left * 1000 + 100);
      else stop();
    };
    active.timer = window.setTimeout(fallback, (end + 0.05) * 1000 + 100);
  } catch {
    stop();
  }
}
