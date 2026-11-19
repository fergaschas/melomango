import { useCallback, useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate" }
  | { kind: "available"; update: Update }
  | { kind: "downloading"; update: Update; percent: number | null }
  | { kind: "installing" }
  | { kind: "error"; message: string }
  | { kind: "devMode" };

/** Seconds after start-up before the automatic check, so it doesn't compete with loading. */
const STARTUP_DELAY_MS = 4000;

/**
 * Checks GitHub Releases for a newer signed version (endpoint and public key in
 * tauri.conf.json), downloads it and restarts the app.
 */
export function useUpdater() {
  const [version, setVersion] = useState<string | null>(null);
  const [state, setState] = useState<UpdateState>({ kind: "idle" });
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
  }, []);

  const checkNow = useCallback(async (manual: boolean) => {
    // In development the "installed" app is not this one: never update it from here.
    if (import.meta.env.DEV) {
      if (manual) setState({ kind: "devMode" });
      return;
    }
    setState({ kind: "checking" });
    setDismissed(false);
    try {
      const update = await check();
      setState(update ? { kind: "available", update } : { kind: "upToDate" });
    } catch (e) {
      // A failed automatic check (offline, no release yet) should not bother anyone.
      setState(manual ? { kind: "error", message: String(e) } : { kind: "idle" });
    }
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => checkNow(false), STARTUP_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [checkNow]);

  const install = useCallback(async () => {
    if (state.kind !== "available") return;
    const { update } = state;
    let total = 0;
    let done = 0;
    try {
      setState({ kind: "downloading", update, percent: null });
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          done += event.data.chunkLength;
          setState({ kind: "downloading", update, percent: total ? Math.min(100, Math.round((done / total) * 100)) : null });
        } else if (event.event === "Finished") {
          setState({ kind: "installing" });
        }
      });
      // On Windows the installer closes the app itself; elsewhere, restart into the new version.
      await relaunch();
    } catch (e) {
      setState({ kind: "error", message: String(e) });
    }
  }, [state]);

  return { version, state, dismissed, dismiss: () => setDismissed(true), checkNow, install };
}

export type Updater = ReturnType<typeof useUpdater>;
