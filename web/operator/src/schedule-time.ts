type Parts = { year: number; month: number; day: number; hour: number; minute: number; second: number; zone: string };

const FMT = new Map<string, Intl.DateTimeFormat>();

function fmt(zone: string): Intl.DateTimeFormat {
  let f = FMT.get(zone);
  if (!f) {
    f = new Intl.DateTimeFormat("en-US", {
      timeZone: zone,
      calendar: "iso8601",
      numberingSystem: "latn",
      hourCycle: "h23",
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      timeZoneName: "short",
    });
    FMT.set(zone, f);
  }
  return f;
}

function partsAt(ms: number, zone: string): Parts {
  const out: Record<string, string> = {};
  for (const p of fmt(zone).formatToParts(new Date(ms))) {
    if (p.type !== "literal") out[p.type] = p.value;
  }
  return {
    year: +out.year,
    month: +out.month,
    day: +out.day,
    hour: +out.hour,
    minute: +out.minute,
    second: +out.second,
    zone: out.timeZoneName ?? "",
  };
}

function naiveMs(p: Parts): number {
  const d = new Date(0);
  d.setUTCFullYear(p.year, p.month - 1, p.day);
  d.setUTCHours(p.hour, p.minute, p.second, 0);
  return d.getTime();
}

function localWallMs(ms: number, zone: string): number {
  return naiveMs(partsAt(ms, zone));
}

function offsetAt(ms: number, zone: string): number {
  return localWallMs(ms, zone) - Math.floor(ms / 1000) * 1000;
}

export function stationScheduleZoneIsValid(zone: string): boolean {
  if (typeof zone !== "string" || zone.trim() === "") return false;
  if (/^[+-]\d{2}(?::?\d{2})?$/.test(zone.trim())) return false;
  try {
    fmt(zone).format(0);
    return true;
  } catch {
    return false;
  }
}

export function stationScheduleInput(unixMs: number, zone: string): string {
  if (!stationScheduleZoneIsValid(zone) || !Number.isFinite(unixMs)) return "";
  try {
    const p = partsAt(unixMs, zone);
    const pad = (n: number, w = 2) => String(n).padStart(w, "0");
    return `${pad(p.year, 4)}-${pad(p.month)}-${pad(p.day)}T${pad(p.hour)}:${pad(p.minute)}`;
  } catch {
    return "";
  }
}

function parseWall(value: string): number {
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,3}))?)?$/.exec(value);
  if (!m) throw new Error(`Invalid datetime "${value}": expected yyyy-mm-ddTHH:mm with optional :ss or .fff.`);
  const y = +m[1]!, mo = +m[2]!, d = +m[3]!, h = +m[4]!, mi = +m[5]!;
  const s = m[6] ? +m[6] : 0;
  const ms = m[7] ? +(m[7] + "00").slice(0, 3) : 0;
  const dt = new Date(0);
  dt.setUTCFullYear(y, mo - 1, d);
  dt.setUTCHours(h, mi, s, ms);
  if (
    y < 1 ||
    dt.getUTCFullYear() !== y ||
    dt.getUTCMonth() !== mo - 1 ||
    dt.getUTCDate() !== d ||
    dt.getUTCHours() !== h ||
    dt.getUTCMinutes() !== mi ||
    dt.getUTCSeconds() !== s
  ) {
    throw new Error(`Invalid calendar datetime "${value}": a field is out of range.`);
  }
  return dt.getTime();
}

export function stationScheduleInstant(value: string, zone: string): number {
  if (!stationScheduleZoneIsValid(zone)) {
    throw new Error(`Invalid IANA time zone "${zone}": use a zone such as "America/New_York".`);
  }
  const wall = parseWall(value);
  const base = Math.floor(wall / 1000) * 1000;
  const target = base;
  const offsets = new Set<number>();
  // Sample ±36h to derive station UTC offsets on both sides of transitions.
  for (let h = -36; h <= 36; h += 6) {
    offsets.add(offsetAt(base + h * 3_600_000, zone));
  }
  const hits = new Set<number>();
  for (const o of offsets) {
    const t = wall - o;
    try {
      if (localWallMs(t, zone) === target) hits.add(t);
    } catch {
      /* out of representable range */
    }
  }
  if (hits.size === 0) {
    throw new Error(
      `The local time ${value} does not exist in "${zone}" (DST gap: clocks jump forward). Choose another time.`,
    );
  }
  if (hits.size > 1) {
    throw new Error(
      `The local time ${value} is ambiguous in "${zone}" (DST fold: clocks repeat). Choose another time.`,
    );
  }
  return [...hits][0]!;
}

export function defaultStationScheduleInput(nowMs: number, zone: string): string {
  const nextMinute = (Math.floor(nowMs / 60_000) + 1) * 60_000;
  const candidates = [nextMinute, nextMinute + 3_600_000];

  for (const candidate of candidates) {
    const value = stationScheduleInput(candidate, zone);
    if (!value) return "";

    try {
      stationScheduleInstant(value, zone);
      return value;
    } catch {
      // retrying an ambiguous default
    }
  }

  return "";
}

export function formatStationScheduleTime(unixMs: number, zone: string): string {
  if (!Number.isFinite(unixMs) || Math.abs(unixMs) > 8640000000000000) return "";
  let usedUtc = !stationScheduleZoneIsValid(zone);
  let p: Parts;
  try {
    p = partsAt(unixMs, usedUtc ? "UTC" : zone);
  } catch {
    usedUtc = true;
    p = partsAt(unixMs, "UTC");
  }
  const pad = (n: number, w = 2) => String(n).padStart(w, "0");
  const label = (p.zone || "UTC") + (usedUtc ? " (UTC fallback)" : "");
  return `${pad(p.year, 4)}-${pad(p.month)}-${pad(p.day)} ${pad(p.hour)}:${pad(p.minute)} ${label}`;
}
