import type { Play, Track } from "./api";
import { splitArtists } from "./artists.ts";

/** What the home page offers, worked out from the listening history and likes. */

const DAY = 86_400_000;
const key = (t: Track) => `${t.source}/${t.id}`;

/** The latest tracks played, each once, newest first. */
export function recentTracks(plays: Play[], limit = 8): Track[] {
  const seen = new Set<string>();
  const out: Track[] = [];
  for (const { track } of plays) {
    if (seen.has(key(track))) continue;
    seen.add(key(track));
    out.push(track);
    if (out.length >= limit) break;
  }
  return out;
}

export interface ArtistStat {
  name: string;
  plays: number;
  /** Cover of their latest song played, to show something. */
  artworkUrl: string | null;
}

/** Most played artists since `since` (collaborations count for each artist). */
export function topArtists(plays: Play[], since: number, limit = 6): ArtistStat[] {
  const stats = new Map<string, ArtistStat>();
  for (const { track, playedAt } of plays) {
    if (playedAt < since) continue;
    for (const name of splitArtists(track.artist)) {
      const id = name.toLowerCase();
      const stat = stats.get(id) ?? { name, plays: 0, artworkUrl: track.artworkUrl };
      stat.plays += 1;
      stat.artworkUrl ??= track.artworkUrl;
      stats.set(id, stat);
    }
  }
  return [...stats.values()].sort((a, b) => b.plays - a.plays).slice(0, limit);
}

/**
 * Songs played several times that haven't sounded for a while: worth bringing back.
 * `plays` is newest first.
 */
export function playAgain(plays: Play[], now: number, limit = 8, quietDays = 14, minPlays = 2): Track[] {
  const byTrack = new Map<string, { track: Track; count: number; last: number }>();
  for (const { track, playedAt } of plays) {
    const entry = byTrack.get(key(track));
    if (entry) entry.count += 1;
    else byTrack.set(key(track), { track, count: 1, last: playedAt });
  }
  return [...byTrack.values()]
    .filter((e) => e.count >= minPlays && now - e.last > quietDays * DAY)
    .sort((a, b) => b.count - a.count || b.last - a.last)
    .slice(0, limit)
    .map((e) => e.track);
}

export interface Mix {
  artist: string;
  tracks: Track[];
}

/** One mix per artist with at least `min` liked songs, biggest first. */
export function likedMixes(liked: Track[], limit = 6, min = 3): Mix[] {
  const byArtist = new Map<string, Mix>();
  for (const track of liked) {
    const main = splitArtists(track.artist)[0];
    if (!main) continue;
    const id = main.toLowerCase();
    const mix = byArtist.get(id) ?? { artist: main, tracks: [] };
    mix.tracks.push(track);
    byArtist.set(id, mix);
  }
  return [...byArtist.values()]
    .filter((m) => m.tracks.length >= min)
    .sort((a, b) => b.tracks.length - a.tracks.length)
    .slice(0, limit);
}

export function shuffled<T>(list: T[], random: () => number = Math.random): T[] {
  const out = list.slice();
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(random() * (i + 1));
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
}

/** "Buenos días" / "Buenas tardes" / "Buenas noches" for the local hour. */
export function greeting(hour: number): string {
  if (hour >= 6 && hour < 14) return "Buenos días";
  if (hour >= 14 && hour < 21) return "Buenas tardes";
  return "Buenas noches";
}
