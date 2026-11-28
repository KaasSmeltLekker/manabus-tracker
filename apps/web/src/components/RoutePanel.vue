<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { activeCity, goCity, goCorridor } from "../router";
import {
  compareCorridorCodes,
  contrastText,
  corridorBadgeLabel,
  corridorCode,
  corridorDisplayName,
  corridorDisplayVia,
  extraStopRoutes,
  formatIdleAge,
  formatOperatingHours,
  lookupCorridorValue,
  normalizeColor,
  sameCorridorCode,
  useBusStore,
  type Bus,
  type BusStop,
} from "../stores/buses";

const props = defineProps<{
  code: string;
  activeBusId?: string | null;
}>();

const emit = defineEmits<{
  focusBus: [id: string];
  focusStop: [id: string];
}>();

const store = useBusStore();
const {
  corridors,
  stops,
  visibleBuses,
  visibleStops,
  corridorColors,
} = storeToRefs(store);

const corridor = computed(
  () => corridors.value.find((c) => corridorCode(c) === props.code) ?? null,
);

const routeColor = computed(
  () =>
    normalizeColor(corridor.value?.color) ??
    lookupCorridorValue(corridorColors.value, props.code) ??
    "#0284c7",
);

const routeName = computed(() =>
  corridor.value ? corridorDisplayName(corridor.value) : props.code,
);

const routeVia = computed(() =>
  corridor.value ? corridorDisplayVia(corridor.value) : null,
);

const operatingHours = computed(() =>
  formatOperatingHours(corridor.value?.jam_operasional),
);

const busList = computed(() =>
  Object.values(visibleBuses.value).sort((a, b) => {
    if (a.stale !== b.stale) return a.stale ? 1 : -1;
    return (a.name ?? a.id).localeCompare(b.name ?? b.id);
  }),
);

function busChipTitle(bus: Bus): string {
  const label = bus.name || bus.plate || bus.id;
  if (bus.stale) {
    return `${label} · Idle (${formatIdleAge(bus.fix_age_s)}) · Klik untuk lihat lokasi terakhir`;
  }
  if (bus.speed != null && Math.round(bus.speed) > 0) {
    return `${label} · ${Math.round(bus.speed)} km/jam · Klik untuk fokus`;
  }
  return `${label} · Aktif · Klik untuk fokus`;
}

const busChipsEl = ref<HTMLElement | null>(null);
const chipScrollOverflow = ref(false);
const chipThumbWidthPct = ref(100);
const chipThumbLeftPct = ref(0);
let chipsResizeObserver: ResizeObserver | null = null;

function syncChipScroll() {
  const el = busChipsEl.value;
  if (!el) {
    chipScrollOverflow.value = false;
    return;
  }
  const maxScroll = el.scrollWidth - el.clientWidth;
  if (maxScroll <= 2) {
    chipScrollOverflow.value = false;
    chipThumbWidthPct.value = 100;
    chipThumbLeftPct.value = 0;
    return;
  }
  chipScrollOverflow.value = true;
  const ratio = Math.max(0.18, Math.min(1, el.clientWidth / el.scrollWidth));
  const progress = Math.max(0, Math.min(1, el.scrollLeft / maxScroll));
  chipThumbWidthPct.value = ratio * 100;
  chipThumbLeftPct.value = progress * (1 - ratio) * 100;
}

watch(busChipsEl, (el) => {
  chipsResizeObserver?.disconnect();
  chipsResizeObserver = null;
  if (el && typeof ResizeObserver !== "undefined") {
    chipsResizeObserver = new ResizeObserver(syncChipScroll);
    chipsResizeObserver.observe(el);
  }
  void nextTick(syncChipScroll);
});

watch(() => props.code, () => {
  if (busChipsEl.value) busChipsEl.value.scrollLeft = 0;
  void nextTick(syncChipScroll);
});

watch(busList, () => {
  void nextTick(syncChipScroll);
});

onBeforeUnmount(() => {
  chipsResizeObserver?.disconnect();
  chipsResizeObserver = null;
});

const sbtColor = computed(() => corridorColors.value.SBT ?? "#fbff00");

const stopsById = computed(() => {
  const map = new Map<string, BusStop>();
  for (const s of stops.value) map.set(s.sh_id, s);
  return map;
});

