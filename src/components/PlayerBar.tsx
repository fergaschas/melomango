import { useEffect } from "react";
import { formatTime } from "../lib/api";
import type { Player } from "../hooks/usePlayer";
import { Artwork } from "./Artwork";
import { ArtistLinks } from "./ArtistLinks";
import { useLibrary } from "../lib/library";
import { useMatchFix } from "../lib/matchFix";
import { friendlyError } from "../lib/errors";
import { usePlaybackTime } from "../lib/clock";
import { useStations } from "../lib/stationsContext";
import { StationLogo } from "./StationsView";
import {
  HeartFilledIcon,
  HeartIcon,
  MicIcon,
  MuteIcon,
  NextIcon,
  PauseIcon,
  PlayIcon,
  PrevIcon,
  QueueIcon,
  RepeatIcon,
  RepeatOneIcon,
  MiniIcon,
  ShuffleIcon,
  SwapIcon,
  VolumeIcon,
} from "./Icons";

export type SidePanel = "lyrics" | "queue" | null;

interface Props {
  player: Player;
  panel: SidePanel;
  onTogglePanel: (panel: "lyrics" | "queue") => void;
  onMini: () => void;
}

const REPEAT_LABEL = {
  off: "Repetir (desactivado)",
  all: "Repetir la lista",
  one: "Repetir esta canción",
} as const;

