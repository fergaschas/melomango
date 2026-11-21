import { useEffect, useState } from "react";
import { cacheUsage, clearCache, discordStatus, type DiscordStatus } from "../lib/api";
import { useDownloads } from "../lib/downloads";
import { CACHE_OPTIONS, formatSize, songs } from "../lib/downloadState";
import { CloseIcon } from "./Icons";
import type { Theme } from "../lib/theme";
import { MAX_CROSSFADE } from "../lib/crossfade";
import { useDialog } from "../lib/useDialog";

const THEMES: [Theme, string][] = [
  ["system", "Como Windows"],
  ["light", "Claro"],
  ["dark", "Oscuro"],
];

const STATUS_TEXT: Record<DiscordStatus["state"], string> = {
  unavailable: "Esta versión de Melomango no tiene configurada la conexión con Discord.",
  idle: "Se mostrará en cuanto suene algo.",
  notRunning: "Discord no está abierto en este equipo. Ábrelo y aparecerá solo.",
  connected: "Conectado: tus amigos de Discord ven lo que escuchas.",
  error: "No se pudo conectar con Discord.",
};

interface Props {
  discordEnabled: boolean;
  onDiscordChange: (on: boolean) => void;
  closeToTray: boolean;
  onCloseToTrayChange: (on: boolean) => void;
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
  crossfade: number;
  onCrossfadeChange: (secs: number) => void;
  normalize: boolean;
  onNormalizeChange: (on: boolean) => void;
  useLocation: boolean;
  onUseLocationChange: (on: boolean) => void;
  onClose: () => void;
}

