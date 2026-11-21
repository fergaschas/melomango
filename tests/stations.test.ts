import { test } from "node:test";
import assert from "node:assert/strict";
import type { Station } from "../src/lib/api.ts";
import {
  addRecentStation,
  countryName,
  distanceText,
  freshPosition,
  MAX_RECENT_STATIONS,
  neighbour,
  POSITION_MAX_AGE_MS,
  quality,
  stationDetail,
} from "../src/lib/stations.ts";

function station(uuid: string, extra: Partial<Station> = {}): Station {
  return {
    uuid,
    name: `Radio ${uuid}`,
    url: `http://${uuid}`,
    homepage: null,
    favicon: null,
    country: "ES",
    state: null,
    tags: [],
    codec: "MP3",
    bitrate: 128,
    lat: null,
    long: null,
    distanceM: null,
    ...extra,
  };
}

test("quality: codec and bitrate, the ones known", () => {
  assert.equal(quality({ codec: "AAC", bitrate: 128 }), "AAC 128");
  assert.equal(quality({ codec: "MP3", bitrate: 0 }), "MP3");
  assert.equal(quality({ codec: "", bitrate: 0 }), "");
});

test("distance in whole kilometres, never zero", () => {
  assert.equal(distanceText(159), "a 1 km");
  assert.equal(distanceText(2400), "a 2 km");
  assert.equal(distanceText(35_268), "a 35 km");
});

test("the line under the name has only what the station has", () => {
  assert.equal(stationDetail(station("a", { state: "Catalunya", distanceM: 2000, codec: "AAC" })), "Catalunya · a 2 km · AAC 128");
  assert.equal(stationDetail(station("b")), "MP3 128");
  assert.equal(stationDetail(station("c", { codec: "", bitrate: 0 })), "");
});

test("heard lately: newest first, once each, without the old distance", () => {
  let list: Station[] = [];
  list = addRecentStation(list, station("a", { distanceM: 500 }));
  list = addRecentStation(list, station("b"));
  list = addRecentStation(list, station("a"));
  assert.deepEqual(
    list.map((s) => s.uuid),
    ["a", "b"],
  );
  assert.equal(list[0]?.distanceM, null);
  for (let i = 0; i < 20; i++) list = addRecentStation(list, station(`s${i}`));
  assert.equal(list.length, MAX_RECENT_STATIONS);
  assert.equal(list[0]?.uuid, "s19");
});

test("next and previous stay inside the list", () => {
  const list = [station("a"), station("b"), station("c")];
  assert.equal(neighbour(list, 0, 1), 1);
  assert.equal(neighbour(list, 2, 1), null);
  assert.equal(neighbour(list, 0, -1), null);
  assert.equal(neighbour(list, 2, -1), 1);
});

test("a saved position is used for a day, rounded, and only if valid", () => {
  const now = 1_800_000_000_000;
  const saved = JSON.stringify({ lat: 41.98365, long: 2.82111, at: now - 1000 });
  assert.deepEqual(freshPosition(saved, now), { lat: 41.98, long: 2.82, at: now - 1000 });
  const old = JSON.stringify({ lat: 41.98, long: 2.82, at: now - POSITION_MAX_AGE_MS - 1 });
  assert.equal(freshPosition(old, now), null);
  assert.equal(freshPosition(JSON.stringify({ lat: 120, long: 2, at: now }), now), null);
  assert.equal(freshPosition("{not json", now), null);
  assert.equal(freshPosition(null, now), null);
});

test("countries by their Spanish name", () => {
  assert.equal(countryName("ES"), "España");
  assert.equal(countryName("DE"), "Alemania");
});
