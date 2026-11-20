import { memo, useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { formatTime, prefetchStream, trackKey, type Track } from "../lib/api";
import { useLibrary } from "../lib/library";
import { useDownloads } from "../lib/downloads";
import { percent, type DownloadProgress } from "../lib/downloadState";
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

/** What a row does, the same functions for every render (they read the latest props). */
interface RowActions {
  play: (index: number) => void;
  remove: (track: Track, index: number) => void;
  like: (track: Track) => void;
  prefetch: (track: Track) => void;
  cancelPrefetch: () => void;
  dragStart: (index: number) => void;
  dragOver: (index: number) => boolean;
  drop: (index: number) => void;
  dragEnd: () => void;
}

export function TrackList({ tracks, current, isPlaying, onPlay, onRemove, removeLabel, onMove, queue }: Props) {
  const library = useLibrary();
  const downloads = useDownloads();
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [dragOver, setDragOver] = useState<number | null>(null);

  // Rows get stable functions, so they re-render only when what they show changes: a song
  // list can have thousands of rows, and the parent re-renders for any player change.
  const latest = useRef({ onPlay, onRemove, onMove, library, dragFrom });
  latest.current = { onPlay, onRemove, onMove, library, dragFrom };
  const hoverTimer = useRef<number | undefined>(undefined);
  const actions = useMemo<RowActions>(
    () => ({
      play: (i) => latest.current.onPlay(i),
      remove: (t, i) => latest.current.onRemove?.(t, i),
      like: (t) => {
        latest.current.library.toggleLike(t);
      },
      // Resting on a row for a moment resolves its audio, so clicking it plays sooner.
      prefetch: (t) => {
        window.clearTimeout(hoverTimer.current);
        hoverTimer.current = window.setTimeout(() => prefetchStream(t), PREFETCH_HOVER_MS);
      },
      cancelPrefetch: () => window.clearTimeout(hoverTimer.current),
      dragStart: (i) => setDragFrom(i),
      dragOver: (i) => {
        if (latest.current.dragFrom === null) return false;
        setDragOver(i);
        return true;
      },
      drop: (i) => {
        const from = latest.current.dragFrom;
        if (from !== null && from !== i) latest.current.onMove?.(from, i);
        setDragFrom(null);
        setDragOver(null);
      },
      dragEnd: () => {
        setDragFrom(null);
        setDragOver(null);
      },
    }),
    [],
  );
  useEffect(() => () => window.clearTimeout(hoverTimer.current), []);

  // The views pass the whole player as `queue`: keep only its (stable) actions.
  const { playNext, addToQueue, startRadio } = queue ?? {};
  const queueActions = useMemo<QueueActions | undefined>(
    () => (playNext && addToQueue ? { playNext, addToQueue, startRadio } : undefined),
    [playNext, addToQueue, startRadio],
  );
  const currentKey = current ? trackKey(current) : null;

  return (
    <ol className="track-list">
      {tracks.map((track, i) => {
        const key = trackKey(track);
        const active = key === currentKey;
        return (
          <TrackRow
            key={key}
            track={track}
            index={i}
            active={active}
            playing={active && isPlaying}
            liked={library.isLiked(track)}
            downloaded={downloads.isDownloaded(track)}
            progress={downloads.progressOf(track)}
            dropTarget={dragOver === i && dragFrom !== i}
            draggable={!!onMove}
            removable={!!onRemove}
            removeLabel={removeLabel}
            queue={queueActions}
            actions={actions}
          />
        );
      })}
    </ol>
  );
}

interface RowProps {
  track: Track;
  index: number;
  active: boolean;
  playing: boolean;
  liked: boolean;
  downloaded: boolean;
  progress: DownloadProgress | undefined;
  dropTarget: boolean;
  draggable: boolean;
  removable: boolean;
  removeLabel?: string;
  queue?: QueueActions;
  actions: RowActions;
}

const TrackRow = memo(function TrackRow({
  track,
  index: i,
  active,
  playing,
  liked,
  downloaded,
  progress,
  dropTarget,
  draggable,
  removable,
  removeLabel,
  queue,
  actions,
}: RowProps) {
  const onKey = (e: KeyboardEvent) => {
    // Ignore keys typed inside the row's own controls (e.g. the new-playlist input).
    if (e.target !== e.currentTarget) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      actions.play(i);
    }
  };
  const classes = ["track", active && "active", dropTarget && "drop-target"].filter(Boolean).join(" ");
  return (
    <li
      className={classes}
      role="button"
      tabIndex={0}
      onClick={() => actions.play(i)}
      onKeyDown={onKey}
      onMouseEnter={() => !active && actions.prefetch(track)}
      onMouseLeave={actions.cancelPrefetch}
      onFocus={(e) => e.target === e.currentTarget && !active && actions.prefetch(track)}
      onBlur={actions.cancelPrefetch}
      draggable={draggable}
      onDragStart={() => actions.dragStart(i)}
      onDragOver={(e) => actions.dragOver(i) && e.preventDefault()}
      onDragEnd={actions.dragEnd}
      onDrop={(e) => {
        e.preventDefault();
        actions.drop(i);
      }}
    >
      <span className="track-num">
        {playing ? <PauseIcon size={14} /> : <span className="n">{i + 1}</span>}
        {!playing && (
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
            ) : progress && progress.state !== "failed" ? (
              <span className="dl-badge pending" title="Descargando">
                <DownloadIcon size={12} />
                {percent(progress) !== null && ` ${percent(progress)} %`}
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
            actions.like(track);
          }}
          aria-label={liked ? "Quitar de Me gusta" : "Añadir a Me gusta"}
          aria-pressed={liked}
          title={liked ? "Quitar de Me gusta" : "Me gusta"}
        >
          {liked ? <HeartFilledIcon size={18} /> : <HeartIcon size={18} />}
        </button>
        <AddToPlaylistMenu track={track} queue={queue} />
        {removable && (
          <button
            className="row-action"
            onClick={(e) => {
              e.stopPropagation();
              actions.remove(track, i);
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
});
