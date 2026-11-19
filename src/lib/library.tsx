import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import * as api from "./api";
import type { PlaylistSummary, Track } from "./api";

interface LibraryValue {
  playlists: PlaylistSummary[];
  likedCount: number;
  /** Bumped after every change, so open views know to reload. */
  version: number;
  toast: string | null;
  isLiked: (t: Track) => boolean;
  toggleLike: (t: Track) => Promise<void>;
  createPlaylist: (name: string) => Promise<PlaylistSummary>;
  renamePlaylist: (id: number, name: string) => Promise<void>;
  deletePlaylist: (id: number) => Promise<void>;
  addToPlaylist: (p: PlaylistSummary, t: Track) => Promise<void>;
  removeFromPlaylist: (id: number, t: Track) => Promise<void>;
  moveInPlaylist: (id: number, from: number, to: number) => Promise<void>;
  notify: (text: string) => void;
}

const LibraryContext = createContext<LibraryValue | null>(null);

export function useLibrary() {
  const value = useContext(LibraryContext);
  if (!value) throw new Error("useLibrary fuera de LibraryProvider");
  return value;
}

export function LibraryProvider({ children }: { children: ReactNode }) {
  const [liked, setLiked] = useState<Set<string>>(new Set());
  const [playlists, setPlaylists] = useState<PlaylistSummary[]>([]);
  const [version, setVersion] = useState(0);
  const [toast, setToast] = useState<string | null>(null);
  const toastTimer = useRef<number | undefined>(undefined);

  const notify = useCallback((text: string) => {
    setToast(text);
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 2800);
  }, []);

  const refresh = useCallback(async () => {
    const [keys, lists] = await Promise.all([api.likedKeys(), api.listPlaylists()]);
    setLiked(new Set(keys));
    setPlaylists(lists);
    setVersion((v) => v + 1);
  }, []);

  useEffect(() => {
    refresh().catch((e) => notify(`No se pudo cargar la biblioteca: ${e}`));
  }, [refresh, notify]);

  /** Runs a change, refreshes, and reports errors as a toast. */
  const change = useCallback(
    async <T,>(action: () => Promise<T>): Promise<T> => {
      try {
        const result = await action();
        await refresh();
        return result;
      } catch (e) {
        notify(String(e));
        throw e;
      }
    },
    [refresh, notify],
  );

  const value = useMemo<LibraryValue>(
    () => ({
      playlists,
      likedCount: liked.size,
      version,
      toast,
      notify,
      isLiked: (t) => liked.has(api.trackKey(t)),
      toggleLike: async (t) => {
        const now = !liked.has(api.trackKey(t));
        // Optimistic: the heart reacts instantly.
        setLiked((prev) => {
          const next = new Set(prev);
          if (now) next.add(api.trackKey(t));
          else next.delete(api.trackKey(t));
          return next;
        });
        await change(() => api.setLiked(t, now));
      },
      createPlaylist: (name) => change(() => api.createPlaylist(name)),
      renamePlaylist: (id, name) => change(() => api.renamePlaylist(id, name)),
      deletePlaylist: (id) => change(() => api.deletePlaylist(id)),
      addToPlaylist: async (p, t) => {
        const added = await change(() => api.addToPlaylist(p.id, t));
        notify(added ? `Añadida a «${p.name}»` : `Ya estaba en «${p.name}»`);
      },
      removeFromPlaylist: (id, t) => change(() => api.removeFromPlaylist(id, t)),
      moveInPlaylist: (id, from, to) => change(() => api.moveInPlaylist(id, from, to)),
    }),
    [playlists, liked, version, toast, notify, change],
  );

  return <LibraryContext.Provider value={value}>{children}</LibraryContext.Provider>;
}
