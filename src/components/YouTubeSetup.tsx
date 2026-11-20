import { useEffect, useRef, useState } from "react";
import { browsePlugins, installCatalogPlugin, installPluginRequirement, pluginRequirements } from "../lib/api";
import logo from "../../app-icon.svg";

const YOUTUBE = "youtube";

/**
 * Shown while there is no source to play from (a fresh install): Melomango plays from
 * YouTube, so it installs the YouTube plugin (and yt-dlp, which it needs) on its own.
 */
export function YouTubeSetup({ onReady }: { onReady: (sourceId: string) => void }) {
  const [step, setStep] = useState("Buscando el plugin de YouTube…");
  const [error, setError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  // One installation per attempt (React runs effects twice in development).
  const started = useRef(-1);

  useEffect(() => {
    if (started.current === attempt) return;
    started.current = attempt;
    (async () => {
      try {
        setError(null);
        setStep("Buscando el plugin de YouTube…");
        const catalog = await browsePlugins();
        const entry = catalog.plugins.find((p) => p.id === YOUTUBE && p.available);
        if (!entry) {
          throw new Error(catalog.errors[0] ?? "No se encontró el plugin de YouTube en el repositorio oficial.");
        }
        setStep("Instalando YouTube…");
        await installCatalogPlugin(entry.registry, entry.id);
        for (const r of await pluginRequirements(YOUTUBE)) {
          if (r.installed || !r.installable) continue;
          setStep(`Instalando ${r.name}…`);
          await installPluginRequirement(YOUTUBE, r.id);
        }
        onReady(YOUTUBE);
      } catch (e) {
        setError(String(e instanceof Error ? e.message : e));
      }
    })();
  }, [attempt, onReady]);

  return (
    <section className="setup-card" aria-live="polite">
      <img src={logo} alt="" width={64} height={64} />
      <h1>Preparando YouTube</h1>
      <p className="subtitle">
        Melomango suena desde YouTube. Lo dejamos listo en unos segundos; solo hace falta la primera vez.
      </p>
      {error ? (
        <>
          <p className="notice error">No se pudo preparar YouTube: {error}</p>
          <button className="btn primary" onClick={() => setAttempt((a) => a + 1)}>
            Reintentar
          </button>
        </>
      ) : (
        <p className="setup-step">
          <span className="spinner" aria-hidden="true" /> {step}
        </p>
      )}
    </section>
  );
}
