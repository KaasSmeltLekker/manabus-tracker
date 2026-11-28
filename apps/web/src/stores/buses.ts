import { markRaw } from "vue";
import { acceptHMRUpdate, defineStore } from "pinia";
import { io, type Socket } from "socket.io-client";
import { SOCKET_BASE, apiUrl } from "../config";
import { approxMeters, projectStopToLine } from "./polylineUtils";
import {
  type Bus,
  type BusStop,
  type CityPopup,
  type Corridor,
  type CorridorLine,
  type Direction,
  type MenuLink,
  type RawStop,
  type UserLocation,
  OPERATORS,
  compactCode,
  compareCorridorCodes,
  corridorCode,
  corridorDisplayName,
  corridorDisplayVia,
  corridorHasTwoDirections,
  findCorridorByCode,
  formatStopLabel,
  lookupCorridorValue,
  normalizeColor,
  resolveCorridorAlias,
  sameCorridorCode,
  trackingCorridorKey,
  wordOverlap,
} from "./transitHelpers";
import {
  corridorLegPoints,
  computeStopArrivals,
  stopServingCorridors,
} from "./arrivals";

export type { Bus, BusStop, CityPopup, Corridor, CorridorLine, MenuLink };
export {
  compareCorridorCodes,
  contrastText,
  corridorBadgeLabel,
  corridorCode,
  corridorDisplayName,
  corridorDisplayVia,
  corridorHasTwoDirections,
  extraStopRoutes,
  formatIdleAge,
  formatOperatingHours,
  formatStopLabel,
  lookupCorridorValue,
  normalizeColor,
  resolveCorridorAlias,
  sameCorridorCode,
  trackingCorridorKey,
} from "./transitHelpers";
export { computeStopArrivals, stopServingCorridors } from "./arrivals";

const HALTE_PASAR_ATOM_2_FD10: BusStop = {
  sh_id: "0040338",
  sh_name: "Halte Pasar Atom 2",
  lat: -7.24275,
  lng: 112.74485,
  kor: "FD10",
  corridors: ["FD10", "FD4"],
  corridorColors: ["#c5cf3a", "#3500f5"],
  color: "#c5cf3a",
  toward: "Terminal Keputih",
};

const SIMO_KATRUNGAN_A: BusStop = {
  sh_id: "0042420-a",
  sh_name: "Halte Simo Katrungan Kidul A",
  lat: -7.26896,
  lng: 112.71546,
  kor: "R5",
  corridors: ["R5"],
  corridorColors: ["#f347ff"],
  color: "#f347ff",
  toward: "Halte Embong Malang",
};

const TERMINAL_JOYOBOYO_FD6: BusStop = {
  sh_id: "0040100",
  sh_name: "Terminal Joyoboyo",
  lat: -7.298991,
  lng: 112.736469,
  kor: "FD6",
  corridors: ["FD3", "FD4", "FD6", "FD9", "R1"],
  corridorColors: ["#7cb9c9", "#3500f5", "#da622e", "#ca2fc5", "#f72626"],
  color: "#da622e",
  toward: "Terminal Joyoboyo",
};

function moveStopAfter(stops: BusStop[], moveId: string, anchorId: string): void {
  const moveIdx = stops.findIndex((s) => s.sh_id === moveId);
  const anchorIdx = stops.findIndex((s) => s.sh_id === anchorId);
  if (moveIdx < 0 || anchorIdx < 0 || moveIdx >= anchorIdx) return;
  const [item] = stops.splice(moveIdx, 1);
  stops.splice(anchorIdx, 0, item);
}

function normalizeLegStops(
  code: string,
  dir: Direction,
  stops: BusStop[],
  pref = "3501",
): BusStop[] {
  if (pref !== "3501") return stops;
  if (code === "FD4" && dir === "b") {
    return stops.filter((s) => s.sh_id !== "0040334");
  }
  if (code === "FD10" && dir === "a") {
    const out = [...stops];
    moveStopAfter(out, "0040338", "0040334");
    return out;
  }
  if (code === "FD10" && dir === "b") {
    const seen = new Set<string>();
    const out: BusStop[] = [];
    for (const s of stops) {
      const next = s.sh_id === "0040334" ? HALTE_PASAR_ATOM_2_FD10 : s;
      if (seen.has(next.sh_id)) continue;
      seen.add(next.sh_id);
      out.push(next);
    }
    return out;
  }
  if (code === "FD6" && dir === "b") {
    const last = stops[stops.length - 1];
    if (last && last.sh_id !== "0040100") {
      const existing = stops.find((s) => s.sh_id === "0040100") ?? TERMINAL_JOYOBOYO_FD6;
      return [...stops, existing];
    }
    return stops;
  }
  if (code === "FD12" && dir === "a") {
    return stops.filter((s) => s.sh_id !== "0037351");
  }
  if (code === "R5" && dir === "a") {
    const seen = new Set<string>();
    const out: BusStop[] = [];
    for (const s of stops) {
      const next = s.sh_id === "0042420" ? SIMO_KATRUNGAN_A : s;
      if (seen.has(next.sh_id)) continue;
      seen.add(next.sh_id);
      out.push(next);
    }
    return out;
  }
  if (code === "FD9" && dir === "b") {
    const out = [...stops];
    moveStopAfter(out, "0043816", "0037944");
    return out;
  }
  if (code !== "FD7" || dir !== "b") return stops;
  const out = [...stops];
  moveStopAfter(out, "0043744", "0043421");
  moveStopAfter(out, "0040353", "0043744");
  return out;
}