const stopsByCoord = computed(() => {
  const map = new Map<string, BusStop[]>();
  for (const s of stops.value) {
    const key = `${s.lat.toFixed(6)},${s.lng.toFixed(6)}`;
    const list = map.get(key);
    if (list) list.push(s);
    else map.set(key, [s]);
  }
  return map;
});

function otherRoutesFor(stop: BusStop): { code: string; color: string; linkable: boolean }[] {
  const current = props.code.trim().toUpperCase();
  const coordKey = `${stop.lat.toFixed(6)},${stop.lng.toFixed(6)}`;
  const candidates: BusStop[] = [stop];
  const byId = stopsById.value.get(stop.sh_id);
  if (byId && byId !== stop) candidates.push(byId);
  const byCoord = stopsByCoord.value.get(coordKey);
  if (byCoord) {
    for (const s of byCoord) {
      if (!candidates.includes(s)) candidates.push(s);
    }
  }

  const isSurabaya = store.pref === "3501";
  const map = new Map<string, { color: string; linkable: boolean }>();
  for (const entry of candidates) {
    for (let i = 0; i < entry.corridors.length; i++) {
      const code = entry.corridors[i].trim().toUpperCase();
      if (!code || (isSurabaya && code === "SBT")) continue;
      if (sameCorridorCode(code, current)) continue;
      if (!map.has(code)) {
        const color =
          lookupCorridorValue(corridorColors.value, code) ??
          entry.corridorColors[i] ??
          "#64748b";
        map.set(code, { color, linkable: true });
      }
    }
  }

  for (const extra of extraStopRoutes(stop, store.pref)) {
    if (!map.has(extra.code)) {
      map.set(extra.code, { color: extra.color, linkable: false });
    }
  }

  return Array.from(map.entries())
    .sort((a, b) => compareCorridorCodes(a[0], b[0]))
    .map(([code, entry]) => ({ code, color: entry.color, linkable: entry.linkable }));
}

const otherRoutesByStopId = computed(() => {
  const map = new Map<string, { code: string; color: string; linkable: boolean }[]>();
  for (const stop of visibleStops.value) {
    if (map.has(stop.sh_id)) continue;
    const list = otherRoutesFor(stop);
    if (list.length > 0) map.set(stop.sh_id, list);
  }
  return map;
});
</script>

