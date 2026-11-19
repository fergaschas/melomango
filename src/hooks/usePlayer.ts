import { useCallback, useEffect, useState } from "react";
import { prefetchStream, streamUrl, type Track } from "../lib/api";

export function usePlayer() {
  const [audio] = useState(() => new Audio());
  const [queue, setQueue] = useState<Track[]>([]);
  const [index, setIndex] = useState(-1);
  // Bumped on every explicit play so replaying the same track reloads it.
  const [loadKey, setLoadKey] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [time, setTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [volume, setVolume] = useState(0.8);
  const [error, setError] = useState<string | null>(null);

  const current = queue[index] ?? null;

  // play() rejects with AbortError when a new track interrupts the previous load: not a failure.
  const onPlayRejected = useCallback((e: unknown) => {
    if (e instanceof DOMException && e.name === "AbortError") return;
    setError("No se pudo reproducir esta canción");
    setIsPlaying(false);
    setIsLoading(false);
  }, []);

  const playQueue = useCallback((tracks: Track[], start: number) => {
    setQueue(tracks);
    setIndex(start);
    setLoadKey((k) => k + 1);
  }, []);

  const toggle = useCallback(() => {
    if (!audio.src) return;
    if (audio.paused) audio.play().catch(onPlayRejected);
    else audio.pause();
  }, [audio, onPlayRejected]);

  const next = useCallback(() => {
    if (index + 1 >= queue.length) return;
    setIndex(index + 1);
    setLoadKey((k) => k + 1);
  }, [index, queue.length]);

  const prev = useCallback(() => {
    if (audio.currentTime > 3 || index <= 0) {
      audio.currentTime = 0;
      return;
    }
    setIndex(index - 1);
    setLoadKey((k) => k + 1);
  }, [audio, index]);

  const seek = useCallback(
    (secs: number) => {
      audio.currentTime = secs;
      setTime(secs);
    },
    [audio],
  );

  // Load and start the current track.
  useEffect(() => {
    if (!current) return;
    audio.src = streamUrl(current);
    setTime(0);
    setDuration(current.durationSecs);
    setError(null);
    setIsLoading(true);
    audio.play().catch(onPlayRejected);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loadKey]);

  // Once the current track is playing, resolve the next one so skipping to it (or
  // reaching it) starts right away.
  const upcoming = queue[index + 1] ?? null;
  useEffect(() => {
    if (upcoming && isPlaying && !isLoading) prefetchStream(upcoming);
  }, [upcoming, isPlaying, isLoading]);

  useEffect(() => {
    audio.volume = volume;
  }, [audio, volume]);

  // Mirror the <audio> element's state into React.
  useEffect(() => {
    const listeners: [string, () => void][] = [
      ["timeupdate", () => setTime(audio.currentTime)],
      ["durationchange", () => Number.isFinite(audio.duration) && setDuration(audio.duration)],
      ["play", () => setIsPlaying(true)],
      ["pause", () => setIsPlaying(false)],
      ["ended", next],
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
          setError("No se pudo reproducir esta canción");
          setIsPlaying(false);
          setIsLoading(false);
        },
      ],
    ];
    listeners.forEach(([ev, fn]) => audio.addEventListener(ev, fn));
    return () => listeners.forEach(([ev, fn]) => audio.removeEventListener(ev, fn));
  }, [audio, next]);

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
    ms.setActionHandler("nexttrack", next);
    ms.setActionHandler("previoustrack", prev);
  }, [toggle, next, prev]);

  return {
    current,
    queue,
    index,
    isPlaying,
    isLoading,
    time,
    duration,
    volume,
    error,
    hasNext: index + 1 < queue.length,
    playQueue,
    toggle,
    next,
    prev,
    seek,
    setVolume,
  };
}

export type Player = ReturnType<typeof usePlayer>;
