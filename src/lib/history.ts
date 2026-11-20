import type { Play, Track } from "./api";

export interface HistoryDay {
  /** "Hoy", "Ayer", "lunes, 17 de noviembre"… */
  label: string;
  /** Tracks played that day, latest first, each once. */
  tracks: Track[];
  /** How many plays that day (counting repeats). */
  plays: number;
}

const startOfDay = (ms: number) => {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
};

/** Groups plays (newest first) by local calendar day. */
export function groupByDay(plays: Play[], now: number = Date.now()): HistoryDay[] {
  const today = startOfDay(now);
  const yesterday = startOfDay(today - 1);
  const days: (HistoryDay & { start: number; seen: Set<string> })[] = [];
  for (const { track, playedAt } of plays) {
    const start = startOfDay(playedAt);
    let day = days[days.length - 1];
    if (!day || day.start !== start) {
      day = { label: dayLabel(start, today, yesterday), tracks: [], plays: 0, start, seen: new Set() };
      days.push(day);
    }
    day.plays += 1;
    const key = `${track.source}/${track.id}`;
    if (!day.seen.has(key)) {
      day.seen.add(key);
      day.tracks.push(track);
    }
  }
  return days.map(({ label, tracks, plays }) => ({ label, tracks, plays }));
}

function dayLabel(start: number, today: number, yesterday: number) {
  if (start === today) return "Hoy";
  if (start === yesterday) return "Ayer";
  const date = new Date(start);
  const sameYear = date.getFullYear() === new Date(today).getFullYear();
  const label = date.toLocaleDateString("es-ES", {
    weekday: "long",
    day: "numeric",
    month: "long",
    ...(sameYear ? {} : { year: "numeric" }),
  });
  return label.charAt(0).toUpperCase() + label.slice(1);
}