<template>
  <section class="route-panel" :style="{ '--route-color': routeColor }">
    <div class="panel-top">
      <button type="button" class="back-btn" @click="goCity(activeCity())">
        <svg viewBox="0 0 20 20" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M15.5 10H4.5M9.5 5l-5 5 5 5" />
        </svg>
        <span>Kembali</span>
      </button>

      <div class="dir-header">
        <span
          class="route-block"
          :style="{ background: routeColor, color: contrastText(routeColor) }"
        >
          {{ corridorBadgeLabel(code) }}
        </span>

        <div class="route-title-card">
          <div class="route-title">{{ routeName }}</div>
          <div v-if="routeVia" class="route-via">{{ routeVia }}</div>
        </div>
      </div>

      <div class="meta-row">
        <span>{{ busList.length }} armada | {{ visibleStops.length }} pemberhentian</span>
        <span v-if="operatingHours">Jam operasional: {{ operatingHours }}</span>
      </div>

      <div class="bus-bar">
        <div v-if="busList.length === 0" class="bus-empty">
          Tidak ada armada yang terdeteksi
        </div>
        <template v-else>
          <div ref="busChipsEl" class="bus-chips" @scroll="syncChipScroll">
            <button
              v-for="bus in busList"
              :key="bus.id"
              type="button"
              class="bus-chip"
              :class="{
                active: bus.id === activeBusId,
                stale: bus.stale && bus.id !== activeBusId,
              }"
              :style="
                bus.id === activeBusId
                  ? { background: routeColor, color: contrastText(routeColor) }
                  : undefined
              "
              :title="busChipTitle(bus)"
              @click="emit('focusBus', bus.id)"
            >
              <span
                class="bus-status-dot"
                :class="bus.stale ? 'idle' : 'live'"
                aria-hidden="true"
              ></span>
              <span>{{ bus.name || bus.plate || bus.id }}</span>
              <span
                v-if="store.pref === '3501' && bus.corridor?.trim().toUpperCase() === 'SBT'"
                class="sbt-block"
                :style="{ background: sbtColor, color: contrastText(sbtColor) }"
              >
                SBT
              </span>
            </button>
          </div>
          <div v-if="chipScrollOverflow" class="bus-scroll-track" aria-hidden="true">
            <span
              class="bus-scroll-thumb"
              :style="{
                width: `${chipThumbWidthPct}%`,
                left: `${chipThumbLeftPct}%`,
              }"
            ></span>
          </div>
        </template>
      </div>
    </div>

    <div v-if="visibleStops.length === 0" class="empty">Memuat halte…</div>
    <ul v-else class="stop-timeline">
      <li
        v-for="(stop, idx) in visibleStops"
        :key="`${stop.sh_id}-${idx}`"
        class="stop-entry"
      >
        <button type="button" class="stop-row" @click="emit('focusStop', stop.sh_id)">
          <span class="rail" aria-hidden="true">
            <span v-if="idx > 0" class="line-top"></span>
            <span v-if="idx < visibleStops.length - 1" class="line-bottom"></span>
            <span class="stop-dot" :class="{ first: idx === 0 }"></span>
          </span>
          <span class="stop-name" :class="{ last: idx === visibleStops.length - 1 }">{{
            stop.sh_name
          }}</span>
        </button>
        <div v-if="otherRoutesByStopId.has(stop.sh_id)" class="stop-transfers">
          <template v-for="other in otherRoutesByStopId.get(stop.sh_id)" :key="other.code">
            <button
              v-if="other.linkable"
              type="button"
              class="transfer-block"
              :style="{ background: other.color, color: contrastText(other.color) }"
              @click="goCorridor(other.code)"
            >
              {{ corridorBadgeLabel(other.code) }}
            </button>
            <span
              v-else
              class="transfer-block static"
              :style="{ background: other.color, color: contrastText(other.color) }"
            >
              {{ corridorBadgeLabel(other.code) }}
            </span>
          </template>
        </div>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.route-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  color: var(--text-primary);
}
.panel-top {
  padding: 12px 14px 0;
  flex-shrink: 0;
}
.back-btn {
  appearance: none;
  border: none;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: 0.84rem;
  font-weight: 500;
  padding: 0 0 10px;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  transition: color 140ms ease;
}
.back-btn:hover {
  color: var(--text-strong);
}
.dir-header {
  display: flex;
  align-items: stretch;
  gap: 10px;
}
.route-block {
  min-width: 58px;
  height: auto;
  align-self: stretch;
  padding: 0 8px;
  box-sizing: border-box;
  border-radius: 14px;
  font-size: 1.35rem;
  font-weight: 700;
  line-height: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.route-title-card {
  flex: 1;
  min-width: 0;
  min-height: 58px;
  box-sizing: border-box;
  background: var(--bg-accent-card);
  border: 1px solid var(--border-subtle);
  border-radius: 14px;
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 2px;
}
.route-title {
  font-size: 0.94rem;
  font-weight: 700;
  line-height: 1.3;
  color: var(--text-strong);
}
.route-via {
  font-size: 0.78rem;
  line-height: 1.3;
  color: var(--text-muted);
}
.meta-row {
  margin-top: 12px;
  padding-bottom: 10px;
  border-bottom: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  font-size: 0.78rem;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}
.bus-bar {
  padding: 10px 0;
  border-bottom: 1px solid var(--border-subtle);
}
.bus-empty {
  font-size: 0.8rem;
  color: var(--text-muted);
  padding: 4px 2px;
}
.bus-chips {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 7px;
  max-height: 148px;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--scroll-thumb) transparent;
}
.bus-chips::-webkit-scrollbar {
  width: 5px;
}
.bus-chips::-webkit-scrollbar-track {
  background: transparent;
}
.bus-chips::-webkit-scrollbar-thumb {
  background: var(--scroll-thumb);
  border-radius: 999px;
}
.bus-scroll-track {
  display: none;
}
.bus-chip {
  appearance: none;
  border: 1px solid var(--border-strong);
  background: var(--bg-chip);
  color: var(--text-chip);
  box-shadow: var(--shadow-chip);
  font-size: 0.8rem;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  padding: 6px 10px;
  border-radius: 10px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
  cursor: pointer;
  flex-shrink: 0;
  transition: background-color 140ms ease, color 140ms ease, border-color 140ms ease;
}
.bus-chip:hover {
  background: var(--bg-chip-hover);
  color: var(--text-chip-hover);
  border-color: var(--border-hover);
}
.bus-chip.stale {
  background: var(--bg-chip-stale);
  color: var(--text-chip-stale);
  border-color: var(--border-subtle);
  box-shadow: none;
}
.bus-chip.stale:hover {
  background: var(--bg-chip-stale-hover);
  color: var(--text-chip-stale-hover);
  border-color: var(--border-hover);
}
.bus-chip.active {
  border-color: var(--border-hover);
}
.bus-status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}
.bus-status-dot.live {
  background: var(--dot-live);
}
.bus-status-dot.idle {
  background: var(--dot-idle);
}
.bus-chip.active .bus-status-dot {
  box-shadow: 0 0 0 1.5px rgba(27, 32, 43, 0.45);
}
.sbt-block {
  padding: 2px 5px;
  border-radius: 5px;
  font-size: 0.62rem;
  font-weight: 700;
  line-height: 1.2;
}
.empty {
  padding: 20px;
  font-size: 0.84rem;
  color: var(--text-muted);
  text-align: center;
}
.stop-timeline {
  list-style: none;
  margin: 0;
  padding: 8px 14px 18px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  -webkit-overflow-scrolling: touch;
  overscroll-behavior: contain;
  scrollbar-width: thin;
  scrollbar-color: var(--scroll-thumb) transparent;
}
.stop-timeline::-webkit-scrollbar {
  width: 6px;
}
.stop-timeline::-webkit-scrollbar-track {
  background: transparent;
}
.stop-timeline::-webkit-scrollbar-thumb {
  background: var(--scroll-thumb);
  border-radius: 999px;
}
.stop-entry {
  display: flex;
  align-items: stretch;
  border-radius: 10px;
  transition: background-color 140ms ease;
}
.stop-entry:hover {
  background: var(--bg-surface-hover);
}
.stop-row {
  flex: 1;
  min-width: 0;
  appearance: none;
  border: none;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
  display: flex;
  align-items: stretch;
  gap: 12px;
  padding: 0 4px;
  cursor: pointer;
}
.rail {
  position: relative;
  width: 14px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}
