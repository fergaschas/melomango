import { Fragment } from "react";
import { splitArtists, useNav } from "../lib/nav";

/** An artist line where each artist's name opens their page. */
export function ArtistLinks({ artist }: { artist: string }) {
  const nav = useNav();
  const names = splitArtists(artist);
  if (names.length === 0) return <>{artist}</>;
  return (
    <>
      {names.map((name, i) => (
        <Fragment key={`${name}-${i}`}>
          {i > 0 && ", "}
          <button
            type="button"
            className="artist-link"
            onClick={(e) => {
              e.stopPropagation();
              nav.openArtist(name);
            }}
            onKeyDown={(e) => e.stopPropagation()}
            title={`Ver a ${name}`}
          >
            {name}
          </button>
        </Fragment>
      ))}
    </>
  );
}
