import { useEffect, useRef, useState, type FormEvent } from "react";
import type { Track } from "../lib/api";
import { useLibrary } from "../lib/library";
import { PlusIcon } from "./Icons";

/** "+" button that opens a small menu to add `track` to a playlist (or a new one). */
export function AddToPlaylistMenu({ track }: { track: Track }) {
  const library = useLibrary();
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
        aria-label="Añadir a playlist"
        title="Añadir a playlist"
      >
        <PlusIcon size={18} />
      </button>
      {open && (
        <div className="menu" role="menu">
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
