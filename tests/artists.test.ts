import { test } from "node:test";
import assert from "node:assert/strict";
import { splitArtists } from "../src/lib/artists.ts";

test("splits an artist line into each artist", () => {
  assert.deepEqual(splitArtists("Shakira, Burna Boy"), ["Shakira", "Burna Boy"]);
  assert.deepEqual(splitArtists("Bad Bunny & Drake"), ["Bad Bunny", "Drake"]);
  assert.deepEqual(splitArtists("Daft Punk feat. Pharrell Williams"), ["Daft Punk", "Pharrell Williams"]);
  assert.deepEqual(splitArtists("ROSALÍA"), ["ROSALÍA"]);
  assert.deepEqual(splitArtists("  "), []);
});