function fallbackLegStops(
  stops: BusStop[],
  corridor: Corridor | undefined,
  dir: Direction,
): BusStop[] {
  if (!corridor) return stops;
  const lineA = corridorLegPoints(corridor, "a");
  const lineB = corridorLegPoints(corridor, "b");
  const activeLine = dir === "a" ? lineA : lineB;
  const otherLine = dir === "a" ? lineB : lineA;
  if (activeLine.length < 2) return stops;

  const origin = (corridor.origin ?? "").trim().toLowerCase();
  const toward = (corridor.toward ?? "").trim().toLowerCase();
  const targetToward = dir === "a" ? toward : origin;
  const startPt = activeLine[0];
  const endPt = activeLine[activeLine.length - 1];

  const matched: { stop: BusStop; pos: number }[] = [];
  for (const stop of stops) {
    const { dist: d1, pos } = projectStopToLine(stop.lat, stop.lng, activeLine);
    const d2 =
      otherLine.length >= 2
        ? projectStopToLine(stop.lat, stop.lng, otherLine).dist
        : Infinity;
    const nearTerminal =
      approxMeters([stop.lat, stop.lng], startPt) <= 70 ||
      approxMeters([stop.lat, stop.lng], endPt) <= 70;
    const stopTw = stop.toward.toLowerCase();
    const towardMatch =
      targetToward.length > 0 &&
      (stopTw === targetToward || wordOverlap(stopTw, targetToward) > 0);
    const tieBreak = Math.abs(d1 - d2) <= 15 && towardMatch;

    if (d1 <= 65 && (d1 <= d2 + 2 || nearTerminal || tieBreak)) {
      matched.push({ stop, pos });
    }
  }
  if (matched.length === 0) return stops;
  matched.sort((a, b) => a.pos - b.pos);
  return matched.map((m) => m.stop);
}

const LINKED_STOP_ROUTES: Record<string, { code: string; color: string }[]> = {
  "0043723": [{ code: "FD7", color: "#4e89b7" }],
  "0043711": [{ code: "R1", color: "#f72626" }],
};

const STOP_COORD_OVERRIDES: Record<string, [number, number]> = {
  "0040338": [-7.24275, 112.74485],
  "0041330": [-7.34335, 112.78639],
  "0043393": [-7.24098, 112.79780],
};

function normalizeStopRow(
  r: RawStop,
  pref = "3501",
  fallbackKor?: string,
  fallbackColor?: string,
): BusStop | null {
  const lat = Number(r.sh_lat);
  const lng = Number(r.sh_lng);
  if (!Number.isFinite(lat) || !Number.isFinite(lng) || (lat === 0 && lng === 0)) {
    return null;
  }
  const isSurabaya = pref === "3501";
  const rawCodes = String(r.in_koridor || r.kor || fallbackKor || "")
    .split("|")
    .map((x) => x.trim().toUpperCase());
  const rawColors = String(r.color_koridor || r.color || fallbackColor || "")
    .split("|")
    .map((x) => normalizeColor(x) ?? "#64748b");
  const corridors: string[] = [];
  const corridorColors: string[] = [];
  for (let i = 0; i < rawCodes.length; i++) {
    const code = rawCodes[i];
    if (!code || (isSurabaya && code === "SBT")) continue;
    corridors.push(code);
    corridorColors.push(
      rawColors[i] ?? normalizeColor(r.color) ?? fallbackColor ?? "#64748b",
    );
  }
  if (corridors.length === 0) {
    if (!fallbackKor || (isSurabaya && fallbackKor === "SBT")) return null;
    corridors.push(fallbackKor);
    corridorColors.push(fallbackColor ?? "#64748b");
  }

  const rawKor = String(r.kor ?? fallbackKor ?? "").trim().toUpperCase();
  const r1Idx = isSurabaya ? corridors.indexOf("R1") : -1;
  const kor =
    !rawKor || (isSurabaya && rawKor === "SBT")
      ? r1Idx >= 0
        ? "R1"
        : corridors[0]
      : rawKor;
  const korIdx = corridors.indexOf(kor);
  const color =
    (korIdx >= 0 ? corridorColors[korIdx] : undefined) ??
    (isSurabaya && rawKor === "SBT" ? corridorColors[0] : normalizeColor(r.color)) ??
    corridorColors[0] ??
    fallbackColor ??
    "#64748b";

  const shId = String(r.sh_id ?? `${lat},${lng}`);
  const coordOverride = isSurabaya ? STOP_COORD_OVERRIDES[shId] : undefined;
  for (const extra of isSurabaya ? (LINKED_STOP_ROUTES[shId] ?? []) : []) {
    if (!corridors.includes(extra.code)) {
      corridors.push(extra.code);
      corridorColors.push(extra.color);
    }
  }

  return markRaw({
    sh_id: shId,
    sh_name: formatStopLabel(String(r.sh_name ?? "Halte").trim()),
    lat: coordOverride ? coordOverride[0] : lat,
    lng: coordOverride ? coordOverride[1] : lng,
    kor,
    corridors,
    corridorColors,
    color,
    toward: formatStopLabel(String(r.toward ?? "").trim()),
  });
}

