import type { LatLng } from "./polylineUtils";
import { type Direction, compactCode } from "./transitHelpers";

const FOLDER_PREF: Record<string, string> = {
  surabaya: "3501",
  sb: "3501",
  "3501": "3501",
  yogyakarta: "1",
  yk: "1",
  "1": "1",
};

const rawFiles = import.meta.glob("./customRoutes/*/*.geojson", {
  eager: true,
  query: "?raw",
  import: "default",
}) as Record<string, string>;

function pushCoords(coords: unknown, out: LatLng[]): void {
  if (!Array.isArray(coords)) return;
  for (const item of coords) {
    if (!Array.isArray(item) || item.length < 2) continue;
    const lng = Number(item[0]);
    const lat = Number(item[1]);
    if (!Number.isFinite(lat) || !Number.isFinite(lng)) continue;
    const prev = out[out.length - 1];
    if (prev && prev[0] === lat && prev[1] === lng) continue;
    out.push([lat, lng]);
  }
}

function collectGeoJsonLines(node: unknown, out: LatLng[]): void {
  if (!node || typeof node !== "object") return;
  const obj = node as Record<string, unknown>;
  const kind = obj.type;
  if (kind === "FeatureCollection" && Array.isArray(obj.features)) {
    for (const feature of obj.features) {
      collectGeoJsonLines(feature, out);
    }
    return;
  }
  if (kind === "Feature") {
    collectGeoJsonLines(obj.geometry, out);
    return;
  }
  if (kind === "GeometryCollection" && Array.isArray(obj.geometries)) {
    for (const geom of obj.geometries) {
      collectGeoJsonLines(geom, out);
    }
    return;
  }
  if (kind === "LineString") {
    pushCoords(obj.coordinates, out);
    return;
  }
  if (kind === "MultiLineString" && Array.isArray(obj.coordinates)) {
    for (const line of obj.coordinates) {
      pushCoords(line, out);
    }
  }
}

function parseGeoJsonLine(raw: string): LatLng[] {
  try {
    const parsed = JSON.parse(raw) as unknown;
    const out: LatLng[] = [];
    collectGeoJsonLines(parsed, out);
    return out;
  } catch {
    return [];
  }
}

function buildCustomRouteMap(files: Record<string, string>): Map<string, LatLng[]> {
  const map = new Map<string, LatLng[]>();
  for (const [path, raw] of Object.entries(files)) {
    const match = path.match(/\/customRoutes\/([^/]+)\/([^/]+)\.geojson$/i);
    if (!match) continue;
    const pref = FOLDER_PREF[match[1].toLowerCase()];
    if (!pref) continue;
    const stem = match[2].trim();
    const dirMatch = stem.match(/^(.+?)[-_ ](a|b)$/i);
    const rawCode = dirMatch ? dirMatch[1] : stem;
    const dir: Direction = dirMatch && dirMatch[2].toLowerCase() === "b" ? "b" : "a";
    const key = `${pref}|${compactCode(rawCode.toUpperCase())}|${dir}`;
    const pts = parseGeoJsonLine(raw);
    if (pts.length >= 2) {
      map.set(key, pts);
    }
  }
  return map;
}

const CUSTOM_ROUTES = buildCustomRouteMap(rawFiles);

export function getCustomCorridorPoints(
  pref: string,
  code: string,
  dir: Direction,
): LatLng[] | null {
  const key = `${pref}|${compactCode(code.trim().toUpperCase())}|${dir}`;
  return CUSTOM_ROUTES.get(key) ?? null;
}

if (import.meta.hot) {
  import.meta.hot.accept(() => {
    window.location.reload();
  });
}