.line-top,
.line-bottom {
  position: absolute;
  left: 50%;
  width: 2px;
  transform: translateX(-50%);
  background: var(--rail-line);
}
.line-top {
  top: 0;
  bottom: 50%;
}
.line-bottom {
  top: 50%;
  bottom: 0;
}
.stop-dot {
  position: relative;
  z-index: 1;
  width: 10px;
  height: 10px;
  box-sizing: border-box;
  border-radius: 50%;
  background: #ffffff;
  border: 2px solid var(--route-color);
  flex-shrink: 0;
}
.stop-dot.first {
  width: 12px;
  height: 12px;
  background: var(--route-color);
  border: none;
}
.stop-name {
  font-size: 0.88rem;
  line-height: 1.35;
  color: var(--text-primary);
  padding: 8px 0;
  flex: 1;
  min-width: 0;
}
.stop-name.last {
  font-weight: 700;
  color: var(--text-strong);
}
.stop-transfers {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 4px;
  padding: 4px 8px 4px 4px;
  flex-shrink: 0;
  max-width: 45%;
}
.transfer-block {
  appearance: none;
  border: none;
  box-sizing: border-box;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 24px;
  height: 20px;
  padding: 0 6px;
  border-radius: 6px;
  font-size: 0.7rem;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  line-height: 1;
  text-align: center;
  cursor: pointer;
}
.transfer-block.static {
  cursor: default;
}

@media (max-width: 768px) {
  .panel-top {
    padding-top: 2px;
  }
  .bus-chips {
    flex-wrap: nowrap;
    max-height: none;
    overflow-x: auto;
    overflow-y: hidden;
    -webkit-overflow-scrolling: touch;
    scrollbar-width: none;
  }
  .bus-chips::-webkit-scrollbar {
    display: none;
  }
  .bus-scroll-track {
    display: block;
    position: relative;
    height: 3px;
    margin-top: 8px;
    border-radius: 999px;
    background: var(--scroll-track);
    overflow: hidden;
  }
  .bus-scroll-thumb {
    position: absolute;
    top: 0;
    bottom: 0;
    border-radius: 999px;
    background: var(--scroll-indicator);
  }
}
</style>