function buildNormalizedStops(rows: RawStop[], pref: string): BusStop[] {
  const stops: BusStop[] = [];
  for (const r of rows) {
    const stop = normalizeStopRow(r, pref);
    if (stop) stops.push(stop);
  }
  let filtered = stops;
  if (pref === "3501") {
    const hasR1Blauran = stops.some((s) => s.sh_id === "0043723");
    filtered = hasR1Blauran ? stops.filter((s) => s.sh_id !== "0043711") : stops;
    if (
      filtered.some((s) => s.sh_id === "0042420") &&
      !filtered.some((s) => s.sh_id === SIMO_KATRUNGAN_A.sh_id)
    ) {
      filtered.push(markRaw(SIMO_KATRUNGAN_A));
    }
  }
  return filtered;
}

const STALE_AFTER_S = 5 * 60;
const PARKED_AFTER_S = 10 * 60;
const DROP_AFTER_S = 60 * 60;

let pruneTimer: ReturnType<typeof setInterval> | null = null;
let geoWatchId: number | null = null;
let syncChannel: BroadcastChannel | null = null;
let visibilityHandler: (() => void) | null = null;
let onlineHandler: (() => void) | null = null;
let shouldPanToUser = false;
const fetchingLegs = new Set<string>();

function geolocationErrorMessage(code: number): string {
  if (code === 1) return "Izin lokasi ditolak";
  if (code === 2) return "Lokasi tidak tersedia";
  if (code === 3) return "Waktu pencarian lokasi habis";
  return "Gagal mendapatkan lokasi";
}

interface CitySnapshot {
  corridors: Corridor[];
  stops: BusStop[];
  legStops: Record<string, BusStop[]>;
}

interface StoredCityPayload {
  savedAt: number;
  corridors: Corridor[];
  stops: BusStop[];
}

const CITY_STORAGE_PREFIX = "ndibise-city-v1:";
const CITY_STORAGE_TTL_MS = 7 * 24 * 60 * 60 * 1000;
const ADMIN_SYNC_CHANNEL = "ndibise-admin-sync";
const cityCache = new Map<string, CitySnapshot>();
const prefetchingCities = new Set<string>();

function readStoredCity(
  pref: string,
): { corridors: Corridor[]; stops: BusStop[] } | null {
  if (typeof localStorage === "undefined") return null;
  try {
    const raw = localStorage.getItem(`${CITY_STORAGE_PREFIX}${pref}`);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<StoredCityPayload>;
    if (typeof parsed?.savedAt === "number" && Date.now() - parsed.savedAt > CITY_STORAGE_TTL_MS) {
      try {
        localStorage.removeItem(`${CITY_STORAGE_PREFIX}${pref}`);
      } catch {
      }
      return null;
    }
    if (
      typeof parsed?.savedAt === "number" &&
      Array.isArray(parsed.corridors) &&
      Array.isArray(parsed.stops) &&
      parsed.corridors.length > 0 &&
      parsed.stops.length > 0
    ) {
      return {
        corridors: parsed.corridors.map((c) => markRaw(c)),
        stops: parsed.stops.map((s) => markRaw(s)),
      };
    }
  } catch {
  }
  return null;
}

function writeStoredCity(pref: string, corridors: Corridor[], stops: BusStop[]): void {
  if (typeof localStorage === "undefined" || corridors.length === 0 || stops.length === 0) {
    return;
  }
  try {
    const payload: StoredCityPayload = {
      savedAt: Date.now(),
      corridors,
      stops,
    };
    localStorage.setItem(`${CITY_STORAGE_PREFIX}${pref}`, JSON.stringify(payload));
  } catch {
  }
}

function buildCodeSets(list: string[]): { codes: Set<string>; compact: Set<string> } {
  return {
    codes: new Set(list),
    compact: new Set(list.map(compactCode)),
  };
}

function inCodeSets(
  code: string,
  sets: { codes: Set<string>; compact: Set<string> },
): boolean {
  return sets.codes.has(code) || sets.compact.has(compactCode(code));
}

function mergeCombinedStops(legs: BusStop[][]): BusStop[] {
  const seenId = new Set<string>();
  const seenNameCoord = new Set<string>();
  const out: BusStop[] = [];
  for (const list of legs) {
    for (const stop of list) {
      if (seenId.has(stop.sh_id)) continue;
      const nameKey = `${stop.sh_name.trim().toLowerCase()}|${stop.lat.toFixed(3)},${stop.lng.toFixed(3)}`;
      if (seenNameCoord.has(nameKey)) continue;
      seenId.add(stop.sh_id);
      seenNameCoord.add(nameKey);
      out.push(stop);
    }
  }
  return out;
}

