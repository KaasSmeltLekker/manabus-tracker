import type { LayerSpecification, StyleSpecification } from "maplibre-gl";
import type { ThemeMode } from "./theme";

const DARK_MAP_BG = "#28241d";

const POSITRON_STYLE_URL = "https://tiles.openfreemap.org/styles/positron";

const HIDDEN_LAYERS = new Set([
  "highway-shield-non-us",
  "highway-shield-us-interstate",
  "road_shield_us",
]);

const PAINT_OVERRIDES: Record<string, Record<string, unknown>> = {
  background: { "background-color": DARK_MAP_BG },
  park: { "fill-color": "#252e22" },
  water: { "fill-color": "#2c4762" },
  landcover_ice_shelf: { "fill-color": "#333940" },
  landcover_glacier: { "fill-color": "#333940" },
  landuse_residential: { "fill-color": "#2d2922" },
  landcover_wood: { "fill-color": "#232c20" },
  waterway: { "line-color": "#2c4762" },
  building: { "fill-color": "#312d26", "fill-outline-color": "#3b362e" },
  tunnel_motorway_casing: { "line-color": "#201d17" },
  tunnel_motorway_inner: { "line-color": "#3b362e" },
  "aeroway-taxiway": { "line-color": "#3f3a32" },
  "aeroway-runway-casing": { "line-color": "#211e18" },
  "aeroway-area": { "fill-color": "#2e2a23" },
  "aeroway-runway": { "line-color": "#4a443b" },
  road_area_pier: { "fill-color": "#2d2922" },
  road_pier: { "line-color": "#3b362e" },
  highway_path: { "line-color": "#322e27" },
  highway_minor: { "line-color": "#38342c" },
  highway_major_casing: { "line-color": "#221f19" },
  highway_major_inner: { "line-color": "#454038" },
  highway_major_subtle: { "line-color": "#3e3931" },
  highway_motorway_casing: { "line-color": "#201d17" },
  highway_motorway_inner: {
    "line-color": ["interpolate", ["linear"], ["zoom"], 5.8, "rgba(79,73,64,0.65)", 6, "#504a41"],
  },
  highway_motorway_subtle: { "line-color": "rgba(74,68,59,0.75)" },
  railway_transit: { "line-color": "#524c43" },
  railway_transit_dashline: { "line-color": DARK_MAP_BG },
  railway_service: { "line-color": "#4d473e" },
  railway_service_dashline: { "line-color": DARK_MAP_BG },
  railway: { "line-color": "#595349" },
  railway_dashline: { "line-color": DARK_MAP_BG },
  highway_motorway_bridge_casing: { "line-color": "#1e1b15" },
  highway_motorway_bridge_inner: {
    "line-color": ["interpolate", ["linear"], ["zoom"], 5.8, "rgba(84,78,68,0.65)", 6, "#544e44"],
  },
  boundary_3: { "line-color": "#585248" },
  boundary_2: { "line-color": "#635c51" },
  boundary_disputed: { "line-color": "#635c51" },
  waterway_line_label: {
    "text-color": "#7ea0be",
    "text-halo-color": "rgba(31,50,70,0.85)",
  },
  water_name_point_label: {
    "text-color": "#88aac9",
    "text-halo-color": "rgba(31,50,70,0.85)",
  },
  water_name_line_label: {
    "text-color": "#88aac9",
    "text-halo-color": "rgba(31,50,70,0.85)",
  },
  "highway-name-path": { "text-color": "#7a7368", "text-halo-color": DARK_MAP_BG },
  "highway-name-minor": { "text-color": "#8f887c", "text-halo-color": DARK_MAP_BG },
  "highway-name-major": { "text-color": "#a8a195", "text-halo-color": DARK_MAP_BG },
  airport: { "text-color": "#aba498", "text-halo-color": DARK_MAP_BG },
  label_other: { "text-color": "#9e988d", "text-halo-color": "#221e18" },
  label_village: { "text-color": "#aca69b", "text-halo-color": "#221e18" },
  label_town: { "text-color": "#b8b2a7", "text-halo-color": "#221e18" },
  label_state: { "text-color": "#a8a195", "text-halo-color": "#221e18" },
  label_city: { "text-color": "#ccc6bb", "text-halo-color": "#1e1b16" },
  label_city_capital: { "text-color": "#ccc6bb", "text-halo-color": "#1e1b16" },
  label_country_3: { "text-color": "#ccc6bb", "text-halo-color": "#1e1b16" },
  label_country_2: { "text-color": "#ccc6bb", "text-halo-color": "#1e1b16" },
  label_country_1: { "text-color": "#ccc6bb", "text-halo-color": "#1e1b16" },
};

