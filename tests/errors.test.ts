import { test } from "node:test";
import assert from "node:assert/strict";
import { friendlyError } from "../src/lib/errors.ts";

test("explains network failures and keeps the original as details", () => {
  const e = friendlyError("error sending request for url (https://api.deezer.com/search/artist?q=x): dns error");
  assert.match(e.message, /conexión/);
  assert.match(e.details ?? "", /api\.deezer\.com/);
  assert.equal(e.retryable, true);
});

test("recognises what YouTube and yt-dlp say", () => {
  assert.match(friendlyError("yt-dlp: ERROR: [youtube] abc: Sign in to confirm you're not a bot").message, /robot/);
  assert.match(friendlyError("yt-dlp: ERROR: [youtube] abc: Video unavailable").message, /ya no está disponible/);
  assert.equal(friendlyError("yt-dlp: ERROR: Video unavailable").retryable, false);
  assert.match(friendlyError("el servidor de audio respondió 403 Forbidden").message, /no deja reproducir/);
  assert.match(friendlyError("HTTP Error 429: Too Many Requests").message, /demasiadas peticiones/);
  assert.match(friendlyError("Deezer: Quota limit exceeded").message, /demasiadas peticiones/);
  assert.equal(friendlyError("no se encontró yt-dlp. Instálalo con winget").retryable, false);
});

test("leaves Melomango's own messages readable", () => {
  const e = friendlyError("Error: no encontré «Dai Dai» en ninguna fuente");
  assert.equal(e.message, "No encontré «Dai Dai» en ninguna fuente.");
  assert.equal(e.details, null);
  assert.equal(friendlyError("").message, "Algo ha fallado.");
});

test("a directory failure says so, whatever the cause", () => {
  const e = friendlyError(
    "No se pudo cargar la lista de emisoras. Comprueba tu conexión y vuelve a intentarlo. (error sending request)",
  );
  assert.equal(e.message, "No se pudo cargar la lista de emisoras. Comprueba tu conexión y vuelve a intentarlo.");
  assert.ok(e.details?.includes("error sending request"));
});
