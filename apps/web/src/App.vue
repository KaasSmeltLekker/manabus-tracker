<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, onBeforeUnmount, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { apiUrl } from "./config";
import { prefetchMapStyle } from "./mapStyle";
import { CITY_PREF, currentRoute, goCorridor, goStop } from "./router";
import {
  contrastText,
  corridorBadgeLabel,
  corridorCode,
  corridorDisplayName,
  lookupCorridorValue,
  normalizeColor,
  useBusStore,
} from "./stores/buses";
import BrandHeader from "./components/BrandHeader.vue";
import HomeView from "./components/HomeView.vue";
import CorridorList from "./components/CorridorList.vue";

const loadMapView = () => import("./components/MapView.vue");
const loadRoutePanel = () => import("./components/RoutePanel.vue");
const loadStopArrivalPanel = () => import("./components/StopArrivalPanel.vue");
const MapView = defineAsyncComponent(loadMapView);
const RoutePanel = defineAsyncComponent(loadRoutePanel);
const StopArrivalPanel = defineAsyncComponent(loadStopArrivalPanel);
const AdminView = defineAsyncComponent(() => import("./components/AdminView.vue"));

const store = useBusStore();
const {
  corridors,
  stops,
  selectedStop,
  corridorColors,
  cityPopup,
  siteConfigPref,
  pref,
  connected,
  hasConnectedOnce,
} = storeToRefs(store);

const dismissedPopupSig = ref<Record<string, string>>({});
let popupBackdropMouseDown = false;

const popupSig = computed(() => {
  const p = cityPopup.value;
  if (!p) return "";
  return JSON.stringify([p.title, p.text, p.image_url]);
});

const activeCityPopup = computed(() => {
  const r = currentRoute.value;
  if (r.name === "home" || r.name === "admin") return null;
  if (siteConfigPref.value !== pref.value) return null;
  const p = cityPopup.value;
  if (!p || !p.enabled) return null;
  if (!p.title.trim() && !p.text.trim() && !p.image_url.trim()) return null;
  if (dismissedPopupSig.value[pref.value] === popupSig.value) return null;
  return p;
});

function dismissCityPopup() {
  if (!pref.value) return;
  dismissedPopupSig.value = {
    ...dismissedPopupSig.value,
    [pref.value]: popupSig.value,
  };
}

function onPopupBackdropDown(ev: MouseEvent) {
  popupBackdropMouseDown = ev.target === ev.currentTarget;
}

function onPopupBackdropClick(ev: MouseEvent) {
  const sameBackdrop = popupBackdropMouseDown && ev.target === ev.currentTarget;
  popupBackdropMouseDown = false;
  if (sameBackdrop) {
    dismissCityPopup();
  }
}

function onPopupKeyDown(ev: KeyboardEvent) {
  if (ev.key === "Escape" && activeCityPopup.value) {
    dismissCityPopup();
  }
}

const focusBus = ref<{ id: string; seq: number } | null>(null);
const focusStop = ref<{ id: string; seq: number } | null>(null);
const activeBusId = ref<string | null>(null);
const sheetCollapsed = ref(false);
const sheetDragging = ref(false);
const sheetDragY = ref(0);
let dragStartY = 0;
let dragStartTime = 0;
let dragPointerId: number | null = null;
let suppressSheetClick = false;
let restoreStartY: number | null = null;
let suppressRestoreClick = false;

const viewportHeight = ref(
  typeof window !== "undefined" ? window.innerHeight : 800,
);

const mq =
  typeof window !== "undefined" ? window.matchMedia("(min-width: 769px)") : null;
const isDesktop = ref(mq ? mq.matches : true);

function onMediaChange(e: MediaQueryListEvent) {
  isDesktop.value = e.matches;
}

function onResize() {
  viewportHeight.value = window.innerHeight;
}

const mobileBottomInset = computed(() => {
  if (
    isDesktop.value ||
    currentRoute.value.name === "home" ||
    currentRoute.value.name === "admin" ||
    currentRoute.value.name === "city" ||
    sheetCollapsed.value
  ) {
    return 0;
  }
  return Math.round(viewportHeight.value * 0.55);
});

const activeCorridorColor = computed(() => {
  const r = currentRoute.value;
  if (r.name !== "corridor") return "#0284c7";
  const found = corridors.value.find((c) => corridorCode(c) === r.code);
  return (
    normalizeColor(found?.color) ??
    lookupCorridorValue(corridorColors.value, r.code) ??
    "#0284c7"
  );
});

const sheetStyle = computed(() =>
  !sheetCollapsed.value && sheetDragY.value > 0
    ? { transform: `translateY(${sheetDragY.value}px)` }
    : undefined,
);

