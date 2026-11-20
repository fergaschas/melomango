import { createContext, useContext } from "react";

/** Navigation any component can trigger (e.g. clicking an artist's name). */
export interface Nav {
  openArtist: (name: string, id?: string) => void;
  openAlbum: (id: string) => void;
  /** A public Deezer playlist. */
  openPublicPlaylist: (id: string) => void;
  /** One of the user's playlists. */
  openLibraryPlaylist: (id: number) => void;
}

const nothing = () => {};
export const NavContext = createContext<Nav>({
  openArtist: nothing,
  openAlbum: nothing,
  openPublicPlaylist: nothing,
  openLibraryPlaylist: nothing,
});

export const useNav = () => useContext(NavContext);

export { splitArtists } from "./artists";
