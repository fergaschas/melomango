import { test } from "node:test";
import assert from "node:assert/strict";
import { greeting, likedMixes, playAgain, recentTracks, shuffled, topArtists } from "../src/lib/home.ts";
import type { Play, Track } from "../src/lib/api.ts";

const t = (id: string, artist = "Artista"): Track => ({
  id,
  source: "test",
  title: `Canción ${id}`,
  artist,
  durationSecs: 200,
  artworkUrl: `https://img/${id}.jpg`,
  genre: null,
});
const DAY = 86_400_000;
const now = 100 * DAY;
const play = (track: Track, daysAgo: number): Play => ({ track, playedAt: now - daysAgo * DAY });

test("recent tracks: newest first, each once", () => {
  const plays = [play(t("a"), 0), play(t("b"), 1), play(t("a"), 2), play(t("c"), 3)];
  assert.deepEqual(recentTracks(plays).map((x) => x.id), ["a", "b", "c"]);
  assert.deepEqual(recentTracks(plays, 2).map((x) => x.id), ["a", "b"]);
});

test("top artists since a date, counting collaborations for each artist", () => {
  const plays = [
    play(t("a", "Shakira, Burna Boy"), 1),
    play(t("b", "Shakira"), 2),
    play(t("c", "Izal"), 3),
    play(t("d", "Izal"), 40),
    play(t("e", "Izal"), 41),
  ];
  const top = topArtists(plays, now - 30 * DAY);
  assert.deepEqual(top.map((a) => [a.name, a.plays]), [["Shakira", 2], ["Burna Boy", 1], ["Izal", 1]]);
  assert.equal(top[0]!.artworkUrl, "https://img/a.jpg");
});

test("play again: often played, quiet for a while", () => {
  const plays = [
    play(t("recent"), 1),
    play(t("recent"), 20),
    play(t("old"), 20),
    play(t("old"), 30),
    play(t("old"), 31),
    play(t("once"), 25),
    play(t("pair"), 16),
    play(t("pair"), 50),
  ];
  assert.deepEqual(playAgain(plays, now).map((x) => x.id), ["old", "pair"]);
});

test("mixes from liked songs, one per artist with enough songs", () => {
  const liked = [t("1", "Vetusta Morla"), t("2", "Vetusta Morla"), t("3", "vetusta morla"), t("4", "Izal"), t("5", "Izal, Zahara")];
  const mixes = likedMixes(liked);
  assert.deepEqual(mixes.map((m) => [m.artist, m.tracks.length]), [["Vetusta Morla", 3]]);
  assert.deepEqual(likedMixes(liked, 6, 2).map((m) => m.artist), ["Vetusta Morla", "Izal"]);
});

test("shuffle keeps every item; greeting follows the hour", () => {
  assert.deepEqual(shuffled([1, 2, 3, 4], () => 0.5).slice().sort(), [1, 2, 3, 4]);
  assert.equal(greeting(9), "Buenos días");
  assert.equal(greeting(16), "Buenas tardes");
  assert.equal(greeting(23), "Buenas noches");
  assert.equal(greeting(3), "Buenas noches");
});
