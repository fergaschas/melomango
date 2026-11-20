/**
 * Volume normalization: each song's loudness is measured while it plays and a gain brings
 * it to a common level (and is remembered for the next time it plays). A limiter after
 * the gains keeps boosted songs from clipping.
 */

/** Level every song is brought to (dBFS of the gated RMS, about −14 LUFS for music). */
export const TARGET_DB = -14;
const MAX_BOOST_DB = 6;
const MAX_CUT_DB = 12;
/** Quieter frames (silence, fades) don't count towards a song's loudness. */
const SILENCE_DB = -50;
/** Frames needed before trusting a measurement (one every SAMPLE_MS). */
export const MIN_FRAMES = 15;
const SAMPLE_MS = 200;
/** Seconds the gain takes to follow a new measurement (time constant). */
const GAIN_SMOOTHING = 1.5;
/** Most music on YouTube is mastered loud: the guess for a song never measured. */
const DEFAULT_LOUDNESS_DB = -10;
const STORE_KEY = "melomango.loudness";
const MAX_REMEMBERED = 2000;
const SETTING_KEY = "melomango.normalize";

/** RMS level of a block of samples, in dBFS. */
export function rmsDb(samples: ArrayLike<number>): number {
  let sum = 0;
  for (let i = 0; i < samples.length; i++) sum += samples[i] * samples[i];
  const rms = Math.sqrt(sum / Math.max(1, samples.length));
  return rms > 0 ? 20 * Math.log10(rms) : -Infinity;
}

/** Average loudness of the frames heard so far (by energy), ignoring near-silence. */
export class LoudnessMeter {
  private energy = 0;
  frames = 0;

  add(db: number) {
    if (!(db > SILENCE_DB)) return;
    this.energy += 10 ** (db / 10);
    this.frames += 1;
  }

  /** The song's loudness in dB, once enough has been heard. */
  get value(): number | null {
    return this.frames >= MIN_FRAMES ? 10 * Math.log10(this.energy / this.frames) : null;
  }
}

/** Gain (dB) that brings a song of this loudness to the target, within limits. */
export function normalizationGainDb(loudnessDb: number): number {
  return Math.min(MAX_BOOST_DB, Math.max(-MAX_CUT_DB, TARGET_DB - loudnessDb));
}

/** The remembered loudness per song, newest last, keeping at most `max`. */
export function remember(store: Record<string, number>, key: string, db: number, max = MAX_REMEMBERED) {
  const next = { ...store };
  delete next[key];
  next[key] = Math.round(db * 10) / 10;
  const keys = Object.keys(next);
  for (const old of keys.slice(0, Math.max(0, keys.length - max))) delete next[old];
  return next;
}

/** Best guess for a song never measured: the average of the ones that were. */
export function typicalLoudness(store: Record<string, number>): number {
  const values = Object.values(store);
  return values.length ? values.reduce((a, b) => a + b, 0) / values.length : DEFAULT_LOUDNESS_DB;
}

export function loadNormalize(): boolean {
  try {
    return localStorage.getItem(SETTING_KEY) !== "off";
  } catch {
    return true;
  }
}

export function saveNormalize(on: boolean) {
  try {
    localStorage.setItem(SETTING_KEY, on ? "on" : "off");
  } catch {
    /* not remembered */
  }
}

function loadStore(): Record<string, number> {
  try {
    const raw = localStorage.getItem(STORE_KEY);
    return raw ? (JSON.parse(raw) as Record<string, number>) : {};
  } catch {
    return {};
  }
}

interface Channel {
  el: HTMLAudioElement;
  gain: GainNode | null;
  analyser: AnalyserNode | null;
  key: string | null;
  meter: LoudnessMeter;
  /** Remembered loudness of the song, if it was measured before. */
  known: number | null;
}

/** Routes the player's audio elements through Web Audio to even out their loudness. */
export class Loudness {
  private ctx: AudioContext | null = null;
  private bus: GainNode | null = null;
  private limiter: DynamicsCompressorNode | null = null;
  private channels = new Map<HTMLAudioElement, Channel>();
  private store = loadStore();
  private timer: number | null = null;
  private buffer = new Float32Array(2048);
  private enabled: boolean;

  constructor(enabled: boolean) {
    this.enabled = enabled;
  }