export const useBusStore = defineStore("buses", {
  state: () => ({
    buses: {} as Record<string, Bus>,
    busesVersion: 0,
    corridors: [] as Corridor[],
    stops: [] as BusStop[],
    legStops: {} as Record<string, BusStop[]>,
    loadingLegs: {} as Record<string, boolean>,
    loadingCorridors: false,
    loadedPref: null as string | null,
    selectedCorridor: null as string | null,
    selectedStopId: null as string | null,
    trackingLocation: false,
    refreshingLocation: false,
    userLocation: null as UserLocation | null,
    locationError: null as string | null,
    locationPanSeq: 0,
    pref: OPERATORS[0].pref,
    menuLinks: [] as MenuLink[],
    cityPopup: null as CityPopup | null,
    siteConfigPref: "",
    socket: null as Socket | null,
    connected: false,
    hasConnectedOnce: false,
    loadError: null as string | null,
    lastUpdateAt: 0,
  }),

  getters: {
    operator: (s) => OPERATORS.find((o) => o.pref === s.pref) ?? OPERATORS[0],
    liveCount: (s) => Object.values(s.buses).filter((b) => !b.stale).length,
    staleCount: (s) => Object.values(s.buses).filter((b) => b.stale).length,
    totalCount: (s) => Object.keys(s.buses).length,
    selectedStop: (s): BusStop | null => {
      const id = s.selectedStopId;
      if (!id) return null;
      const fromAll = s.stops.find((st) => st.sh_id === id);
      if (fromAll) return fromAll;
      for (const list of Object.values(s.legStops)) {
        const hit = list.find((st) => st.sh_id === id);
        if (hit) return hit;
      }
      return null;
    },
    selectedStopCorridorCodes(): string[] {
      const stop = this.selectedStop;
      if (!stop) return [];
      return stopServingCorridors(stop, this.stops, this.corridorColors, this.pref)
        .filter((b) => b.trackable)
        .map((b) => b.code);
    },
    isSelectedStopLoading(): boolean {
      const codes = this.selectedStopCorridorCodes;
      if (codes.length === 0) return false;
      return codes.some(
        (code) => this.loadingLegs[`${code}|a`] || this.loadingLegs[`${code}|b`],
      );
    },
    visibleCorridors: (s): Corridor[] =>
      s.corridors
        .filter((c) => !(s.pref === "3501" && corridorCode(c) === "SBT"))
        .sort((a, b) => {
          const oa = typeof a.sort_order === "number" ? a.sort_order : Number.MAX_SAFE_INTEGER;
          const ob = typeof b.sort_order === "number" ? b.sort_order : Number.MAX_SAFE_INTEGER;
          if (oa !== ob) return oa - ob;
          return compareCorridorCodes(corridorCode(a), corridorCode(b));
        }),
    corridorColors: (s): Record<string, string> => {
      const map: Record<string, string> = {};
      for (const c of s.corridors) {
        const color = normalizeColor(c.color);
        if (!color) continue;
        const normalized = corridorCode(c);
        if (normalized) {
          map[normalized] = color;
          map[compactCode(normalized)] = color;
        }
        const sourceKor = c.source_kor?.trim().toUpperCase();
        if (sourceKor) {
          map[sourceKor] = color;
          map[compactCode(sourceKor)] = color;
        }
        const idKey = c.id?.trim().toUpperCase();
        if (idKey && !(idKey in map)) {
          map[idKey] = color;
          map[compactCode(idKey)] = color;
        }
      }
      return map;
    },
    busCountsByCorridor: (s): Record<string, number> => {
      const counts: Record<string, number> = {};
      for (const b of Object.values(s.buses)) {
        if (!b.corridor) continue;
        const tracked = trackingCorridorKey(b.corridor.trim().toUpperCase(), s.pref);
        const key = resolveCorridorAlias(s.corridors, tracked);
        counts[key] = (counts[key] ?? 0) + 1;
        const compact = compactCode(key);
        if (compact !== key) {
          counts[compact] = (counts[compact] ?? 0) + 1;
        }
      }
      return counts;
    },
    stopCountsByCorridor: (s): Record<string, number> => {
      const counts: Record<string, number> = {};
      for (const stop of s.stops) {
        const seen = new Set<string>();
        for (const raw of [stop.kor, ...stop.corridors]) {
          const key = raw.trim().toUpperCase();
          if (key) seen.add(key);
        }
        for (const key of seen) {
          counts[key] = (counts[key] ?? 0) + 1;
          const compact = compactCode(key);
          if (compact !== key) {
            counts[compact] = (counts[compact] ?? 0) + 1;
          }
        }
      }
      return counts;
    },
    visibleBuses(s): Record<string, Bus> {
      if (!s.selectedCorridor) {
        if (s.selectedStopId && this.selectedStopCorridorCodes.length > 0) {
          const sets = buildCodeSets(this.selectedStopCorridorCodes);
          const filtered: Record<string, Bus> = {};
          for (const [id, b] of Object.entries(s.buses)) {
            const rawCode = b.corridor?.trim().toUpperCase();
            const tracked = rawCode ? trackingCorridorKey(rawCode, s.pref) : undefined;
            const code = tracked ? resolveCorridorAlias(s.corridors, tracked) : undefined;
            if (code && inCodeSets(code, sets)) {
              filtered[id] = b;
            }
          }
          return filtered;
        }
        return s.buses;
      }
      const target = s.selectedCorridor;
      const filtered: Record<string, Bus> = {};
      for (const [id, b] of Object.entries(s.buses)) {
        const rawCode = b.corridor?.trim().toUpperCase();
        const tracked = rawCode ? trackingCorridorKey(rawCode, s.pref) : undefined;
        const code = tracked ? resolveCorridorAlias(s.corridors, tracked) : undefined;
        if (code && sameCorridorCode(code, target)) {
          filtered[id] = b;
        }
      }
      return filtered;
    },
    visibleStops(s): BusStop[] {
      if (!s.selectedCorridor) {
        if (s.selectedStopId && this.selectedStopCorridorCodes.length > 0) {
          const sets = buildCodeSets(this.selectedStopCorridorCodes);
          const matched = s.stops.filter(
            (stop) =>
              stop.sh_id === s.selectedStopId ||
              inCodeSets(stop.kor, sets) ||
              stop.corridors.some((code) => inCodeSets(code, sets)),
          );
          const sel = this.selectedStop;
          if (sel && !matched.some((st) => st.sh_id === sel.sh_id)) {
            matched.push(sel);
          }
          return matched;
        }
        return s.stops;
      }
      const target = s.selectedCorridor;
      const corridor = findCorridorByCode(s.corridors, target);
      const corridorStops = s.stops.filter(
        (stop) =>
          sameCorridorCode(stop.kor, target) ||
          stop.corridors.some((code) => sameCorridorCode(code, target)),
      );
      const cachedA = s.legStops[`${target}|a`];
      const legA =
        cachedA && cachedA.length > 0
          ? cachedA
          : normalizeLegStops(
              target,
              "a",
              fallbackLegStops(corridorStops, corridor, "a"),
              s.pref,
            );
      const twoDir = !corridor || corridorHasTwoDirections(corridor);
      const cachedB = twoDir ? s.legStops[`${target}|b`] : undefined;
      const legB = twoDir
        ? cachedB && cachedB.length > 0
          ? cachedB
          : normalizeLegStops(
              target,
              "b",
              fallbackLegStops(corridorStops, corridor, "b"),
              s.pref,
            )
        : [];
      return mergeCombinedStops([legA, legB, corridorStops]);
    },
    corridorLines: (s): CorridorLine[] => {
      const lines: CorridorLine[] = [];
      for (const c of s.corridors) {
        const code = corridorCode(c);
        if (s.pref === "3501" && code === "SBT") continue;
        const segments = [corridorLegPoints(c, "a"), corridorLegPoints(c, "b")].filter(
          (seg) => seg.length >= 2,
        );
        if (segments.length === 0) continue;
        lines.push({
          code,
          name: corridorDisplayName(c),
          via: corridorDisplayVia(c),
          color: normalizeColor(c.color) ?? "#0284c7",
          segments,
        });
      }
      return lines;
    },
    visibleCorridorLines(): CorridorLine[] {
      const lines: CorridorLine[] = this.corridorLines;
      if (!this.selectedCorridor) {
        if (this.selectedStopId && this.selectedStopCorridorCodes.length > 0) {
          const sets = buildCodeSets(this.selectedStopCorridorCodes);
          const filtered = lines.filter((l) => inCodeSets(l.code, sets));
          return filtered.length > 0 ? filtered : lines;
        }
        return lines;
      }
      const corridor = findCorridorByCode(this.corridors, this.selectedCorridor);
      if (!corridor) return [];
      const segA = corridorLegPoints(corridor, "a");
      const segB = corridorLegPoints(corridor, "b");
      const segments = [segA, segB].filter((seg) => seg.length >= 2);
      if (segments.length === 0) return [];
      return [
        {
          code: corridorCode(corridor),
          name: corridorDisplayName(corridor),
          via: corridorDisplayVia(corridor),
          color: normalizeColor(corridor.color) ?? "#0284c7",
          segments,
        },
      ];
    },
  },

  actions: {
    syncCityCache(pref: string) {
      if (this.corridors.length > 0 && this.stops.length > 0) {
        cityCache.set(pref, {
          corridors: this.corridors,
          stops: this.stops,
          legStops: this.legStops,
        });
        writeStoredCity(pref, this.corridors, this.stops);
      }
    },

    async fetchCorridors() {
      const targetPref = this.pref;
      if (this.corridors.length === 0) {
        this.loadingCorridors = true;
      }
      try {
        const resp = await fetch(
          apiUrl(`/api/corridors?pref=${encodeURIComponent(targetPref)}`),
          { cache: "no-store" },
        );
        if (!resp.ok) {
          if (this.pref === targetPref && this.corridors.length === 0) {
            this.loadError = "Gagal memuat data rute. Periksa koneksi lalu coba lagi.";
          }
          return;
        }
        const json = await resp.json();
        if (this.pref !== targetPref) return;
        if (!Array.isArray(json?.data)) {
          if (this.corridors.length === 0) {
            this.loadError = "Gagal memuat data rute. Periksa koneksi lalu coba lagi.";
          }
          return;
        }
        const rows = json.data as Corridor[];
        this.corridors = rows.map((c) => markRaw({ ...c, pref: c.pref || targetPref }));
        this.loadError = null;
        this.syncCityCache(targetPref);
        if (this.selectedCorridor) {
          const target = this.selectedCorridor;
          const found = findCorridorByCode(this.corridors, target);
          void this.fetchLegStops(target, "a");
          if (!found || corridorHasTwoDirections(found)) {
            void this.fetchLegStops(target, "b");
          }
        }
      } catch {
        if (this.pref === targetPref && this.corridors.length === 0) {
          this.loadError = "Gagal memuat data rute. Periksa koneksi lalu coba lagi.";
        }
      } finally {
        if (this.pref === targetPref) {
          this.loadingCorridors = false;
        }
      }
    },

    async fetchStops() {
      const targetPref = this.pref;
      try {
        const resp = await fetch(apiUrl(`/api/routes?pref=${encodeURIComponent(targetPref)}`), {
          cache: "no-store",
        });
        if (!resp.ok) return;
        const json = await resp.json();
        if (this.pref !== targetPref || !Array.isArray(json?.data)) return;
        const rows = json.data as RawStop[];
        this.stops = buildNormalizedStops(rows, targetPref);
        this.syncCityCache(targetPref);
      } catch {
      }
    },

    retryLoadCity() {
      this.loadError = null;
      void this.fetchCorridors();
      void this.fetchStops();
      void this.fetchSiteConfig(this.pref);
    },

    async fetchSiteConfig(pref?: string) {
      const targetPref = pref ?? this.pref;
      try {
        const resp = await fetch(
          apiUrl(`/api/site-config?pref=${encodeURIComponent(targetPref)}`),
          {
            cache: "no-store",
          },
        );
        if (!resp.ok) return;
        const json = await resp.json();
        this.menuLinks = Array.isArray(json.menu_links) ? (json.menu_links as MenuLink[]) : [];
        if (this.pref === targetPref) {
          const rawPopup = json.welcome_popup;
          this.cityPopup =
            rawPopup && typeof rawPopup === "object"
              ? {
                  enabled: Boolean(rawPopup.enabled),
                  title: String(rawPopup.title ?? ""),
                  text: String(rawPopup.text ?? ""),
                  image_url: String(rawPopup.image_url ?? ""),
                }
              : null;
          this.siteConfigPref = targetPref;
        }
      } catch {
      }
    },

    async invalidateCity(pref: string, broadcast = true) {
      cityCache.delete(pref);
      if (typeof localStorage !== "undefined") {
        try {
          localStorage.removeItem(`${CITY_STORAGE_PREFIX}${pref}`);
        } catch {
        }
      }
      if (typeof sessionStorage !== "undefined") {
        try {
          sessionStorage.removeItem(`${CITY_STORAGE_PREFIX}${pref}`);
        } catch {
        }
      }
      if (broadcast && typeof BroadcastChannel !== "undefined") {
        try {
          const ch = new BroadcastChannel(ADMIN_SYNC_CHANNEL);
          ch.postMessage({ pref });
          ch.close();
        } catch {
        }
      }
      if (this.pref === pref) {
        this.legStops = {};
        this.loadedPref = pref;
        await Promise.all([this.fetchCorridors(), this.fetchStops(), this.fetchSiteConfig(pref)]);
      } else {
        if (this.loadedPref === pref) {
          this.loadedPref = null;
        }
        void this.fetchSiteConfig(this.pref);
      }
    },

    async fetchLegStops(code: string, dir: Direction) {
      const targetPref = this.pref;
      const key = `${code}|${dir}`;
      const fetchKey = `${targetPref}|${key}`;
      if (this.legStops[key] || fetchingLegs.has(fetchKey)) return;
      const corridor = findCorridorByCode(this.corridors, code);
      if (!corridor?.origin || !corridor?.toward) return;
      if (dir === "b" && !corridorHasTwoDirections(corridor)) return;

      const wantOrigin = dir === "a" ? corridor.origin : corridor.toward;
      const wantToward = dir === "a" ? corridor.toward : corridor.origin;
      const fallbackColor = normalizeColor(corridor.color) ?? "#64748b";
      fetchingLegs.add(fetchKey);
      this.loadingLegs[key] = true;
      try {
        const params = new URLSearchParams({
          pref: targetPref,
          kor: corridor.kor ?? code,
          origin: wantOrigin,
          toward: wantToward,
        });
        const resp = await fetch(apiUrl(`/api/route-corridor?${params.toString()}`));
        if (!resp.ok) return;
        const json = await resp.json();
        if (this.pref !== targetPref) return;
        const rows = (json.data ?? []) as RawStop[];
        if (!Array.isArray(rows) || rows.length === 0) return;
        const stops: BusStop[] = [];
        for (const r of rows) {
          const stop = normalizeStopRow(r, targetPref, code, fallbackColor);
          if (stop) stops.push(stop);
        }
        if (stops.length > 0 && this.pref === targetPref) {
          this.legStops[key] = normalizeLegStops(code, dir, stops, targetPref);
        }
      } catch {
      } finally {
        fetchingLegs.delete(fetchKey);
        if (this.pref === targetPref) {
          delete this.loadingLegs[key];
        }
      }
    },

    setSelectedStop(stopId: string | null) {
      this.selectedStopId = stopId;
    },

    ensureStopLegs(stop: BusStop) {
      const badges = stopServingCorridors(stop, this.stops, this.corridorColors, this.pref);
      for (const badge of badges) {
        if (!badge.trackable) continue;
        const corridor = findCorridorByCode(this.corridors, badge.code);
        void this.fetchLegStops(badge.code, "a");
        if (!corridor || corridorHasTwoDirections(corridor)) {
          void this.fetchLegStops(badge.code, "b");
        }
      }
    },

    selectCorridor(code: string | null) {
      if (code === null) {
        this.selectedCorridor = null;
        return;
      }
      const norm = code.trim().toUpperCase();
      this.selectedCorridor = this.selectedCorridor === norm ? null : norm;
    },

    setSelectedCorridor(code: string | null) {
      if (code === null) {
        this.selectedCorridor = null;
        return;
      }
      const norm = trackingCorridorKey(code.trim().toUpperCase(), this.pref) || null;
      if (this.selectedCorridor !== norm) {
        this.selectedCorridor = norm;
      }
      if (norm) {
        const corridor = findCorridorByCode(this.corridors, norm);
        void this.fetchLegStops(norm, "a");
        if (!corridor || corridorHasTwoDirections(corridor)) {
          void this.fetchLegStops(norm, "b");
        }
      }
    },

    connectLive() {
      if (this.socket) return;
      const socket = io(SOCKET_BASE, {
        path: "/socket.io",
        transports: ["websocket", "polling"],
        reconnection: true,
        reconnectionAttempts: Infinity,
        reconnectionDelay: 1000,
        reconnectionDelayMax: 15000,
      });
      this.socket = socket;

      socket.on("connect", () => {
        this.connected = true;
        this.hasConnectedOnce = true;
        socket.emit("subscribe", this.pref);
      });
      socket.on("disconnect", () => {
        this.connected = false;
      });

      const refreshLiveState = () => {
        if (this.socket?.connected) {
          this.socket.emit("subscribe", this.pref);
        } else {
          this.socket?.connect();
        }
        if (this.loadError || this.corridors.length === 0) {
          this.retryLoadCity();
        }
      };

      if (typeof document !== "undefined" && !visibilityHandler) {
        visibilityHandler = () => {
          if (document.visibilityState === "visible") {
            refreshLiveState();
          }
        };
        document.addEventListener("visibilitychange", visibilityHandler);
      }
      if (typeof window !== "undefined" && !onlineHandler) {
        onlineHandler = () => {
          refreshLiveState();
        };
        window.addEventListener("online", onlineHandler);
      }

      socket.on("snapshot", (list: Omit<Bus, "receivedAt">[]) => {
        const now = Date.now();
        const next: Record<string, Bus> = {};
        for (const b of list) {
          if (b.pref === this.pref && b.fix_age_s <= DROP_AFTER_S) {
            next[b.id] = markRaw({ ...b, receivedAt: now });
          }
        }
        this.buses = next;
        this.busesVersion++;
        this.lastUpdateAt = now;
      });
      socket.on("bus", (b: Omit<Bus, "receivedAt">) => {
        if (b.pref !== this.pref) return;
        if (b.fix_age_s > DROP_AFTER_S) {
          if (b.id in this.buses) {
            delete this.buses[b.id];
            this.busesVersion++;
          }
          return;
        }
        const now = Date.now();
        this.buses[b.id] = markRaw({ ...b, receivedAt: now });
        this.busesVersion++;
        this.lastUpdateAt = now;
      });

      pruneTimer = setInterval(() => this.prune(), 10_000);
      if (typeof BroadcastChannel !== "undefined" && !syncChannel) {
        syncChannel = new BroadcastChannel(ADMIN_SYNC_CHANNEL);
        syncChannel.onmessage = (ev: MessageEvent<{ pref?: string }>) => {
          const updatedPref = ev.data?.pref;
          if (typeof updatedPref === "string" && updatedPref) {
            void this.invalidateCity(updatedPref, false);
          }
        };
      }
    },

    prefetchCity(pref: string) {
      if (this.pref === pref && this.loadedPref === pref) return;
      if (!cityCache.has(pref)) {
        const stored = readStoredCity(pref);
        if (stored) {
          cityCache.set(pref, {
            corridors: stored.corridors,
            stops: stored.stops,
            legStops: {},
          });
        }
      }
      if (prefetchingCities.has(pref)) return;
      prefetchingCities.add(pref);
      void Promise.all([
        fetch(apiUrl(`/api/corridors?pref=${encodeURIComponent(pref)}`), { cache: "no-store" }).then(
          (r) => (r.ok ? r.json() : null),
        ),
        fetch(apiUrl(`/api/routes?pref=${encodeURIComponent(pref)}`), { cache: "no-store" }).then(
          (r) => (r.ok ? r.json() : null),
        ),
      ])
        .then(([cJson, sJson]) => {
          const cRows = (cJson?.data ?? []) as Corridor[];
          const sRows = (sJson?.data ?? []) as RawStop[];
          if (!Array.isArray(cRows) || !Array.isArray(sRows) || cRows.length === 0 || sRows.length === 0) {
            return;
          }
          const corridors = cRows.map((c) => markRaw({ ...c, pref: c.pref || pref }));
          const stops = buildNormalizedStops(sRows, pref);
          cityCache.set(pref, { corridors, stops, legStops: {} });
          writeStoredCity(pref, corridors, stops);
          if (this.pref === pref && (this.corridors.length === 0 || this.stops.length === 0)) {
            this.corridors = corridors;
            this.stops = stops;
            this.loadingCorridors = false;
            this.loadedPref = pref;
          }
        })
        .catch(() => {})
        .finally(() => {
          prefetchingCities.delete(pref);
        });
    },

    loadCity(pref: string) {
      if (pref !== this.pref) {
        if (this.corridors.length > 0 && this.stops.length > 0) {
          cityCache.set(this.pref, {
            corridors: this.corridors,
            stops: this.stops,
            legStops: this.legStops,
          });
        }
        this.pref = pref;
        this.cityPopup = null;
        this.siteConfigPref = "";
        this.loadError = null;
        this.buses = {};
        this.busesVersion++;
        this.loadingLegs = {};
        this.selectedCorridor = null;
        this.selectedStopId = null;
        this.socket?.emit("subscribe", pref);
        const memCached = cityCache.get(pref);
        if (memCached) {
          this.corridors = memCached.corridors;
          this.stops = memCached.stops;
          this.legStops = memCached.legStops;
          this.loadingCorridors = false;
          this.loadedPref = pref;
          void this.fetchCorridors();
          void this.fetchStops();
          void this.fetchSiteConfig(pref);
          return;
        }
        this.corridors = [];
        this.stops = [];
        this.legStops = {};
      }
      if (this.siteConfigPref !== pref) {
        void this.fetchSiteConfig(pref);
      }
      if (this.loadedPref !== pref) {
        const stored = readStoredCity(pref);
        if (stored) {
          this.corridors = stored.corridors;
          this.stops = stored.stops;
          this.loadingCorridors = false;
          cityCache.set(pref, {
            corridors: stored.corridors,
            stops: stored.stops,
            legStops: this.legStops,
          });
        }
        this.loadedPref = pref;
        void this.fetchCorridors();
        void this.fetchStops();
      }
    },

    setPref(pref: string) {
      this.loadCity(pref);
    },

    prune() {
      const now = Date.now();
      let changed = false;
      for (const [id, b] of Object.entries(this.buses)) {
        const elapsed = (now - b.receivedAt) / 1000;
        const age = b.fix_age_s + elapsed;
        if (age > DROP_AFTER_S) {
          delete this.buses[id];
          changed = true;
          continue;
        }
        const nextAge = Math.floor(age);
        const isStationary = b.speed != null && Math.round(b.speed) === 0;
        const nextStopped = isStationary ? Math.floor((b.stopped_s ?? 0) + elapsed) : 0;
        const staleDue =
          age > STALE_AFTER_S &&
          (!b.stale || Math.floor(nextAge / 60) !== Math.floor(b.fix_age_s / 60));
        const parkedTransition =
          isStationary && (b.stopped_s ?? 0) < PARKED_AFTER_S && nextStopped >= PARKED_AFTER_S;
        if (staleDue || parkedTransition) {
          this.buses[id] = markRaw({
            ...b,
            stale: age > STALE_AFTER_S,
            fix_age_s: nextAge,
            stopped_s: nextStopped,
            receivedAt: now,
          });
          changed = true;
        }
      }
      if (changed) this.busesVersion++;
    },

    locateUser() {
      if (typeof navigator === "undefined" || !("geolocation" in navigator)) {
        this.locationError = "Lokasi tidak didukung di browser ini";
        return;
      }
      if (this.trackingLocation) {
        this.locationError = null;
        this.refreshingLocation = true;
        if (this.userLocation) {
          this.locationPanSeq++;
        }
        navigator.geolocation.getCurrentPosition(
          (pos) => {
            this.userLocation = {
              lat: pos.coords.latitude,
              lng: pos.coords.longitude,
              accuracy: pos.coords.accuracy,
            };
            this.refreshingLocation = false;
            this.locationError = null;
            this.locationPanSeq++;
          },
          (err) => {
            this.refreshingLocation = false;
            if (err.code === 1) {
              this.stopLocation();
            }
            this.locationError = geolocationErrorMessage(err.code);
          },
          { enableHighAccuracy: true, maximumAge: 0, timeout: 15000 },
        );
        return;
      }
      this.trackingLocation = true;
      this.refreshingLocation = true;
      this.locationError = null;
      shouldPanToUser = true;
      geoWatchId = navigator.geolocation.watchPosition(
        (pos) => {
          this.userLocation = {
            lat: pos.coords.latitude,
            lng: pos.coords.longitude,
            accuracy: pos.coords.accuracy,
          };
          this.refreshingLocation = false;
          this.locationError = null;
          if (shouldPanToUser) {
            shouldPanToUser = false;
            this.locationPanSeq++;
          }
        },
        (err) => {
          this.stopLocation();
          this.locationError = geolocationErrorMessage(err.code);
        },
        { enableHighAccuracy: true, maximumAge: 5000, timeout: 15000 },
      );
    },

    stopLocation() {
      if (geoWatchId !== null && typeof navigator !== "undefined" && "geolocation" in navigator) {
        navigator.geolocation.clearWatch(geoWatchId);
      }
      geoWatchId = null;
      shouldPanToUser = false;
      this.trackingLocation = false;
      this.refreshingLocation = false;
      this.userLocation = null;
      this.locationError = null;
    },

    disconnectLive() {
      if (pruneTimer) clearInterval(pruneTimer);
      pruneTimer = null;
      syncChannel?.close();
      syncChannel = null;
      if (typeof document !== "undefined" && visibilityHandler) {
        document.removeEventListener("visibilitychange", visibilityHandler);
        visibilityHandler = null;
      }
      if (typeof window !== "undefined" && onlineHandler) {
        window.removeEventListener("online", onlineHandler);
        onlineHandler = null;
      }
      this.stopLocation();
      this.socket?.disconnect();
      this.socket = null;
      this.connected = false;
    },
  },
});

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useBusStore, import.meta.hot));
}
