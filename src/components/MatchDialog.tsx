import { useEffect, useState, type FormEvent } from "react";
import { chooseMatch, formatTime, matchOptions, type MatchOptions, type Track } from "../lib/api";
import { Artwork } from "./Artwork";
import { CloseIcon, SearchIcon } from "./Icons";
import { useDialog } from "../lib/useDialog";

interface Props {
  track: Track;
  onClose: () => void;
  /** Called after the user picks a recording for `track`. */
  onChosen: (choice: Track) => void;
}

/** "No es esta canción": choose which recording an imported track plays from. */
export function MatchDialog({ track, onClose, onChosen }: Props) {
  const [options, setOptions] = useState<MatchOptions | null>(null);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [choosing, setChoosing] = useState<string | null>(null);

  async function load(q?: string) {
    setLoading(true);
    setError(null);
    try {
      const result = await matchOptions(track, q);
      setOptions(result);
      setQuery(result.query);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [track.source, track.id]);

  const dialog = useDialog<HTMLElement>(onClose);

  function onSearch(e: FormEvent) {
    e.preventDefault();
    load(query);
  }

  async function choose(choice: Track) {
    setChoosing(`${choice.source}/${choice.id}`);
    try {
      await chooseMatch(track, choice);
      onChosen(choice);
    } catch (e) {
      setError(String(e));
      setChoosing(null);
    }
  }

  const isCurrent = (c: Track) => options?.current?.source === c.source && options?.current?.id === c.id;

  return (
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <section ref={dialog} tabIndex={-1} className="dialog wide" role="dialog" aria-modal="true" aria-labelledby="match-title">
        <header className="dialog-header">
          <h2 id="match-title">¿Cuál es la buena?</h2>
          <button className="icon-btn" onClick={onClose} aria-label="Cerrar">
            <CloseIcon />
          </button>
        </header>
        <p className="dialog-intro">
          Elige la versión de <strong>{track.title}</strong> · {track.artist}
          {track.durationSecs > 0 && ` (${formatTime(track.durationSecs)})`}. Melomango la recordará para siempre.
        </p>

        <form className="search dialog-search" onSubmit={onSearch}>
          <SearchIcon size={16} />
          <input value={query} onChange={(e) => setQuery(e.target.value)} aria-label="Buscar otra versión" />
          <button className="btn ghost small" disabled={loading || !query.trim()}>
            Buscar
          </button>
        </form>

        {error && <p className="notice error">{error}</p>}
        {loading && <p className="status">Buscando versiones…</p>}
        {!loading && options && options.candidates.length === 0 && (
          <p className="status">No hay resultados. Prueba a buscar de otra forma.</p>
        )}

        {!loading && options && options.candidates.length > 0 && (
          <ul className="source-list collection-list match-list">
            {options.candidates.map((c) => {
              const key = `${c.source}/${c.id}`;
              const current = isCurrent(c);
              const off = track.durationSecs > 0 && Math.abs(c.durationSecs - track.durationSecs) > 30;
              return (
                <li key={key} className={current ? "current" : undefined}>
                  <Artwork src={c.artworkUrl} />
                  <div className="source-text">
                    <span className="source-name">{c.title}</span>
                    <span className="source-desc">
                      {c.artist} · <span className={off ? "duration-off" : undefined}>{formatTime(c.durationSecs)}</span>
                      {c.source !== "youtube" && ` · ${c.source}`}
                    </span>
                  </div>
                  {current ? (
                    <span className="muted-note">Suena esta</span>
                  ) : (
                    <button className="btn primary small" onClick={() => choose(c)} disabled={choosing !== null}>
                      {choosing === key ? "Eligiendo…" : "Elegir"}
                    </button>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </section>
    </div>
  );
}
