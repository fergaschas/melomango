import { useEffect, useState } from "react";
import { discordStatus, type DiscordStatus } from "../lib/api";
import { CloseIcon } from "./Icons";
import type { Theme } from "../lib/theme";

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
  onClose: () => void;
}

export function SettingsDialog({
  discordEnabled,
  onDiscordChange,
  closeToTray,
  onCloseToTrayChange,
  theme,
  onThemeChange,
  onClose,
}: Props) {
  const [status, setStatus] = useState<DiscordStatus | null>(null);

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

  useEffect(() => {
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [onClose]);

  const unavailable = status?.state === "unavailable";

  return (
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <section className="dialog" role="dialog" aria-modal="true" aria-labelledby="settings-title">
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
