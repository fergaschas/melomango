import { createContext, useContext } from "react";
import type { Track } from "./api";

/**
 * "No es esta canción": tracks imported from metadata-only services (Spotify) play from a
 * recording Melomango picked; the user can choose another one.
 */
export interface MatchFix {
  /** Whether `track` plays through a match (and so can be fixed). */
  canFix: (track: Track) => boolean;
  open: (track: Track) => void;
}

export const MatchFixContext = createContext<MatchFix>({ canFix: () => false, open: () => {} });

export const useMatchFix = () => useContext(MatchFixContext);