const STYLE_STORAGE_KEY = "ndibise-map-style-v1";
const STYLE_TTL_MS = 24 * 60 * 60 * 1000;

interface StoredMapStyle {
  savedAt: number;
  style: StyleSpecification;
}

let cachedRawPromise: Promise<StyleSpecification> | null = null;

function readCachedStyle(): StoredMapStyle | null {
  if (typeof localStorage === "undefined") return null;
  try {
    const raw = localStorage.getItem(STYLE_STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as Partial<StoredMapStyle>;
    if (
      typeof parsed?.savedAt === "number" &&
      parsed.style &&
      Array.isArray(parsed.style.layers)
    ) {
      return { savedAt: parsed.savedAt, style: parsed.style };
    }
  } catch {
  }
  return null;
}

function writeCachedStyle(style: StyleSpecification): void {
  if (typeof localStorage === "undefined") return;
  try {
    const payload: StoredMapStyle = { savedAt: Date.now(), style };
    localStorage.setItem(STYLE_STORAGE_KEY, JSON.stringify(payload));
  } catch {
  }
}

function transformLayer(layer: LayerSpecification, isDark: boolean): LayerSpecification {
  const next: Record<string, unknown> = { ...layer };
  const override = isDark ? PAINT_OVERRIDES[layer.id] : undefined;
  if (override) {
    const basePaint = (layer as { paint?: Record<string, unknown> }).paint ?? {};
    next.paint = { ...basePaint, ...override };
  }
  if (HIDDEN_LAYERS.has(layer.id)) {
    const baseLayout = (layer as { layout?: Record<string, unknown> }).layout ?? {};
    next.layout = { ...baseLayout, visibility: "none" };
  }
  return next as LayerSpecification;
}

function fetchPositronFromNetwork(): Promise<StyleSpecification> {
  return fetch(POSITRON_STYLE_URL).then(async (resp) => {
    if (!resp.ok) {
      throw new Error(`HTTP ${resp.status}`);
    }
    const style = (await resp.json()) as StyleSpecification;
    writeCachedStyle(style);
    return style;
  });
}

function fetchRawPositron(): Promise<StyleSpecification> {
  if (cachedRawPromise) return cachedRawPromise;
  const stored = readCachedStyle();
  if (stored) {
    cachedRawPromise = Promise.resolve(stored.style);
    if (Date.now() - stored.savedAt > STYLE_TTL_MS) {
      void fetchPositronFromNetwork()
        .then((fresh) => {
          cachedRawPromise = Promise.resolve(fresh);
        })
        .catch(() => {});
    }
    return cachedRawPromise;
  }
  cachedRawPromise = fetchPositronFromNetwork().catch((err) => {
    cachedRawPromise = null;
    throw err;
  });
  return cachedRawPromise;
}

export function loadMapStyle(mode: ThemeMode): Promise<StyleSpecification> {
  const isDark = mode !== "light";
  return fetchRawPositron().then((raw) => ({
    ...raw,
    layers: (raw.layers ?? []).map((layer) => transformLayer(layer, isDark)),
  }));
}

export function prefetchMapStyle(): void {
  void fetchRawPositron().catch(() => {});
}

