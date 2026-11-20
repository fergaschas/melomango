import { useCallback, useEffect, useRef, useState } from "react";
import { prefetchStream, radioTracks, recordPlay, streamUrl, trackKey, type Track } from "../lib/api";
import * as q from "../lib/queue";

const SAVE_KEY = "melomango.player";
const SAVE_EVERY_MS = 5000;
const SEEK_STEP = 5;
/** Queued songs sent to the radio so it doesn't suggest them again (the latest ones). */
const RADIO_EXCLUDE = 400;
/** A track counts as listened after this many seconds of actual playback (or half of it, if shorter). */
const PLAY_COUNTS_AFTER = 30;

interface Saved {
  queue: q.SavedQueue;
  time: number;
  volume: number;
  muted: boolean;
  autoRadio?: boolean;
}

function loadSaved(): Saved | null {
  try {
    const raw = localStorage.getItem(SAVE_KEY);
    return raw ? (JSON.parse(raw) as Saved) : null;
  } catch {
    return null;
  }
}

export function usePlayer() {
  const [audio] = useState(() => new Audio());
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
  const [time, setTime] = useState(saved?.time ?? 0);
  const [duration, setDuration] = useState(0);
  const [volume, setVolume] = useState(saved?.volume ?? 0.8);
  const [muted, setMuted] = useState(saved?.muted ?? false);
  const [autoRadio, setAutoRadio] = useState(saved?.autoRadio ?? true);
  const [radioLoading, setRadioLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const queueRef = useRef(queue);
  queueRef.current = queue;
  /** Position to jump to once the restored track's metadata is known. */
  const pendingSeek = useRef<number | null>(null);
  /** False until the user plays something: errors while restoring stay silent. */
  const userStarted = useRef(false);
  /** Bumped to make the same track load again from scratch (another recording was chosen). */
  const reloads = useRef(0);
  /** Seconds actually played of the current load (seeking doesn't count), and whether it was recorded. */
  const listened = useRef({ secs: 0, last: null as number | null, recorded: false });
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
    listened.current = { secs: 0, last: null, recorded: false };
  };

  const onPlayRejected = useCallback((e: unknown) => {
    // play() rejects with AbortError when a new track interrupts the previous load: not a failure.
    if (e instanceof DOMException && e.name === "AbortError") return;
    setError("No se pudo reproducir esta canción");
    setIsPlaying(false);
    setIsLoading(false);
  }, []);

  /** Plays the queue's current entry, from `resumeAt` seconds or the start. */
  const start = useCallback((next: q.QueueState, resumeAt: number | null = null) => {
    pendingSeek.current = resumeAt;
    userStarted.current = true;
    // Updated right away: what runs next in the same event (e.g. the radio) sees it.
    queueRef.current = next;
    setQueue(next);
    setLoadKey((k) => k + 1);
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
    if (audio.paused) audio.play().catch(onPlayRejected);
    else audio.pause();
  }, [audio, onPlayRejected, start]);

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
        const next = q.appendRadio(queueRef.current, tracks);
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
          fillRadio().then((found) => {
            const after = found ? q.advance(queueRef.current, false) : "stop";
            if (after !== "stop" && after !== "same") start(after);
          });
        }
        return;
      }
      if (result === "same") {
        resetListened(); // every repetition is a play
        audio.currentTime = 0;
        audio.play().catch(onPlayRejected);
        return;
      }
      start(result);
    },
    [audio, onPlayRejected, start, fillRadio],
  );

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
      setTime(secs);
    },
    [audio],
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
    audio.src = streamUrl(current) + (reloads.current ? `?r=${reloads.current}` : "");
    setDuration(current.durationSecs);
    setError(null);
    if (restoring) {
      pendingSeek.current = saved?.time ?? null;
      return;
    }
    setTime(pendingSeek.current ?? 0);
    setIsLoading(true);
    audio.play().catch(onPlayRejected);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loadKey]);

  useEffect(() => {
    audio.volume = volume;
  }, [audio, volume]);

  useEffect(() => {
    audio.muted = muted;
  }, [audio, muted]);

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
          setTime(audio.currentTime);
          // Count real playback time towards the history.
          const l = listened.current;
          const now = audio.currentTime;
          if (!audio.paused && l.last !== null) {
            const delta = now - l.last;
            if (delta > 0 && delta < 2) l.secs += delta;
          }
          l.last = now;
          const track = currentRef.current;
          const length = Number.isFinite(audio.duration) ? audio.duration : (track?.durationSecs ?? 0);
          const needed = Math.min(PLAY_COUNTS_AFTER, length > 0 ? length / 2 : PLAY_COUNTS_AFTER);
          if (track && !l.recorded && l.secs >= needed) {
            l.recorded = true;
            recordPlay(track).catch(() => {});
          }
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
      ["play", () => setIsPlaying(true)],
      ["pause", () => setIsPlaying(false)],
      ["ended", () => next(true)],
      [
        "playing",
        () => {
          setError(null);
          setIsLoading(false);
        },
      ],
      ["waiting", () => setIsLoading(true)],
      [
        "error",
        () => {
          setIsPlaying(false);
          setIsLoading(false);
          // A restored track failing in the background (e.g. offline) is retried on play.
          if (userStarted.current) setError("No se pudo reproducir esta canción");
        },
      ],
    ];
    listeners.forEach(([ev, fn]) => audio.addEventListener(ev, fn));
    return () => listeners.forEach(([ev, fn]) => audio.removeEventListener(ev, fn));
  }, [audio, next]);

  // Remember the queue, position and volume for next time.
  const save = useCallback(() => {
    try {
      const data: Saved = {
        queue: q.serialize(queueRef.current),
        time: pendingSeek.current ?? audio.currentTime,
        volume: audio.volume,
        muted: audio.muted,
        autoRadio: autoRadioRef.current,
      };
      localStorage.setItem(SAVE_KEY, JSON.stringify(data));
    } catch {
      /* storage unavailable or full: not important */
    }
  }, [audio]);

  useEffect(save, [queue, volume, muted, autoRadio, save]);

  useEffect(() => {
    const timer = window.setInterval(() => !audio.paused && save(), SAVE_EVERY_MS);
    audio.addEventListener("pause", save);
    window.addEventListener("beforeunload", save);
    return () => {
      window.clearInterval(timer);
      audio.removeEventListener("pause", save);
      window.removeEventListener("beforeunload", save);
    };
  }, [audio, save]);

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
    ms.setActionHandler("play", toggle);
    ms.setActionHandler("pause", toggle);
    ms.setActionHandler("nexttrack", () => next());
    ms.setActionHandler("previoustrack", prev);
  }, [toggle, next, prev]);

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
    time,
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
