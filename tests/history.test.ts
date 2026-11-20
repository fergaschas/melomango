import { test } from "node:test";
import assert from "node:assert/strict";
import { groupByDay } from "../src/lib/history.ts";
import type { Play, Track } from "../src/lib/api.ts";

const t = (id: string): Track => ({
  id,
  source: "test",
  title: `Canción ${id}`,
  artist: "Artista",
  durationSecs: 180,
  artworkUrl: null,
  genre: null,
});
const at = (y: number, m: number, d: number, h = 12) => new Date(y, m - 1, d, h).getTime();
const play = (id: string, when: number): Play => ({ track: t(id), playedAt: when });

test("groups by day with friendly labels, each track once per day", () => {
  const now = at(2026, 11, 20, 18);
  const days = groupByDay(
    [
      play("a", at(2026, 11, 20, 17)),
      play("b", at(2026, 11, 20, 16)),
      play("a", at(2026, 11, 20, 9)),
      play("c", at(2026, 11, 19, 23)),
      play("d", at(2026, 11, 17)),
      play("e", at(2025, 12, 31)),
    ],
    now,
  );
  assert.deepEqual(
    days.map((d) => [d.label, d.tracks.map((x) => x.id), d.plays]),
    [
      ["Hoy", ["a", "b"], 3],
      ["Ayer", ["c"], 1],
      ["Martes, 17 de noviembre", ["d"], 1],
      ["Miércoles, 31 de diciembre de 2025", ["e"], 1],
    ],
  );
});

test("an empty history has no days", () => {
  assert.deepEqual(groupByDay([]), []);
});
