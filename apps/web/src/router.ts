import { ref } from "vue";
import { BASE_PATH } from "./config";

export type CityId = "surabaya" | "yogyakarta";

export const CITY_PREF: Record<CityId, string> = {
  surabaya: "3501",
  yogyakarta: "1",
};

type PageRoute =
  | { name: "home" }
  | { name: "admin" }
  | { name: "city"; city: CityId }
  | { name: "all"; city: CityId }
  | { name: "corridor"; city: CityId; code: string }
  | { name: "stop"; city: CityId; stopId: string };

function isCityId(s: string): s is CityId {
  return s === "surabaya" || s === "yogyakarta";
}

function normalizeCorridorParam(raw: string, city: CityId): string | null {
  try {
    const decoded = decodeURIComponent(raw).trim().toUpperCase();
    if (!decoded) return null;
    return city === "surabaya" && decoded === "SBT" ? "R1" : decoded;
  } catch {
    return null;
  }
}

function decodeStopParam(raw: string): string | null {
  try {
    const decoded = decodeURIComponent(raw).trim();
    return decoded || null;
  } catch {
    return null;
  }
}

function stripBase(pathname: string): string {
  if (!BASE_PATH) return pathname || "/";
  if (pathname === BASE_PATH) return "/";
  if (pathname.startsWith(`${BASE_PATH}/`)) {
    return pathname.slice(BASE_PATH.length) || "/";
  }
  return pathname || "/";
}

function withBase(path: string): string {
  const normalized = path.startsWith("/") ? path : `/${path}`;
  if (!BASE_PATH) return normalized;
  return normalized === "/" ? `${BASE_PATH}/` : `${BASE_PATH}${normalized}`;
}

function parsePath(rawPathname: string): PageRoute {
  const pathname = stripBase(rawPathname);
  if (/^\/atmin\/?$/i.test(pathname)) {
    return { name: "admin" };
  }
  const cityMatch = pathname.match(/^\/(surabaya|yogyakarta)\/?$/i);
  if (cityMatch) {
    const city = cityMatch[1].toLowerCase();
    if (isCityId(city)) return { name: "city", city };
  }

  const cityAllMatch = pathname.match(/^\/(surabaya|yogyakarta)\/semua\/?$/i);
  if (cityAllMatch) {
    const city = cityAllMatch[1].toLowerCase();
    if (isCityId(city)) return { name: "all", city };
  }

  const cityStopMatch = pathname.match(/^\/(surabaya|yogyakarta)\/halte\/([^/]+)\/?$/i);
  if (cityStopMatch) {
    const city = cityStopMatch[1].toLowerCase();
    const stopId = decodeStopParam(cityStopMatch[2]);
    if (isCityId(city) && stopId) return { name: "stop", city, stopId };
    return { name: "home" };
  }

  const cityCorridorMatch = pathname.match(/^\/(surabaya|yogyakarta)\/rute\/([^/]+)\/?$/i);
  if (cityCorridorMatch) {
    const city = cityCorridorMatch[1].toLowerCase();
    if (isCityId(city)) {
      const code = normalizeCorridorParam(cityCorridorMatch[2], city);
      if (code) return { name: "corridor", city, code };
    }
    return { name: "home" };
  }

  if (pathname === "/semua" || pathname === "/semua/") {
    return { name: "all", city: "surabaya" };
  }
  const stopMatch = pathname.match(/^\/halte\/([^/]+)\/?$/i);
  if (stopMatch) {
    const stopId = decodeStopParam(stopMatch[1]);
    return stopId ? { name: "stop", city: "surabaya", stopId } : { name: "home" };
  }
  const match = pathname.match(/^\/rute\/([^/]+)\/?$/i);
  if (match) {
    const code = normalizeCorridorParam(match[1], "surabaya");
    return code ? { name: "corridor", city: "surabaya", code } : { name: "home" };
  }
  return { name: "home" };
}

export const currentRoute = ref<PageRoute>(parsePath(window.location.pathname));

let internalDepth = 0;
let lastNonStopPath =
  currentRoute.value.name === "stop"
    ? `/${currentRoute.value.city}`
    : stripBase(window.location.pathname);

export function activeCity(): CityId {
  const r = currentRoute.value;
  return r.name === "home" || r.name === "admin" ? "surabaya" : r.city;
}

function navigate(path: string, options?: { replace?: boolean }) {
  if (currentRoute.value.name !== "stop") {
    lastNonStopPath = stripBase(window.location.pathname);
  }
  const target = withBase(path);
  if (window.location.pathname !== target) {
    if (options?.replace) {
      window.history.replaceState({ depth: internalDepth }, "", target);
    } else {
      internalDepth++;
      window.history.pushState({ depth: internalDepth }, "", target);
    }
  }
  currentRoute.value = parsePath(window.location.pathname);
  if (currentRoute.value.name !== "stop") {
    lastNonStopPath = stripBase(window.location.pathname);
  }
}

export function goHome() {
  navigate("/");
}

export function goAdmin() {
  navigate("/atmin");
}

export function goCity(city: CityId) {
  navigate(`/${city}`);
}

export function goAll(city?: CityId) {
  navigate(`/${city ?? activeCity()}/semua`);
}

export function goCorridor(code: string, city?: CityId) {
  const targetCity = city ?? activeCity();
  const norm = code.trim().toUpperCase();
  const target = targetCity === "surabaya" && norm === "SBT" ? "R1" : norm;
  navigate(`/${targetCity}/rute/${encodeURIComponent(target)}`);
}

export function goStop(stopId: string, city?: CityId) {
  const trimmed = stopId.trim();
  if (!trimmed) return;
  const targetCity = city ?? activeCity();
  const isReplacing = currentRoute.value.name === "stop";
  navigate(`/${targetCity}/halte/${encodeURIComponent(trimmed)}`, {
    replace: isReplacing,
  });
}

export function goBackOrHome() {
  if (internalDepth > 0) {
    window.history.back();
  } else if (lastNonStopPath && lastNonStopPath !== stripBase(window.location.pathname)) {
    navigate(lastNonStopPath);
  } else {
    goCity(activeCity());
  }
}

window.addEventListener("popstate", (e: PopStateEvent) => {
  const stateDepth = (e.state as { depth?: unknown } | null)?.depth;
  internalDepth = typeof stateDepth === "number" ? stateDepth : 0;
  currentRoute.value = parsePath(window.location.pathname);
  if (currentRoute.value.name !== "stop") {
    lastNonStopPath = stripBase(window.location.pathname);
  }
});

