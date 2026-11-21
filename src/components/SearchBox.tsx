import { useEffect, useRef, useState, type RefObject } from "react";
import { searchSuggestions, type Suggestions, type Track } from "../lib/api";
import { loadRecent, matchingRecent, removeRecent, saveRecent } from "../lib/recentSearches";
import { Artwork } from "./Artwork";
import { ClockIcon, CloseIcon, SearchIcon } from "./Icons";

const DEBOUNCE_MS = 250;

type Item =
  | { kind: "recent"; text: string }
  | { kind: "artist"; id: string; name: string; pictureUrl: string | null }
  | { kind: "track"; track: Track }
  | { kind: "search"; text: string };

interface Props {
  value: string;
  onChange: (text: string) => void;
  /** Run a search (and remember it). */
  onSearch: (query: string) => void;
  onOpenArtist: (name: string, id: string) => void;
  onPlayTrack: (track: Track) => void;
  placeholder: string;
  inputRef: RefObject<HTMLInputElement | null>;
}

/** The search box: recent searches when empty, artist and song suggestions while typing. */
export function SearchBox({ value, onChange, onSearch, onOpenArtist, onPlayTrack, placeholder, inputRef }: Props) {
  const [open, setOpen] = useState(false);
  const [recent, setRecent] = useState<string[]>(loadRecent);
  const [hints, setHints] = useState<Suggestions | null>(null);
  const [active, setActive] = useState(-1);
  const latest = useRef("");

  // Suggestions for what is typed, a moment after the last key.
  useEffect(() => {
    const q = value.trim();
    latest.current = q;
    if (q.length < 2) return;
    const timer = window.setTimeout(() => {
      searchSuggestions(q)
        .then((found) => latest.current === q && setHints(found))
        .catch(() => latest.current === q && setHints(null));
    }, DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [value]);

  const typed = value.trim();
  const items: Item[] = typed
    ? [
        ...matchingRecent(recent, typed).map((text): Item => ({ kind: "recent", text })),
        ...(hints?.artists ?? []).map((a): Item => ({ kind: "artist", id: a.id, name: a.name, pictureUrl: a.pictureUrl })),
        ...(hints?.tracks ?? []).map((track): Item => ({ kind: "track", track })),
        { kind: "search", text: typed },
      ]
    : recent.map((text): Item => ({ kind: "recent", text }));

  function choose(item: Item) {
    setOpen(false);
    setActive(-1);
    inputRef.current?.blur();
    switch (item.kind) {
      case "recent":
      case "search":
        onChange(item.text);
        onSearch(item.text);
        setRecent(loadRecent());
        break;
      case "artist":
        onOpenArtist(item.name, item.id);
        break;
      case "track":
        onPlayTrack(item.track);
        break;
    }
  }

  function forget(text: string) {
    const next = removeRecent(recent, text);
    setRecent(next);
    saveRecent(next);
  }

  function clearAll() {
    setRecent([]);
    saveRecent([]);
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      if (!items.length) return;
      e.preventDefault();
      setOpen(true);
      // -1 is the box itself; past either end it wraps around.
      setActive((i) =>
        e.key === "ArrowDown" ? (i + 1 >= items.length ? -1 : i + 1) : i <= -1 ? items.length - 1 : i - 1,
      );
    } else if (e.key === "Enter") {
      e.preventDefault();
      const item = open ? items[active] : undefined;
      if (item) choose(item);
      else choose({ kind: "search", text: typed });
    } else if (e.key === "Escape") {
      if (open) setOpen(false);
      else e.currentTarget.blur();
    }
  }

  const showList = open && items.length > 0;
  const sections: { title: string; kinds: Item["kind"][] }[] = typed
    ? [
        { title: "Búsquedas recientes", kinds: ["recent"] },
        { title: "Artistas", kinds: ["artist"] },
        { title: "Canciones", kinds: ["track"] },
        { title: "", kinds: ["search"] },
      ]
    : [{ title: "Búsquedas recientes", kinds: ["recent"] }];

  return (
    <form
      className="search search-box"
      onSubmit={(e) => {
        e.preventDefault();
        choose({ kind: "search", text: typed });
      }}
    >
      <SearchIcon size={18} />
      <input
        ref={inputRef}
        id="search-input"
        value={value}
        onChange={(e) => {
          onChange(e.target.value);
          // Right away (not in an effect): an Enter typed straight after must not pick an
          // option, nor a suggestion for the previous text.
          setActive(-1);
          setHints(null);
          setOpen(true);
        }}
        onFocus={() => {
          setRecent(loadRecent());
          setActive(-1);
          setOpen(true);
        }}
        onBlur={() => setOpen(false)}
        onKeyDown={onKeyDown}
        placeholder={placeholder}
        aria-label="Buscar"
        title="Buscar (Ctrl+F)"
        role="combobox"
        aria-expanded={showList}
        aria-controls="search-suggestions"
        aria-autocomplete="list"
        // The option chosen with the arrows, so screen readers announce it.
        aria-activedescendant={showList && active >= 0 && active < items.length ? `search-option-${active}` : undefined}
        autoComplete="off"
      />
      {showList && (
        <div
          className="search-dropdown"
          id="search-suggestions"
          role="listbox"
          // Keep the focus in the box while clicking an option.
          onMouseDown={(e) => e.preventDefault()}
        >
          {sections.map(({ title, kinds }) => {
            const entries = items.map((item, index) => ({ item, index })).filter(({ item }) => kinds.includes(item.kind));
            if (!entries.length) return null;
            return (
              <div key={title || "search"} className="dropdown-section" role="group" aria-label={title || "Buscar"}>
                {title && (
                  <div className="dropdown-title">
                    <span>{title}</span>
                    {!typed && kinds[0] === "recent" && (
                      <button type="button" className="link-btn" onClick={clearAll}>
                        Borrar todo
                      </button>
                    )}
                  </div>
                )}
                {entries.map(({ item, index }) => (
                  <div
                    key={index}
                    id={`search-option-${index}`}
                    role="option"
                    aria-selected={index === active}
                    className={`dropdown-item${index === active ? " active" : ""}`}
                    // Only a real mouse movement marks an option (not the list appearing under it).
                    onMouseMove={() => active !== index && setActive(index)}
                    onClick={() => choose(item)}
                  >
                    <ItemContent item={item} />
                    {item.kind === "recent" && (
                      <button
                        type="button"
                        className="row-action"
                        aria-label={`Quitar «${item.text}» de las recientes`}
                        onClick={(e) => {
                          e.stopPropagation();
                          forget(item.text);
                        }}
                      >
                        <CloseIcon size={14} />
                      </button>
                    )}
                  </div>
                ))}
              </div>
            );
          })}
        </div>
      )}
    </form>
  );
}

function ItemContent({ item }: { item: Item }) {
  switch (item.kind) {
    case "recent":
      return (
        <span className="dropdown-main">
          <span className="dropdown-icon">
            <ClockIcon size={16} />
          </span>
          <span className="dropdown-text">{item.text}</span>
        </span>
      );
    case "search":
      return (
        <span className="dropdown-main">
          <span className="dropdown-icon">
            <SearchIcon size={16} />
          </span>
          <span className="dropdown-text">
            Buscar «<strong>{item.text}</strong>»
          </span>
        </span>
      );
    case "artist":
      return (
        <span className="dropdown-main">
          <span className="dropdown-art round">
            <Artwork src={item.pictureUrl} />
          </span>
          <span className="dropdown-text two-lines">
            <strong>{item.name}</strong>
            <span className="dropdown-sub">Artista</span>
          </span>
        </span>
      );
    case "track":
      return (
        <span className="dropdown-main">
          <span className="dropdown-art">
            <Artwork src={item.track.artworkUrl} />
          </span>
          <span className="dropdown-text two-lines">
            <strong>{item.track.title}</strong>
            <span className="dropdown-sub">Canción · {item.track.artist}</span>
          </span>
        </span>
      );
  }
}
