import { useState, type KeyboardEvent } from "react";
import { formatTime, trackKey, type Track } from "../lib/api";
import { useLibrary } from "../lib/library";
import { AddToPlaylistMenu } from "./AddToPlaylistMenu";
import { Artwork } from "./Artwork";
import { CloseIcon, HeartFilledIcon, HeartIcon, PauseIcon, PlayIcon } from "./Icons";

interface Props {
  tracks: Track[];
  current: Track | null;
  isPlaying: boolean;
  onPlay: (index: number) => void;
  /** Shows a "remove" button on each row (e.g. inside a playlist). */
  onRemove?: (track: Track, index: number) => void;
  /** Enables drag & drop reordering. */
  onMove?: (from: number, to: number) => void;
}

export function TrackList({ tracks, current, isPlaying, onPlay, onRemove, onMove }: Props) {
  const library = useLibrary();
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [dragOver, setDragOver] = useState<number | null>(null);

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
                <span className="track-artist">{track.artist}</span>
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
              <AddToPlaylistMenu track={track} />
              {onRemove && (
                <button
                  className="row-action"
                  onClick={(e) => {
                    e.stopPropagation();
                    onRemove(track, i);
                  }}
                  aria-label="Quitar de la playlist"
                  title="Quitar de la playlist"
                >
                  <CloseIcon size={18} />
                </button>
              )}
            </span>
            <span className="track-dur">{formatTime(track.durationSecs)}</span>
          </li>
        );
      })}
    </ol>
  );
}
