import { useEffect, useMemo, useRef, useState } from "react";
import { getLyrics, trackKey, type Lyrics } from "../lib/api";
import type { Player } from "../hooks/usePlayer";
import { CloseIcon } from "./Icons";

const OFFSET_STEP_MS = 500;
/** After the user scrolls by hand, stop following the song for a moment. */
const MANUAL_SCROLL_PAUSE_MS = 3000;

type State =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string }
  | { kind: "ready"; lyrics: Lyrics };

export function LyricsPanel({ player, onClose }: { player: Player; onClose: () => void }) {
  const { current, time } = player;
  const [state, setState] = useState<State>({ kind: "idle" });
  const [offsetMs, setOffsetMs] = useState(0);
  const lineRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const manualScrollAt = useRef(0);
  const key = current ? trackKey(current) : null;

  useEffect(() => {
    setOffsetMs(0);
    if (!current) {
      setState({ kind: "idle" });
      return;
    }
    let cancelled = false;
    setState({ kind: "loading" });
    getLyrics(current)
      .then((lyrics) => !cancelled && setState({ kind: "ready", lyrics }))
      .catch((e) => !cancelled && setState({ kind: "error", message: String(e) }));
    return () => {
      cancelled = true;
    };
    // Only refetch when the track changes, not on every player update.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  const synced = state.kind === "ready" ? state.lyrics.synced : [];

  const activeIndex = useMemo(() => {
    const now = time * 1000 + offsetMs;
    let index = -1;
    for (let i = 0; i < synced.length && synced[i].timeMs <= now; i++) index = i;
    return index;
  }, [synced, time, offsetMs]);

  useEffect(() => {
    if (activeIndex < 0 || Date.now() - manualScrollAt.current < MANUAL_SCROLL_PAUSE_MS) return;
    lineRefs.current[activeIndex]?.scrollIntoView({ block: "center", behavior: "smooth" });
  }, [activeIndex]);

  const markManualScroll = () => {
    manualScrollAt.current = Date.now();
  };

  return (
    <aside className="lyrics-panel" aria-label="Letra">
      <header className="lyrics-header">
        <div className="lyrics-heading">
          <span className="eyebrow">Letra</span>
          {current && <span className="lyrics-track">{current.title} · {current.artist}</span>}
        </div>
        <button className="icon-btn" onClick={onClose} aria-label="Cerrar letra">
          <CloseIcon />
        </button>
      </header>

      <div className="lyrics-body" onWheel={markManualScroll} onTouchMove={markManualScroll}>
        {state.kind === "idle" && <p className="lyrics-status">Pon una canción para ver su letra.</p>}
        {state.kind === "loading" && <p className="lyrics-status">Buscando la letra…</p>}
        {state.kind === "error" && <p className="lyrics-status">No se pudo cargar la letra: {state.message}</p>}
        {state.kind === "ready" && !state.lyrics.found && (
          <p className="lyrics-status">No hay letra para esta canción en LRCLIB.</p>
        )}
        {state.kind === "ready" && state.lyrics.instrumental && <p className="lyrics-status">🎻 Instrumental</p>}

        {synced.length > 0 &&
          synced.map((line, i) => (
            <button
              key={`${line.timeMs}-${i}`}
              ref={(el) => {
                lineRefs.current[i] = el;
              }}
              className={`lyric-line${i === activeIndex ? " active" : ""}${i < activeIndex ? " past" : ""}`}
              onClick={() => player.seek(Math.max(0, (line.timeMs - offsetMs) / 1000))}
              title="Ir a este momento"
            >
              {line.text || "♪"}
            </button>
          ))}

        {state.kind === "ready" && synced.length === 0 && state.lyrics.plain && (
          <>
            <p className="lyrics-note">Letra sin sincronizar</p>
            <p className="lyrics-plain">{state.lyrics.plain}</p>
          </>
        )}
      </div>

      <footer className="lyrics-footer">
        {synced.length > 0 ? (
          <div className="offset" aria-label="Desfase de la letra">
            <button className="btn ghost small" onClick={() => setOffsetMs((o) => o - OFFSET_STEP_MS)} title="La letra va adelantada">
              −0,5 s
            </button>
            <span className="offset-value">{offsetMs === 0 ? "Sin desfase" : `${offsetMs > 0 ? "+" : "−"}${Math.abs(offsetMs / 1000).toFixed(1).replace(".", ",")} s`}</span>
            <button className="btn ghost small" onClick={() => setOffsetMs((o) => o + OFFSET_STEP_MS)} title="La letra va retrasada">
              +0,5 s
            </button>
          </div>
        ) : (
          <span />
        )}
        <span className="credit">Letras de LRCLIB</span>
      </footer>
    </aside>
  );
}
