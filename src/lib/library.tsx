import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import * as api from "./api";
import type { PlaylistSummary, Track } from "./api";

interface LibraryValue {
  playlists: PlaylistSummary[];
  likedCount: number;
  /** Bumped when the liked songs change, so views that list them reload. */
  likesVersion: number;
  /** Bumped when playlists change, so an open playlist reloads. A like doesn't touch it. */
  playlistsVersion: number;
  isLiked: (t: Track) => boolean;
  /** Never rejects: resolves whether it was saved (if not, the heart goes back and a toast says why). */
  toggleLike: (t: Track) => Promise<boolean>;
  createPlaylist: (name: string) => Promise<PlaylistSummary>;
  renamePlaylist: (id: number, name: string) => Promise<void>;
  deletePlaylist: (id: number) => Promise<void>;
  addToPlaylist: (p: PlaylistSummary, t: Track) => Promise<void>;
  removeFromPlaylist: (id: number, t: Track) => Promise<void>;
  /** Never rejects: resolves whether it was saved (if not, open views reload the saved order). */
  moveInPlaylist: (id: number, from: number, to: number) => Promise<boolean>;
  notify: (text: string) => void;
  /** Reloads likes and playlists (e.g. after an import). */
  refresh: () => Promise<void>;
}

const LibraryContext = createContext<LibraryValue | null>(null);
/** The message shown at the bottom. Its own context: showing one doesn't re-render every row. */
const ToastContext = createContext<string | null>(null);

export function useLibrary() {
  const value = useContext(LibraryContext);
  if (!value) throw new Error("useLibrary fuera de LibraryProvider");
  return value;
}

export function useToast() {
  return useContext(ToastContext);
}

type Scope = "likes" | "playlists";

export function LibraryProvider({ children }: { children: ReactNode }) {
  const [liked, setLiked] = useState<Set<string>>(new Set());
  const likedRef = useRef(liked);
  likedRef.current = liked;
  const [playlists, setPlaylists] = useState<PlaylistSummary[]>([]);
  const [likesVersion, setLikesVersion] = useState(0);
  const [playlistsVersion, setPlaylistsVersion] = useState(0);
  const [toast, setToast] = useState<string | null>(null);
  const toastTimer = useRef<number | undefined>(undefined);

  const notify = useCallback((text: string) => {
    setToast(text);
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 2800);
  }, []);

  const reload = useCallback(async (scope: Scope) => {
    if (scope === "likes") {
      setLiked(new Set(await api.likedKeys()));
      setLikesVersion((v) => v + 1);
    } else {
      setPlaylists(await api.listPlaylists());
      setPlaylistsVersion((v) => v + 1);
    }
  }, []);

  const refresh = useCallback(async () => {
    await Promise.all([reload("likes"), reload("playlists")]);
  }, [reload]);

  useEffect(() => {
    refresh().catch((e) => notify(`No se pudo cargar la biblioteca: ${e}`));
  }, [refresh, notify]);

  /** Runs a change, reloads what it touched, and reports errors as a toast. */
  const change = useCallback(
    async <T,>(action: () => Promise<T>, scope: Scope = "playlists"): Promise<T> => {
      try {
        const result = await action();
        await reload(scope);
        return result;
      } catch (e) {
        notify(String(e));
        // Whatever was shown ahead of time (a heart, a new order) goes back to what is saved.
        reload(scope).catch(() => {});
        throw e;
      }
    },
    [reload, notify],
  );

  /** `change` for actions nobody waits on: the error is already a toast, so it resolves false. */
  const attempt = useCallback(
    async (action: () => Promise<unknown>, scope: Scope = "playlists"): Promise<boolean> => {
      try {
        await change(action, scope);
        return true;
      } catch {
        return false;
      }
    },
    [change],
  );

  // Stable functions: they read the latest likes through a ref.
  const actions = useMemo(
    () => ({
      notify,
      refresh,
      isLiked: (t: Track) => likedRef.current.has(api.trackKey(t)),
      toggleLike: async (t: Track) => {
        const key = api.trackKey(t);
        const now = !likedRef.current.has(key);
        const show = (on: boolean) =>
          setLiked((prev) => {
            const next = new Set(prev);
            if (on) next.add(key);
            else next.delete(key);
            return next;
          });
        // Optimistic: the heart reacts instantly, and goes back if saving fails.
        show(now);
        const saved = await attempt(() => api.setLiked(t, now), "likes");
        if (!saved) show(!now);
        return saved;
      },
      createPlaylist: (name: string) => change(() => api.createPlaylist(name)),
      renamePlaylist: (id: number, name: string) => change(() => api.renamePlaylist(id, name)),
      deletePlaylist: (id: number) => change(() => api.deletePlaylist(id)),
      addToPlaylist: async (p: PlaylistSummary, t: Track) => {
        const added = await change(() => api.addToPlaylist(p.id, t));
        notify(added ? `Añadida a «${p.name}»` : `Ya estaba en «${p.name}»`);
      },
      removeFromPlaylist: (id: number, t: Track) => change(() => api.removeFromPlaylist(id, t)),
      moveInPlaylist: (id: number, from: number, to: number) => attempt(() => api.moveInPlaylist(id, from, to)),
    }),
    [notify, refresh, change, attempt],
  );

  const value = useMemo<LibraryValue>(
    () => ({
      ...actions,
      // A new function when the likes change, so components that ask it re-render.
      isLiked: (t) => liked.has(api.trackKey(t)),
      playlists,
      likedCount: liked.size,
      likesVersion,
      playlistsVersion,
    }),
    [actions, liked, playlists, likesVersion, playlistsVersion],
  );

  return (
    <LibraryContext.Provider value={value}>
      <ToastContext.Provider value={toast}>{children}</ToastContext.Provider>
    </LibraryContext.Provider>
  );
}
