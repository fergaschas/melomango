import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { setCloseToTray, trayUpdate } from "../lib/api";
import type { Player } from "./usePlayer";

const CLOSE_KEY = "melomango.closeToTray";

function savedCloseToTray() {
  try {
    return localStorage.getItem(CLOSE_KEY) !== "0";
  } catch {
    return true;
  }
}

/** Whether closing the window keeps Melomango playing in the tray (on by default). */
export function useCloseToTraySetting(): [boolean, (on: boolean) => void] {
  const [on, setOn] = useState(savedCloseToTray);
  useEffect(() => {
    try {
      localStorage.setItem(CLOSE_KEY, on ? "1" : "0");
    } catch {
      /* storage unavailable: not important */
    }
    setCloseToTray(on);
  }, [on]);
  return [on, setOn];
}

/** The tray icon's menu controls the player; its tooltip shows the song. */
export function useTray(player: Player) {
  // The listener is registered once and always calls the latest player functions.
  const latest = useRef(player);
  latest.current = player;

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listen<string>("tray-action", ({ payload }) => {
      const p = latest.current;
      if (payload === "toggle") p.toggle();
      else if (payload === "next") p.next();
      else if (payload === "prev") p.prev();
    })
      .then((stop) => (cancelled ? stop() : (unlisten = stop)))
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const { current, live, isPlaying } = player;
  const title = live
    ? `${live.station.name} — ${live.title ?? "En directo"}`
    : current
      ? `${current.title} — ${current.artist}`
      : null;
  useEffect(() => {
    trayUpdate(title, isPlaying);
  }, [title, isPlaying]);
}