export function PlayerBar({ player, panel, onTogglePanel, onMini }: Props) {
  const { current, live, isPlaying, isLoading, duration, volume, muted, error, hasNext, shuffle, repeat } = player;
  const time = usePlaybackTime(player.clock);
  const library = useLibrary();
  const stations = useStations();
  const stationLiked = live ? stations.isLiked(live.station) : false;
  const stationLikeLabel = stationLiked ? "Quitar de tus emisoras" : "Guardar en tus emisoras";
  const matchFix = useMatchFix();
  const liked = current ? library.isLiked(current) : false;
  const problem = error ? friendlyError(error) : null;

  // The bar only has room for a few words: the whole explanation goes in a toast.
  useEffect(() => {
    if (problem) library.notify(problem.message);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [error]);

  return (
    <footer className="player">
      <div className="now-playing">
        {live ? <StationLogo src={live.station.favicon} /> : <Artwork src={current?.artworkUrl} />}
        <div className="track-text">
          <span className="track-title">{live ? live.station.name : (current?.title ?? "Nada sonando")}</span>
          {problem ? (
            <span
              className="track-artist player-error"
              title={[problem.message, problem.details].filter(Boolean).join("\n\n")}
            >
              <span className="player-error-text">⚠ Error</span>
              <button className="link-btn" onClick={player.reloadCurrent}>
                Reintentar
              </button>
            </span>
          ) : live ? (
            // What the station says is playing, if it says.
            <span className="track-artist" title={live.title ?? undefined}>
              {isLoading ? "Sintonizando…" : (live.title ?? "En directo")}
            </span>
          ) : (
            <span className="track-artist">
              {isLoading ? "Cargando…" : current ? <ArtistLinks artist={current.artist} /> : "Elige algo, melómano"}
            </span>
          )}
        </div>
        {live && (
          <button
            className={`row-action heart${stationLiked ? " on" : ""}`}
            onClick={() => stations.toggleLike(live.station)}
            aria-label={stationLikeLabel}
            aria-pressed={stationLiked}
            title={stationLikeLabel}
          >
            {stationLiked ? <HeartFilledIcon size={20} /> : <HeartIcon size={20} />}
          </button>
        )}
        {current && (
          <button
            className={`row-action heart${liked ? " on" : ""}`}
            onClick={() => library.toggleLike(current)}
            aria-label={liked ? "Quitar de Me gusta" : "Añadir a Me gusta"}
            aria-pressed={liked}
            title={`${liked ? "Quitar de Me gusta" : "Me gusta"} (L)`}
          >
            {liked ? <HeartFilledIcon size={20} /> : <HeartIcon size={20} />}
          </button>
        )}
        {current && matchFix.canFix(current) && (
          <button
            className="row-action"
            onClick={() => matchFix.open(current)}
            aria-label="No es esta canción"
            title="¿No es esta canción? Elige otra versión"
          >
            <SwapIcon size={18} />
          </button>
        )}
      </div>

      <div className="controls">
        <div className="buttons">
          {/* A station has no queue to shuffle or repeat. */}
          {!live && (
            <button
              className={`icon-btn mode-btn${shuffle ? " on" : ""}`}
              onClick={player.toggleShuffle}
              aria-pressed={shuffle}
              aria-label="Aleatorio"
              title={`Aleatorio${shuffle ? " (activado)" : ""} (S)`}
            >
              <ShuffleIcon size={18} />
            </button>
          )}
          <button
            className="icon-btn"
            onClick={player.prev}
            disabled={!player.hasPrev}
            aria-label={live ? "Emisora anterior" : "Anterior"}
            title={live ? "Emisora anterior (Ctrl+←)" : "Anterior (Ctrl+←)"}
          >
            <PrevIcon />
          </button>
          <button
            className="play-btn"
            onClick={player.toggle}
            disabled={!current && !live}
            aria-label={isPlaying ? "Pausa" : "Reproducir"}
            title={`${isPlaying ? "Pausa" : "Reproducir"} (espacio)`}
          >
            {isPlaying ? <PauseIcon size={22} /> : <PlayIcon size={22} />}
          </button>
          <button
            className="icon-btn"
            onClick={player.next}
            disabled={!hasNext}
            aria-label={live ? "Emisora siguiente" : "Siguiente"}
            title={live ? "Emisora siguiente (Ctrl+→)" : "Siguiente (Ctrl+→)"}
          >
            <NextIcon />
          </button>
          {!live && (
            <button
              className={`icon-btn mode-btn${repeat !== "off" ? " on" : ""}`}
              onClick={player.cycleRepeat}
              aria-label={REPEAT_LABEL[repeat]}
              title={`${REPEAT_LABEL[repeat]} (R)`}
            >
              {repeat === "one" ? <RepeatOneIcon size={18} /> : <RepeatIcon size={18} />}
            </button>
          )}
        </div>
        {live ? (
          <div className="seek live-now">
            <span className={`live-dot${isPlaying && !isLoading ? " on" : ""}`} aria-hidden="true" />
            En directo
          </div>
        ) : (
          <div className="seek">
            <span>{formatTime(time)}</span>
            <input
              type="range"
              min={0}
              max={duration || 0}
              step={0.5}
              value={Math.min(time, duration || 0)}
              onChange={(e) => player.seek(Number(e.target.value))}
              disabled={!current}
              aria-label="Posición"
            />
            <span>{formatTime(duration)}</span>
          </div>
        )}
      </div>

      <div className="volume">
        {/* Lyrics and the queue are for songs. */}
        {!live && (
          <button
            className={`icon-btn lyrics-toggle${panel === "lyrics" ? " on" : ""}`}
            onClick={() => onTogglePanel("lyrics")}
            aria-pressed={panel === "lyrics"}
            aria-label="Letra"
            title="Letra (Y)"
          >
            <MicIcon size={18} />
          </button>
        )}
        {!live && (
          <button
            className={`icon-btn lyrics-toggle${panel === "queue" ? " on" : ""}`}
            onClick={() => onTogglePanel("queue")}
            aria-pressed={panel === "queue"}
            aria-label="Cola"
            title="Cola (Q)"
          >
            <QueueIcon size={18} />
          </button>
        )}
        <button className="icon-btn" onClick={onMini} aria-label="Mini reproductor" title="Mini reproductor (siempre encima)">
          <MiniIcon size={18} />
        </button>
        <button
          className="icon-btn"
          onClick={player.toggleMute}
          aria-pressed={muted}
          aria-label={muted ? "Activar sonido" : "Silenciar"}
          title={`${muted ? "Activar sonido" : "Silenciar"} (M)`}
        >
          {muted ? <MuteIcon size={18} /> : <VolumeIcon size={18} />}
        </button>
        <input
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={muted ? 0 : volume}
          onChange={(e) => {
            if (muted) player.toggleMute();
            player.setVolume(Number(e.target.value));
          }}
          aria-label="Volumen"
          title="Volumen (Ctrl+↑ / Ctrl+↓)"
        />
      </div>
    </footer>
  );
}