function onSheetPointerDown(e: PointerEvent) {
  if (sheetCollapsed.value) return;
  dragPointerId = e.pointerId;
  dragStartY = e.clientY;
  dragStartTime = performance.now();
  sheetDragY.value = 0;
  sheetDragging.value = true;
  (e.currentTarget as HTMLElement | null)?.setPointerCapture?.(e.pointerId);
}

function onSheetPointerMove(e: PointerEvent) {
  if (!sheetDragging.value || e.pointerId !== dragPointerId) return;
  sheetDragY.value = Math.max(0, e.clientY - dragStartY);
}

function onSheetPointerUp(e: PointerEvent) {
  if (!sheetDragging.value || e.pointerId !== dragPointerId) return;
  const deltaY = Math.max(0, e.clientY - dragStartY);
  const velocity = deltaY / Math.max(1, performance.now() - dragStartTime);
  sheetDragging.value = false;
  sheetDragY.value = 0;
  dragPointerId = null;
  if (deltaY >= 10) {
    suppressSheetClick = true;
    setTimeout(() => {
      suppressSheetClick = false;
    }, 0);
  }
  if (deltaY > 60 || (deltaY > 24 && velocity > 0.35)) {
    sheetCollapsed.value = true;
  }
}

function onSheetPointerCancel() {
  sheetDragging.value = false;
  sheetDragY.value = 0;
  dragPointerId = null;
}

function onSheetBarClick() {
  if (suppressSheetClick) return;
  sheetCollapsed.value = true;
}

function onRestorePointerDown(e: PointerEvent) {
  restoreStartY = e.clientY;
}

function onRestorePointerUp(e: PointerEvent) {
  if (restoreStartY !== null && restoreStartY - e.clientY > 24) {
    suppressRestoreClick = true;
    setTimeout(() => {
      suppressRestoreClick = false;
    }, 0);
    sheetCollapsed.value = false;
  }
  restoreStartY = null;
}

function onRestoreClick() {
  if (suppressRestoreClick) return;
  sheetCollapsed.value = false;
}

function onFocusBus(id: string) {
  activeBusId.value = id;
  focusBus.value = { id, seq: (focusBus.value?.seq ?? 0) + 1 };
}

function onFocusStop(id: string) {
  focusStop.value = { id, seq: (focusStop.value?.seq ?? 0) + 1 };
}

function onSelectSearchStop(payload: { code: string; stopId: string }) {
  onFocusStop(payload.stopId);
  goCorridor(payload.code);
}

function onOpenStopArrival(stopId: string) {
  goStop(stopId);
}

const mapMode = computed<"all" | "single" | "stop">(() => {
  if (currentRoute.value.name === "corridor") return "single";
  if (currentRoute.value.name === "stop") return "stop";
  return "all";
});

const activeStopId = computed(() =>
  currentRoute.value.name === "stop" ? currentRoute.value.stopId : null,
);

let prefetchIdleId: number | null = null;
let prefetchTimerId: ReturnType<typeof setTimeout> | null = null;
let mobileMapWarmed = false;
let mobileMapIdleId: number | null = null;
let mobileMapTimerId: ReturnType<typeof setTimeout> | null = null;

function shouldAvoidBackgroundPrefetch(): boolean {
  if (typeof navigator === "undefined") return false;
  const conn = (
    navigator as Navigator & {
      connection?: { saveData?: boolean; effectiveType?: string };
    }
  ).connection;
  return Boolean(
    conn?.saveData || conn?.effectiveType === "slow-2g" || conn?.effectiveType === "2g",
  );
}

onMounted(() => {
  mq?.addEventListener("change", onMediaChange);
  window.addEventListener("resize", onResize);
  window.addEventListener("keydown", onPopupKeyDown);
  store.connectLive();
  if (currentRoute.value.name === "home") {
    void fetch(apiUrl("/api/health"), { cache: "no-store" }).catch(() => {});
  }
  if (typeof window !== "undefined" && !shouldAvoidBackgroundPrefetch()) {
    const runPrefetch = () => {
      prefetchIdleId = null;
      prefetchTimerId = null;
      void loadRoutePanel();
      void loadStopArrivalPanel();
      if (isDesktop.value) {
        void loadMapView();
        prefetchMapStyle();
      }
    };
    if (typeof window.requestIdleCallback === "function") {
      prefetchIdleId = window.requestIdleCallback(runPrefetch, { timeout: 1500 });
    } else {
      prefetchTimerId = setTimeout(runPrefetch, 600);
    }
  }
});

