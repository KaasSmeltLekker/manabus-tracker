export type Direction = "a" | "b";

export interface UserLocation {
  lat: number;
  lng: number;
  accuracy: number;
}

export interface MenuLink {
  enabled: boolean;
  label: string;
  url: string;
}

export interface CityPopup {
  enabled: boolean;
  title: string;
  text: string;
  image_url: string;
}

export interface Bus {
  id: string;
  pref: string;
  name: string | null;
  plate: string | null;
  lat: number;
  lng: number;
  heading: number | null;
  speed: number | null;
  corridor: string | null;
  toward: string | null;
  next_halte: string | null;
  prev_halte: string | null;
  passengers: number | null;
  fix_age_s: number;
  stale: boolean;
  stopped_s?: number;
  receivedAt: number;
}

export interface Corridor {
  id: string;
  kor?: string;
  kode?: string;
  corridor?: string;
  route_code?: string;
  source_kor?: string;
  sort_order?: number;
  route_name: string;
  via?: string | null;
  origin?: string;
  toward?: string;
  jam_operasional?: string | null;
  color: string;
  pref: string;
  custom?: boolean;
  points_a?: string | null;
  points_b?: string | null;
}

export interface CorridorLine {
  code: string;
  name: string;
  via?: string | null;
  color: string;
  segments: [number, number][][];
}

export interface BusStop {
  sh_id: string;
  sh_name: string;
  lat: number;
  lng: number;
  kor: string;
  corridors: string[];
  corridorColors: string[];
  color: string;
  toward: string;
}

export interface RawStop {
  sh_id?: string | number;
  sh_name?: string;
  sh_lat?: string | number;
  sh_lng?: string | number;
  kor?: string;
  in_koridor?: string;
  color_koridor?: string;
  color?: string;
  toward?: string;
}

interface Operator {
  pref: string;
  city: "surabaya" | "yogyakarta";
  shortCode: string;
  label: string;
  name: string;
  center: [number, number];
}

export interface StopRouteBadge {
  code: string;
  color: string;
  trackable: boolean;
}

export interface StopArrival {
  busId: string;
  busName: string;
  plate: string | null;
  corridor: string;
  corridorColor: string;
  etaSeconds: number;
  etaMinutes: number;
  isArrivingNow: boolean;
  distanceMeters: number;
  segmentText: string;
  destination: string;
  fixAtMs: number;
  speedKmh: number | null;
}

export const OPERATORS: Operator[] = [
  {
    pref: "3501",
    city: "surabaya",
    shortCode: "SB",
    label: "Kota Surabaya",
    name: "Suroboyo Bus",
    center: [-7.2575, 112.7521],
  },
  {
    pref: "1",
    city: "yogyakarta",
    shortCode: "YK",
    label: "Yogyakarta",
    name: "Trans Jogja Istimewa",
    center: [-7.785927, 110.391675],
  },
];

const ROUTE_NAMES: Record<string, string> = {
  R1: "Purabaya-Tanjung Perak",
  R2: "Kejawan-UNESA",
  R4: "Purabaya-UNAIR C",
  R5: "Benowo-Tunjungan",
  FD2: "PnR Mayjend-Balai Kota",
  FD3: "T.I.J.-Gunung Anyar",
  FD4: "SIER-Kota Lama",
  FD5: "PnR Mayjend-RS BDH",
  FD6: "T.I.J.-Lakarsantri",
  FD7: "Bratang-St. Pasar Turi",
  FD8: "T.O.W.-UNESA",
  FD9: "Menanggal-Manukan",
  FD10: "Keputih-Pasar Atom",
  FD11: "Bratang-Shelter Bulak",
  FD12: "Purabaya-ITS-Kenpark",
};

const YK_ROUTE_NAMES: Record<string, string> = {
  "1A": "Prambanan-Malioboro",
  "1B": "Adisucipto-Ngabean",
  "2A": "Condongcatur-XT Square-Condongcatur",
  "2B": "Condongcatur-XT Square-Condongcatur",
  "3A": "Giwangan-Condongcatur-Giwangan",
  "3B": "Giwangan-Condongcatur-Giwangan",
  "4A": "Giwangan-Condongcatur-Giwangan",
  "4B": "Giwangan-Condongcatur-Giwangan",
  "5A": "Jombor-Janti",
  "5B": "Jombor-Adisucipto",
  "6": "Gamping-Malioboro",
  "8": "Jombor-Malioboro",
  "9": "Giwangan-Jombor",
  "10": "Gamping-Kusumanegara",
  "11": "Giwangan-Cik Di Tiro",
  "12": "Condongcatur-Pakem",
  "13": "Godean-Malioboro",
  "14": "Adisucipto-Pakem",
  "15": "Palbapang-Malioboro",
  L1: "Jombor-Malioboro",
  "L-1": "Jombor-Malioboro",
};

