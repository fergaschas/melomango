import { formatTime } from "../lib/api";
import type { Player } from "../hooks/usePlayer";
import { Artwork } from "./Artwork";
import { useLibrary } from "../lib/library";
import { HeartFilledIcon, HeartIcon, NextIcon, PauseIcon, PlayIcon, PrevIcon, VolumeIcon } from "./Icons";

export function PlayerBar({ player }: { player: Player }) {
  const { current, isPlaying, isLoading, time, duration, volume, error, hasNext } = player;
  const library = useLibrary();
  const liked = current ? library.isLiked(current) : false;

  return (
    <footer className="player">
      <div className="now-playing">
        <Artwork src={current?.artworkUrl} />
        <div className="track-text">
          <span className="track-title">{current?.title ?? "Nada sonando"}</span>
          <span className="track-artist">{error ?? (isLoading ? "Cargando…" : current?.artist) ?? "Elige algo, melómano"}</span>
        </div>
        {current && (
          <button
            className={`row-action heart${liked ? " on" : ""}`}
            onClick={() => library.toggleLike(current)}
            aria-label={liked ? "Quitar de Me gusta" : "Añadir a Me gusta"}
            aria-pressed={liked}
          >
            {liked ? <HeartFilledIcon size={20} /> : <HeartIcon size={20} />}
          </button>
        )}
      </div>

      <div className="controls">
        <div className="buttons">
          <button className="icon-btn" onClick={player.prev} disabled={!current} aria-label="Anterior">
            <PrevIcon />
          </button>
          <button className="play-btn" onClick={player.toggle} disabled={!current} aria-label={isPlaying ? "Pausa" : "Reproducir"}>
            {isPlaying ? <PauseIcon size={22} /> : <PlayIcon size={22} />}
          </button>
          <button className="icon-btn" onClick={player.next} disabled={!hasNext} aria-label="Siguiente">
            <NextIcon />
          </button>
        </div>
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
      </div>

      <div className="volume">
        <VolumeIcon size={18} />
        <input
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={volume}
          onChange={(e) => player.setVolume(Number(e.target.value))}
          aria-label="Volumen"
        />
      </div>
    </footer>
  );
}
