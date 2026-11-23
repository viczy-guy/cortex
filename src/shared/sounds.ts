type SoundKind = "green" | "amber" | "red";
type Note = [frequency: number, start: number, duration: number, waveform: OscillatorType];

const SOUNDS: Record<SoundKind, { rank: number; repeats: number; interval: number; notes: Note[] }> = {
  green: {
    rank: 1, repeats: 2, interval: 0.95,
    notes: [[587, 0, 0.16, "triangle"], [740, 0.16, 0.16, "triangle"], [988, 0.32, 0.34, "triangle"]],
  },
  amber: {
    rank: 2, repeats: 2, interval: 1.05,
    notes: [[196, 0, 0.12, "triangle"], [196, 0.16, 0.12, "triangle"], [196, 0.32, 0.12, "triangle"], [1319, 0.5, 0.35, "sine"]],
  },
  red: {
    rank: 3, repeats: 4, interval: 0.5,
    notes: [[740, 0, 0.23, "sawtooth"], [554, 0.25, 0.23, "sawtooth"]],
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
  const { rank, repeats, interval, notes } = SOUNDS[kind];
  if (!force && active && active.rank >= rank) return;
  stop();

  try {
    const context = new AudioContext();
    // Some webviews start a context suspended; resuming is free where allowed.
    void context.resume().catch(() => { /* Audio is optional. */ });
    active = { context, rank };
    const done = (): void => { if (active?.context === context) stop(); };
    let last: OscillatorNode | undefined;
    const start = context.currentTime + 0.01;
    let end = start;
    for (let i = 0; i < repeats; i++) {
      for (const [frequency, offset, duration, waveform] of notes) {
        const at = start + i * interval + offset;
        end = at + duration;
        const oscillator = context.createOscillator();
        const gain = context.createGain();
        const peak = waveform === "sawtooth" ? 0.08 : 0.2;
        oscillator.type = waveform;
        oscillator.frequency.value = frequency;
        gain.gain.setValueAtTime(0, at);
        gain.gain.linearRampToValueAtTime(peak, at + 0.005);
        gain.gain.setValueAtTime(peak, end - 0.025);
        gain.gain.linearRampToValueAtTime(0, end);
        oscillator.connect(gain);
        gain.connect(context.destination);
        oscillator.start(at);
        oscillator.stop(end);
        last = oscillator;
      }
    }
    if (last) last.onended = done;
    // A suspended context may never dispatch an oscillator's ended event.
    active.timer = window.setTimeout(done, (end - context.currentTime) * 1000 + 100);
  } catch {
    stop();
  }
}
