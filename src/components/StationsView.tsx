import { memo, useEffect, useMemo, useState, type FormEvent } from "react";
import {
  stationCountries,
  stationsNear,
  stationsSearch,
  stationsTop,
  systemCountry,
  systemLocation,
  type Station,
  type StationCountry,
} from "../lib/api";
import { countryName, loadPosition, savePosition, stationDetail } from "../lib/stations";
import { useStations } from "../lib/stationsContext";
import { useDialog } from "../lib/useDialog";
import type { Player } from "../hooks/usePlayer";
import { ErrorNotice } from "./ErrorNotice";
import { CloseIcon, HeartFilledIcon, HeartIcon, PauseIcon, PlayIcon, RadioIcon, SearchIcon } from "./Icons";

const LOADING = "Sintonizando…";
const NO_RESULTS = "No hay emisoras con ese nombre. Prueba con otro o con un género («rock», «noticias»).";
const NO_LOCATION = "Activa la ubicación aproximada para ver las emisoras de tu zona. Solo se usa para eso.";

/** A station's logo, or the radio icon when it has none (or it doesn't load). */
export function StationLogo({ src }: { src: string | null | undefined }) {
  const [failed, setFailed] = useState<string | null>(null);
  if (!src || failed === src) {
    return (
      <span className="art-placeholder station-logo-empty">
        <RadioIcon size={22} />
      </span>
    );
  }
  return <img className="station-logo" src={src} alt="" loading="lazy" onError={() => setFailed(src)} />;
}

/** What a list request gives: the stations, or why not (`null` while it loads or isn't asked). */
interface Loaded {
  stations: Station[] | null;
  error: string | null;
  loading: boolean;
}

