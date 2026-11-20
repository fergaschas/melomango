/** "Shakira, Burna Boy" / "Bad Bunny & Drake" / "A feat. B" -> each artist. */
export function splitArtists(artist: string): string[] {
  return artist
    .split(/\s*,\s*|\s+&\s+|\s+(?:feat\.?|ft\.?|featuring)\s+/i)
    .map((a) => a.trim())
    .filter(Boolean);
}
