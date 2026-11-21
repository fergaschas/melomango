import { useEffect, useRef } from "react";
import type { Player } from "./usePlayer";
import { describeTarget, ownsKey } from "../lib/keyTarget";

/** Shown in the shortcuts dialog; keep in sync with `useShortcuts`. */
export interface ShortcutHelp {
  /** Key combinations, each a list of keys pressed together. */
  combos: string[][];
  /** Shown between combos: "/" pairs them with the action ("next / previous"). */
  sep?: "/" | "o";
  action: string;
}

export const SHORTCUTS: ShortcutHelp[] = [
  { combos: [["Espacio"]], action: "Reproducir o pausar" },
  { combos: [["Ctrl", "→"], ["Ctrl", "←"]], action: "Siguiente / anterior" },
  { combos: [["→"], ["←"]], action: "Avanzar / retroceder 5 segundos" },
  { combos: [["Ctrl", "↑"], ["Ctrl", "↓"]], action: "Subir / bajar el volumen" },
  { combos: [["M"]], action: "Silenciar" },
  { combos: [["S"]], action: "Aleatorio" },
  { combos: [["R"]], action: "Repetir: lista, canción o nada" },
  { combos: [["L"]], action: "Me gusta en la canción que suena" },
  { combos: [["Q"]], action: "Mostrar u ocultar la cola" },
  { combos: [["Y"]], action: "Mostrar u ocultar la letra" },
  { combos: [["Ctrl", "F"], ["/"]], sep: "o", action: "Buscar" },
  { combos: [["?"]], action: "Ver estos atajos" },
];

const VOLUME_STEP = 0.05;

interface Handlers {
  player: Player;
  onLike: () => void;
  onSearch: () => void;
  onTogglePanel: (panel: "lyrics" | "queue") => void;
  onHelp: () => void;
}

/** Keyboard shortcuts for the whole window (ignored while typing or with a dialog open). */
export function useShortcuts(handlers: Handlers) {
  // Always call the latest handlers without re-registering the listener.
  const ref = useRef(handlers);
  ref.current = handlers;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.altKey || e.metaKey || ownsKey(describeTarget(e.target), e.key)) return;
      if (document.querySelector('[aria-modal="true"]')) return;
      const { player, onLike, onSearch, onTogglePanel, onHelp } = ref.current;
      const ctrl = e.ctrlKey;
      const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;

      const action: (() => void) | undefined = ctrl
        ? {
            ArrowRight: player.next,
            ArrowLeft: player.prev,
            ArrowUp: () => player.setVolume(Math.min(1, player.volume + VOLUME_STEP)),
            ArrowDown: () => player.setVolume(Math.max(0, player.volume - VOLUME_STEP)),
            f: onSearch,
          }[key]
        : {
            " ": player.toggle,
            ArrowRight: () => player.seekBy(player.seekStep),
            ArrowLeft: () => player.seekBy(-player.seekStep),
            m: player.toggleMute,
            s: player.toggleShuffle,
            r: player.cycleRepeat,
            l: onLike,
            q: () => onTogglePanel("queue"),
            y: () => onTogglePanel("lyrics"),
            "/": onSearch,
            "?": onHelp,
          }[key];

      if (!action) return;
      // Also stops Space from scrolling the page.
      e.preventDefault();
      action();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}

