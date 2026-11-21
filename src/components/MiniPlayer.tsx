import { formatTime } from "../lib/api";
import { useLibrary } from "../lib/library";
import type { Player } from "../hooks/usePlayer";
import { Artwork } from "./Artwork";
import { ExpandIcon, HeartFilledIcon, HeartIcon, NextIcon, PauseIcon, PlayIcon, PrevIcon } from "./Icons";
import { usePlaybackTime } from "../lib/clock";
import { StationLogo } from "./StationsView";

/** The small always-on-top player: cover, song, controls and progress. */
export function MiniPlayer({ player, onExpand }: { player: Player; onExpand: () => void }) {
  const library = useLibrary();
  const { current, live, isPlaying, isLoading, duration, error, hasNext } = player;
  const time = usePlaybackTime(player.clock);
  const liked = current ? library.isLiked(current) : false;

  return (
    <div className="mini-player">
      {live ? <StationLogo src={live.station.favicon} /> : <Artwork src={current?.artworkUrl} />}
      <div className="mini-main">
        <div className="mini-text">
          <span className="track-title">{live ? live.station.name : (current?.title ?? "Nada sonando")}</span>
          <span className="track-artist">
            {error
              ? "⚠ No se pudo reproducir"
              : live
                ? isLoading
                  ? "Sintonizando…"
                  : (live.title ?? "En directo")
                : isLoading
                  ? "Cargando…"
                  : (current?.artist ?? "Elige algo, melómano")}
          </span>
        </div>
        <div className="mini-controls">
          <button className="icon-btn" onClick={player.prev} disabled={!player.hasPrev} aria-label="Anterior">
            <PrevIcon size={18} />
          </button>
          <button
            className="play-btn mini-play"
            onClick={player.toggle}
            disabled={!current && !live}
            aria-label={isPlaying ? "Pausa" : "Reproducir"}
          >
            {isPlaying ? <PauseIcon size={18} /> : <PlayIcon size={18} />}
          </button>
          <button className="icon-btn" onClick={player.next} disabled={!hasNext} aria-label="Siguiente">
            <NextIcon size={18} />
          </button>
          {current && (
            <button
              className={`row-action heart${liked ? " on" : ""}`}
              onClick={() => library.toggleLike(current)}
              aria-label={liked ? "Quitar de Me gusta" : "Añadir a Me gusta"}
              aria-pressed={liked}
            >
              {liked ? <HeartFilledIcon size={18} /> : <HeartIcon size={18} />}
            </button>
          )}
        </div>
        {live ? (
          <div className="mini-seek live-now">
            <span className={`live-dot${isPlaying && !isLoading ? " on" : ""}`} aria-hidden="true" />
            En directo
          </div>
        ) : (
          <div className="mini-seek">
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
      <button className="icon-btn mini-expand" onClick={onExpand} aria-label="Volver al tamaño normal" title="Volver al tamaño normal">
        <ExpandIcon size={18} />
      </button>
    </div>
  );
}
