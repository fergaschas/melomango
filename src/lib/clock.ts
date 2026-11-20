import { useSyncExternalStore } from "react";

/**
 * The playback position, outside React state. It changes about four times a second while
 * music plays, and only the progress bar, the lyrics and the mini player show it: as
 * state, every change re-rendered the whole app (thousands of song rows included).
 */
export class Clock {
  private value: number;
  private listeners = new Set<() => void>();

  constructor(initial = 0) {
    this.value = initial;
  }

  get = (): number => this.value;

  set(seconds: number) {
    if (seconds === this.value) return;
    this.value = seconds;
    this.listeners.forEach((listener) => listener());
  }

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
}

/** The current position in seconds; re-renders only the component that asks for it. */
export function usePlaybackTime(clock: Clock): number {
  return useSyncExternalStore(clock.subscribe, clock.get);
}
