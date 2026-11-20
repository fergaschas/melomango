import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import * as api from "./api";
import type { Track } from "./api";
import { useLibrary } from "./library";
import type { DownloadProgress } from "./downloadState";

interface DownloadsValue {
  /** "source/id" of every downloaded song. */
  downloaded: ReadonlySet<string>;
  /** Downloads queued or running (and the last that failed), by key. */
  progress: ReadonlyMap<string, DownloadProgress>;
  /** Songs queued or downloading right now. */
  active: number;
  /** Bumped when the set of downloads changes, so the Descargas view reloads. */
  version: number;
  isDownloaded: (t: Track) => boolean;
  progressOf: (t: Track) => DownloadProgress | undefined;
  download: (tracks: Track[]) => void;
  /** Cancels or deletes the downloads of these songs. */
  remove: (tracks: Track[]) => Promise<void>;
  refresh: () => Promise<void>;
}

const DownloadsContext = createContext<DownloadsValue | null>(null);

export function useDownloads() {
  const value = useContext(DownloadsContext);
  if (!value) throw new Error("useDownloads fuera de DownloadsProvider");
  return value;
}

export function DownloadsProvider({ children }: { children: ReactNode }) {
  const { notify } = useLibrary();
  const [downloaded, setDownloaded] = useState<Set<string>>(new Set());
  const [progress, setProgress] = useState<Map<string, DownloadProgress>>(new Map());
  const [version, setVersion] = useState(0);
  const titles = useRef(new Map<string, string>());

  const refresh = useCallback(async () => {
    setDownloaded(new Set(await api.downloadedKeys()));
    setVersion((v) => v + 1);
  }, []);

  useEffect(() => {
    refresh().catch(() => {});
  }, [refresh]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let gone = false;
    listen<DownloadProgress>("download-progress", ({ payload }) => {
      setProgress((prev) => {
        const next = new Map(prev);
        if (payload.state === "done" || payload.state === "cancelled") next.delete(payload.key);
        else next.set(payload.key, payload);
        return next;
      });
      if (payload.state === "done") {
        setDownloaded((prev) => (prev.has(payload.key) ? prev : new Set(prev).add(payload.key)));
        setVersion((v) => v + 1);
      } else if (payload.state === "failed") {
        const title = titles.current.get(payload.key) ?? "la canción";
        notify(`No se pudo descargar «${title}»${payload.message ? `: ${payload.message}` : ""}`);
      }
    }).then((fn) => (gone ? fn() : (unlisten = fn)));
    return () => {
      gone = true;
      unlisten?.();
    };
  }, [notify]);

  const value = useMemo<DownloadsValue>(
    () => ({
      downloaded,
      progress,
      active: [...progress.values()].filter((p) => p.state === "queued" || p.state === "downloading").length,
      version,
      isDownloaded: (t) => downloaded.has(api.trackKey(t)),
      progressOf: (t) => progress.get(api.trackKey(t)),
      download: (tracks) => {
        const wanted = tracks.filter((t) => {
          const key = api.trackKey(t);
          const phase = progress.get(key)?.state;
          return !downloaded.has(key) && phase !== "queued" && phase !== "downloading";
        });
        if (!wanted.length) return;
        wanted.forEach((t) => titles.current.set(api.trackKey(t), t.title));
        // Shown as queued right away, before the backend says so.
        setProgress((prev) => {
          const next = new Map(prev);
          for (const t of wanted) {
            const key = api.trackKey(t);
            next.set(key, { key, state: "queued", received: 0, total: null, message: null });
          }
          return next;
        });
        api.downloadTracks(wanted).catch((e) => notify(`No se pudo descargar: ${e}`));
      },
      remove: async (tracks) => {
        try {
          for (const t of tracks) await api.removeDownload(t);
        } catch (e) {
          notify(`No se pudo quitar la descarga: ${e}`);
        }
        await refresh();
      },
      refresh,
    }),
    [downloaded, progress, version, notify, refresh],
  );

  return <DownloadsContext.Provider value={value}>{children}</DownloadsContext.Provider>;
}
