import { useMemo, useState } from "react";
import type { Track } from "../lib/api";
import { arrange, DEFAULT_SORT, defaultDesc, type SortKey, type SortState } from "../lib/sortTracks";
import { SearchIcon } from "./Icons";

const SORT_PREFIX = "melomango.sort.";

function loadSort(listKey: string): SortState {
  try {
    const saved = JSON.parse(localStorage.getItem(SORT_PREFIX + listKey) ?? "null");
    if (saved && typeof saved.key === "string" && typeof saved.desc === "boolean") return saved;
  } catch {
    /* storage unavailable or corrupt: default order */
  }
  return DEFAULT_SORT;
}

/** Filter text and sort order for one list (the order is remembered per list). */
export function useArrangement(listKey: string, tracks: Track[], addedAt?: number[]) {
  const [filter, setFilter] = useState("");
  const [sorts, setSorts] = useState<Record<string, SortState>>({});
  const sort = sorts[listKey] ?? loadSort(listKey);

  const setSort = (next: SortState) => {
    setSorts((s) => ({ ...s, [listKey]: next }));
    try {
      localStorage.setItem(SORT_PREFIX + listKey, JSON.stringify(next));
    } catch {
      /* storage unavailable: not important */
    }
  };

  const rows = useMemo(() => arrange(tracks, addedAt, filter, sort), [tracks, addedAt, filter, sort]);
  return {
    rows,
    /** The tracks in the order shown (what "Reproducir" and each row play). */
    visible: useMemo(() => rows.map((r) => r.track), [rows]),
    filter,
    setFilter,
    sort,
    setSort,
    /** Reordering by dragging only makes sense in the list's own order, unfiltered. */
    canReorder: sort.key === "custom" && !sort.desc && !filter.trim(),
  };
}

interface Props {
  filter: string;
  onFilter: (text: string) => void;
  sort: SortState;
  onSort: (sort: SortState) => void;
  /** Sort options for this list, with their labels. */
  options: [SortKey, string][];
  shown: number;
  total: number;
}

export function TrackTools({ filter, onFilter, sort, onSort, options, shown, total }: Props) {
  return (
    <div className="track-tools">
      <label className="search track-filter">
        <SearchIcon size={16} />
        <input
          id="track-filter"
          value={filter}
          onChange={(e) => onFilter(e.target.value)}
          onKeyDown={(e) => e.key === "Escape" && onFilter("")}
          placeholder="Filtrar por título o artista"
          aria-label="Filtrar canciones"
        />
      </label>
      {filter.trim() && (
        <span className="track-count">
          {shown} de {total}
        </span>
      )}
      <label className="track-sort">
        <span>Ordenar por</span>
        <select
          id="track-sort"
          value={sort.key}
          onChange={(e) => {
            const key = e.target.value as SortKey;
            onSort({ key, desc: defaultDesc(key) });
          }}
        >
          {options.map(([key, label]) => (
            <option key={key} value={key}>
              {label}
            </option>
          ))}
        </select>
      </label>
      <button
        className="icon-btn sort-dir"
        onClick={() => onSort({ ...sort, desc: !sort.desc })}
        aria-label={sort.desc ? "Orden descendente (cambiar a ascendente)" : "Orden ascendente (cambiar a descendente)"}
        title="Invertir el orden"
      >
        {sort.desc ? "↓" : "↑"}
      </button>
    </div>
  );
}
