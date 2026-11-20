import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent as ReactKeyboardEvent } from "react";
import type { Track } from "../lib/api";
import { useLibrary } from "../lib/library";
import { useMatchFix } from "../lib/matchFix";
import { useDownloads } from "../lib/downloads";
import { percent } from "../lib/downloadState";
import { CloseIcon, DownloadDoneIcon, DownloadIcon, PlayNextIcon, PlusIcon, QueueAddIcon, RadioIcon, SwapIcon } from "./Icons";

export interface QueueActions {
  playNext: (t: Track) => void;
  addToQueue: (t: Track) => void;
  startRadio?: (t: Track) => void;
}

/**
 * "+" button: play next / add to the queue, or add `track` to a playlist (or a new one).
 *
 * Closed, it reads no context: it sits in every song row, and a like or a download
 * elsewhere must not re-render thousands of them. The menu itself does, while open.
 */
export function AddToPlaylistMenu({ track, queue }: { track: Track; queue?: QueueActions }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    // Escape closes the menu (only: a dialog around it stays) and goes back to its button.
    const esc = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      setOpen(false);
      button.current?.focus();
    };
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  return (
    <div className="menu-anchor" ref={ref} onClick={(e) => e.stopPropagation()}>
      <button
        ref={button}
        className={`row-action${open ? " on" : ""}`}
        onClick={() => setOpen((o) => !o)}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label="Añadir a la cola o a una playlist"
        title="Añadir a la cola o a una playlist"
      >
        <PlusIcon size={18} />
      </button>
      {open && (
        <Menu
          track={track}
          queue={queue}
          close={() => {
            setOpen(false);
            button.current?.focus();
          }}
        />
      )}
    </div>
  );
}

function Menu({ track, queue, close }: { track: Track; queue?: QueueActions; close: () => void }) {
  const library = useLibrary();
  const matchFix = useMatchFix();
  const downloads = useDownloads();
  const downloaded = downloads.isDownloaded(track);
  const loading = downloads.progressOf(track);
  const inProgress = loading && (loading.state === "queued" || loading.state === "downloading");
  const [newName, setNewName] = useState("");
  const menu = useRef<HTMLDivElement>(null);

  // Opening it puts the focus on its first option.
  useEffect(() => {
    menu.current?.querySelector<HTMLElement>("[role=menuitem]")?.focus();
  }, []);

  /** Arrows, Home and End move between the options (not while typing a new name). */
  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (e.target instanceof HTMLInputElement) return;
    const items = [...(menu.current?.querySelectorAll<HTMLElement>("[role=menuitem]") ?? [])];
    const at = items.indexOf(document.activeElement as HTMLElement);
    const next = { ArrowDown: at + 1, ArrowUp: at - 1, Home: 0, End: items.length - 1 }[e.key];
    if (next === undefined || items.length === 0) return;
    e.preventDefault();
    items[(next + items.length) % items.length]?.focus();
  };

  async function createAndAdd(e: FormEvent) {
    e.preventDefault();
    if (!newName.trim()) return;
    const playlist = await library.createPlaylist(newName);
    await library.addToPlaylist(playlist, track);
    setNewName("");
    close();
  }

  return (
    <div className="menu" role="menu" aria-label={`Opciones de «${track.title}»`} ref={menu} onKeyDown={onKeyDown}>
      {queue && (
        <>
          <button
            role="menuitem"
            className="menu-item"
            onClick={() => {
              close();
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
              close();
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
                close();
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
      <button
        role="menuitem"
        className="menu-item"
        onClick={() => {
          close();
          if (downloaded || inProgress) downloads.remove([track]);
          else {
            downloads.download([track]);
            library.notify(`Descargando «${track.title}»…`);
          }
        }}
      >
        <span className="menu-icon-label">
          {downloaded ? (
            <>
              <DownloadDoneIcon size={16} /> Borrar la descarga
            </>
          ) : inProgress ? (
            <>
              <CloseIcon size={16} /> Cancelar la descarga
              {percent(loading) !== null && ` (${percent(loading)} %)`}
            </>
          ) : (
            <>
              <DownloadIcon size={16} /> Descargar
            </>
          )}
        </span>
      </button>
      <hr className="menu-sep" />
      {matchFix.canFix(track) && (
        <>
          <button
            role="menuitem"
            className="menu-item"
            onClick={() => {
              close();
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
            close();
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
  );
}
