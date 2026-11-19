import { formatTime, type Track } from "../lib/api";
import { Artwork } from "./Artwork";
import { PauseIcon, PlayIcon } from "./Icons";

interface Props {
  tracks: Track[];
  current: Track | null;
  isPlaying: boolean;
  onPlay: (index: number) => void;
}

export function TrackList({ tracks, current, isPlaying, onPlay }: Props) {
  return (
    <ol className="track-list">
      {tracks.map((track, i) => {
        const active = current?.id === track.id && current.source === track.source;
        return (
          <li key={`${track.source}/${track.id}`}>
            <button className={`track${active ? " active" : ""}`} onClick={() => onPlay(i)}>
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
              <span className="track-dur">{formatTime(track.durationSecs)}</span>
            </button>
          </li>
        );
      })}
    </ol>
  );
}
