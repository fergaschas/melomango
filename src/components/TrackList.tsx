import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { formatTime, prefetchStream, trackKey, type Track } from "../lib/api";
import { useLibrary } from "../lib/library";
import { useDownloads } from "../lib/downloads";
import { percent } from "../lib/downloadState";
import { AddToPlaylistMenu, type QueueActions } from "./AddToPlaylistMenu";
import { Artwork } from "./Artwork";
import { ArtistLinks } from "./ArtistLinks";
import { CloseIcon, DownloadDoneIcon, DownloadIcon, HeartFilledIcon, HeartIcon, PauseIcon, PlayIcon } from "./Icons";

interface Props {
  tracks: Track[];
  current: Track | null;
  isPlaying: boolean;
  onPlay: (index: number) => void;
  /** Shows a "remove" button on each row (e.g. inside a playlist). */
  onRemove?: (track: Track, index: number) => void;
  /** What the remove button says (by default, removing from the playlist). */
  removeLabel?: string;
  /** Enables drag & drop reordering. */
  onMove?: (from: number, to: number) => void;
  /** Adds "play next" and "add to queue" to each row's menu. */
  queue?: QueueActions;
}

const PREFETCH_HOVER_MS = 350;

export function TrackList({ tracks, current, isPlaying, onPlay, onRemove, removeLabel, onMove, queue }: Props) {
  const library = useLibrary();
  const downloads = useDownloads();
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [dragOver, setDragOver] = useState<number | null>(null);

  // Resting on a row for a moment resolves its audio, so clicking it plays sooner.
  const hoverTimer = useRef<number | undefined>(undefined);
  const startPrefetch = (track: Track) => {
    window.clearTimeout(hoverTimer.current);
    hoverTimer.current = window.setTimeout(() => prefetchStream(track), PREFETCH_HOVER_MS);
  };
  const cancelPrefetch = () => window.clearTimeout(hoverTimer.current);
  useEffect(() => cancelPrefetch, []);

  const onKey = (e: KeyboardEvent, i: number) => {
    // Ignore keys typed inside the row's own controls (e.g. the new-playlist input).
    if (e.target !== e.currentTarget) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onPlay(i);
    }
  };

  return (
    <ol className="track-list">
      {tracks.map((track, i) => {
        const active = current !== null && trackKey(current) === trackKey(track);
        const liked = library.isLiked(track);
        const downloaded = downloads.isDownloaded(track);
        const loading = downloads.progressOf(track);
        const classes = ["track", active && "active", dragOver === i && dragFrom !== i && "drop-target"]
          .filter(Boolean)
          .join(" ");
        return (
          <li
            key={trackKey(track)}
            className={classes}
            role="button"
            tabIndex={0}
            onClick={() => onPlay(i)}
            onKeyDown={(e) => onKey(e, i)}
            onMouseEnter={() => !active && startPrefetch(track)}
            onMouseLeave={cancelPrefetch}
            onFocus={(e) => e.target === e.currentTarget && !active && startPrefetch(track)}
            onBlur={cancelPrefetch}
            draggable={!!onMove}
            onDragStart={() => setDragFrom(i)}
            onDragOver={(e) => {
              if (dragFrom === null) return;
              e.preventDefault();
              setDragOver(i);
            }}
            onDragEnd={() => {
              setDragFrom(null);
              setDragOver(null);
            }}
            onDrop={(e) => {
              e.preventDefault();
              if (dragFrom !== null && dragFrom !== i) onMove?.(dragFrom, i);
              setDragFrom(null);
              setDragOver(null);
            }}
          >
            <span className="track-num">
              {active && isPlaying ? <PauseIcon size={14} /> : <span className="n">{i + 1}</span>}
              {!(active && isPlaying) && (
                <span className="hover-play">
                  <PlayIcon size={14} />
                </span>
              )}
            </span>
            <span className="track-main">
              <Artwork src={track.artworkUrl} />
              <span className="track-text">
                <span className="track-title">{track.title}</span>
                <span className="track-artist">
                  {downloaded ? (
                    <span className="dl-badge" title="Descargada: suena sin conexión" aria-label="Descargada">
                      <DownloadDoneIcon size={12} />
                    </span>
                  ) : loading && loading.state !== "failed" ? (
                    <span className="dl-badge pending" title="Descargando">
                      <DownloadIcon size={12} />
                      {percent(loading) !== null && ` ${percent(loading)} %`}
                    </span>
                  ) : null}
                  <ArtistLinks artist={track.artist} />
                </span>
              </span>
            </span>
            <span className="track-genre">{track.genre}</span>
            <span className="track-actions">
              <button
                className={`row-action heart${liked ? " on" : ""}`}
                onClick={(e) => {
                  e.stopPropagation();
                  library.toggleLike(track);
                }}
                aria-label={liked ? "Quitar de Me gusta" : "Añadir a Me gusta"}
                aria-pressed={liked}
                title={liked ? "Quitar de Me gusta" : "Me gusta"}
              >
                {liked ? <HeartFilledIcon size={18} /> : <HeartIcon size={18} />}
              </button>
              <AddToPlaylistMenu track={track} queue={queue} />
              {onRemove && (
                <button
                  className="row-action"
                  onClick={(e) => {
                    e.stopPropagation();
                    onRemove(track, i);
                  }}
                  aria-label={removeLabel ?? "Quitar de la playlist"}
                  title={removeLabel ?? "Quitar de la playlist"}
                >
                  <CloseIcon size={18} />
                </button>
              )}
            </span>
            <span className="track-dur">{track.durationSecs > 0 ? formatTime(track.durationSecs) : "–"}</span>
          </li>
        );
      })}
    </ol>
  );
}
