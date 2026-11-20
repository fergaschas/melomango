import { useCallback, useEffect, useMemo, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { clearHistory, HISTORY_EVENT, recentPlays, type Play } from "../lib/api";
import { groupByDay } from "../lib/history";
import { useLibrary } from "../lib/library";
import type { Player } from "../hooks/usePlayer";
import { ClockIcon, TrashIcon } from "./Icons";
import { TrackList } from "./TrackList";

/** What the user listened to lately, by day. */
export function HistoryView({ player }: { player: Player }) {
  const library = useLibrary();
  const [plays, setPlays] = useState<Play[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    recentPlays()
      .then(setPlays)
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    load();
    window.addEventListener(HISTORY_EVENT, load);
    return () => window.removeEventListener(HISTORY_EVENT, load);
  }, [load]);

  const days = useMemo(() => groupByDay(plays ?? []), [plays]);

  async function onClear() {
    const ok = await ask("Se borrará todo lo que has escuchado. Tus Me gusta y playlists no cambian.", {
      title: "Borrar historial",
      kind: "warning",
      okLabel: "Borrar",
      cancelLabel: "Cancelar",
    });
    if (!ok) return;
    try {
      await clearHistory();
      setPlays([]);
      library.notify("Historial borrado");
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <>
      <header className="collection-header">
        <div className="history-cover" aria-hidden="true">
          <ClockIcon size={64} />
        </div>
        <div className="collection-info">
          <span className="eyebrow">Colección</span>
          <h1>Historial</h1>
          <p className="subtitle">
            {plays ? `${plays.length} escuchas recientes` : "…"} · Solo se guarda en tu equipo
          </p>
          <div className="collection-actions">
            {plays && plays.length > 0 && (
              <button className="btn ghost" onClick={onClear}>
                <TrashIcon size={16} /> Borrar historial
              </button>
            )}
          </div>
        </div>
      </header>
      {error && <p className="status error">{error}</p>}
      {plays?.length === 0 && (
        <p className="status">Aún no has escuchado nada. Las canciones aparecen aquí tras sonar 30 segundos.</p>
      )}
      {days.map((day) => (
        <section key={day.label} className="history-day">
          <h2 className="history-day-title">
            {day.label}
            <span className="history-day-count">
              {day.plays} {day.plays === 1 ? "escucha" : "escuchas"}
            </span>
          </h2>
          <TrackList
            tracks={day.tracks}
            current={player.current}
            isPlaying={player.isPlaying}
            queue={player}
            onPlay={(i) => player.playQueue(day.tracks, i)}
          />
        </section>
      ))}
    </>
  );
}
