import { useState } from "react";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { installPlugin, openPluginsDir, uninstallPlugin, type SourceInfo } from "../lib/api";
import { CloseIcon } from "./Icons";

interface Props {
  sources: SourceInfo[];
  onChanged: (selectId?: string) => void;
  onClose: () => void;
}

type Notice = { kind: "ok" | "error"; text: string } | null;

export function PluginsDialog({ sources, onChanged, onClose }: Props) {
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<Notice>(null);

  async function run(action: () => Promise<void>) {
    setBusy(true);
    setNotice(null);
    try {
      await action();
    } catch (e) {
      setNotice({ kind: "error", text: String(e) });
    } finally {
      setBusy(false);
    }
  }

  const onInstall = () =>
    run(async () => {
      const dir = await open({ directory: true, title: "Elige la carpeta del plugin (la que contiene plugin.json)" });
      if (typeof dir !== "string") return;
      const info = await installPlugin(dir);
      setNotice({ kind: "ok", text: `${info.name} ${info.version ?? ""} instalado.` });
      onChanged(info.id);
    });

  const onUninstall = (source: SourceInfo) =>
    run(async () => {
      const yes = await ask(`¿Desinstalar ${source.name}?`, { title: "Melomango", kind: "warning" });
      if (!yes) return;
      await uninstallPlugin(source.id);
      setNotice({ kind: "ok", text: `${source.name} desinstalado.` });
      onChanged();
    });

  return (
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <section className="dialog" role="dialog" aria-modal="true" aria-labelledby="plugins-title">
        <header className="dialog-header">
          <h2 id="plugins-title">Plugins</h2>
          <button className="icon-btn" onClick={onClose} aria-label="Cerrar">
            <CloseIcon />
          </button>
        </header>

        <p className="dialog-intro">
          Añade fuentes de música instalando plugins. Un plugin es un programa que se ejecuta en tu equipo:
          instala solo los que sean de confianza.
        </p>

        <ul className="source-list">
          {sources.map((s) => (
            <li key={s.id}>
              <div className="source-text">
                <span className="source-name">
                  {s.name}
                  {s.version && <span className="source-version">v{s.version}</span>}
                  <span className={`badge ${s.kind}`}>{s.kind === "builtin" ? "Integrada" : "Plugin"}</span>
                </span>
                {s.description && <span className="source-desc">{s.description}</span>}
              </div>
              {s.kind === "plugin" && (
                <button className="btn ghost" onClick={() => onUninstall(s)} disabled={busy}>
                  Desinstalar
                </button>
              )}
            </li>
          ))}
        </ul>

        {notice && <p className={`notice ${notice.kind}`}>{notice.text}</p>}

        <footer className="dialog-actions">
          <button className="btn ghost" onClick={() => run(openPluginsDir)} disabled={busy}>
            Abrir carpeta de plugins
          </button>
          <button className="btn primary" onClick={onInstall} disabled={busy}>
            Instalar plugin…
          </button>
        </footer>
      </section>
    </div>
  );
}
