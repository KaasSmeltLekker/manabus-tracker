<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import L from "leaflet";
import "leaflet/dist/leaflet.css";
import { setWorkerUrl } from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { maplibreGL } from "@maplibre/maplibre-gl-leaflet";
import { storeToRefs } from "pinia";
import { assetUrl } from "../config";
import { loadMapStyle } from "../mapStyle";
import { goCorridor } from "../router";
import { theme, type ThemeMode } from "../theme";
import {
  contrastText,
  corridorBadgeLabel,
  formatIdleAge,
  formatStopLabel,
  lookupCorridorValue,
  resolveCorridorAlias,
  stopServingCorridors,
  trackingCorridorKey,
  useBusStore,
  type Bus,
  type BusStop,
  type CorridorLine,
} from "../stores/buses";

if (import.meta.env.PROD) {
  setWorkerUrl(assetUrl("maplibre-gl-worker.mjs"));
}

const props = defineProps<{
  mode: "all" | "single" | "stop";
  focusBus: { id: string; seq: number } | null;
  focusStop: { id: string; seq: number } | null;
  activeStopId?: string | null;
  bottomInset?: number;
}>();

const emit = defineEmits<{
  openStopArrival: [stopId: string];
}>();

const store = useBusStore();
const {
  busesVersion,
  corridors,
  visibleBuses,
  visibleStops,
  visibleCorridorLines,
  selectedCorridor,
  selectedDirection,
  selectedStopId,
  selectedStop,
  stops,
  operator,
  corridorColors,
  userLocation,
  locationPanSeq,
  trackingLocation,
  refreshingLocation,
  locationError,
} = storeToRefs(store);

const FALLBACK_COLOR = "#0284c7";
const PARKED_AFTER_S = 10 * 60;

function isBusParked(b: Bus): boolean {
  return b.speed != null && Math.round(b.speed) === 0 && (b.stopped_s ?? 0) >= PARKED_AFTER_S;
}

interface BusMarkerEntry {
  marker: L.Marker;
  lat: number;
  lng: number;
  iconKey: string;
  tipKey: string;
}

const mapEl = ref<HTMLElement | null>(null);
const canZoomIn = ref(true);
const canZoomOut = ref(true);

function syncZoomState() {
  if (!map) return;
  const z = map.getZoom();
  canZoomIn.value = z < map.getMaxZoom();
  canZoomOut.value = z > map.getMinZoom();
}

function zoomIn() {
  map?.zoomIn();
}

function zoomOut() {
  map?.zoomOut();
}

let map: L.Map | undefined;
let glLayer: ReturnType<typeof maplibreGL> | undefined;
let routeRenderer: L.Canvas | undefined;
let routeCasingLayer: L.LayerGroup | undefined;
let routeLayer: L.LayerGroup | undefined;
let activeStopLayer: L.LayerGroup | undefined;
let stopRenderer: L.Canvas | undefined;
let stopLayer: L.LayerGroup | undefined;
let userLayer: L.LayerGroup | undefined;
let fittedCode: string | null = null;
let fittedStopId: string | null = null;
let pendingFocusStopId: string | null = null;
let disposed = false;
let hoverCloseTimer: ReturnType<typeof setTimeout> | null = null;
let hoveredPopupMarker: L.CircleMarker | null = null;
let pinnedPopupMarker: L.CircleMarker | null = null;
let resizeObserver: ResizeObserver | undefined;
const markers = new Map<string, BusMarkerEntry>();
const stopMarkers = new Map<string, L.CircleMarker>();

function cancelPopupClose() {
  if (hoverCloseTimer) {
    clearTimeout(hoverCloseTimer);
    hoverCloseTimer = null;
  }
}

function schedulePopupClose(target: L.CircleMarker) {
  cancelPopupClose();
  if (pinnedPopupMarker === target) return;
  hoverCloseTimer = setTimeout(() => {
    hoverCloseTimer = null;
    if (hoveredPopupMarker === target && pinnedPopupMarker !== target) {
      target.closePopup();
    }
  }, 260);
}