export function SettingsDialog({
  discordEnabled,
  onDiscordChange,
  closeToTray,
  onCloseToTrayChange,
  theme,
  onThemeChange,
  crossfade,
  onCrossfadeChange,
  normalize,
  onNormalizeChange,
  useLocation,
  onUseLocationChange,
  onClose,
}: Props) {
  const [status, setStatus] = useState<DiscordStatus | null>(null);
  const downloads = useDownloads();
  const [usage, setUsage] = useState<{ songs: number; bytes: number } | null>(null);

  // What the cache holds now (it shrinks right away when its size is lowered).
  useEffect(() => {
    const timer = window.setTimeout(() => cacheUsage().then(setUsage).catch(() => {}), 150);
    return () => window.clearTimeout(timer);
  }, [downloads.cacheLimit]);

  async function onClearCache() {
    await clearCache().catch(() => {});
    setUsage(await cacheUsage().catch(() => null));
  }

  // The connection changes on its own (Discord opened or closed): check while open.
  useEffect(() => {
    let alive = true;
    const check = () =>
      discordStatus()
        .then((s) => alive && setStatus(s))
        .catch(() => {});
    check();
    const timer = window.setInterval(check, 2000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, []);

  const dialog = useDialog<HTMLElement>(onClose);

  const unavailable = status?.state === "unavailable";

  return (
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <section ref={dialog} tabIndex={-1} className="dialog" role="dialog" aria-modal="true" aria-labelledby="settings-title">
        <header className="dialog-header">
          <h2 id="settings-title">Ajustes</h2>
          <button className="icon-btn" onClick={onClose} aria-label="Cerrar">
            <CloseIcon />
          </button>
        </header>

        <div className="settings-row theme-row">
          <span>
            <strong>Tema</strong>
            <span className="radio-switch-note">Claro de mango, oscuro de hueso tostado, o lo que use Windows.</span>
          </span>
          <div className="segmented" role="radiogroup" aria-label="Tema">
            {THEMES.map(([value, label]) => (
              <button
                key={value}
                role="radio"
                aria-checked={theme === value}
                className={theme === value ? "active" : undefined}
                onClick={() => onThemeChange(value)}
              >
                {label}
              </button>
            ))}
          </div>
        </div>

        <div className="settings-row crossfade-row">
          <label htmlFor="crossfade">
            <strong>Fundido entre canciones</strong>
            <span className="radio-switch-note">
              La siguiente empieza antes de que acabe la actual y se mezclan suavemente.
            </span>
          </label>
          <div className="crossfade-control">
            <input
              type="range"
              id="crossfade"
              min={0}
              max={MAX_CROSSFADE}
              step={1}
              value={crossfade}
              onChange={(e) => onCrossfadeChange(Number(e.target.value))}
              aria-valuetext={crossfade ? `${crossfade} segundos` : "Desactivado"}
            />
            <span className="crossfade-value">{crossfade ? `${crossfade} s` : "No"}</span>
          </div>
        </div>

        <label className="radio-switch settings-row">
          <input
            type="checkbox"
            id="normalize-volume"
            checked={normalize}
            onChange={(e) => onNormalizeChange(e.target.checked)}
          />
          <span className="switch-track" aria-hidden="true">
            <span className="switch-thumb" />
          </span>
          <span>
            <strong>Normalizar el volumen</strong>
            <span className="radio-switch-note">
              Todas las canciones suenan a un volumen parecido, sin tener que tocar la rueda entre una y otra.
            </span>
          </span>
        </label>

        <div className="settings-row theme-row cache-row">
          <span>
            <strong>Caché automática</strong>
            <span className="radio-switch-note">
              Guarda lo que escuchas a menudo (y tus Me gusta) para que empiece al momento y suene sin conexión. Si se
              llena, se va lo que hace más que no escuchas.
            </span>
            {usage && (
              <span className="radio-switch-note cache-usage">
                Ahora: {songs(usage.songs)} · {formatSize(usage.bytes)}
                {usage.songs > 0 && (
                  <>
                    {" · "}
                    <button type="button" className="link-btn" onClick={onClearCache}>
                      Vaciar
                    </button>
                  </>
                )}
              </span>
            )}
          </span>
          <div className="segmented" role="radiogroup" aria-label="Tamaño de la caché">
            {CACHE_OPTIONS.map((bytes) => (
              <button
                key={bytes}
                role="radio"
                aria-checked={downloads.cacheLimit === bytes}
                className={downloads.cacheLimit === bytes ? "active" : undefined}
                onClick={() => downloads.setCacheLimit(bytes)}
              >
                {bytes === 0 ? "No" : `${bytes / 1024 ** 3} GB`}
              </button>
            ))}
          </div>
        </div>

        <label className="radio-switch settings-row">
          <input
            type="checkbox"
            id="station-location"
            checked={useLocation}
            onChange={(e) => onUseLocationChange(e.target.checked)}
          />
          <span className="switch-track" aria-hidden="true">
            <span className="switch-thumb" />
          </span>
          <span>
            <strong>Emisoras cerca de ti</strong>
            <span className="radio-switch-note">
              Usa la ubicación aproximada de Windows (a 1 km) para poner primero las emisoras de tu zona. Solo se usa para
              eso y no sale de tu equipo salvo para pedir la lista.
            </span>
          </span>
        </label>

        <label className="radio-switch settings-row">
          <input
            type="checkbox"
            id="close-to-tray"
            checked={closeToTray}
            onChange={(e) => onCloseToTrayChange(e.target.checked)}
          />
          <span className="switch-track" aria-hidden="true">
            <span className="switch-thumb" />
          </span>
          <span>
            <strong>Seguir sonando al cerrar la ventana</strong>
            <span className="radio-switch-note">
              Melomango se queda en la bandeja del sistema, junto al reloj. Para salir del todo, usa «Salir» en el menú
              de su icono.
            </span>
          </span>
        </label>

        <label className="radio-switch settings-row">
          <input
            type="checkbox"
            id="discord-presence"
            checked={discordEnabled && !unavailable}
            disabled={unavailable}
            onChange={(e) => onDiscordChange(e.target.checked)}
          />
          <span className="switch-track" aria-hidden="true">
            <span className="switch-thumb" />
          </span>
          <span>
            <strong>Mostrar lo que escuchas en Discord</strong>
            <span className="radio-switch-note">
              «Escuchando Melomango» con la canción, el artista y la portada, mientras suena.
            </span>
          </span>
        </label>
        {status && (
          <p className={`settings-status ${status.state}`} role="status">
            {!discordEnabled && !unavailable
              ? "Desactivado: Discord no muestra nada."
              : STATUS_TEXT[status.state] + (status.state === "error" ? ` ${status.message}` : "")}
          </p>
        )}
      </section>
    </div>
  );
}
