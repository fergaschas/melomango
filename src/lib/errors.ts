/**
 * Turns the technical errors that reach the page (Rust, yt-dlp, HTTP) into a sentence
 * that says what happened and what to do. The original text stays available as details.
 */

export interface FriendlyError {
  /** What to show. */
  message: string;
  /** The original text, when it differs from the message (for "Detalles"). */
  details: string | null;
  /** Whether trying again may help. */
  retryable: boolean;
}

interface Rule {
  test: RegExp;
  message: string;
  retryable?: boolean;
}

const RULES: Rule[] = [
  {
    test: /no se encontró yt-dlp|yt-dlp.*(not found|no encontrado)/i,
    message: "Falta yt-dlp, el programa que usa el plugin de YouTube. Instálalo con el botón de abajo o desde Plugins.",
    retryable: false,
  },
  {
    test: /sign in to confirm|not a bot|confirm you.?re not/i,
    message: "YouTube pide confirmar que no eres un robot. Espera unos minutos y vuelve a intentarlo.",
  },
  {
    test: /429|too many requests|quota limit|rate.?limit/i,
    message: "El servicio está recibiendo demasiadas peticiones. Espera un momento y vuelve a intentarlo.",
  },
  {
    test: /video unavailable|private video|has been removed|no longer available|vídeo no válido/i,
    message: "Este vídeo ya no está disponible en YouTube. Elige otra versión con ⇄ o pasa a la siguiente.",
    retryable: false,
  },
  {
    test: /servidor de audio respondió 403|403 forbidden/i,
    message: "YouTube no deja reproducir este vídeo ahora mismo. Vuelve a intentarlo o elige otra versión con ⇄.",
  },
  {
    test: /error sending request|dns error|failed to lookup|no such host|connection (refused|reset|closed)|os error 1100[14]|network|failed to fetch|no se pudo conectar/i,
    message: "No hay conexión con el servicio. Comprueba tu conexión a internet y vuelve a intentarlo.",
  },
  {
    test: /timed? ?out|tardó demasiado|deadline/i,
    message: "El servicio tardó demasiado en responder. Vuelve a intentarlo.",
  },
];

export function friendlyError(raw: unknown): FriendlyError {
  const text = String(raw ?? "")
    .replace(/^(Error|error):\s*/, "")
    .trim();
  const rule = RULES.find((r) => r.test.test(text));
  if (rule) return { message: rule.message, details: text || null, retryable: rule.retryable ?? true };
  // Already a sentence for people (most of Melomango's own messages): just tidy it.
  const message = text ? text.charAt(0).toUpperCase() + text.slice(1) : "Algo ha fallado.";
  return { message: /[.!?…»)]$/.test(message) ? message : `${message}.`, details: null, retryable: true };
}