onBeforeUnmount(() => {
  delete document.documentElement.dataset.city;
  if (
    prefetchIdleId !== null &&
    typeof window !== "undefined" &&
    typeof window.cancelIdleCallback === "function"
  ) {
    window.cancelIdleCallback(prefetchIdleId);
    prefetchIdleId = null;
  }
  if (prefetchTimerId !== null) {
    clearTimeout(prefetchTimerId);
    prefetchTimerId = null;
  }
  if (
    mobileMapIdleId !== null &&
    typeof window !== "undefined" &&
    typeof window.cancelIdleCallback === "function"
  ) {
    window.cancelIdleCallback(mobileMapIdleId);
    mobileMapIdleId = null;
  }
  if (mobileMapTimerId !== null) {
    clearTimeout(mobileMapTimerId);
    mobileMapTimerId = null;
  }
  mq?.removeEventListener("change", onMediaChange);
  window.removeEventListener("resize", onResize);
  window.removeEventListener("keydown", onPopupKeyDown);
  store.disconnectLive();
});

watch([currentRoute, corridors, isDesktop], ([route, list, desktop]) => {
  if (
    desktop ||
    mobileMapWarmed ||
    route.name !== "city" ||
    list.length === 0 ||
    shouldAvoidBackgroundPrefetch() ||
    typeof window === "undefined"
  ) {
    return;
  }
  mobileMapWarmed = true;
  const warmMobileMap = () => {
    mobileMapIdleId = null;
    mobileMapTimerId = null;
    void loadMapView();
    prefetchMapStyle();
  };
  if (typeof window.requestIdleCallback === "function") {
    mobileMapIdleId = window.requestIdleCallback(warmMobileMap, { timeout: 3000 });
  } else {
    mobileMapTimerId = setTimeout(warmMobileMap, 1500);
  }
});

watch(
  currentRoute,
  (route, prevRoute) => {
    if (route.name === "home" || route.name === "admin") {
      delete document.documentElement.dataset.city;
      if (route.name === "home") {
        dismissedPopupSig.value = {};
      }
    } else {
      document.documentElement.dataset.city = route.city;
      if (prevRoute?.name === "home") {
        void store.fetchSiteConfig(CITY_PREF[route.city]);
      }
    }
    if (typeof document !== "undefined") {
      const themeMeta = document.querySelector('meta[name="theme-color"]');
      if (themeMeta) {
        const nextThemeColor =
          route.name === "admin"
            ? "#7f1d1d"
            : route.name !== "home" && route.city === "yogyakarta"
              ? "#eab308"
              : "#715d80";
        themeMeta.setAttribute("content", nextThemeColor);
      }
    }
    activeBusId.value = null;
    sheetCollapsed.value = false;
    sheetDragging.value = false;
    sheetDragY.value = 0;
  },
  { immediate: true },
);

watch(
  [currentRoute, corridors, stops],
  ([route, list]) => {
    if (route.name === "home" || route.name === "admin") {
      store.setSelectedStop(null);
      store.setSelectedCorridor(null);
      document.title =
        route.name === "admin"
          ? "Admin Panel · ManaBus?"
          : "ManaBus? · Pelacak transum berbasis web";
      return;
    }

    store.loadCity(CITY_PREF[route.city]);

    if (route.name === "stop") {
      store.setSelectedCorridor(null);
      store.setSelectedStop(route.stopId);
      const stop = store.selectedStop;
      if (stop) {
        store.ensureStopLegs(stop);
        document.title = `${stop.sh_name} · ManaBus?`;
      } else {
        document.title = "Kedatangan Halte · ManaBus?";
      }
    } else if (route.name === "corridor") {
      store.setSelectedStop(null);
      store.setSelectedCorridor(route.code);
      const found = list.find((c) => corridorCode(c) === route.code);
      const label = corridorBadgeLabel(route.code);
      document.title = found
        ? `${label} ${corridorDisplayName(found)} · ManaBus?`
        : `${label} · ManaBus?`;
    } else if (route.name === "city") {
      store.setSelectedStop(null);
      store.setSelectedCorridor(null);
      document.title = `${store.operator.label} · ManaBus?`;
    } else if (route.name === "all") {
      store.setSelectedStop(null);
      store.setSelectedCorridor(null);
      document.title = `Semua Rute ${store.operator.label} · ManaBus?`;
    }
  },
  { immediate: true },
);
</script>