  /** Call once per element, before it gets a source. */
  attach(el: HTMLAudioElement) {
    // Without CORS the browser would hand Web Audio silence.
    el.crossOrigin = "anonymous";
    this.channels.set(el, { el, gain: null, analyser: null, key: null, meter: new LoudnessMeter(), known: null });
    // Routed on the first play: the audio context may only start after a user action.
    el.addEventListener("play", () => this.route());
  }

  /** A new song is loading on `el`. */
  setTrack(el: HTMLAudioElement, key: string) {
    const c = this.channels.get(el);
    if (!c) return;
    this.save(c);
    c.key = key;
    c.meter = new LoudnessMeter();
    c.known = this.store[key] ?? null;
    this.applyGain(c, true);
  }

  setEnabled(on: boolean) {
    this.enabled = on;
    this.connectBus();
    this.channels.forEach((c) => this.applyGain(c, true));
  }

  private route() {
    if (typeof AudioContext === "undefined") return;
    if (!this.ctx) {
      try {
        this.ctx = new AudioContext();
      } catch {
        return;
      }
      const ctx = this.ctx;
      this.bus = ctx.createGain();
      this.limiter = ctx.createDynamicsCompressor();
      this.limiter.threshold.value = -1;
      this.limiter.knee.value = 0;
      this.limiter.ratio.value = 20;
      this.limiter.attack.value = 0.003;
      this.limiter.release.value = 0.25;
      this.limiter.connect(ctx.destination);
      this.connectBus();
      // Once routed, a suspended context would mean silence: wake it up.
      ctx.onstatechange = () => {
        if (ctx.state === "suspended") ctx.resume().catch(() => {});
      };
    }
    const ctx = this.ctx;
    const connect = () => {
      this.channels.forEach((c) => {
        if (c.gain) return;
        let source: MediaElementAudioSourceNode;
        try {
          source = ctx.createMediaElementSource(c.el);
        } catch {
          return;
        }
        c.gain = ctx.createGain();
        c.analyser = ctx.createAnalyser();
        c.analyser.fftSize = this.buffer.length;
        source.connect(c.analyser);
        source.connect(c.gain);
        c.gain.connect(this.bus!);
        this.applyGain(c, true);
      });
      if (this.timer === null) this.timer = window.setInterval(() => this.sample(), SAMPLE_MS);
    };
    // Elements keep playing straight to the speakers until the context runs.
    if (ctx.state === "running") connect();
    else
      ctx
        .resume()
        .then(() => ctx.state === "running" && connect())
        .catch(() => {});
  }

  private connectBus() {
    if (!this.ctx || !this.bus || !this.limiter) return;
    this.bus.disconnect();
    this.bus.connect(this.enabled ? this.limiter : this.ctx.destination);
  }

  private sample() {
    this.channels.forEach((c) => {
      const { el, analyser } = c;
      // The element's volume (and the crossfade) is applied before Web Audio: undo it.
      if (!analyser || !c.key || el.paused || el.muted || el.volume < 0.02) return;
      analyser.getFloatTimeDomainData(this.buffer);
      c.meter.add(rmsDb(this.buffer) - 20 * Math.log10(el.volume));
      if (c.meter.frames >= MIN_FRAMES) this.applyGain(c, false);
      if (c.meter.frames > 0 && c.meter.frames % 50 === 0) this.save(c);
    });
  }

  private applyGain(c: Channel, immediate: boolean) {
    if (!c.gain || !this.ctx) return;
    const loudness = c.meter.value ?? c.known ?? typicalLoudness(this.store);
    const db = this.enabled ? normalizationGainDb(loudness) : 0;
    const value = 10 ** (db / 20);
    const now = this.ctx.currentTime;
    if (immediate) {
      c.gain.gain.cancelScheduledValues(now);
      c.gain.gain.setValueAtTime(value, now);
    } else {
      c.gain.gain.setTargetAtTime(value, now, GAIN_SMOOTHING);
    }
  }

  private save(c: Channel) {
    const value = c.meter.value;
    if (!c.key || value === null || c.meter.frames < MIN_FRAMES * 2) return;
    this.store = remember(this.store, c.key, value);
    try {
      localStorage.setItem(STORE_KEY, JSON.stringify(this.store));
    } catch {
      /* not remembered */
    }
  }
}