const YK_ROUTE_VIA: Record<string, string> = {
  "2A": "via Malioboro",
  "2B": "via Ngabean",
  "3A": "via Adisucipto-Malioboro",
  "3B": "via Lowanu-UGM",
  "4A": "via Lempuyangan-Kusumanegara",
  "4B": "via Kusumanegara-Lempuyangan",
  "8": "via Demak Ijo",
  L1: "via Jl. Magelang",
  "L-1": "via Jl. Magelang",
};

const IGNORED_STOP_WORDS = new Set(["halte", "terminal", "shelter", "rsud", "rs"]);

const JTM1 = { code: "JTM1", color: "#36780a" };
const JTM2 = { code: "JTM2", color: "#0389d2" };
const JTM5 = { code: "JTM5", color: "#340c7e" };

const EXTRA_BY_ID: Record<string, { code: string; color: string }[]> = {
  "0042372": [JTM2],
  "0042278": [JTM2],
  "0038274": [JTM1, JTM5],
  "0043849": [JTM5],
  "0043855": [JTM5],
  "0043854": [JTM5],
};

const EXTRA_BY_NAME: Record<string, { code: string; color: string }[]> = {
  "halte siwalankerto 2": [JTM2],
  "halte dukuh menanggal": [JTM2],
  "terminal purabaya": [JTM1, JTM5],
  "tanjung torawitan a": [JTM5],
  "tanjung torawitan b": [JTM5],
  "barunawati b": [JTM5],
};

export function compactCode(code: string): string {
  return code.replace(/\s+/g, "");
}

export function sameCorridorCode(a: string, b: string): boolean {
  return a === b || compactCode(a) === compactCode(b);
}

export function findCorridorByCode(corridors: Corridor[], target: string): Corridor | undefined {
  const targetCompact = compactCode(target);
  return corridors.find((c) => {
    const code = corridorCode(c);
    return code === target || compactCode(code) === targetCompact;
  });
}

export function lookupCorridorValue<T>(
  map: Record<string, T>,
  code: string,
): T | undefined {
  return map[code] ?? map[compactCode(code)];
}

export function corridorCode(c: Corridor): string {
  return (c.kor ?? c.kode ?? c.corridor ?? c.route_code ?? c.id).trim().toUpperCase();
}

export function corridorDisplayName(c: Corridor): string {
  if (c.custom) return c.route_name;
  const pref = String(c.pref ?? "");
  const code = corridorCode(c);
  if (pref === "3501") {
    if (code in ROUTE_NAMES) return ROUTE_NAMES[code];
    if (code === "FD13") return c.route_name.replace(/\bMohamad\b/g, "Moh.");
  } else if (pref === "1") {
    if (code in YK_ROUTE_NAMES) return YK_ROUTE_NAMES[code];
  }
  return c.route_name;
}

export function corridorDisplayVia(c: Corridor): string | null {
  if (typeof c.via === "string") {
    return c.via.trim() || null;
  }
  if (String(c.pref ?? "") !== "1") return null;
  return YK_ROUTE_VIA[corridorCode(c)] ?? null;
}

export function formatStopLabel(text: string): string {
  return text
    .replace(/\bngabean\b/g, "Ngabean")
    .replace(/\bJl\.?\s+Mataram\s+Hotel\s+Akur\b/gi, "Jl. Mataram")
    .replace(/\bMako\s+Lanal\s+Yogyakarta\b/gi, "TPB TNI AL (Utara)")
    .replace(/\bTPB\s+TNI\s+AL\s+1\b/gi, "TPB TNI AL (Selatan)");
}

export function corridorBadgeLabel(code: string): string {
  const norm = code.trim().toUpperCase();
  const match = norm.match(/^FD(\d+)$/);
  return match ? match[1] : norm;
}

export function corridorHasTwoDirections(c: Corridor | null | undefined): boolean {
  return Boolean(c?.points_b?.trim());
}

export function trackingCorridorKey(code: string, pref = "3501"): string {
  return pref === "3501" && code === "SBT" ? "R1" : code;
}

