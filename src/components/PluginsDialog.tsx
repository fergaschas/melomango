import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import { ask, open } from "@tauri-apps/plugin-dialog";
import {
  addRegistry,
  browsePlugins,
  installCatalogPlugin,
  installPlugin,
  listRegistries,
  openPluginsDir,
  pluginOriginChange,
  removeRegistry,
  uninstallPlugin,
  type Catalog,
  type CatalogPlugin,
  type RegistryInfo,
  type SourceInfo,
} from "../lib/api";
import { installPluginRequirement, pluginRequirements, type Requirement } from "../lib/api";
import { setupRequirements } from "../lib/requirements";
import { CloseIcon, SearchIcon } from "./Icons";

interface Props {
  sources: SourceInfo[];
  onChanged: (selectId?: string) => void;
  onClose: () => void;
}

type Tab = "explore" | "installed";
type Notice = { kind: "ok" | "error" | "info"; text: string } | null;

export function PluginsDialog({ sources, onChanged, onClose }: Props) {
  const [tab, setTab] = useState<Tab>("explore");
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice>(null);

  /** After installing a plugin: install what it needs (yt-dlp…) and report. */
  async function afterInstall(info: SourceInfo) {
    onChanged(info.id);
    const extras = await setupRequirements(info, (text) => setNotice({ kind: "info", text }));
    const also = extras.length ? ` (y ${extras.join(", ")})` : "";
    setNotice({
      kind: "ok",
      text: info.isLibrary
        ? `${info.name} ${info.version ?? ""} instalado${also}. Conéctalo con el botón «Importar playlists» (⬇) de la barra lateral.`
        : `${info.name} ${info.version ?? ""} instalado${also}. Ya aparece junto a las demás fuentes.`,
    });
  }

  async function run(key: string, action: () => Promise<void>) {
    setBusy(key);
    setNotice(null);
    try {
      await action();
    } catch (e) {
      setNotice({ kind: "error", text: String(e) });
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <section className="dialog" role="dialog" aria-modal="true" aria-labelledby="plugins-title">
        <header className="dialog-header">
          <h2 id="plugins-title">Plugins</h2>
          <button className="icon-btn" onClick={onClose} aria-label="Cerrar">
            <CloseIcon />
          </button>
        </header>

        <div className="dialog-tabs" role="tablist">
          <button role="tab" aria-selected={tab === "explore"} className={tab === "explore" ? "active" : ""} onClick={() => setTab("explore")}>
            Explorar
          </button>
          <button role="tab" aria-selected={tab === "installed"} className={tab === "installed" ? "active" : ""} onClick={() => setTab("installed")}>
            Instalados
          </button>
        </div>

        <p className="dialog-intro">
          Un plugin es un programa que se ejecuta en tu equipo: instala solo los de repositorios en los que confíes.
        </p>

        {tab === "explore" ? (
          <ExploreTab sources={sources} busy={busy} run={run} onInstalled={afterInstall} />
        ) : (
          <InstalledTab
            sources={sources}
            busy={busy}
            run={run}
            onChanged={onChanged}
            setNotice={setNotice}
            afterInstall={afterInstall}
          />
        )}

        {notice && <p className={`notice ${notice.kind}`}>{notice.text}</p>}
      </section>
    </div>
  );
}

type Run = (key: string, action: () => Promise<void>) => Promise<void>;

function ExploreTab({
  sources,
  busy,
  run,
  onInstalled,
}: {
  sources: SourceInfo[];
  busy: string | null;
  run: Run;
  onInstalled: (info: SourceInfo) => Promise<void>;
}) {
  const [catalog, setCatalog] = useState<Catalog | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");

  const load = useCallback(() => {
    setCatalog(null);
    setLoadError(null);
    browsePlugins().then(setCatalog).catch((e) => setLoadError(String(e)));
  }, []);

  useEffect(load, [load]);

  const results = useMemo(() => {
    const q = filter.trim().toLowerCase();
    const plugins = catalog?.plugins ?? [];
    if (!q) return plugins;
    return plugins.filter((p) =>
      [p.id, p.name, p.description ?? "", p.author ?? "", ...p.tags].some((field) => field.toLowerCase().includes(q)),
    );
  }, [catalog, filter]);

  const install = (p: CatalogPlugin) =>
    run(`install:${p.registry}:${p.id}`, async () => {
      // Another repository's plugin with the same id would replace the installed one.
      const installedFrom = await pluginOriginChange(p.registry, p.id);
      if (installedFrom !== null) {
        const yes = await ask(
          `Ya tienes un plugin «${p.id}» instalado desde otro repositorio (${installedFrom}). ` +
            `Si instalas el de ${p.registryName}, lo reemplazará y se borrarán sus datos (sesiones iniciadas y ajustes). ¿Continuar?`,
          { title: "Melomango", kind: "warning" },
        );
        if (!yes) return;
      }
      const info = await installCatalogPlugin(p.registry, p.id, installedFrom !== null);
      await onInstalled(info);
    });

  return (
    <>
      <label className="search dialog-search">
        <SearchIcon size={16} />
        <input
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder="Busca plugins: youtube, podcasts…"
          aria-label="Buscar plugins"
          autoFocus
        />
      </label>

      {!catalog && !loadError && <p className="status">Buscando plugins…</p>}
      {loadError && <p className="notice error">{loadError}</p>}
      {catalog?.errors.map((e) => (
        <p key={e} className="notice error">No se pudo leer un repositorio: {e}</p>
      ))}
      {catalog && results.length === 0 && <p className="status">Ningún plugin coincide con «{filter}».</p>}

      <ul className="source-list">
        {results.map((p) => {
          const installed = sources.find((s) => s.id === p.id && s.kind === "plugin");
          const key = `install:${p.registry}:${p.id}`;
          let action;
          if (!p.available) action = <span className="muted-note">No disponible para tu sistema</span>;
          else if (installed && installed.version === p.version) action = <span className="badge builtin">Instalado</span>;
          else
            action = (
              <button className="btn primary" onClick={() => install(p)} disabled={busy !== null}>
                {busy === key ? "Instalando…" : installed ? `Actualizar a v${p.version}` : "Instalar"}
              </button>
            );
          return (
            <li key={`${p.registry}/${p.id}`}>
              <div className="source-text">
                <span className="source-name">
                  {p.name}
                  <span className="source-version">v{p.version}</span>
                </span>
                {p.description && <span className="source-desc">{p.description}</span>}
                <span className="source-meta">
                  {p.author && <>de {p.author} · </>}
                  {p.registryName}
                  {p.tags.map((t) => (
                    <span key={t} className="tag">{t}</span>
                  ))}
                </span>
              </div>
              {action}
            </li>
          );
        })}
      </ul>

      <RegistriesSection busy={busy} run={run} onChanged={load} />
    </>
  );
}

function RegistriesSection({ busy, run, onChanged }: { busy: string | null; run: Run; onChanged: () => void }) {
  const [registries, setRegistries] = useState<RegistryInfo[]>([]);
  const [url, setUrl] = useState("");

  useEffect(() => {
    listRegistries().then(setRegistries).catch(() => {});
  }, []);

  const onAdd = (e: FormEvent) => {
    e.preventDefault();
    run("registry:add", async () => {
      setRegistries(await addRegistry(url));
      setUrl("");
      onChanged();
    });
  };

  return (
    <details className="registries">
      <summary>Repositorios ({registries.length})</summary>
      <ul>
        {registries.map((r) => (
          <li key={r.url}>
            <span className="registry-url" title={r.url}>{r.url}</span>
            {r.official ? (
              <span className="badge builtin">Oficial</span>
            ) : (
              <button
                className="btn ghost small"
                disabled={busy !== null}
                onClick={() =>
                  run("registry:remove", async () => {
                    setRegistries(await removeRegistry(r.url));
                    onChanged();
                  })
                }
              >
                Quitar
              </button>
            )}
          </li>
        ))}
      </ul>
      <form className="registry-add" onSubmit={onAdd}>
        <input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://…/index.json" aria-label="URL del repositorio" />
        <button className="btn ghost small" disabled={busy !== null || !url.trim()}>
          Añadir
        </button>
      </form>
    </details>
  );
}

function InstalledTab({
  sources,
  busy,
  run,
  onChanged,
  setNotice,
  afterInstall,
}: {
  sources: SourceInfo[];
  busy: string | null;
  run: Run;
  onChanged: (selectId?: string) => void;
  setNotice: (n: Notice) => void;
  afterInstall: (info: SourceInfo) => Promise<void>;
}) {
  const onInstallFolder = () =>
    run("folder", async () => {
      const dir = await open({ directory: true, title: "Elige la carpeta del plugin (la que contiene plugin.json)" });
      if (typeof dir !== "string") return;
      await afterInstall(await installPlugin(dir));
    });

  const onUninstall = (source: SourceInfo) =>
    run(`uninstall:${source.id}`, async () => {
      const yes = await ask(`¿Desinstalar ${source.name}?`, { title: "Melomango", kind: "warning" });
      if (!yes) return;
      await uninstallPlugin(source.id);
      setNotice({ kind: "ok", text: `${source.name} desinstalado.` });
      onChanged();
    });

  return (
    <>
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
              {s.kind === "plugin" && <RequirementsLine plugin={s} busy={busy} run={run} />}
            </div>
            {s.kind === "plugin" && (
              <button className="btn ghost" onClick={() => onUninstall(s)} disabled={busy !== null}>
                Desinstalar
              </button>
            )}
          </li>
        ))}
      </ul>
      <footer className="dialog-actions">
        <button className="btn ghost" onClick={() => run("dir", openPluginsDir)} disabled={busy !== null}>
          Abrir carpeta de plugins
        </button>
        <button className="btn ghost" onClick={onInstallFolder} disabled={busy !== null}>
          Instalar desde carpeta…
        </button>
      </footer>
    </>
  );
}

/** "Necesita: yt-dlp ✓" or "falta" with a button to install it. */
function RequirementsLine({ plugin, busy, run }: { plugin: SourceInfo; busy: string | null; run: Run }) {
  const [requirements, setRequirements] = useState<Requirement[]>([]);

  useEffect(() => {
    pluginRequirements(plugin.id).then(setRequirements).catch(() => setRequirements([]));
  }, [plugin.id, plugin.version]);

  if (requirements.length === 0) return null;
  return (
    <span className="requirements">
      Necesita:
      {requirements.map((r) =>
        r.installed ? (
          <span key={r.id} className="req ok">
            {r.name} ✓
          </span>
        ) : (
          <span key={r.id} className="req missing">
            {r.name} (falta)
            {r.installable && (
              <button
                className="link-btn"
                disabled={busy !== null}
                onClick={() =>
                  run(`req:${plugin.id}:${r.id}`, async () => {
                    setRequirements(await installPluginRequirement(plugin.id, r.id));
                  })
                }
              >
                {busy === `req:${plugin.id}:${r.id}` ? "Instalando…" : "Instalar"}
              </button>
            )}
          </span>
        ),
      )}
    </span>
  );
}
