import { useCallback, useEffect, useRef, useState } from "react";
import { prefetchStream, radioTracks, recordPlay, streamUrl, trackKey, type Track } from "../lib/api";
import * as q from "../lib/queue";
import {
  clampCrossfade,
  FADE_START_TIMEOUT_MS,
  fadeGains,
  loadCrossfade,
  rescueFade,
  saveCrossfade,
  shouldStartFade,
} from "../lib/crossfade";
import { loadNormalize, Loudness, saveNormalize } from "../lib/loudness";
import { Clock } from "../lib/clock";
import { freshListen, listenTick, parseSaved, type Listened, type Saved } from "../lib/playerState";

const SAVE_KEY = "melomango.player";
/** The position, saved on its own (often) so the queue is not rewritten every few seconds. */
const POSITION_KEY = "melomango.player.position";
const SAVE_EVERY_MS = 5000;
const SEEK_STEP = 5;
const CANT_PLAY = "No se pudo reproducir esta canción";

/** Why the stream failed: the backend answers the stream URL with the reason as text. */
async function streamFailure(src: string): Promise<string | null> {
  try {
    const response = await fetch(src, { headers: { Range: "bytes=0-1" } });
    if (response.ok) return null;
    const text = (await response.text()).trim();
    return text && text.length < 600 && !text.startsWith("<") ? text : null;
  } catch {
    return null;
  }
}
/** Queued songs sent to the radio so it doesn't suggest them again (the latest ones). */
const RADIO_EXCLUDE = 400;

/** A blend in progress: `old` fades out while `incoming` (now the main element) fades in. */
interface Fade {
  old: HTMLAudioElement;
  incoming: HTMLAudioElement;
  /** The volume ramp, once the incoming song sounds. */
  timer: number | null;
  /** Gives up waiting if the incoming song never starts (the volume would stay locked). */
  watchdog: number;
  /** The queue while the old song was current, to go back to it if the new one fails. */
  previous: q.QueueState;
}

/** Stops an element and lets go of its stream. */
function release(el: HTMLAudioElement) {
  el.pause();
  el.removeAttribute("src");
  el.load();
}

function loadSaved(): Saved | null {
  try {
    return parseSaved(localStorage.getItem(SAVE_KEY), localStorage.getItem(POSITION_KEY));
  } catch {
    return null; // storage unavailable
  }
}

