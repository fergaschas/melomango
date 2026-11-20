import { createContext, useContext } from "react";

/** Navigation any component can trigger (e.g. clicking an artist's name). */
export interface Nav {
  openArtist: (name: string, id?: string) => void;
  openAlbum: (id: string) => void;
}

export const NavContext = createContext<Nav>({ openArtist: () => {}, openAlbum: () => {} });

export const useNav = () => useContext(NavContext);

export { splitArtists } from "./artists";
