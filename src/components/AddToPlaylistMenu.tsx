import { useEffect, useRef, useState, type FormEvent } from "react";
import type { Track } from "../lib/api";
import { useLibrary } from "../lib/library";
import { useMatchFix } from "../lib/matchFix";
import { PlayNextIcon, PlusIcon, QueueAddIcon, RadioIcon, SwapIcon } from "./Icons";

export interface QueueActions {
  playNext: (t: Track) => void;
  addToQueue: (t: Track) => void;
  startRadio?: (t: Track) => void;
}

/** "+" button: play next / add to the queue, or add `track` to a playlist (or a new one). */
export function AddToPlaylistMenu({ track, queue }: { track: Track; queue?: QueueActions }) {
  const library = useLibrary();
  const matchFix = useMatchFix();
  const [open, setOpen] = useState(false);
  const [newName, setNewName] = useState("");
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  async function createAndAdd(e: FormEvent) {
    e.preventDefault();
    if (!newName.trim()) return;
    const playlist = await library.createPlaylist(newName);
    await library.addToPlaylist(playlist, track);
    setNewName("");
    setOpen(false);
  }

  return (
    <div className="menu-anchor" ref={ref} onClick={(e) => e.stopPropagation()}>
      <button
        className={`row-action${open ? " on" : ""}`}
        onClick={() => setOpen((o) => !o)}
        aria-label="Añadir a la cola o a una playlist"
        title="Añadir a la cola o a una playlist"
      >
        <PlusIcon size={18} />
      </button>
      {open && (
        <div className="menu" role="menu">
          {queue && (
            <>
              <button
                role="menuitem"
                className="menu-item"
                onClick={() => {
                  setOpen(false);
                  queue.playNext(track);
                  library.notify(`«${track.title}» sonará a continuación`);
                }}
              >
                <span className="menu-icon-label">
                  <PlayNextIcon size={16} /> Reproducir a continuación
                </span>
              </button>
              <button
                role="menuitem"
                className="menu-item"
                onClick={() => {
                  setOpen(false);
                  queue.addToQueue(track);
                  library.notify(`«${track.title}» añadida a la cola`);
                }}
              >
                <span className="menu-icon-label">
                  <QueueAddIcon size={16} /> Añadir a la cola
                </span>
              </button>
              {queue.startRadio && (
                <button
                  role="menuitem"
                  className="menu-item"
                  onClick={() => {
                    setOpen(false);
                    queue.startRadio?.(track);
                    library.notify(`Radio de «${track.title}»: después vendrán canciones parecidas`);
                  }}
                >
                  <span className="menu-icon-label">
                    <RadioIcon size={16} /> Iniciar radio
                  </span>
                </button>
              )}
              <hr className="menu-sep" />
            </>
          )}
          {matchFix.canFix(track) && (
            <>
              <button
                role="menuitem"
                className="menu-item"
                onClick={() => {
                  setOpen(false);
                  matchFix.open(track);
                }}
              >
                <span className="menu-icon-label">
                  <SwapIcon size={16} /> No es esta canción…
                </span>
              </button>
              <hr className="menu-sep" />
            </>
          )}
          <p className="menu-title">Añadir a playlist</p>
          {library.playlists.map((p) => (
            <button
              key={p.id}
              role="menuitem"
              className="menu-item"
              onClick={async () => {
                setOpen(false);
                await library.addToPlaylist(p, track);
              }}
            >
              <span>{p.name}</span>
              <span className="menu-count">{p.trackCount}</span>
            </button>
          ))}
          <form className="menu-new" onSubmit={createAndAdd}>
            <input
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="Nueva playlist…"
              aria-label="Nombre de la nueva playlist"
              autoFocus={library.playlists.length === 0}
            />
            <button className="btn primary small" disabled={!newName.trim()}>
              Crear
            </button>
          </form>
        </div>
      )}
    </div>
  );
}