export function usePlayer() {
  // Two elements, so the next song can start while the current one fades out (crossfade).
  // `audio` is the one playing the current song; the other is idle or fading out.
  const [normalize, setNormalizeState] = useState(loadNormalize);
  const [loudness] = useState(() => new Loudness(normalize));
  const [elements] = useState(() => {
    const pair = [new Audio(), new Audio()] as const;
    pair.forEach((el) => loudness.attach(el));
    return pair;
  });
  const [audio, setAudio] = useState<HTMLAudioElement>(elements[0]);
  const [saved] = useState(loadSaved);
  const [queue, setQueue] = useState<q.QueueState>(() => {
    try {
      return saved ? q.deserialize(saved.queue) : q.emptyQueue;
    } catch {
      return q.emptyQueue;
    }
  });
  // Bumped on every explicit play so replaying the same track reloads it. 0 = just
  // opened: the saved track is loaded paused, at the saved position.
  const [loadKey, setLoadKey] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [clock] = useState(() => new Clock(saved?.time ?? 0));
  const [duration, setDuration] = useState(0);
  const [volume, setVolume] = useState(saved?.volume ?? 0.8);
  const [muted, setMuted] = useState(saved?.muted ?? false);
  const [autoRadio, setAutoRadio] = useState(saved?.autoRadio ?? true);
  const [radioLoading, setRadioLoading] = useState(false);
  /** Bumped on every seek, for things that follow the position (Discord's progress bar). */
  const [seekCount, setSeekCount] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [crossfade, setCrossfadeState] = useState(loadCrossfade);

  const queueRef = useRef(queue);
  queueRef.current = queue;
  /** The main element, switched right away when a blend starts (before React re-renders). */
  const mainRef = useRef(audio);
  mainRef.current = audio;
  const volumeRef = useRef(volume);
  volumeRef.current = volume;
  const crossfadeRef = useRef(crossfade);
  crossfadeRef.current = crossfade;
  const fade = useRef<Fade | null>(null);
  /** Position to jump to once the restored track's metadata is known. */
  const pendingSeek = useRef<number | null>(null);
  /** False until the user plays something: errors while restoring stay silent. */
  const userStarted = useRef(false);
  /** Bumped to make the same track load again from scratch (another recording was chosen). */
  const reloads = useRef(0);
  /** Seconds actually played of the current load (seeking doesn't count), and whether it was recorded. */
  const listened = useRef<Listened>(freshListen());
  const currentRef = useRef<Track | null>(null);
  const autoRadioRef = useRef(autoRadio);
  autoRadioRef.current = autoRadio;
  /** The radio request in flight (one per seed entry), and the last seed that found nothing. */
  const radioJob = useRef<{ uid: number; promise: Promise<boolean> } | null>(null);
  const radioFailed = useRef<number | null>(null);

  const currentEntry = q.currentItem(queue);
  const current = currentEntry?.track ?? null;
  currentRef.current = current;

  const resetListened = () => {
    listened.current = freshListen();
  };

  /** What to do when `el.play()` is rejected. */
  const onPlayRejected = useCallback((el: HTMLAudioElement) => (e: unknown) => {
    // play() rejects with AbortError when a new track interrupts the previous load: not a failure.
    if (e instanceof DOMException && e.name === "AbortError") return;
    // An element that is no longer the main one (a blend that was given up) says nothing.
    if (el !== mainRef.current) return;
    setError(CANT_PLAY);
    setIsPlaying(false);
    setIsLoading(false);
  }, []);

  /** Ends a blend now: the old song stops and the new one plays at full volume. */
  const endFade = useCallback(() => {
    const f = fade.current;
    if (!f) return;
    fade.current = null;
    if (f.timer !== null) window.clearInterval(f.timer);
    window.clearTimeout(f.watchdog);
    if (f.old !== mainRef.current) release(f.old);
    mainRef.current.volume = volumeRef.current;
  }, []);

  /** Set once `next` exists (it is defined further down and needs what is defined here). */
  const nextRef = useRef<(auto?: boolean) => void>(() => {});
  /** An element whose blend into the next song failed: it doesn't try again. */
  const noFadeFrom = useRef<HTMLAudioElement | null>(null);

  /**
   * The incoming song of a blend failed to load. Instead of silence: the old song becomes
   * the current one again and plays to its end, or, if it already ended, on to the next.
   */
  const abandonFade = useCallback(() => {
    const f = fade.current;
    if (!f) return;
    fade.current = null;
    if (f.timer !== null) window.clearInterval(f.timer);
    window.clearTimeout(f.watchdog);
    if (rescueFade(f.old) === "keep-old") {
      release(f.incoming);
      // Its last seconds would start the same blend again: it just plays to the end.
      noFadeFrom.current = f.old;
      mainRef.current = f.old;
      setAudio(f.old);
      f.old.volume = volumeRef.current;
      queueRef.current = f.previous;
      setQueue(f.previous);
      setDuration(f.old.duration);
      clock.set(f.old.currentTime);
      setIsPlaying(true);
      setIsLoading(false);
      setError(null);
    } else {
      release(f.old);
      nextRef.current(true);
    }
  }, [clock]);

  /** Plays the queue's current entry, from `resumeAt` seconds or the start. */
  const start = useCallback(
    (next: q.QueueState, resumeAt: number | null = null, blend = false) => {
      if (!blend) endFade();
      noFadeFrom.current = null;
      pendingSeek.current = resumeAt;
      userStarted.current = true;
      // Updated right away: what runs next in the same event (e.g. the radio) sees it.
      queueRef.current = next;
      setQueue(next);
      setLoadKey((k) => k + 1);
    },
    [endFade],
  );

  /** Starts the next song on the idle element while the current one keeps playing. */
  const beginFade = useCallback(() => {
    if (fade.current || noFadeFrom.current === mainRef.current) return;
    const result = q.advance(queueRef.current, true);
    if (result === "stop" || result === "same") return; // the radio or a repeat take over at the end
    const old = mainRef.current;
    const incoming = old === elements[0] ? elements[1] : elements[0];
    // If the new song is still silent by then, stop waiting: it plays at full volume
    // whenever it arrives, and the volume control works again.
    const watchdog = window.setTimeout(() => {
      if (fade.current?.incoming === incoming && fade.current.timer === null) endFade();
    }, FADE_START_TIMEOUT_MS);
    fade.current = { old, incoming, timer: null, watchdog, previous: queueRef.current };
    mainRef.current = incoming;
    setAudio(incoming);
    start(result, null, true);
  }, [elements, start, endFade]);

  /** Once the new song sounds, blend the two volumes. */
  const rampFade = useCallback(() => {
    const f = fade.current;
    if (!f || f.timer !== null) return;
    window.clearTimeout(f.watchdog);
    const { old, incoming } = f;
    const left = Number.isFinite(old.duration) ? old.duration - old.currentTime : 0;
    const finished = old.ended || old.paused || left <= 0;
    const secs = crossfadeRef.current;
    const fadeOut = finished ? 0 : Math.min(secs, left);
    // If the old song already ran out (the new one was slow to load), just a short fade in.
    const fadeIn = finished ? 0.3 : Math.max(secs, fadeOut);
    const startedAt = performance.now();
    f.timer = window.setInterval(() => {
      const t = (performance.now() - startedAt) / 1000;
      const g = fadeGains(t, fadeIn, fadeOut);
      incoming.volume = volumeRef.current * g.in;
      if (!old.ended) old.volume = volumeRef.current * g.out;
      if (g.in >= 1 && (g.out <= 0 || old.ended)) endFade();
    }, 50);
  }, [endFade]);

  const setNormalize = useCallback(
    (on: boolean) => {
      setNormalizeState(on);
      saveNormalize(on);
      loudness.setEnabled(on);
    },
    [loudness],
  );

  const setCrossfade = useCallback((secs: number) => {
    const value = clampCrossfade(secs);
    setCrossfadeState(value);
    saveCrossfade(value);
  }, []);

  const playQueue = useCallback((tracks: Track[], index: number) => start(q.play(queueRef.current, tracks, index)), [start]);

  const toggle = useCallback(() => {
    if (!queueRef.current.items.length) return;
    // Nothing loaded (or the restored load failed): load it now, keeping the position.
    if (!audio.src || audio.error) {
      start(queueRef.current, audio.currentTime || pendingSeek.current);
      return;
    }
    userStarted.current = true;
    if (audio.paused) audio.play().catch(onPlayRejected(audio));
    else {
      endFade(); // pausing mid-blend silences both songs
      audio.pause();
    }
  }, [audio, onPlayRejected, start, endFade]);

  /** Appends songs like the last queued one. Resolves whether it found any. */
  const fillRadio = useCallback((): Promise<boolean> => {
    const s = queueRef.current;
    const last = s.items[s.items.length - 1];
    if (!last) return Promise.resolve(false);
    if (radioJob.current?.uid === last.uid) return radioJob.current.promise;
    if (radioFailed.current === last.uid) return Promise.resolve(false);
    const exclude = s.items.slice(-RADIO_EXCLUDE).map((i) => trackKey(i.track));
    setRadioLoading(true);
    const promise = radioTracks(last.track, exclude)
      .then((tracks) => {
        if (!tracks.length) {
          radioFailed.current = last.uid;
          return false;
        }
        // The user may have started another queue while the radio searched.
        const next = q.appendRadioFor(queueRef.current, last.uid, tracks);
        if (!next) return false;
        queueRef.current = next;
        setQueue(next);
        return true;
      })
      .catch(() => {
        radioFailed.current = last.uid;
        return false;
      })
      .finally(() => {
        if (radioJob.current?.uid === last.uid) radioJob.current = null;
        setRadioLoading(false);
      });
    radioJob.current = { uid: last.uid, promise };
    return promise;
  }, []);

  const next = useCallback(
    (auto = false) => {
      const result = q.advance(queueRef.current, auto);
      if (result === "stop") {
        // The queue is over: the radio keeps the music going.
        if (autoRadioRef.current && queueRef.current.repeat === "off") {
          const from = q.currentItem(queueRef.current)?.uid ?? null;
          fillRadio().then((found) => {
            // Only move on if the user didn't pick another song while the radio searched.
            const after = found ? q.advanceAfterRadio(queueRef.current, from) : null;
            if (after) start(after);
          });
        }
        return;
      }
      if (result === "same") {
        resetListened(); // every repetition is a play
        audio.currentTime = 0;
        audio.play().catch(onPlayRejected(audio));
        return;
      }
      start(result);
    },
    [audio, onPlayRejected, start, fillRadio],
  );
  nextRef.current = next;

  const prev = useCallback(() => {
    const s = queueRef.current;
    const previous = q.back(s);
    if (audio.currentTime > 3 || previous === s) {
      audio.currentTime = 0;
      return;
    }
    start(previous);
  }, [audio, start]);

  const seek = useCallback(
    (secs: number) => {
      audio.currentTime = secs;
      clock.set(secs);
    },
    [audio, clock],
  );

  const seekBy = useCallback(
    (delta: number) => {
      if (!audio.src) return;
      const end = Number.isFinite(audio.duration) ? audio.duration : Infinity;
      seek(Math.min(Math.max(0, audio.currentTime + delta), end));
    },
    [audio, seek],
  );

  // Queue editing. Starting from an empty queue plays right away.
  const edit = useCallback(
    (change: (s: q.QueueState) => q.QueueState) => {
      const before = queueRef.current;
      const after = change(before);
      if (before.index < 0 && after.index >= 0) start(after);
      else {
        queueRef.current = after;
        setQueue(after);
      }
    },
    [start],
  );

  const toggleShuffle = useCallback(() => edit((s) => q.setShuffle(s, !s.shuffle)), [edit]);
  const cycleRepeat = useCallback(() => edit(q.cycleRepeat), [edit]);
  const playNext = useCallback((t: Track) => edit((s) => q.playNext(s, t)), [edit]);
  const addToQueue = useCallback((t: Track) => edit((s) => q.addToQueue(s, t)), [edit]);
  const removeFromQueue = useCallback((uid: number) => edit((s) => q.remove(s, uid)), [edit]);
  const moveInQueue = useCallback((from: number, to: number) => edit((s) => q.move(s, from, to)), [edit]);
  const clearUpcoming = useCallback(() => edit(q.clearUpcoming), [edit]);
  const jumpTo = useCallback((uid: number) => start(q.jump(queueRef.current, uid)), [start]);
  /** Loads the current track again (e.g. it now plays from another recording). */
  const reloadCurrent = useCallback(() => {
    if (!queueRef.current.items.length) return;
    reloads.current += 1;
    start(queueRef.current);
  }, [start]);
  const toggleMute = useCallback(() => setMuted((m) => !m), []);
  const toggleAutoRadio = useCallback(() => setAutoRadio((r) => !r), []);
  /** Plays `track` followed by songs like it. */
  const startRadio = useCallback(
    (track: Track) => {
      start(q.play(queueRef.current, [track], 0));
      fillRadio();
    },
    [start, fillRadio],
  );

  // Load the current track: playing it, or (on startup) paused at the saved position.
  useEffect(() => {
    if (!current) return;
    const restoring = loadKey === 0;
    resetListened();
    // A different URL makes the WebView fetch it again instead of reusing what it has.
    loudness.setTrack(audio, trackKey(current));
    audio.src = streamUrl(current) + (reloads.current ? `?r=${reloads.current}` : "");
    // Starting a blend: the new song comes in from silence.
    if (fade.current?.incoming === audio) audio.volume = 0;
    setDuration(current.durationSecs);
    setError(null);
    if (restoring) {
      pendingSeek.current = saved?.time ?? null;
      return;
    }
    clock.set(pendingSeek.current ?? 0);
    setIsLoading(true);
    audio.play().catch(onPlayRejected(audio));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loadKey]);

  useEffect(() => {
    const f = fade.current;
    if (!f) audio.volume = volume;
    // Waiting for the new song: the old one is what sounds, so it takes the change.
    else if (f.timer === null) f.old.volume = volume;
    // Mid-blend the ramp sets both volumes (from volumeRef).
  }, [audio, volume]);

  useEffect(() => {
    elements.forEach((el) => (el.muted = muted));
  }, [elements, muted]);

  // While the last queued song plays, the radio already looks for what comes after.
  const isLast = q.atEnd(queue);
  useEffect(() => {
    if (autoRadio && queue.repeat === "off" && isLast && isPlaying) fillRadio();
  }, [autoRadio, queue.repeat, isLast, isPlaying, currentEntry?.uid, fillRadio]);

  // Once the current track is playing, resolve the next one so skipping to it (or
  // reaching it) starts right away.
  const upcomingTrack = q.peekNext(queue);
  useEffect(() => {
    if (upcomingTrack && isPlaying && !isLoading) prefetchStream(upcomingTrack);
  }, [upcomingTrack, isPlaying, isLoading]);

  // Mirror the <audio> element's state into React.
  useEffect(() => {
    const listeners: [string, () => void][] = [
      [
        "timeupdate",
        () => {
          clock.set(audio.currentTime);
          if (!audio.paused && shouldStartFade(crossfadeRef.current, audio.currentTime, audio.duration)) {
            beginFade();
          }
          // Count real playback time towards the history.
          const track = currentRef.current;
          const length = Number.isFinite(audio.duration) ? audio.duration : (track?.durationSecs ?? 0);
          const { next, counts } = listenTick(listened.current, audio.currentTime, !audio.paused, length);
          listened.current = next;
          if (track && counts) recordPlay(track).catch(() => {});
        },
      ],
      [
        "seeking",
        () => {
          listened.current.last = null;
        },
      ],
      ["durationchange", () => Number.isFinite(audio.duration) && setDuration(audio.duration)],
      [
        "loadedmetadata",
        () => {
          if (pendingSeek.current !== null) {
            audio.currentTime = pendingSeek.current;
            pendingSeek.current = null;
          }
        },
      ],
      ["seeked", () => setSeekCount((n) => n + 1)],
      ["play", () => setIsPlaying(true)],
      ["pause", () => setIsPlaying(false)],
      ["ended", () => next(true)],
      [
        "playing",
        () => {
          setError(null);
          setIsLoading(false);
          if (fade.current?.incoming === audio) rampFade();
        },
      ],
      ["waiting", () => setIsLoading(true)],
      [
        "error",
        () => {
          // The song coming in during a blend failed: don't let the music stop.
          if (fade.current?.incoming === audio) {
            abandonFade();
            return;
          }
          endFade();
          setIsPlaying(false);
          setIsLoading(false);
          // A restored track failing in the background (e.g. offline) is retried on play.
          if (!userStarted.current) return;
          setError(CANT_PLAY);
          const src = audio.src;
          streamFailure(src).then((reason) => {
            if (reason && audio.src === src) setError(reason);
          });
        },
      ],
    ];
    // An element that stopped being the main one (it is fading out) no longer drives the player.
    const guarded = listeners.map(([ev, fn]): [string, () => void] => [ev, () => audio === mainRef.current && fn()]);
    guarded.forEach(([ev, fn]) => audio.addEventListener(ev, fn));
    return () => guarded.forEach(([ev, fn]) => audio.removeEventListener(ev, fn));
  }, [audio, next, beginFade, rampFade, endFade, abandonFade, clock]);

  // Remember the queue, position and volume for next time. The queue (up to thousands of
  // songs) is written only when it changes; while music plays, only the position.
  const savePosition = useCallback(() => {
    try {
      localStorage.setItem(POSITION_KEY, String(pendingSeek.current ?? audio.currentTime));
    } catch {
      /* storage unavailable: not important */
    }
  }, [audio]);

  const save = useCallback(() => {
    try {
      const data: Saved = {
        queue: q.serialize(queueRef.current),
        time: pendingSeek.current ?? audio.currentTime,
        volume: volumeRef.current,
        muted: audio.muted,
        autoRadio: autoRadioRef.current,
      };
      localStorage.setItem(SAVE_KEY, JSON.stringify(data));
      savePosition();
    } catch {
      /* storage unavailable or full: not important */
    }
  }, [audio, savePosition]);

  useEffect(save, [queue, volume, muted, autoRadio, save]);

  useEffect(() => {
    const timer = window.setInterval(() => !audio.paused && savePosition(), SAVE_EVERY_MS);
    audio.addEventListener("pause", savePosition);
    window.addEventListener("beforeunload", savePosition);
    return () => {
      window.clearInterval(timer);
      audio.removeEventListener("pause", savePosition);
      window.removeEventListener("beforeunload", savePosition);
    };
  }, [audio, savePosition]);

  // Media keys and the OS "now playing" panel (SMTC on Windows) via the Media Session API.
  useEffect(() => {
    if (!("mediaSession" in navigator) || !current) return;
    navigator.mediaSession.metadata = new MediaMetadata({
      title: current.title,
      artist: current.artist,
      artwork: current.artworkUrl ? [{ src: current.artworkUrl, sizes: "480x480" }] : [],
    });
  }, [current]);

  useEffect(() => {
    if (!("mediaSession" in navigator)) return;
    const ms = navigator.mediaSession;
    // Separate handlers: a «pause» from the system (or headphones) while already paused
    // must not start the music, as a shared toggle did.
    ms.setActionHandler("play", () => mainRef.current.paused && toggle());
    ms.setActionHandler("pause", () => !mainRef.current.paused && toggle());
    ms.setActionHandler("nexttrack", () => next());
    ms.setActionHandler("previoustrack", prev);
    ms.setActionHandler("seekto", (d) => d.seekTime !== undefined && seek(d.seekTime));
  }, [toggle, next, prev, seek]);

  // The system's progress bar (it moves on its own between updates while playing).
  useEffect(() => {
    if (!("mediaSession" in navigator) || !navigator.mediaSession.setPositionState) return;
    try {
      if (!current || !Number.isFinite(duration) || duration <= 0) {
        navigator.mediaSession.setPositionState();
        return;
      }
      navigator.mediaSession.setPositionState({
        duration,
        position: Math.min(Math.max(0, clock.get()), duration),
        playbackRate: 1,
      });
    } catch {
      /* the position may be past a duration that just changed: the next update fixes it */
    }
  }, [current, duration, isPlaying, seekCount, clock]);

  return {
    current,
    currentUid: currentEntry?.uid ?? null,
    queue: queue.items,
    index: queue.index,
    upcoming: q.upcoming(queue),
    shuffle: queue.shuffle,
    repeat: queue.repeat,
    isPlaying,
    isLoading,
    clock,
    duration,
    volume,
    muted,
    error,
    hasNext:
      q.peekNext(queue) !== null ||
      (queue.repeat === "all" && queue.items.length > 0) ||
      (autoRadio && queue.repeat === "off" && queue.index >= 0),
    autoRadio,
    radioLoading,
    seekCount,
    crossfade,
    setCrossfade,
    normalize,
    setNormalize,
    toggleAutoRadio,
    startRadio,
    playQueue,
    toggle,
    next: () => next(false),
    prev,
    seek,
    seekBy,
    seekStep: SEEK_STEP,
    setVolume,
    toggleMute,
    toggleShuffle,
    cycleRepeat,
    playNext,
    addToQueue,
    removeFromQueue,
    moveInQueue,
    clearUpcoming,
    jumpTo,
    reloadCurrent,
  };
}

export type Player = ReturnType<typeof usePlayer>;
