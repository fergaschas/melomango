import { Fragment, useState } from "react";
import { formatTime, type Track } from "../lib/api";
import type { Player } from "../hooks/usePlayer";
import { Artwork } from "./Artwork";
import { CloseIcon } from "./Icons";

/** Side panel with the current track and what plays next: reorder, remove, jump. */
export function QueuePanel({ player, onClose }: { player: Player; onClose: () => void }) {
  const { current, upcoming, index, shuffle, repeat, autoRadio, radioLoading } = player;
  const firstRadio = upcoming.findIndex((item) => item.radio);
  const radioWorks = autoRadio && repeat === "off";
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [dragOver, setDragOver] = useState<number | null>(null);

  return (
    <aside className="lyrics-panel side-panel queue-panel" aria-label="Cola de reproducción">
      <header className="lyrics-header">
        <div className="lyrics-heading">
          <span className="eyebrow">Cola</span>
          <span className="lyrics-track">
            {[shuffle && "Aleatorio", repeat === "all" && "Repetir lista", repeat === "one" && "Repetir canción"]
              .filter(Boolean)
              .join(" · ") || "En orden"}
          </span>
        </div>
        <button className="icon-btn" onClick={onClose} aria-label="Cerrar cola">
          <CloseIcon />
        </button>
      </header>

      <label className="radio-switch" title="Cuando se acaba la cola, sigue con canciones parecidas">
        <input type="checkbox" id="auto-radio" checked={autoRadio} onChange={player.toggleAutoRadio} />
        <span className="switch-track" aria-hidden="true">
          <span className="switch-thumb" />
        </span>
        <span>
          <strong>Radio automática</strong>
          <span className="radio-switch-note">
            {repeat !== "off" && autoRadio ? "En pausa mientras repites" : "Al acabar la cola, canciones parecidas"}
          </span>
        </span>
      </label>

      <div className="queue-body">
        {!current && <p className="lyrics-status">La cola está vacía. Pon algo de música.</p>}

        {current && (
          <>
            <p className="queue-section">Sonando</p>
            <QueueRow track={current} active />
          </>
        )}

        {current && (
          <div className="queue-section-head">
            <p className="queue-section">A continuación</p>
            {upcoming.length > 0 && (
              <button className="link-btn" onClick={player.clearUpcoming}>
                Vaciar
              </button>
            )}
          </div>
        )}
        {current && upcoming.length === 0 && (
          <p className="queue-empty">
            {radioLoading
              ? "Buscando canciones parecidas…"
              : repeat === "all"
                ? "Al acabar, la lista vuelve a empezar."
                : radioWorks
                  ? "Al acabar, la radio seguirá con canciones parecidas."
                  : "No hay nada más en la cola. Añade canciones con el botón + de cada una."}
          </p>
        )}

        <ol className="queue-list">
          {upcoming.map((item, i) => {
            const position = index + 1 + i; // index in the whole queue
            return (
              <Fragment key={item.uid}>
              {i === firstRadio && (
                <li className="queue-radio-head" aria-hidden="true">
                  Desde la radio
                </li>
              )}
              <li
                className={dragOver === position && dragFrom !== position ? "drop-target" : undefined}
                draggable
                onDragStart={() => setDragFrom(position)}
                onDragOver={(e) => {
                  if (dragFrom === null) return;
                  e.preventDefault();
                  setDragOver(position);
                }}
                onDragEnd={() => {
                  setDragFrom(null);
                  setDragOver(null);
                }}
                onDrop={(e) => {
                  e.preventDefault();
                  if (dragFrom !== null) player.moveInQueue(dragFrom, position);
                  setDragFrom(null);
                  setDragOver(null);
                }}
              >
                <QueueRow
                  track={item.track}
                  onPlay={() => player.jumpTo(item.uid)}
                  onRemove={() => player.removeFromQueue(item.uid)}
                  // Without a mouse: Alt+↑/↓ moves it (within what is still to play).
                  onMoveBy={(step) => {
                    const to = position + step;
                    if (to > index && to < index + 1 + upcoming.length) player.moveInQueue(position, to);
                  }}
                />
              </li>
              </Fragment>
            );
          })}
        </ol>
        {upcoming.length > 1 && <p className="hint">Arrastra para cambiar el orden (o Alt+↑/↓ con el teclado).</p>}
      </div>
    </aside>
  );
}

function QueueRow({
  track,
  active,
  onPlay,
  onRemove,
  onMoveBy,
}: {
  track: Track;
  active?: boolean;
  onPlay?: () => void;
  onRemove?: () => void;
  onMoveBy?: (step: -1 | 1) => void;
}) {
  return (
    <div className={`queue-row${active ? " active" : ""}`}>
      <button
        className="queue-main"
        onClick={onPlay}
        disabled={!onPlay}
        title={onPlay ? (onMoveBy ? "Reproducir ahora (Alt+↑/↓ para moverla)" : "Reproducir ahora") : undefined}
        aria-keyshortcuts={onMoveBy ? "Alt+ArrowUp Alt+ArrowDown" : undefined}
        onKeyDown={(e) => {
          if (!onMoveBy || !e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
          e.preventDefault();
          const button = e.currentTarget;
          onMoveBy(e.key === "ArrowUp" ? -1 : 1);
          // React moves the row node, and moving a node drops its focus: put it back.
          requestAnimationFrame(() => button.focus());
        }}
      >
        <Artwork src={track.artworkUrl} />
        <span className="track-text">
          <span className="track-title">{track.title}</span>
          <span className="track-artist">{track.artist}</span>
        </span>
      </button>
      <span className="queue-dur">{track.durationSecs > 0 ? formatTime(track.durationSecs) : "–"}</span>
      {onRemove && (
        <button className="row-action" onClick={onRemove} aria-label={`Quitar «${track.title}» de la cola`} title="Quitar de la cola">
          <CloseIcon size={16} />
        </button>
      )}
    </div>
  );
}