export function resolveCorridorAlias(corridors: Corridor[], rawKey: string): string {
  const norm = rawKey.trim().toUpperCase();
  if (!norm) return "";
  for (const c of corridors) {
    if (c.source_kor && sameCorridorCode(c.source_kor.trim().toUpperCase(), norm)) {
      return corridorCode(c);
    }
  }
  return norm;
}

function parseCorridorRank(code: string): [number, number, string] {
  const rMatch = code.match(/^R(\d+)$/);
  if (rMatch) return [0, Number(rMatch[1]), code];
  const fdMatch = code.match(/^FD(\d+)$/);
  if (fdMatch) return [1, Number(fdMatch[1]), code];
  const numMatch = code.match(/^(\d+)([A-Z]*)$/);
  if (numMatch) return [2, Number(numMatch[1]), numMatch[2] || ""];
  const dashMatch = code.match(/^([A-Z]+)-?(\d+)$/);
  if (dashMatch && dashMatch[1] !== "JTM") return [3, Number(dashMatch[2]), dashMatch[1]];
  const jtmMatch = code.match(/^JTM(\d+)$/);
  if (jtmMatch) return [4, Number(jtmMatch[1]), code];
  return [5, 0, code];
}

export function extraStopRoutes(
  stop: Pick<BusStop, "sh_id" | "sh_name">,
  pref = "3501",
): { code: string; color: string }[] {
  if (pref !== "3501") return [];
  return (
    EXTRA_BY_ID[stop.sh_id] ??
    EXTRA_BY_NAME[stop.sh_name.trim().toLowerCase()] ??
    []
  );
}

export function compareCorridorCodes(a: string, b: string): number {
  const [ga, na, sa] = parseCorridorRank(a);
  const [gb, nb, sb] = parseCorridorRank(b);
  if (ga !== gb) return ga - gb;
  if (na !== nb) return na - nb;
  return sa.localeCompare(sb);
}

export function formatIdleAge(fixAgeS: number): string {
  const mins = Math.max(1, Math.round(fixAgeS / 60));
  if (mins < 60) return `${mins} menit lalu`;
  const hrs = Math.floor(mins / 60);
  const rem = mins % 60;
  return rem === 0 ? `${hrs} jam lalu` : `${hrs} jam ${rem} menit lalu`;
}

export function formatOperatingHours(raw?: string | null): string | null {
  const s = raw?.trim();
  if (!s) return null;
  return s.replace(/\s*-\s*/g, "-");
}

export function normalizeColor(raw?: string): string | null {
  const s = raw?.replace(/["'<>]/g, "").trim();
  if (!s) return null;
  return /^[0-9a-f]{3,8}$/i.test(s) ? `#${s}` : s;
}

export function contrastText(color: string): string {
  const hex = color.replace(/^#/, "");
  const full =
    hex.length === 3
      ? hex
          .split("")
          .map((c) => c + c)
          .join("")
      : hex;
  if (!/^[0-9a-f]{6}$/i.test(full)) return "#ffffff";
  const r = parseInt(full.slice(0, 2), 16) / 255;
  const g = parseInt(full.slice(2, 4), 16) / 255;
  const b = parseInt(full.slice(4, 6), 16) / 255;
  const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return lum > 0.58 ? "#0f172a" : "#ffffff";
}

function stopKeywords(text: string): string[] {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9\s]/g, " ")
    .split(/\s+/)
    .filter((w) => w.length >= 3 && !IGNORED_STOP_WORDS.has(w));
}

export function wordOverlap(a: string, b: string): number {
  const wordsA = stopKeywords(a);
  const wordsB = new Set(stopKeywords(b));
  let count = 0;
  for (const w of wordsA) {
    if (wordsB.has(w)) count++;
  }
  return count;
}

export function busMatchesDirection(
  bus: Bus,
  corridor: Corridor | undefined,
  dir: Direction,
): boolean {
  if (!corridor) return true;
  if (!corridorHasTwoDirections(corridor)) return true;
  const origin = (corridor.origin ?? "").trim().toLowerCase();
  const toward = (corridor.toward ?? "").trim().toLowerCase();
  if (origin && toward && origin === toward) return true;
  const busToward = (bus.toward ?? "").trim().toLowerCase();
  if (!origin || !toward || !busToward) return true;

  if (busToward === toward) return dir === "a";
  if (busToward === origin) return dir === "b";

  const scoreA = wordOverlap(busToward, toward);
  const scoreB = wordOverlap(busToward, origin);
  if (scoreA !== scoreB) {
    return dir === (scoreA > scoreB ? "a" : "b");
  }
  return dir === "a";
}
