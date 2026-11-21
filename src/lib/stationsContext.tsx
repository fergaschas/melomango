import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { likedStations, setStationLiked, type Station } from "./api";
import { useLibrary } from "./library";
import {
  addRecentStation,
  loadChosenCountry,
  loadRecentStations,
  loadUseLocation,
  saveChosenCountry,
  saveRecentStations,
  saveUseLocation,
} from "./stations";

interface StationsValue {
  /** «Tus emisoras», the newest first. */
  liked: Station[];
  isLiked: (s: Pick<Station, "uuid">) => boolean;
  /** Never rejects: resolves whether it was saved (if not, a toast says why). */
  toggleLike: (s: Station) => Promise<boolean>;
  /** «Escuchadas hace poco». */
  recent: Station[];
  addRecent: (s: Station) => void;
  /** Whether «Cerca de ti» may use the approximate location (Ajustes). */
  useLocation: boolean;
  setUseLocation: (on: boolean) => void;
  /** The country picked by hand, if any. */
  chosenCountry: string | null;
  chooseCountry: (code: string) => void;
}

const StationsContext = createContext<StationsValue | null>(null);

export function useStations() {
  const value = useContext(StationsContext);
  if (!value) throw new Error("useStations fuera de StationsProvider");
  return value;
}

/** Favourite stations and the settings of «Emisoras», shared by the screen, the player and Ajustes. */
export function StationsProvider({ children }: { children: ReactNode }) {
  const { notify } = useLibrary();
  const [liked, setLiked] = useState<Station[]>([]);
  const likedRef = useRef(liked);
  likedRef.current = liked;
  const [recent, setRecent] = useState(loadRecentStations);
  const [useLocation, setUseLocationState] = useState(loadUseLocation);
  const [chosenCountry, setChosenCountry] = useState(loadChosenCountry);

  const reload = useCallback(() => likedStations().then(setLiked), []);

  useEffect(() => {
    reload().catch(() => {});
  }, [reload]);

  const toggleLike = useCallback(
    async (station: Station) => {
      const now = !likedRef.current.some((s) => s.uuid === station.uuid);
      // Optimistic: the heart reacts at once, and goes back if saving fails.
      setLiked((list) => (now ? [station, ...list] : list.filter((s) => s.uuid !== station.uuid)));
      try {
        await setStationLiked(station, now);
        notify(now ? `«${station.name}» guardada en tus emisoras` : `«${station.name}» quitada de tus emisoras`);
        return true;
      } catch (e) {
        notify(String(e));
        return false;
      } finally {
        reload().catch(() => {});
      }
    },
    [notify, reload],
  );

  const addRecent = useCallback((station: Station) => {
    setRecent((list) => {
      const next = addRecentStation(list, station);
      saveRecentStations(next);
      return next;
    });
  }, []);

  const setUseLocation = useCallback((on: boolean) => {
    saveUseLocation(on);
    setUseLocationState(on);
  }, []);

  const chooseCountry = useCallback((code: string) => {
    saveChosenCountry(code);
    setChosenCountry(code);
  }, []);

  const value = useMemo<StationsValue>(() => {
    const uuids = new Set(liked.map((s) => s.uuid));
    return {
      liked,
      isLiked: (s) => uuids.has(s.uuid),
      toggleLike,
      recent,
      addRecent,
      useLocation,
      setUseLocation,
      chosenCountry,
      chooseCountry,
    };
  }, [liked, toggleLike, recent, addRecent, useLocation, setUseLocation, chosenCountry, chooseCountry]);

  return <StationsContext.Provider value={value}>{children}</StationsContext.Provider>;
}
