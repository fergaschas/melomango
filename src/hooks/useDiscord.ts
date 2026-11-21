import { useEffect, useRef, useState } from "react";
import { discordSet, stationKey, trackKey } from "../lib/api";
import type { Player } from "./usePlayer";

const ENABLED_KEY = "melomango.discord";
const CLEAR_AFTER_MS = 1500;
/** Lets the player settle (new track at 0, restored position) before reading it. */
const SEND_AFTER_MS = 300;

function savedEnabled() {
  try {
    return localStorage.getItem(ENABLED_KEY) !== "0";
  } catch {
    return true;
  }
}

/** Whether to show what is playing in Discord (on by default), remembered per computer. */
export function useDiscordSetting(): [boolean, (on: boolean) => void] {
  const [enabled, setEnabled] = useState(savedEnabled);
  useEffect(() => {
    try {
      localStorage.setItem(ENABLED_KEY, enabled ? "1" : "0");
    } catch {
      /* storage unavailable: not important */
    }
  }, [enabled]);
  return [enabled, setEnabled];
}

/**
 * Keeps Discord's "Escuchando Melomango" in step with the player: shown while a track
 * plays (with its progress), cleared when paused, stopped or turned off.
 */
export function useDiscordPresence(player: Player, enabled: boolean) {
  const { current, live, isPlaying, duration, seekCount } = player;
  // A station counts as changed when what it says is playing changes.
  const key = live ? `${stationKey(live.station)}/${live.title ?? ""}` : current ? trackKey(current) : null;
  // Buffering after a seek or between tracks still counts as playing: Discord allows only a
  // few updates in a row, and clearing and showing it again would make it flicker.
  const playing = enabled && (current !== null || live !== null) && isPlaying;
  const timer = useRef<number | undefined>(undefined);
  // The latest position, read when the update goes out (a new track has reset it by then).
  const latest = useRef({ current, live, duration, clock: player.clock });
  latest.current = { current, live, duration, clock: player.clock };

  useEffect(() => {
    window.clearTimeout(timer.current);
    if (!playing) {
      // Only a pause that lasts clears it: a quick pause/play keeps it shown.
      timer.current = window.setTimeout(() => discordSet(null), CLEAR_AFTER_MS);
      return;
    }
    timer.current = window.setTimeout(() => {
      const { current: track, live: station, duration: length, clock } = latest.current;
      const time = clock.get();
      if (station) {
        // No progress bar: a station has no length.
        discordSet({
          title: station.title ?? station.station.name,
          artist: station.title ? station.station.name : "En directo",
          artworkUrl: station.station.favicon,
          durationSecs: 0,
          positionSecs: 0,
        });
        return;
      }
      if (!track) return;
      discordSet({
        title: track.title,
        artist: track.artist,
        artworkUrl: track.artworkUrl,
        durationSecs: length || track.durationSecs,
        positionSecs: time,
      });
    }, SEND_AFTER_MS);
  }, [playing, key, duration, seekCount]);

  // Clear it when the view goes away (Discord also clears it when the app quits).
  useEffect(
    () => () => {
      window.clearTimeout(timer.current);
      discordSet(null);
    },
    [],
  );
}
