import type { Updater } from "../hooks/useUpdater";
import { CloseIcon } from "./Icons";

/** Floating card shown when a new version is available or being installed. */
export function UpdateCard({ updater }: { updater: Updater }) {
  const { state, dismissed } = updater;
  const visible =
    (state.kind === "available" && !dismissed) || state.kind === "downloading" || state.kind === "installing";
  if (!visible) return null;

  return (
    <div className="update-card" role="status">
      {state.kind === "available" && (
        <>
          <div className="update-head">
            <strong>Nueva versión: Melomango {state.update.version}</strong>
            <button className="icon-btn" onClick={updater.dismiss} aria-label="Más tarde">
              <CloseIcon size={16} />
            </button>
          </div>
          {state.update.body && <p className="update-notes">{state.update.body}</p>}
          <div className="update-actions">
            <button className="btn ghost small" onClick={updater.dismiss}>
              Más tarde
            </button>
            <button className="btn primary small" onClick={updater.install}>
              Actualizar ahora
            </button>
          </div>
        </>
      )}
      {state.kind === "downloading" && (
        <>
          <strong>Descargando Melomango {state.update.version}…</strong>
          <div className="progress" aria-label="Progreso de la descarga">
            <div
              className={`progress-bar${state.percent === null ? " indeterminate" : ""}`}
              style={state.percent === null ? undefined : { width: `${state.percent}%` }}
            />
          </div>
          <span className="update-sub">{state.percent === null ? "Preparando…" : `${state.percent} %`}</span>
        </>
      )}
      {state.kind === "installing" && (
        <>
          <strong>Instalando la actualización…</strong>
          <span className="update-sub">Melomango se reiniciará en unos segundos.</span>
        </>
      )}
    </div>
  );
}

/** One-line status for the manual "check for updates" button. */
export function updateStatusText(updater: Updater): string | null {
  switch (updater.state.kind) {
    case "checking":
      return "Buscando…";
    case "upToDate":
      return "Estás al día ✓";
    case "available":
      return `Versión ${updater.state.update.version} disponible`;
    case "downloading":
      return "Descargando…";
    case "installing":
      return "Instalando…";
    case "error":
      return "No se pudo comprobar";
    case "devMode":
      return "Solo en la app instalada";
    default:
      return null;
  }
}