function esc(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function isDarkColor(color: string): boolean {
  const hex = color.replace(/^#/, "");
  const full =
    hex.length === 3
      ? hex
          .split("")
          .map((c) => c + c)
          .join("")
      : hex;
  if (!/^[0-9a-f]{6}$/i.test(full)) return false;
  const r = parseInt(full.slice(0, 2), 16) / 255;
  const g = parseInt(full.slice(2, 4), 16) / 255;
  const b = parseInt(full.slice(4, 6), 16) / 255;
  return 0.2126 * r + 0.7152 * g + 0.0722 * b < 0.22;
}

function busDisplayCorridor(b: Bus): string {
  if (!b.corridor) return "";
  const raw = b.corridor.trim().toUpperCase();
  const tracked = trackingCorridorKey(raw, store.pref);
  return resolveCorridorAlias(corridors.value, tracked);
}

function busColor(b: Bus, colorMap: Record<string, string>): string {
  if (!b.corridor) return FALLBACK_COLOR;
  const key = busDisplayCorridor(b);
  return (
    lookupCorridorValue(colorMap, key) ??
    lookupCorridorValue(colorMap, b.corridor.trim().toUpperCase()) ??
    FALLBACK_COLOR
  );
}

function arrowIcon(color: string, heading: number, stale: boolean): L.DivIcon {
  const deg = Number.isFinite(heading) ? Math.round(heading) : 0;
  return L.divIcon({
    className: "bus-arrow-icon",
    iconSize: [34, 34],
    iconAnchor: [17, 17],
    html: `<div class="bus-arrow${stale ? " stale" : ""}" style="transform: rotate(${deg}deg)"><svg viewBox="0 0 24 24" width="28" height="28" aria-hidden="true"><path d="M12 2L20 21L12 16.5L4 21Z" fill="${color}" stroke="#ffffff" stroke-width="1.8" stroke-linejoin="round"/></svg></div>`,
  });
}

function tooltip(b: Bus, color: string): string {
  const code = busDisplayCorridor(b);
  const head = `<div class="bus-tip-head"><span class="bus-tip-badge" style="background:${color};color:${contrastText(color)}">${esc(corridorBadgeLabel(code || "?"))}</span><span class="bus-tip-name">${esc(b.name ?? "")}</span></div>`;
  const toward = b.toward
    ? `<div class="bus-tip-toward">${esc(formatStopLabel(b.toward))}</div>`
    : "";
  const statusText =
    b.speed != null
      ? Math.round(b.speed) > 0
        ? `${Math.round(b.speed)} km/jam · Bergerak`
        : isBusParked(b)
          ? "0 km/jam · Terparkir"
          : "0 km/jam · Berhenti"
      : b.plate
        ? esc(b.plate)
        : "";
  const meta = statusText ? `<div class="bus-tip-meta">${statusText}</div>` : "";
  const stale = b.stale
    ? `<div class="bus-tip-stale">Diperbarui ${formatIdleAge(b.fix_age_s)}</div>`
    : "";
  return `${head}${toward}${meta}${stale}`;
}

function routeTooltip(line: CorridorLine): string {
  const head = `<div class="bus-tip-head"><span class="bus-tip-badge" style="background:${line.color};color:${contrastText(line.color)}">${esc(corridorBadgeLabel(line.code))}</span><span class="bus-tip-name">${esc(line.name)}</span></div>`;
  const via = line.via ? `<div class="bus-tip-toward">${esc(line.via)}</div>` : "";
  return `${head}${via}`;
}

function buildStopPopup(stop: BusStop, colorMap: Record<string, string>): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = "stop-popup";

  const title = document.createElement("div");
  title.className = "stop-popup-name";
  title.textContent = stop.sh_name;
  wrap.appendChild(title);

  const badges = stopServingCorridors(stop, stops.value, colorMap, store.pref);
  if (badges.length > 0) {
    const row = document.createElement("div");
    row.className = "stop-popup-badges";
    for (const badge of badges) {
      const chip = document.createElement("span");
      chip.className = "bus-tip-badge";
      chip.style.background = badge.color;
      chip.style.color = contrastText(badge.color);
      chip.textContent = corridorBadgeLabel(badge.code);
      row.appendChild(chip);
    }
    wrap.appendChild(row);
  }

  const btn = document.createElement("button");
  btn.type = "button";
  btn.className = "stop-popup-btn";
  btn.textContent = "Lihat kedatangan";
  btn.addEventListener("click", (e) => {
    e.stopPropagation();
    map?.closePopup();
    emit("openStopArrival", stop.sh_id);
  });
  wrap.appendChild(btn);

  return wrap;
}

function effectiveBottomInset(): number {
  if (!map) return 0;
  const raw = props.bottomInset ?? 0;
  if (raw <= 0) return 0;
  return Math.min(raw, Math.max(0, map.getSize().y - 120));
}

function setViewWithInset(latlng: L.LatLngExpression, zoom: number) {
  if (!map) return;
  const inset = effectiveBottomInset();
  if (inset > 0) {
    const pt = map.project(latlng, zoom);
    const shifted = map.unproject(L.point(pt.x, pt.y + inset / 2), zoom);
    map.setView(shifted, zoom);
  } else {
    map.setView(latlng, zoom);
  }
}

function fitActiveStop() {
  if (!map || props.mode !== "stop" || !props.activeStopId) return;
  if (fittedStopId === props.activeStopId) return;
  const marker = stopMarkers.get(props.activeStopId);
  if (marker) {
    setViewWithInset(marker.getLatLng(), Math.max(map.getZoom(), 15));
    fittedStopId = props.activeStopId;
    return;
  }
  const sel = selectedStop.value;
  if (sel && sel.sh_id === props.activeStopId) {
    setViewWithInset([sel.lat, sel.lng], Math.max(map.getZoom(), 15));
    fittedStopId = props.activeStopId;
  }
}

function fitSelectedRoute() {
  if (!map) return;
  if (props.mode === "stop") {
    fittedCode = null;
    fitActiveStop();
    return;
  }
  fittedStopId = null;
  if (props.mode !== "single") {
    if (fittedCode !== null) {
      fittedCode = null;
      setViewWithInset(operator.value.center, 12);
    }
    return;
  }
  const code = selectedCorridor.value;
  if (!code || fittedCode === code) return;
  const lines = visibleCorridorLines.value;
  if (lines.length === 0) return;
  const bounds = L.latLngBounds([]);
  for (const line of lines) {
    for (const seg of line.segments) {
      for (const pt of seg) {
        bounds.extend(pt);
      }
    }
  }
  if (bounds.isValid()) {
    const inset = effectiveBottomInset();
    map.fitBounds(bounds, {
      paddingTopLeft: [40, 40],
      paddingBottomRight: [40, 40 + inset],
      maxZoom: 15,
    });
    fittedCode = code;
  }
}

function isLightColor(color: string): boolean {
  const hex = color.replace(/^#/, "");
  const full =
    hex.length === 3
      ? hex
          .split("")
          .map((c) => c + c)
          .join("")
      : hex;
  if (!/^[0-9a-f]{6}$/i.test(full)) return false;
  const r = parseInt(full.slice(0, 2), 16) / 255;
  const g = parseInt(full.slice(2, 4), 16) / 255;
  const b = parseInt(full.slice(4, 6), 16) / 255;
  return 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.72;
}

function syncRoutes() {
  if (!map || !routeCasingLayer || !routeLayer || !routeRenderer) return;
  routeCasingLayer.clearLayers();
  routeLayer.clearLayers();
  const focused = selectedCorridor.value !== null;
  const lines = visibleCorridorLines.value;

  for (const line of lines) {
    const casingColor =
      theme.value === "light"
        ? isLightColor(line.color)
          ? "#475569"
          : "#ffffff"
        : isDarkColor(line.color)
          ? "#d6d0c4"
          : "#181511";
    const casingOpacity =
      theme.value === "light"
        ? focused
          ? 0.95
          : 0.88
        : isDarkColor(line.color)
          ? focused
            ? 0.85
            : 0.65
          : focused
            ? 0.92
            : 0.78;
    for (const segment of line.segments) {
      L.polyline(segment, {
        pane: "routesPane",
        renderer: routeRenderer,
        color: casingColor,
        weight: focused ? 8 : 5.5,
        opacity: casingOpacity,
        lineCap: "round",
        lineJoin: "round",
        interactive: false,
      }).addTo(routeCasingLayer);
    }
  }

  const allowRouteInteraction = props.mode === "all" && !focused;
  for (const line of lines) {
    const tip = allowRouteInteraction ? routeTooltip(line) : "";
    for (const segment of line.segments) {
      const polyline = L.polyline(segment, {
        pane: "routesPane",
        renderer: routeRenderer,
        color: line.color,
        weight: focused ? 5 : 3.5,
        opacity: focused ? 0.95 : 0.82,
        lineCap: "round",
        lineJoin: "round",
        interactive: allowRouteInteraction,
      });
      if (allowRouteInteraction) {
        polyline.bindTooltip(tip, { sticky: true, opacity: 1 }).on("click", () => {
          goCorridor(line.code);
        });
      }
      polyline.addTo(routeLayer);
    }
  }
}

let styledActiveStopId: string | null = null;

function stopStyleFor(
  isActive: boolean,
  filtered: boolean,
  touch: boolean,
  color: string,
) {
  return {
    radius: isActive
      ? touch
        ? 7.5
        : 6.5
      : filtered
        ? touch
          ? 6
          : 5
        : touch
          ? 4.5
          : 3.5,
    color,
    weight: isActive ? 3 : filtered ? 2 : 1.5,
    opacity: isActive ? 1 : filtered ? 0.95 : 0.85,
  };
}

function updateActiveStopMarkerStyle() {
  if (!map || !stopLayer) return;
  const nextId = props.activeStopId ?? null;
  if (styledActiveStopId === nextId) return;
  const filtered = selectedCorridor.value !== null || props.mode === "stop";
  const touch =
    typeof window !== "undefined" && window.matchMedia("(pointer: coarse)").matches;
  const sel = selectedCorridor.value;
  const activeColor = sel ? (lookupCorridorValue(corridorColors.value, sel) ?? null) : null;
  if (styledActiveStopId) {
    const prevMarker = stopMarkers.get(styledActiveStopId);
    const prevStop = visibleStops.value.find((s) => s.sh_id === styledActiveStopId);
    if (prevMarker && prevStop) {
      const style = stopStyleFor(false, filtered, touch, activeColor ?? prevStop.color);
      prevMarker.setRadius(style.radius);
      prevMarker.setStyle({
        color: style.color,
        weight: style.weight,
        opacity: style.opacity,
      });
    }
  }
  styledActiveStopId = nextId;
  if (nextId) {
    const nextMarker = stopMarkers.get(nextId);
    const nextStop = visibleStops.value.find((s) => s.sh_id === nextId);
    if (nextMarker && nextStop) {
      const style = stopStyleFor(true, filtered, touch, activeColor ?? nextStop.color);
      nextMarker.setRadius(style.radius);
      nextMarker.setStyle({
        color: style.color,
        weight: style.weight,
        opacity: style.opacity,
      });
    }
  }
}

function syncActiveStopRing() {
  if (!map || !activeStopLayer) return;
  activeStopLayer.clearLayers();
  const id = props.activeStopId;
  if (!id) return;
  let latlng: L.LatLngExpression | null = null;
  const marker = stopMarkers.get(id);
  if (marker) {
    latlng = marker.getLatLng();
  } else if (selectedStop.value && selectedStop.value.sh_id === id) {
    latlng = [selectedStop.value.lat, selectedStop.value.lng];
  } else {
    const found = stops.value.find((s) => s.sh_id === id);
    if (found) latlng = [found.lat, found.lng];
  }
  if (!latlng) return;
  L.marker(latlng, {
    icon: L.divIcon({
      className: "active-stop-icon",
      iconSize: [28, 28],
      iconAnchor: [14, 14],
      html: '<div class="active-stop-ring"></div>',
    }),
    interactive: false,
    zIndexOffset: 600,
  }).addTo(activeStopLayer);
}

function syncStops() {
  if (!map || !stopLayer || !stopRenderer) return;
  cancelPopupClose();
  hoveredPopupMarker = null;
  pinnedPopupMarker = null;
  stopLayer.clearLayers();
  stopMarkers.clear();
  styledActiveStopId = props.activeStopId ?? null;
  const filtered = selectedCorridor.value !== null || props.mode === "stop";
  const sel = selectedCorridor.value;
  const activeColor = sel ? (lookupCorridorValue(corridorColors.value, sel) ?? null) : null;
  const touch =
    typeof window !== "undefined" && window.matchMedia("(pointer: coarse)").matches;

  for (const stop of visibleStops.value) {
    const isActive = props.activeStopId === stop.sh_id;
    const style = stopStyleFor(isActive, filtered, touch, activeColor ?? stop.color);
    const marker = L.circleMarker([stop.lat, stop.lng], {
      pane: "stopsPane",
      renderer: stopRenderer,
      radius: style.radius,
      color: style.color,
      weight: style.weight,
      opacity: style.opacity,
      fillColor: "#ffffff",
      fillOpacity: 0.95,
    })
      .bindPopup(() => buildStopPopup(stop, corridorColors.value), {
        className: "stop-popup-shell",
        minWidth: 210,
        maxWidth: 270,
        offset: [0, -2],
        autoPan: false,
      })
      .addTo(stopLayer);

    marker.off("click");
    if (touch) {
      marker.on("click", () => {
        if (marker.isPopupOpen()) {
          marker.closePopup();
        } else {
          marker.openPopup();
        }
      });
    } else {
      marker.on("mouseover", () => {
        cancelPopupClose();
        hoveredPopupMarker = marker;
        if (!marker.isPopupOpen()) {
          pinnedPopupMarker = null;
          marker.openPopup();
        }
      });
      marker.on("mouseout", () => {
        schedulePopupClose(marker);
      });
      marker.on("click", () => {
        cancelPopupClose();
        if (pinnedPopupMarker === marker && marker.isPopupOpen()) {
          pinnedPopupMarker = null;
          marker.closePopup();
        } else {
          pinnedPopupMarker = marker;
          hoveredPopupMarker = marker;
          if (!marker.isPopupOpen()) {
            marker.openPopup();
          }
        }
      });
      marker.on("popupopen", () => {
        const el = marker.getPopup()?.getElement();
        if (!el) return;
        el.onmouseenter = () => cancelPopupClose();
        el.onmouseleave = () => schedulePopupClose(marker);
      });
      marker.on("popupclose", () => {
        if (pinnedPopupMarker === marker) pinnedPopupMarker = null;
        if (hoveredPopupMarker === marker) hoveredPopupMarker = null;
      });
    }

    stopMarkers.set(stop.sh_id, marker);
  }

  syncActiveStopRing();

  if (pendingFocusStopId) {
    const target = stopMarkers.get(pendingFocusStopId);
    if (target) {
      setViewWithInset(target.getLatLng(), Math.max(map.getZoom(), 15));
      if (!(props.mode === "stop" && pendingFocusStopId === props.activeStopId)) {
        cancelPopupClose();
        pinnedPopupMarker = target;
        hoveredPopupMarker = target;
        target.openPopup();
      }
      pendingFocusStopId = null;
    }
  }
}

let markerRafId: number | null = null;

function scheduleSyncMarkers() {
  if (markerRafId !== null) return;
  markerRafId = requestAnimationFrame(() => {
    markerRafId = null;
    syncMarkers();
  });
}

function busTipKey(b: Bus, color: string): string {
  const speed = b.speed != null && Number.isFinite(b.speed) ? Math.round(b.speed) : "";
  const age = b.stale ? formatIdleAge(b.fix_age_s) : "";
  return `${color}|${busDisplayCorridor(b)}|${b.name ?? ""}|${b.toward ?? ""}|${speed}|${isBusParked(b) ? 1 : 0}|${b.plate ?? ""}|${age}`;
}

function syncMarkers() {
  if (!map || disposed) return;
  const current = visibleBuses.value;
  const colorMap = corridorColors.value;

  for (const [id, entry] of markers) {
    if (!current[id]) {
      entry.marker.remove();
      markers.delete(id);
    }
  }
  for (const [id, bus] of Object.entries(current)) {
    const color = busColor(bus, colorMap);
    const deg = bus.heading != null && Number.isFinite(bus.heading) ? Math.round(bus.heading) : 0;
    const iconKey = `${color}|${deg}|${bus.stale ? 1 : 0}`;
    const tipKey = busTipKey(bus, color);

    const entry = markers.get(id);
    if (!entry) {
      const tip = tooltip(bus, color);
      const marker = L.marker([bus.lat, bus.lng], {
        icon: arrowIcon(color, deg, bus.stale),
      }).addTo(map);
      marker.bindTooltip(tip, { opacity: 1 });
      markers.set(id, { marker, lat: bus.lat, lng: bus.lng, iconKey, tipKey });
    } else {
      if (entry.lat !== bus.lat || entry.lng !== bus.lng) {
        entry.marker.setLatLng([bus.lat, bus.lng]);
        entry.lat = bus.lat;
        entry.lng = bus.lng;
      }
      if (entry.iconKey !== iconKey) {
        entry.marker.setIcon(arrowIcon(color, deg, bus.stale));
        entry.iconKey = iconKey;
      }
      if (entry.tipKey !== tipKey) {
        entry.marker.setTooltipContent(tooltip(bus, color));
        entry.tipKey = tipKey;
      }
    }
  }
}

function syncUserLocation() {
  if (!map || !userLayer) return;
  userLayer.clearLayers();
  const loc = userLocation.value;
  if (!loc) return;
  L.circle([loc.lat, loc.lng], {
    radius: Math.min(loc.accuracy, 500),
    color: "#0284c7",
    weight: 1,
    opacity: 0.35,
    fillColor: "#38bdf8",
    fillOpacity: 0.14,
    interactive: false,
  }).addTo(userLayer);
  L.marker([loc.lat, loc.lng], {
    icon: L.divIcon({
      className: "user-loc-icon",
      iconSize: [20, 20],
      iconAnchor: [10, 10],
      html: '<div class="user-loc-dot"></div>',
    }),
    zIndexOffset: 1000,
  })
    .bindTooltip("Lokasi GPSmu", { opacity: 1 })
    .addTo(userLayer);
}

let appliedMapTheme: ThemeMode | null = null;

async function applyMapStyle(mode: ThemeMode) {
  try {
    const style = await loadMapStyle(mode);
    if (disposed || theme.value !== mode) return;
    glLayer?.getMaplibreMap()?.setStyle(style);
    appliedMapTheme = mode;
  } catch {
  }
}

onMounted(() => {
  disposed = false;
  if (!mapEl.value) return;
  map = L.map(mapEl.value, { maxZoom: 19, zoomControl: false }).setView(
    operator.value.center,
    12,
  );
  map.on("zoomend", syncZoomState);
  if (effectiveBottomInset() > 0) {
    setViewWithInset(operator.value.center, 12);
  }

  const routesPane = map.createPane("routesPane");
  routesPane.style.zIndex = "410";
  const stopsPane = map.createPane("stopsPane");
  stopsPane.style.zIndex = "420";

  routeRenderer = L.canvas({ pane: "routesPane", padding: 0.5 }).addTo(map);
  routeCasingLayer = L.layerGroup().addTo(map);
  routeLayer = L.layerGroup().addTo(map);
  activeStopLayer = L.layerGroup().addTo(map);
  stopRenderer = L.canvas({ pane: "stopsPane", padding: 0.5 }).addTo(map);
  stopLayer = L.layerGroup().addTo(map);
  userLayer = L.layerGroup().addTo(map);

  syncRoutes();
  syncStops();
  syncMarkers();
  syncUserLocation();
  fitSelectedRoute();
  fitActiveStop();
  syncZoomState();

  resizeObserver = new ResizeObserver(() => {
    if (!map) return;
    map.invalidateSize({ animate: false });
    requestAnimationFrame(() => {
      if (!disposed) glLayer?.getMaplibreMap()?.resize();
    });
  });
  resizeObserver.observe(mapEl.value);

  const initialTheme = theme.value;
  void loadMapStyle(initialTheme)
    .then((style) => {
      if (!map || disposed) return;
      glLayer = maplibreGL({
        style,
        attributionControl: {
          customAttribution:
            'OpenFreeMap &copy; <a href="https://www.openmaptiles.org/">OpenMapTiles</a> Data from <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>',
        },
      }).addTo(map);
      appliedMapTheme = initialTheme;
      if (theme.value !== initialTheme) {
        void applyMapStyle(theme.value);
      }
    })
    .catch(() => {});
});

onBeforeUnmount(() => {
  disposed = true;
  if (markerRafId !== null) {
    cancelAnimationFrame(markerRafId);
    markerRafId = null;
  }
  styledActiveStopId = null;
  pendingFocusStopId = null;
  cancelPopupClose();
  hoveredPopupMarker = null;
  pinnedPopupMarker = null;
  appliedMapTheme = null;
  resizeObserver?.disconnect();
  resizeObserver = undefined;
  markers.clear();
  stopMarkers.clear();
  userLayer?.clearLayers();
  stopLayer?.clearLayers();
  activeStopLayer?.clearLayers();
  routeLayer?.clearLayers();
  routeCasingLayer?.clearLayers();
  userLayer = undefined;
  stopLayer = undefined;
  activeStopLayer = undefined;
  stopRenderer = undefined;
  routeLayer = undefined;
  routeCasingLayer = undefined;
  routeRenderer = undefined;
  glLayer = undefined;
  map?.remove();
  map = undefined;
});

watch(theme, (next) => {
  syncRoutes();
  if (glLayer && appliedMapTheme !== next) {
    void applyMapStyle(next);
  }
});

watch(operator, (op) => {
  fittedCode = null;
  fittedStopId = null;
  pendingFocusStopId = null;
  if (map) setViewWithInset(op.center, 12);
});

watch([visibleCorridorLines, selectedCorridor, () => props.mode], () => {
  syncRoutes();
  fitSelectedRoute();
});
watch(
  [visibleStops, selectedCorridor, corridorColors, () => props.mode],
  syncStops,
);
watch(
  [() => props.activeStopId, () => props.mode, visibleStops, selectedStop],
  () => {
    updateActiveStopMarkerStyle();
    syncActiveStopRing();
    fitActiveStop();
  },
);
watch(
  [busesVersion, selectedCorridor, selectedDirection, selectedStopId, corridorColors, corridors],
  scheduleSyncMarkers,
);
watch(userLocation, syncUserLocation, { deep: true });
watch(locationPanSeq, (seq) => {
  if (seq > 0 && map && userLocation.value) {
    setViewWithInset(
      [userLocation.value.lat, userLocation.value.lng],
      Math.max(map.getZoom(), 15),
    );
  }
});
watch(
  () => props.focusBus,
  (focus) => {
    if (!map || !focus) return;
    const entry = markers.get(focus.id);
    if (!entry) return;
    setViewWithInset(entry.marker.getLatLng(), Math.max(map.getZoom(), 16));
    entry.marker.openTooltip();
  },
);
watch(
  () => props.focusStop,
  (focus) => {
    if (!map || !focus) return;
    const marker = stopMarkers.get(focus.id);
    if (!marker) {
      pendingFocusStopId = focus.id;
      return;
    }
    pendingFocusStopId = null;
    setViewWithInset(marker.getLatLng(), Math.max(map.getZoom(), 15));
    if (!(props.mode === "stop" && focus.id === props.activeStopId)) {
      cancelPopupClose();
      pinnedPopupMarker = marker;
      hoveredPopupMarker = marker;
      marker.openPopup();
    }
  },
);
</script>

<template>
  <div class="map-wrap">
    <div ref="mapEl" class="map"></div>

    <div class="map-top-right">
      <div class="map-controls-stack">
        <div class="map-zoom-group" role="group" aria-label="Kontrol zoom peta">
          <button
            type="button"
            class="map-zoom-btn"
            :disabled="!canZoomIn"
            aria-label="Perbesar peta"
            title="Perbesar"
            @click="zoomIn"
          >
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true">
              <path d="M12 5v14M5 12h14" />
            </svg>
          </button>
          <span class="map-zoom-sep" aria-hidden="true"></span>
          <button
            type="button"
            class="map-zoom-btn"
            :disabled="!canZoomOut"
            aria-label="Perkecil peta"
            title="Perkecil"
            @click="zoomOut"
          >
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true">
              <path d="M5 12h14" />
            </svg>
          </button>
        </div>

        <button
          type="button"
          class="map-loc-btn"
          :class="{
            active: trackingLocation,
            locating: refreshingLocation || (trackingLocation && !userLocation),
          }"
          :aria-label="trackingLocation ? 'Perbarui lokasi saya' : 'Tampilkan lokasi saya'"
          :title="trackingLocation ? 'Perbarui lokasi saya' : 'Tampilkan lokasi saya'"
          @click="store.locateUser()"
        >
          <svg viewBox="0 0 24 24" width="21" height="21" fill="currentColor" aria-hidden="true">
            <path d="M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8zm8.94 3A8.994 8.994 0 0 0 13 3.06V1h-2v2.06A8.994 8.994 0 0 0 3.06 11H1v2h2.06A8.994 8.994 0 0 0 11 20.94V23h2v-2.06A8.994 8.994 0 0 0 20.94 13H23v-2h-2.06zM12 19a7 7 0 1 1 0-14 7 7 0 0 1 0 14z" />
          </svg>
        </button>
      </div>

      <div v-if="locationError" class="map-loc-err">{{ locationError }}</div>
    </div>
  </div>
</template>

<style scoped>
.map-wrap {
  position: relative;
  width: 100%;
  height: 100%;
}
.map {
  width: 100%;
  height: 100%;
}
.map-top-right {
  position: absolute;
  top: 12px;
  right: 12px;
  z-index: 800;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 8px;
  pointer-events: none;
}
.map-controls-stack {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}
.map-zoom-group {
  box-sizing: border-box;
  display: inline-flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  width: 40px;
  padding: 4px;
  border-radius: 999px;
  border: 1.5px solid var(--border-strong);
  background: var(--bg-surface);
  box-shadow: var(--shadow-float);
  pointer-events: auto;
}
.map-zoom-btn {
  box-sizing: border-box;
  appearance: none;
  border: none;
  padding: 0;
  margin: 0;
  width: 30px;
  height: 30px;
  border-radius: 50%;
  background: transparent;
  color: var(--text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  -webkit-tap-highlight-color: initial;
  transition: background-color 140ms ease, color 140ms ease;
}
.map-zoom-sep {
  width: 18px;
  height: 1px;
  margin: 1px auto;
  background: var(--border-strong);
  flex-shrink: 0;
}
.map-zoom-btn:hover:not(:disabled) {
  background: var(--bg-surface-hover);
  color: var(--text-strong);
}
.map-zoom-btn:disabled {
  opacity: 0.38;
  cursor: default;
}
.map-loc-btn {
  box-sizing: border-box;
  pointer-events: auto;
  appearance: none;
  width: 48px;
  height: 48px;
  border-radius: 50%;
  border: 1.5px solid var(--border-strong);
  background: var(--bg-surface);
  color: var(--text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  box-shadow: var(--shadow-float);
  transition: background-color 140ms ease, color 140ms ease, border-color 140ms ease, transform 120ms ease;
}
.map-loc-btn:hover {
  background: var(--bg-surface-hover);
  color: var(--text-strong);
}
.map-loc-btn:active {
  transform: scale(0.96);
}
.map-loc-btn.active {
  background: var(--city-accent);
  border-color: transparent;
  color: var(--city-accent-fg);
}
.map-loc-btn.active:hover {
  background: var(--city-accent-hover);
  color: var(--city-accent-fg);
}
.map-loc-btn.locating svg {
  animation: loc-pulse 1.2s ease-in-out infinite;
}
@keyframes loc-pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.4;
  }
}
.map-loc-err {
  pointer-events: auto;
  max-width: 240px;
  text-align: right;
  background: var(--err-bg);
  border: 1px solid var(--err-border);
  color: var(--err-text);
  font-size: 0.74rem;
  padding: 6px 10px;
  border-radius: 10px;
}

@media (max-width: 768px) {
  .map-top-right {
    top: 10px;
    right: 10px;
  }
  .map-zoom-group {
    width: 44px;
  }
  .map-zoom-btn {
    width: 34px;
    height: 34px;
  }
  .map-loc-btn {
    width: 50px;
    height: 50px;
  }
}
</style>

<style>
.bus-arrow-icon {
  background: transparent;
  border: none;
}
.bus-arrow {
  width: 34px;
  height: 34px;
  display: flex;
  align-items: center;
  justify-content: center;
  filter: drop-shadow(0 1px 3px rgba(0, 0, 0, 0.65));
}
.bus-arrow svg {
  transition: transform 120ms ease;
}
.bus-arrow-icon:hover .bus-arrow svg {
  transform: scale(1.18);
}
.bus-arrow.stale {
  opacity: 0.45;
  filter: saturate(0.4) drop-shadow(0 1px 2px rgba(0, 0, 0, 0.5));
}
.user-loc-icon {
  background: transparent;
  border: none;
}
.user-loc-dot {
  width: 14px;
  height: 14px;
  margin: 3px;
  border-radius: 50%;
  background: #0284c7;
  border: 2.5px solid #ffffff;
  box-shadow: 0 0 0 4px rgba(56, 189, 248, 0.3), 0 1px 5px rgba(0, 0, 0, 0.6);
}
</style>