/** Runs `load` whenever `key` changes (`null`: nothing to ask). `retry` asks again. */
function useStationList(key: string | null, load: () => Promise<Station[]>, retry: number): Loaded {
  const [state, setState] = useState<Loaded>({ stations: null, error: null, loading: false });
  useEffect(() => {
    if (key === null) {
      setState({ stations: null, error: null, loading: false });
      return;
    }
    let cancelled = false;
    setState((s) => ({ ...s, error: null, loading: true }));
    load()
      .then((stations) => !cancelled && setState({ stations, error: null, loading: false }))
      .catch((e) => !cancelled && setState({ stations: null, error: String(e), loading: false }));
    return () => {
      cancelled = true;
    };
    // `load` is a new function on every render: `key` says when the request changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, retry]);
  return state;
}

/** «Emisoras»: near the user, the most listened in their country, search, favourites and recent. */
export function StationsView({ player }: { player: Player }) {
  const stations = useStations();
  const [typed, setTyped] = useState("");
  const [query, setQuery] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const [choosing, setChoosing] = useState(false);

  // The country: picked by hand, or the system's region.
  const [systemCode, setSystemCode] = useState<string | null | undefined>(undefined);
  useEffect(() => {
    systemCountry()
      .then(setSystemCode)
      .catch(() => setSystemCode(null));
  }, []);
  const country = stations.chosenCountry ?? systemCode ?? null;

  // Where the user is, if they allow it (asked again after a day).
  const [position, setPosition] = useState(loadPosition);
  const [locating, setLocating] = useState(false);
  const [locationError, setLocationError] = useState<string | null>(null);
  useEffect(() => {
    if (!stations.useLocation) {
      setPosition(null);
      setLocationError(null);
      return;
    }
    if (position) return;
    let cancelled = false;
    setLocating(true);
    setLocationError(null);
    systemLocation()
      .then((at) => {
        if (cancelled || at.lat === null || at.long === null) return;
        savePosition(at.lat, at.long);
        setPosition(loadPosition());
      })
      .catch((e) => !cancelled && setLocationError(String(e)))
      .finally(() => !cancelled && setLocating(false));
    return () => {
      cancelled = true;
    };
  }, [stations.useLocation, position, retry]);

  const near = useStationList(
    position ? `${position.lat},${position.long}` : null,
    () => stationsNear(position!.lat, position!.long),
    retry,
  );
  const top = useStationList(country, () => stationsTop(country!), retry);
  const found = useStationList(query === null ? null : `${query}|${country ?? ""}`, () => stationsSearch(query!, country), retry);

  function onSearch(e: FormEvent) {
    e.preventDefault();
    setQuery(typed.trim() || null);
  }

  function clearSearch() {
    setTyped("");
    setQuery(null);
  }

  const chooser = (
    <button className="btn ghost small" onClick={() => setChoosing(true)}>
      Elegir país
    </button>
  );

  return (
    <>
      <header className="collection-header">
        <div className="history-cover" aria-hidden="true">
          <RadioIcon size={64} />
        </div>
        <div className="collection-info">
          <span className="eyebrow">En directo</span>
          <h1>Emisoras</h1>
          <p className="subtitle">
            Radio por internet, con las de tu zona primero
            {country && (
              <>
                {" · "}
                <button className="link-btn" onClick={() => setChoosing(true)} title="Elegir país">
                  {countryName(country)}
                </button>
              </>
            )}
          </p>
          <form className="search station-search" role="search" onSubmit={onSearch}>
            <SearchIcon size={16} />
            <input
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
              onKeyDown={(e) => e.key === "Escape" && clearSearch()}
              placeholder="Buscar emisoras"
              aria-label="Buscar emisoras"
            />
            {(typed || query) && (
              <button type="button" className="icon-btn" onClick={clearSearch} aria-label="Borrar la búsqueda">
                <CloseIcon size={16} />
              </button>
            )}
          </form>
        </div>
      </header>

      {query !== null ? (
        <section className="catalog-section first">
          <h2>Resultados para “{query}”</h2>
          <StationSection
            loaded={found}
            player={player}
            empty={NO_RESULTS}
            onRetry={() => setRetry((r) => r + 1)}
          />
        </section>
      ) : (
        <>
          <section className="catalog-section first">
            <h2>Cerca de ti</h2>
            {!stations.useLocation ? (
              <div className="station-prompt">
                <p>{NO_LOCATION}</p>
                <div className="error-actions">
                  <button className="btn primary small" onClick={() => stations.setUseLocation(true)}>
                    Activar ubicación
                  </button>
                  {chooser}
                </div>
              </div>
            ) : locating ? (
              <p className="status">{LOADING}</p>
            ) : locationError ? (
              <ErrorNotice error={locationError} onRetry={() => setRetry((r) => r + 1)}>
                {chooser}
              </ErrorNotice>
            ) : (
              <StationSection
                loaded={near}
                player={player}
                empty="No hay emisoras a menos de 50 km que digan dónde están. Las más escuchadas del país están abajo."
                onRetry={() => setRetry((r) => r + 1)}
              />
            )}
          </section>

          <section className="catalog-section">
            <div className="catalog-section-head">
              <h2>{country ? `Lo más escuchado en ${countryName(country)}` : "Lo más escuchado"}</h2>
              {country && chooser}
            </div>
            {country ? (
              <StationSection
                loaded={top}
                player={player}
                empty="No hay emisoras de este país."
                onRetry={() => setRetry((r) => r + 1)}
              />
            ) : systemCode === undefined ? (
              <p className="status">{LOADING}</p>
            ) : (
              <div className="station-prompt">
                <p>Elige un país para ver sus emisoras.</p>
                <div className="error-actions">{chooser}</div>
              </div>
            )}
          </section>

          {stations.liked.length > 0 && (
            <section className="catalog-section">
              <h2>Tus emisoras</h2>
              <StationList stations={stations.liked} player={player} />
            </section>
          )}

          {stations.recent.length > 0 && (
            <section className="catalog-section">
              <h2>Escuchadas hace poco</h2>
              <StationList stations={stations.recent} player={player} />
            </section>
          )}
        </>
      )}

      {choosing && (
        <CountryDialog
          current={country}
          onChoose={(code) => {
            stations.chooseCountry(code);
            setChoosing(false);
          }}
          onClose={() => setChoosing(false)}
        />
      )}
    </>
  );
}

function StationSection({
  loaded,
  player,
  empty,
  onRetry,
}: {
  loaded: Loaded;
  player: Player;
  empty: string;
  onRetry: () => void;
}) {
  if (loaded.loading && !loaded.stations) return <p className="status">{LOADING}</p>;
  if (loaded.error) return <ErrorNotice error={loaded.error} onRetry={onRetry} />;
  if (!loaded.stations) return null;
  if (loaded.stations.length === 0) return <p className="status">{empty}</p>;
  return <StationList stations={loaded.stations} player={player} />;
}

/** Stations to play; «Siguiente» and «Anterior» then move within this list. */
export function StationList({ stations, player }: { stations: Station[]; player: Player }) {
  const favourites = useStations();
  const playingUuid = player.live?.station.uuid ?? null;
  return (
    <ol className="track-list station-list">
      {stations.map((station, i) => {
        const active = station.uuid === playingUuid;
        return (
          <StationRow
            key={station.uuid}
            station={station}
            active={active}
            playing={active && player.isPlaying}
            liked={favourites.isLiked(station)}
            onPlay={() => (active ? player.toggle() : player.playStation(stations, i))}
            onLike={() => favourites.toggleLike(station)}
          />
        );
      })}
    </ol>
  );
}

const StationRow = memo(function StationRow({
  station,
  active,
  playing,
  liked,
  onPlay,
  onLike,
}: {
  station: Station;
  active: boolean;
  playing: boolean;
  liked: boolean;
  onPlay: () => void;
  onLike: () => void;
}) {
  const detail = stationDetail(station);
  const likeLabel = liked ? "Quitar de tus emisoras" : "Guardar en tus emisoras";
  return (
    <li className={`track station${active ? " active" : ""}`} onClick={onPlay}>
      <button
        className="track-num"
        onClick={(e) => {
          e.stopPropagation();
          onPlay();
        }}
        aria-label={`${playing ? "Pausa" : "Escuchar"}: ${station.name}`}
        aria-current={active ? "true" : undefined}
        title={playing ? "Pausa" : "Escuchar"}
      >
        {playing ? <PauseIcon size={14} /> : <PlayIcon size={14} />}
      </button>
      <span className="track-main">
        <StationLogo src={station.favicon} />
        <span className="track-text">
          <span className="track-title">{station.name}</span>
          {detail && <span className="track-artist">{detail}</span>}
        </span>
      </span>
      <span className="track-genre">{station.tags.slice(0, 3).join(", ")}</span>
      <span className="track-actions">
        <button
          className={`row-action heart${liked ? " on" : ""}`}
          onClick={(e) => {
            e.stopPropagation();
            onLike();
          }}
          aria-label={likeLabel}
          aria-pressed={liked}
          title={likeLabel}
        >
          {liked ? <HeartFilledIcon size={18} /> : <HeartIcon size={18} />}
        </button>
      </span>
    </li>
  );
});

/** Picks the country of «Lo más escuchado» (remembered, ahead of the system's region). */
function CountryDialog({
  current,
  onChoose,
  onClose,
}: {
  current: string | null;
  onChoose: (code: string) => void;
  onClose: () => void;
}) {
  const dialog = useDialog<HTMLElement>(onClose);
  const [countries, setCountries] = useState<StationCountry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [retry, setRetry] = useState(0);

  useEffect(() => {
    setError(null);
    stationCountries()
      .then(setCountries)
      .catch((e) => setError(String(e)));
  }, [retry]);

  const fold = (s: string) => s.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "");
  const shown = useMemo(() => {
    const named = (countries ?? []).map((c) => ({ ...c, label: countryName(c.code) }));
    named.sort((a, b) => a.label.localeCompare(b.label, "es"));
    const f = fold(filter.trim());
    return f ? named.filter((c) => fold(c.label).includes(f) || fold(c.name).includes(f)) : named;
  }, [countries, filter]);

  return (
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <section ref={dialog} tabIndex={-1} className="dialog" role="dialog" aria-modal="true" aria-labelledby="country-title">
        <header className="dialog-header">
          <h2 id="country-title">Elegir país</h2>
          <button className="icon-btn" onClick={onClose} aria-label="Cerrar">
            <CloseIcon />
          </button>
        </header>
        <p className="dialog-intro">Sus emisoras más escuchadas salen en «Emisoras». Melomango lo recuerda.</p>
        <label className="search dialog-search">
          <SearchIcon size={16} />
          <input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Buscar país" data-autofocus />
        </label>
        {error && <ErrorNotice error={error} onRetry={() => setRetry((r) => r + 1)} />}
        {!countries && !error && <p className="status">{LOADING}</p>}
        {countries && (
          <ul className="country-list">
            {shown.map((c) => (
              <li key={c.code}>
                <button
                  className={`country${c.code === current ? " active" : ""}`}
                  onClick={() => onChoose(c.code)}
                  aria-current={c.code === current ? "true" : undefined}
                >
                  <span>{c.label}</span>
                  <span className="country-count">
                    {c.stations.toLocaleString("es")} {c.stations === 1 ? "emisora" : "emisoras"}
                  </span>
                </button>
              </li>
            ))}
            {shown.length === 0 && <li className="status">Ningún país se llama así.</li>}
          </ul>
        )}
      </section>
    </div>
  );
}