<template>
  <div v-if="currentRoute.name === 'home'" class="shell home-shell">
    <HomeView />
  </div>

  <div v-else-if="currentRoute.name === 'admin'" class="shell admin-shell">
    <BrandHeader />
    <AdminView />
  </div>

  <div v-else-if="isDesktop" class="shell desktop">
    <div class="map-pane">
      <MapView
        :mode="mapMode"
        :focus-bus="focusBus"
        :focus-stop="focusStop"
        :active-stop-id="activeStopId"
        @open-stop-arrival="onOpenStopArrival"
      />
    </div>
    <aside class="sidebar">
      <BrandHeader />
      <div v-if="hasConnectedOnce && !connected" class="conn-banner" role="status">
        Koneksi terputus — menghubungkan ulang…
      </div>
      <div class="sidebar-body">
        <StopArrivalPanel
          v-if="currentRoute.name === 'stop'"
          :stop-id="currentRoute.stopId"
          :active-bus-id="activeBusId"
          @focus-bus="onFocusBus"
          @recenter-stop="onFocusStop"
        />
        <RoutePanel
          v-else-if="currentRoute.name === 'corridor'"
          :code="currentRoute.code"
          :active-bus-id="activeBusId"
          @focus-bus="onFocusBus"
          @focus-stop="onFocusStop"
        />
        <CorridorList v-else @select-stop="onSelectSearchStop" />
      </div>
    </aside>
  </div>

  <div v-else class="shell mobile" :class="{ 'sheet-hidden': sheetCollapsed }">
    <BrandHeader />
    <div v-if="hasConnectedOnce && !connected" class="conn-banner" role="status">
      Koneksi terputus — menghubungkan ulang…
    </div>

    <div v-if="currentRoute.name === 'city'" class="mobile-list-pane">
      <CorridorList show-all-routes-button @select-stop="onSelectSearchStop" />
    </div>

    <div v-else class="mobile-stage">
      <div class="map-pane">
        <MapView
          :mode="mapMode"
          :focus-bus="focusBus"
          :focus-stop="focusStop"
          :active-stop-id="activeStopId"
          :bottom-inset="mobileBottomInset"
          @open-stop-arrival="onOpenStopArrival"
        />
      </div>
      <section
        class="mobile-sheet"
        :class="{ collapsed: sheetCollapsed, dragging: sheetDragging }"
        :style="sheetStyle"
        :aria-hidden="sheetCollapsed"
      >
        <button
          type="button"
          class="sheet-bar"
          :aria-expanded="!sheetCollapsed"
          aria-label="Sembunyikan panel rute"
          @pointerdown="onSheetPointerDown"
          @pointermove="onSheetPointerMove"
          @pointerup="onSheetPointerUp"
          @pointercancel="onSheetPointerCancel"
          @click="onSheetBarClick"
        >
          <span class="sheet-handle" aria-hidden="true"></span>
        </button>
        <div class="sheet-body">
          <StopArrivalPanel
            v-if="currentRoute.name === 'stop'"
            :stop-id="currentRoute.stopId"
            :active-bus-id="activeBusId"
            @focus-bus="onFocusBus"
            @recenter-stop="onFocusStop"
          />
          <RoutePanel
            v-else-if="currentRoute.name === 'corridor'"
            :code="currentRoute.code"
            :active-bus-id="activeBusId"
            @focus-bus="onFocusBus"
            @focus-stop="onFocusStop"
          />
          <CorridorList v-else @select-stop="onSelectSearchStop" />
        </div>
      </section>
      <button
        v-if="sheetCollapsed"
        type="button"
        class="sheet-restore-btn"
        :aria-label="currentRoute.name === 'corridor' ? 'Tampilkan info rute' : currentRoute.name === 'stop' ? 'Tampilkan kedatangan halte' : 'Tampilkan daftar rute'"
        @pointerdown="onRestorePointerDown"
        @pointerup="onRestorePointerUp"
        @click="onRestoreClick"
      >
        <template v-if="currentRoute.name === 'corridor'">
          <span
            class="restore-badge"
            :style="{
              background: activeCorridorColor,
              color: contrastText(activeCorridorColor),
            }"
          >
            {{ corridorBadgeLabel(currentRoute.code) }}
          </span>
          <span>Tampilkan rute</span>
        </template>
        <template v-else-if="currentRoute.name === 'stop'">
          <svg class="restore-pin" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M12 21s-6-5.33-6-10a6 6 0 1 1 12 0c0 4.67-6 10-6 10z" />
            <circle cx="12" cy="11" r="2.2" />
          </svg>
          <span class="restore-stop-name">{{ selectedStop?.sh_name ?? "Kedatangan halte" }}</span>
        </template>
        <span v-else>Daftar rute</span>
        <svg viewBox="0 0 20 20" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M5 12.5l5-5 5 5" />
        </svg>
      </button>
    </div>
  </div>

  <Teleport to="body">
    <div
      v-if="activeCityPopup"
      class="city-popup-backdrop"
      @mousedown="onPopupBackdropDown"
      @click="onPopupBackdropClick"
    >
      <div
        class="city-popup-card"
        role="dialog"
        aria-modal="true"
        :aria-label="activeCityPopup.title || store.operator.label"
      >
        <div class="city-popup-head">
          <h2 class="city-popup-title">
            {{ activeCityPopup.title || store.operator.label }}
          </h2>
          <button
            type="button"
            class="city-popup-close"
            aria-label="Tutup popup"
            @click="dismissCityPopup()"
          >
            ×
          </button>
        </div>
        <div class="city-popup-body">
          <img
            v-if="activeCityPopup.image_url"
            :src="activeCityPopup.image_url"
            :alt="activeCityPopup.title || store.operator.label"
            class="city-popup-img"
          />
          <p v-if="activeCityPopup.text" class="city-popup-text">
            {{ activeCityPopup.text }}
          </p>
        </div>
        <div class="city-popup-foot">
          <button
            type="button"
            class="city-popup-ok"
            @click="dismissCityPopup()"
          >
            Tutup
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style>
@font-face {
  font-family: "DM Sans";
  src: url("/fonts/DMSans-Variable.ttf") format("truetype");
  font-weight: 100 1000;
  font-style: normal;
  font-display: swap;
}
@font-face {
  font-family: "DM Sans";
  src: url("/fonts/DMSans-Italic-Variable.ttf") format("truetype");
  font-weight: 100 1000;
  font-style: italic;
  font-display: swap;
}
@font-face {
  font-family: "Fira Sans";
  src: url("/fonts/FiraSans-Regular.ttf") format("truetype");
  font-weight: 400;
  font-style: normal;
  font-display: swap;
}
@font-face {
  font-family: "Fira Sans";
  src: url("/fonts/FiraSans-Italic.ttf") format("truetype");
  font-weight: 400;
  font-style: italic;
  font-display: swap;
}
@font-face {
  font-family: "Fira Sans";
  src: url("/fonts/FiraSans-Medium.ttf") format("truetype");
  font-weight: 500;
  font-style: normal;
  font-display: swap;
}
@font-face {
  font-family: "Fira Sans";
  src: url("/fonts/FiraSans-SemiBold.ttf") format("truetype");
  font-weight: 600;
  font-style: normal;
  font-display: swap;
}
@font-face {
  font-family: "Fira Sans";
  src: url("/fonts/FiraSans-Bold.ttf") format("truetype");
  font-weight: 700;
  font-style: normal;
  font-display: swap;
}
@font-face {
  font-family: "Montserrat";
  src: url("/fonts/Montserrat-Regular.ttf") format("truetype");
  font-weight: 400;
  font-style: normal;
  font-display: swap;
}
@font-face {
  font-family: "Montserrat";
  src: url("/fonts/Montserrat-Bold.ttf") format("truetype");
  font-weight: 700;
  font-style: normal;
  font-display: swap;
}
:root {
  color-scheme: dark;
  --font-ui: "DM Sans", system-ui, -apple-system, sans-serif;
  --bg-app: #0f141a;
  --bg-surface: #18202a;
  --bg-surface-hover: #1f2936;
  --bg-surface-active: #243040;
  --bg-accent-card: #18202a;
  --bg-accent-card-hover: #1f2936;
  --bg-elevated: #18202a;
  --bg-elevated-hover: #212c3a;
  --bg-popup: #18202a;
  --bg-attribution: #0f141a;
  --bg-map: #1a1f26;
  --bg-chip: #1e2936;
  --bg-chip-hover: #273547;
  --text-chip: #f1f5f9;
  --text-chip-hover: #ffffff;
  --bg-chip-stale: #131a22;
  --bg-chip-stale-hover: #1b2430;
  --text-chip-stale: #94a3b8;
  --text-chip-stale-hover: #e2e8f0;
  --dot-live: #22c55e;
  --dot-idle: #f59e0b;
  --shadow-chip: none;
  --bg-control-soft: #1b2430;
  --bg-recenter: #18202a;
  --bg-recenter-hover: #222e3d;
  --bg-swap: #222e3d;
  --bg-swap-hover: #2c3b4e;
  --border-swap: #314154;
  --text-swap: #f1f5f9;
  --text-swap-hover: #ffffff;
  --border-subtle: #263241;
  --border-hairline: #263241;
  --border-strong: #314154;
  --border-hover: #42566e;
  --border-popover: #334459;
  --text-primary: #eef2f7;
  --text-strong: #ffffff;
  --text-secondary: #cbd5e1;
  --text-muted: #8b9db3;
  --text-origin: #9cb0c7;
  --text-stale-tip: #fbbf24;
  --text-link: #60a5fa;
  --rail-line: #3b4c61;
  --ep-line: #3b4c61;
  --handle-bg: #3b4c61;
  --handle-bg-hover: #586e8a;
  --scroll-thumb: #2d3b4d;
  --scroll-track: #141b24;
  --scroll-indicator: #475c75;
  --shadow-float: 0 4px 14px rgba(0, 0, 0, 0.48);
  --shadow-popover: 0 12px 28px rgba(0, 0, 0, 0.55);
  --shadow-dropdown: 0 20px 28px -4px rgba(0, 0, 0, 0.65), 0 8px 12px -4px rgba(0, 0, 0, 0.45);
  --shadow-sheet: 0 -8px 28px rgba(0, 0, 0, 0.52);
  --shadow-restore: 0 8px 24px rgba(0, 0, 0, 0.55);
  --err-bg: #2a1418;
  --err-text: #f87171;
  --err-border: #5f242c;
  --active-ring-border: #c4b5d0;
  --active-ring-bg: rgba(167, 139, 250, 0.22);
  --menu-hover: #222e3d;
  --switch-track: #314154;
  --city-accent: #715d80;
  --city-accent-hover: #826d93;
  --city-accent-fg: #ffffff;
}
:root[data-theme="light"] {
  color-scheme: light;
  --bg-app: #f4f6f9;
  --bg-surface: #ffffff;
  --bg-surface-hover: #f1f5f9;
  --bg-surface-active: #f3eef7;
  --bg-accent-card: #ffffff;
  --bg-accent-card-hover: #f1f5f9;
  --bg-elevated: #ffffff;
  --bg-elevated-hover: #ffffff;
  --bg-popup: #ffffff;
  --bg-attribution: #ffffff;
  --bg-map: #e9ecef;
  --bg-chip: #ffffff;
  --bg-chip-hover: #f1f5f9;
  --text-chip: #0f172a;
  --text-chip-hover: #0f172a;
  --bg-chip-stale: #e6ebf1;
  --bg-chip-stale-hover: #dce3ec;
  --text-chip-stale: #64748b;
  --text-chip-stale-hover: #334155;
  --dot-live: #10b981;
  --dot-idle: #f59e0b;
  --shadow-chip: 0 1px 2px rgba(15, 23, 42, 0.05);
  --bg-control-soft: #f1f5f9;
  --bg-recenter: #ffffff;
  --bg-recenter-hover: #f1f5f9;
  --bg-swap: #f1f5f9;
  --bg-swap-hover: #e2e8f0;
  --border-swap: rgba(15, 23, 42, 0.12);
  --text-swap: #1b202b;
  --text-swap-hover: #0f172a;
  --border-subtle: rgba(15, 23, 42, 0.09);
  --border-hairline: #e5e5e5;
  --border-strong: rgba(15, 23, 42, 0.14);
  --border-hover: rgba(15, 23, 42, 0.24);
  --border-popover: #cbd5e1;
  --text-primary: #1e293b;
  --text-strong: #0f172a;
  --text-secondary: #334155;
  --text-muted: #64748b;
  --text-origin: #475569;
  --text-stale-tip: #b45309;
  --text-link: #0284c7;
  --rail-line: #cbd5e1;
  --ep-line: rgba(15, 23, 42, 0.22);
  --handle-bg: rgba(15, 23, 42, 0.22);
  --handle-bg-hover: rgba(15, 23, 42, 0.42);
  --scroll-thumb: rgba(15, 23, 42, 0.18);
  --scroll-track: rgba(15, 23, 42, 0.1);
  --scroll-indicator: rgba(15, 23, 42, 0.36);
  --shadow-float: 0 4px 14px rgba(15, 23, 42, 0.14);
  --shadow-popover: 0 10px 28px rgba(15, 23, 42, 0.16);
  --shadow-dropdown: 0 20px 25px -5px rgba(15, 23, 42, 0.18), 0 8px 10px -6px rgba(15, 23, 42, 0.12);
  --shadow-sheet: 0 -6px 24px rgba(15, 23, 42, 0.14);
  --shadow-restore: 0 6px 20px rgba(15, 23, 42, 0.18);
  --err-bg: #ffffff;
  --err-text: #dc2626;
  --err-border: rgba(220, 38, 38, 0.35);
  --active-ring-border: #715d80;
  --active-ring-bg: rgba(113, 93, 128, 0.22);
  --menu-hover: rgba(15, 23, 42, 0.06);
  --switch-track: rgba(15, 23, 42, 0.18);
}
:root[data-city="yogyakarta"] {
  --font-ui: "Fira Sans", system-ui, -apple-system, sans-serif;
  --city-accent: #eab308;
  --city-accent-hover: #facc15;
  --city-accent-fg: #0f172a;
  --active-ring-border: #eab308;
  --active-ring-bg: rgba(234, 179, 8, 0.22);
}
html,
body,
#app {
  margin: 0;
  height: 100%;
  overflow: hidden;
  overscroll-behavior: none;
  background: var(--bg-app);
  color: var(--text-primary);
  font-family: var(--font-ui);
  -webkit-tap-highlight-color: transparent;
}
button,
a {
  touch-action: manipulation;
}
button {
  font-family: inherit;
}
.shell {
  width: 100vw;
  height: 100vh;
  height: 100dvh;
  overflow: hidden;
  font-family: var(--font-ui);
}
.shell.home-shell,
.shell.admin-shell {
  display: flex;
  flex-direction: column;
  background: var(--bg-app);
}
.shell.desktop {
  display: flex;
  flex-direction: row;
}
.shell.mobile {
  display: flex;
  flex-direction: column;
  background: var(--bg-app);
}
.map-pane {
  flex: 1;
  min-width: 0;
  min-height: 0;
  height: 100%;
  position: relative;
}
.mobile-stage {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.shell.mobile .map-pane {
  width: 100%;
  height: 100%;
}
.sidebar {
  width: 360px;
  flex-shrink: 0;
  height: 100%;
  background: var(--bg-app);
  border-left: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.sidebar-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.mobile-list-pane {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding-bottom: env(safe-area-inset-bottom);
  background: var(--bg-app);
}
.mobile-sheet {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 1000;
  background: var(--bg-app);
  border-radius: 20px 20px 0 0;
  box-shadow: var(--shadow-sheet);
  border-top: 1px solid var(--border-subtle);
  height: 55dvh;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  padding-bottom: env(safe-area-inset-bottom);
  transform: translateY(0);
  transition: transform 240ms cubic-bezier(0.22, 1, 0.36, 1);
  will-change: transform;
}
.mobile-sheet.dragging {
  transition: none;
}
.mobile-sheet.collapsed {
  transform: translateY(100%);
  pointer-events: none;
}
.sheet-bar {
  position: relative;
  appearance: none;
  border: none;
  background: transparent;
  height: 36px;
  padding: 0 14px;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
  touch-action: none;
  user-select: none;
}
.sheet-bar:hover .sheet-handle,
.sheet-bar:active .sheet-handle {
  background: var(--handle-bg-hover);
}
.sheet-handle {
  width: 44px;
  height: 4.5px;
  border-radius: 999px;
  background: var(--handle-bg);
  transition: background-color 140ms ease;
}
.sheet-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.sheet-restore-btn {
  position: absolute;
  left: 50%;
  bottom: calc(18px + env(safe-area-inset-bottom));
  transform: translateX(-50%);
  z-index: 1001;
  appearance: none;
  border: 1px solid var(--border-strong);
  background: var(--bg-elevated);
  color: var(--text-primary);
  border-radius: 999px;
  padding: 10px 16px;
  font-size: 0.84rem;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  box-shadow: var(--shadow-restore);
  cursor: pointer;
  touch-action: none;
  white-space: nowrap;
}
.sheet-restore-btn:active {
  background: var(--bg-elevated-hover);
}
.restore-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  min-width: 24px;
  height: 20px;
  padding: 0 6px;
  border-radius: 6px;
  font-size: 0.72rem;
  font-weight: 700;
  line-height: 1;
}
.restore-pin {
  color: var(--active-ring-border);
  flex-shrink: 0;
}
.restore-stop-name {
  max-width: 65vw;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.leaflet-container {
  background: var(--bg-map);
  font-family: var(--font-ui) !important;
}
.leaflet-control-attribution {
  background: var(--bg-attribution) !important;
  color: var(--text-muted) !important;
  border-top-left-radius: 6px;
  padding: 1px 6px !important;
  font-size: 9.5px !important;
  line-height: 1.3 !important;
}
.leaflet-control-attribution a {
  color: var(--text-link) !important;
}
.leaflet-tooltip {
  background: var(--bg-popup) !important;
  border: 1px solid var(--border-strong) !important;
  border-radius: 12px !important;
  padding: 9px 12px !important;
  color: var(--text-primary) !important;
  box-shadow: var(--shadow-popover) !important;
}
.leaflet-tooltip-top::before {
  border-top-color: var(--bg-popup) !important;
}
.leaflet-tooltip-bottom::before {
  border-bottom-color: var(--bg-popup) !important;
}
.leaflet-tooltip-left::before {
  border-left-color: var(--bg-popup) !important;
}
.leaflet-tooltip-right::before {
  border-right-color: var(--bg-popup) !important;
}
.bus-tip-head {
  display: flex;
  align-items: center;
  gap: 7px;
  font-weight: 700;
  font-size: 13px;
}
.bus-tip-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  min-width: 24px;
  height: 20px;
  padding: 0 6px;
  border-radius: 6px;
  font-size: 11px;
  font-weight: 700;
  line-height: 1;
}
.bus-tip-name {
  color: var(--text-strong);
}
.bus-tip-toward {
  margin-top: 4px;
  font-size: 12px;
  color: var(--text-secondary);
}
.bus-tip-meta {
  margin-top: 2px;
  font-size: 11.5px;
  color: var(--text-muted);
}
.bus-tip-stale {
  margin-top: 3px;
  font-size: 11px;
  color: var(--text-stale-tip);
  font-style: italic;
}
.leaflet-popup-content-wrapper {
  background: var(--bg-popup) !important;
  color: var(--text-primary) !important;
  border: 1px solid var(--border-strong) !important;
  border-radius: 14px !important;
  box-shadow: var(--shadow-popover) !important;
  padding: 0 !important;
}
.leaflet-popup-content {
  margin: 12px 14px !important;
  width: auto !important;
}
.leaflet-popup-tip {
  background: var(--bg-popup) !important;
  border: 1px solid var(--border-strong);
  box-shadow: none !important;
}
.leaflet-container a.leaflet-popup-close-button {
  color: var(--text-muted) !important;
  top: 8px !important;
  right: 8px !important;
  width: 22px !important;
  height: 22px !important;
  font-size: 16px !important;
  line-height: 22px !important;
  border-radius: 6px;
}
.leaflet-container a.leaflet-popup-close-button:hover {
  color: var(--text-strong) !important;
  background: var(--menu-hover);
}
.stop-popup {
  min-width: 182px;
}
.stop-popup-name {
  padding-right: 18px;
  font-size: 0.9rem;
  font-weight: 700;
  line-height: 1.3;
  color: var(--text-strong);
}
.stop-popup-badges {
  margin-top: 7px;
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.stop-popup-btn {
  margin-top: 10px;
  width: 100%;
  appearance: none;
  border: none;
  border-radius: 10px;
  background: #715d80;
  color: #ffffff;
  font-family: inherit;
  font-size: 0.82rem;
  font-weight: 700;
  padding: 8px 12px;
  cursor: pointer;
  transition: background-color 140ms ease;
}
.stop-popup-btn:hover {
  background: #826d93;
}
:root[data-city="yogyakarta"] .stop-popup-btn {
  background: #eab308;
  color: #0f172a;
}
:root[data-city="yogyakarta"] .stop-popup-btn:hover {
  background: #facc15;
}
.active-stop-icon {
  background: transparent;
  border: none;
  pointer-events: none;
}
.active-stop-ring {
  width: 28px;
  height: 28px;
  box-sizing: border-box;
  border-radius: 50%;
  border: 2.5px solid var(--active-ring-border);
  background: var(--active-ring-bg);
  animation: active-stop-pulse 1.8s ease-out infinite;
}
@keyframes active-stop-pulse {
  0% {
    transform: scale(0.75);
    opacity: 0.95;
  }
  70% {
    transform: scale(1.2);
    opacity: 0.25;
  }
  100% {
    transform: scale(1.25);
    opacity: 0;
  }
}

@media (max-width: 768px) {
  .leaflet-control-attribution {
    font-size: 8.5px !important;
    padding: 1px 5px !important;
  }
  .shell.mobile .leaflet-control-attribution {
    margin-bottom: 55dvh !important;
    transition: margin-bottom 240ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .shell.mobile.sheet-hidden .leaflet-control-attribution {
    margin-bottom: 0 !important;
  }
}

.city-popup-backdrop {
  position: fixed;
  inset: 0;
  z-index: 2600;
  background: rgba(15, 20, 26, 0.68);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  box-sizing: border-box;
}
.city-popup-card {
  width: 100%;
  max-width: 440px;
  max-height: calc(100dvh - 32px);
  background: var(--bg-popup);
  color: var(--text-primary);
  border: 1px solid var(--border-popover);
  border-radius: 12px;
  box-shadow: var(--shadow-dropdown);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.city-popup-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}
.city-popup-title {
  margin: 0;
  font-size: 1rem;
  font-weight: 700;
  color: var(--text-strong);
  min-width: 0;
}
.city-popup-close {
  appearance: none;
  border: 1px solid var(--border-subtle);
  background: var(--bg-app);
  color: var(--text-secondary);
  width: 30px;
  height: 30px;
  border-radius: 8px;
  font-size: 1.15rem;
  line-height: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
}
.city-popup-close:hover {
  color: var(--text-strong);
  border-color: var(--border-hover);
}
.city-popup-body {
  padding: 16px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.city-popup-img {
  width: 100%;
  max-height: 280px;
  object-fit: contain;
  border-radius: 8px;
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  display: block;
}
.city-popup-text {
  margin: 0;
  font-size: 0.92rem;
  line-height: 1.55;
  color: var(--text-primary);
  white-space: pre-line;
  word-break: break-word;
}
.city-popup-foot {
  padding: 12px 16px;
  border-top: 1px solid var(--border-subtle);
  display: flex;
  justify-content: flex-end;
  flex-shrink: 0;
}
.city-popup-ok {
  appearance: none;
  border: none;
  border-radius: 8px;
  background: #715d80;
  color: #ffffff;
  font-family: inherit;
  font-size: 0.85rem;
  font-weight: 700;
  padding: 8px 18px;
  cursor: pointer;
}
.city-popup-ok:hover {
  background: #826d93;
}
:root[data-city="yogyakarta"] .city-popup-ok {
  background: #eab308;
  color: #0f172a;
}
:root[data-city="yogyakarta"] .city-popup-ok:hover {
  background: #facc15;
}
.conn-banner {
  background: var(--err-bg);
  color: var(--err-text);
  border-bottom: 1px solid var(--err-border);
  padding: 7px 14px;
  font-size: 0.78rem;
  font-weight: 600;
  text-align: center;
  flex-shrink: 0;
}
</style>
