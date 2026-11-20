import { useEffect, useMemo, useState } from "react";
import { HISTORY_EVENT, likedTracks, recentPlays, trendingTracks, type Play, type SourceInfo, type Track } from "../lib/api";
import { greeting, likedMixes, playAgain, recentTracks, shuffled, topArtists } from "../lib/home";
import { useLibrary } from "../lib/library";
import { useNav } from "../lib/nav";
import type { Player } from "../hooks/usePlayer";
import { CardGrid, type Card } from "./CatalogViews";
import { TrackList } from "./TrackList";

const MONTH = 30 * 86_400_000;
const TRENDING_PREVIEW = 5;

/** The start page: what you listened to, your artists, mixes of your likes and what's trending. */
export function HomeView({
  source,
  player,
  onSeeTrending,
}: {
  source: SourceInfo | null;
  player: Player;
  onSeeTrending: () => void;
}) {
  const library = useLibrary();
  const nav = useNav();
  const [plays, setPlays] = useState<Play[]>([]);
  const [liked, setLiked] = useState<Track[]>([]);
  const [trending, setTrending] = useState<Track[] | null>(null);

  useEffect(() => {
    const load = () => recentPlays(3000).then(setPlays).catch(() => {});
    load();
    window.addEventListener(HISTORY_EVENT, load);
    return () => window.removeEventListener(HISTORY_EVENT, load);
  }, []);

  useEffect(() => {
    likedTracks().then(setLiked).catch(() => {});
  }, [library.version]);

  useEffect(() => {
    if (!source?.supportsTrending) {
      setTrending([]);
      return;
    }
    let cancelled = false;
    setTrending(null);
    trendingTracks(source.id)
      .then((t) => !cancelled && setTrending(t))
      .catch(() => !cancelled && setTrending([]));
    return () => {
      cancelled = true;
    };
  }, [source]);

  const now = Date.now();
  const recent = useMemo(() => recentTracks(plays, 8), [plays]);
  const again = useMemo(() => playAgain(plays, now, 8), [plays]); // eslint-disable-line react-hooks/exhaustive-deps
  const artists = useMemo(() => topArtists(plays, now - MONTH, 6), [plays]); // eslint-disable-line react-hooks/exhaustive-deps
  const mixes = useMemo(() => likedMixes(liked, 5), [liked]);

  const trackCards = (tracks: Track[]): Card[] =>
    tracks.map((t, i) => ({
      key: `${t.source}/${t.id}`,
      title: t.title,
      subtitle: t.artist,
      imageUrl: t.artworkUrl,
      onOpen: () => player.playQueue(tracks, i),
    }));

  const mixCards: Card[] = [
    ...(liked.length > 1
      ? [
          {
            key: "liked-shuffle",
            title: "Tus Me gusta",
            subtitle: `${liked.length} canciones en aleatorio`,
            imageUrl: liked[0]?.artworkUrl ?? null,
            onOpen: () => player.playQueue(shuffled(liked), 0),
          },
        ]
      : []),
    ...mixes.map((m) => ({
      key: `mix-${m.artist}`,
      title: `Mezcla de ${m.artist}`,
      subtitle: `${m.tracks.length} canciones que te gustan`,
      imageUrl: m.tracks.find((t) => t.artworkUrl)?.artworkUrl ?? null,
      onOpen: () => player.playQueue(shuffled(m.tracks), 0),
    })),
  ];

  const artistCards: Card[] = artists.map((a) => ({
    key: a.name,
    title: a.name,
    subtitle: `${a.plays} ${a.plays === 1 ? "escucha" : "escuchas"} este mes`,
    imageUrl: a.artworkUrl,
    round: true,
    onOpen: () => nav.openArtist(a.name),
  }));

  const playlistCards: Card[] = library.playlists.slice(0, 6).map((p) => ({
    key: `pl-${p.id}`,
    title: p.name,
    subtitle: `${p.trackCount} canciones`,
    imageUrl: p.artworkUrls[0] ?? null,
    onOpen: () => nav.openLibraryPlaylist(p.id),
  }));

  const isNew = plays.length === 0 && liked.length === 0;
  const top = trending ?? [];

  return (
    <>
      <h1>{greeting(new Date().getHours())}</h1>
      <p className="subtitle">
        {isNew
          ? "Pon algo de música: este inicio se irá llenando con lo que escuchas y lo que te gusta."
          : "Lo tuyo, para seguir escuchando."}
      </p>

      <HomeRow title="Escuchado hace poco" cards={trackCards(recent)} />
      <HomeRow title="Tus artistas del mes" cards={artistCards} />
      <HomeRow title="Mezclas de tus Me gusta" cards={mixCards} note="En aleatorio; después sigue la radio." />
      <HomeRow title="Vuelve a escuchar" cards={trackCards(again)} note="Las escuchabas a menudo y hace tiempo que no suenan." />
      <HomeRow title="Tus playlists" cards={playlistCards} />

      {source?.supportsTrending && (
        <section className="catalog-section">
          <div className="catalog-section-head">
            <h2>Tendencias en {source.name}</h2>
            {top.length > TRENDING_PREVIEW && (
              <button className="link-btn" onClick={onSeeTrending}>
                Ver todas
              </button>
            )}
          </div>
          {trending === null ? (
            <p className="status">Pelando mangos…</p>
          ) : top.length === 0 ? (
            <p className="status">
              No se pudieron cargar las tendencias.{" "}
              <button className="link-btn" onClick={onSeeTrending}>
                Ver por qué
              </button>
            </p>
          ) : (
            <TrackList
              tracks={top.slice(0, TRENDING_PREVIEW)}
              current={player.current}
              isPlaying={player.isPlaying}
              queue={player}
              onPlay={(i) => player.playQueue(top, i)}
            />
          )}
        </section>
      )}
    </>
  );
}

function HomeRow({ title, cards, note }: { title: string; cards: Card[]; note?: string }) {
  if (!cards.length) return null;
  return (
    <section className="catalog-section">
      <div className="catalog-section-head">
        <h2>{title}</h2>
        {note && <span className="home-note">{note}</span>}
      </div>
      <CardGrid cards={cards.slice(0, 8)} row />
    </section>
  );
}
