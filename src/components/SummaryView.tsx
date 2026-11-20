import { useEffect, useState } from "react";
import { monthSummary, summaryMonths, type MonthSummary } from "../lib/api";
import { useNav } from "../lib/nav";
import type { Player } from "../hooks/usePlayer";
import { Artwork } from "./Artwork";
import { ArtistLinks } from "./ArtistLinks";
import { ErrorNotice } from "./ErrorNotice";
import { ChartIcon, PlayIcon } from "./Icons";

const thisMonth = () => {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
};

/** "2026-11" -> "Noviembre de 2026". */
function monthLabel(month: string) {
  const [y, m] = month.split("-").map(Number);
  const label = new Date(y, m - 1, 1).toLocaleDateString("es-ES", { month: "long", year: "numeric" });
  return label.charAt(0).toUpperCase() + label.slice(1);
}

const number = (n: number) => new Intl.NumberFormat("es-ES").format(n);
const plays = (n: number) => `${number(n)} ${n === 1 ? "escucha" : "escuchas"}`;

/** "Tu resumen del mes": what the user listened to most, month by month. */
export function SummaryView({ player }: { player: Player }) {
  const nav = useNav();
  const [months, setMonths] = useState<string[] | null>(null);
  const [month, setMonth] = useState<string | null>(null);
  const [summary, setSummary] = useState<MonthSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    summaryMonths()
      .then((list) => {
        setMonths(list);
        setMonth((m) => m ?? list[0] ?? thisMonth());
      })
      .catch((e) => setError(String(e)));
  }, [attempt]);

  useEffect(() => {
    if (!month) return;
    let cancelled = false;
    setSummary(null);
    monthSummary(month)
      .then((s) => !cancelled && setSummary(s))
      .catch((e) => !cancelled && setError(String(e)));
    return () => {
      cancelled = true;
    };
  }, [month, attempt]);

  const index = months && month ? months.indexOf(month) : -1;
  const older = months && index >= 0 ? months[index + 1] : undefined;
  const newer = months && index > 0 ? months[index - 1] : undefined;
  const top = summary?.topTracks ?? [];
  const tracks = top.map((t) => t.track);

  return (
    <>
      <header className="collection-header">
        <div className="history-cover" aria-hidden="true">
          <ChartIcon size={64} />
        </div>
        <div className="collection-info">
          <span className="eyebrow">Tu resumen</span>
          <h1>{month ? monthLabel(month) : "…"}</h1>
          <div className="month-nav">
            <button className="btn ghost small" onClick={() => older && setMonth(older)} disabled={!older}>
              ← {older ? monthLabel(older) : "Mes anterior"}
            </button>
            <button className="btn ghost small" onClick={() => newer && setMonth(newer)} disabled={!newer}>
              {newer ? monthLabel(newer) : "Mes siguiente"} →
            </button>
          </div>
        </div>
      </header>

      {error && (
        <ErrorNotice
          error={error}
          onRetry={() => {
            setError(null);
            setAttempt((a) => a + 1);
          }}
        />
      )}
      {!error && month && !summary && <p className="status">Contando tus escuchas…</p>}
      {summary && summary.plays === 0 && (
        <p className="status">
          Aún no hay escuchas en {monthLabel(summary.month).toLowerCase()}. Las canciones cuentan tras sonar 30 segundos.
        </p>
      )}

      {summary && summary.plays > 0 && (
        <>
          <div className="summary-stats">
            <Stat value={number(summary.plays)} label={summary.plays === 1 ? "escucha" : "escuchas"} />
            <Stat value={number(summary.minutes)} label="minutos" />
            <Stat value={number(summary.songs)} label={summary.songs === 1 ? "canción" : "canciones"} />
            <Stat value={number(summary.artists)} label={summary.artists === 1 ? "artista" : "artistas"} />
          </div>

          {top[0] && (
            <section className="catalog-section">
              <h2>Tu canción del mes</h2>
              <div className="song-of-month">
                <Artwork src={top[0].track.artworkUrl} />
                <div className="song-of-month-text">
                  <span className="best-name">{top[0].track.title}</span>
                  <span className="track-artist">
                    <ArtistLinks artist={top[0].track.artist} />
                  </span>
                  <span className="chip-kind">{plays(top[0].plays)}</span>
                </div>
                <button className="btn primary play-all" onClick={() => player.playQueue(tracks, 0)}>
                  <PlayIcon size={16} /> Reproducir tus favoritas
                </button>
              </div>
            </section>
          )}

          <div className="summary-columns">
            <section className="catalog-section">
              <h2>Tus artistas</h2>
              <ol className="ranking">
                {summary.topArtists.map((a, i) => (
                  <li key={a.name}>
                    <span className="rank">{i + 1}</span>
                    <Artwork src={a.artworkUrl} />
                    <button className="artist-link strong ranking-name" onClick={() => nav.openArtist(a.name)}>
                      {a.name}
                    </button>
                    <span className="ranking-count">{plays(a.plays)}</span>
                  </li>
                ))}
              </ol>
            </section>

            <section className="catalog-section">
              <h2>Tus canciones</h2>
              <ol className="ranking">
                {top.map((t, i) => (
                  <li key={`${t.track.source}/${t.track.id}`}>
                    <span className="rank">{i + 1}</span>
                    <Artwork src={t.track.artworkUrl} />
                    <button
                      className="ranking-song"
                      onClick={() => player.playQueue(tracks, i)}
                      title="Reproducir desde aquí"
                    >
                      <span className="track-title">{t.track.title}</span>
                      <span className="track-artist">{t.track.artist}</span>
                    </button>
                    <span className="ranking-count">{plays(t.plays)}</span>
                  </li>
                ))}
              </ol>
            </section>
          </div>
          <p className="credit-line">
            Solo con lo escuchado en este equipo. Los minutos son aproximados (suma de la duración de cada canción).
          </p>
        </>
      )}
    </>
  );
}

function Stat({ value, label }: { value: string; label: string }) {
  return (
    <div className="stat">
      <span className="stat-value">{value}</span>
      <span className="stat-label">{label}</span>
    </div>
  );
}
