<script setup lang="ts">
import { computed, ref } from "vue";
import { storeToRefs } from "pinia";
import { goAll, goCorridor, goStop } from "../router";
import {
  compareCorridorCodes,
  contrastText,
  corridorBadgeLabel,
  corridorCode,
  corridorDisplayName,
  corridorDisplayVia,
  lookupCorridorValue,
  normalizeColor,
  useBusStore,
  type BusStop,
  type Corridor,
} from "../stores/buses";

defineProps<{
  showAllRoutesButton?: boolean;
}>();

const emit = defineEmits<{
  selectStop: [payload: { code: string; stopId: string }];
}>();

const store = useBusStore();
const {
  pref,
  loadingCorridors,
  loadError,
  visibleCorridors,
  stops,
  busCountsByCorridor,
  stopCountsByCorridor,
  corridorColors,
} = storeToRefs(store);

const query = ref("");

const normalizedQuery = computed(() => query.value.trim().toLowerCase());

const filteredCorridors = computed(() => {
  const q = normalizedQuery.value;
  if (!q) return visibleCorridors.value;
  return visibleCorridors.value.filter((c) => {
    const code = corridorCode(c);
    const badge = corridorBadgeLabel(code);
    const name = corridorDisplayName(c);
    const via = corridorDisplayVia(c) ?? "";
    return (
      name.toLowerCase().includes(q) ||
      via.toLowerCase().includes(q) ||
      code.toLowerCase().includes(q) ||
      badge.toLowerCase().includes(q) ||
      (c.origin ?? "").toLowerCase().includes(q) ||
      (c.toward ?? "").toLowerCase().includes(q)
    );
  });
});

const matchedStops = computed(() => {
  const q = normalizedQuery.value;
  if (!q) return [];
  const results: BusStop[] = [];
  for (const s of stops.value) {
    if (s.sh_name.toLowerCase().includes(q)) {
      results.push(s);
      if (results.length >= 20) break;
    }
  }
  return results;
});

function busCountFor(c: Corridor): number {
  return lookupCorridorValue(busCountsByCorridor.value, corridorCode(c)) ?? 0;
}

function stopCountFor(c: Corridor): number {
  return lookupCorridorValue(stopCountsByCorridor.value, corridorCode(c)) ?? 0;
}

function swatchColor(c: Corridor): string {
  return normalizeColor(c.color) ?? "#0284c7";
}

function stopBadgesFor(stop: BusStop): { code: string; color: string }[] {
  const isSurabaya = pref.value === "3501";
  const map = new Map<string, string>();
  for (let i = 0; i < stop.corridors.length; i++) {
    const code = stop.corridors[i].trim().toUpperCase();
    if (!code || (isSurabaya && code === "SBT") || map.has(code)) continue;
    const color =
      lookupCorridorValue(corridorColors.value, code) ??
      stop.corridorColors[i] ??
      "#64748b";
    map.set(code, color);
  }
  return Array.from(map.entries())
    .sort((a, b) => compareCorridorCodes(a[0], b[0]))
    .map(([code, color]) => ({ code, color }));
}

function onSelectStop(stop: BusStop, preferredCode?: string) {
  const badges = stopBadgesFor(stop);
  const target = preferredCode ?? badges[0]?.code ?? stop.kor;
  if (!target || (pref.value === "3501" && target === "SBT")) return;
  goCorridor(target);
  emit("selectStop", { code: target, stopId: stop.sh_id });
}
</script>

<template>
  <section class="corridor-list">
    <div class="search-wrap">
      <div class="search-box">
        <svg class="search-icon" viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <circle cx="11" cy="11" r="7" />
          <path d="M20 20l-3.5-3.5" />
        </svg>
        <input
          v-model="query"
          type="search"
          class="search-input"
          placeholder="Cari rute atau halte"
          aria-label="Cari rute atau halte"
        />
        <button
          v-if="query"
          type="button"
          class="clear-btn"
          aria-label="Hapus pencarian"
          @click="query = ''"
        >
          ×
        </button>
      </div>
    </div>

    <div class="list-scroll">
      <button
        v-if="showAllRoutesButton && !normalizedQuery"
        type="button"
        class="all-routes-btn"
        @click="goAll()"
      >
        <span class="info">
          <span class="name">Semua Rute</span>
          <span class="meta">Lacak seluruh koridor dan armada</span>
        </span>
      </button>

      <a
        v-if="pref === '1' && !normalizedQuery"
        class="transit-map-btn"
        href="https://s.id/TransitMapYK"
        target="_blank"
        rel="noopener noreferrer"
      >
        <span class="info">
          <span class="name">Peta Transportasi Umum</span>
          <span class="meta">s.id/TransitMapYK</span>
        </span>
      </a>

      <ul v-if="filteredCorridors.length > 0" class="card-group">
        <li v-for="c in filteredCorridors" :key="c.id" class="card-row">
          <button
            type="button"
            class="corridor-item"
            @click="goCorridor(corridorCode(c))"
          >
            <span
              class="route-block"
              :style="{ background: swatchColor(c), color: contrastText(swatchColor(c)) }"
            >
              {{ corridorBadgeLabel(corridorCode(c)) }}
            </span>
            <span class="info">
              <span class="name">{{ corridorDisplayName(c) }}</span>
              <span v-if="corridorDisplayVia(c)" class="via">{{ corridorDisplayVia(c) }}</span>
              <span class="meta">
                {{ busCountFor(c) }} armada | {{ stopCountFor(c) }} pemberhentian
              </span>
            </span>
          </button>
        </li>
      </ul>

      <template v-if="matchedStops.length > 0">
        <p class="section-label">Halte</p>
        <ul class="card-group">
          <li v-for="s in matchedStops" :key="s.sh_id" class="card-row">
            <div class="stop-match-row">
              <button
                type="button"
                class="stop-match-btn"
                @click="goStop(s.sh_id)"
              >
                <span class="stop-match-name">{{ s.sh_name }}</span>
              </button>
              <div class="stop-match-badges">
                <button
                  v-for="b in stopBadgesFor(s)"
                  :key="b.code"
                  type="button"
                  class="mini-badge"
                  :style="{ background: b.color, color: contrastText(b.color) }"
                  @click.stop="onSelectStop(s, b.code)"
                >
                  {{ corridorBadgeLabel(b.code) }}
                </button>
              </div>
            </div>
          </li>
        </ul>
      </template>

      <div
        v-if="filteredCorridors.length === 0 && matchedStops.length === 0"
        class="empty-state"
      >
        <template v-if="loadError && !normalizedQuery">
          <p class="load-err-msg">{{ loadError }}</p>
          <button type="button" class="retry-btn" @click="store.retryLoadCity()">
            Coba lagi
          </button>
        </template>
        <template v-else>
          {{ loadingCorridors && !normalizedQuery ? "Memuat rute…" : "Tidak ada rute atau halte yang cocok" }}
        </template>
      </div>
    </div>
  </section>
</template>

<style scoped>
.corridor-list {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  color: var(--text-primary);
}
.search-wrap {
  padding: 12px 14px;
  flex-shrink: 0;
}
.search-box {
  display: flex;
  align-items: center;
  gap: 9px;
  background: var(--bg-surface);
  border-radius: 14px;
  padding: 0 12px;
  height: 42px;
  box-sizing: border-box;
  border: 1px solid var(--border-subtle);
  transition: border-color 140ms ease;
}
.search-box:focus-within {
  border-color: var(--border-hover);
}
.search-icon {
  color: var(--text-muted);
  flex-shrink: 0;
}
.search-input {
  flex: 1;
  min-width: 0;
  border: none;
  background: transparent;
  color: var(--text-primary);
  font: inherit;
  font-size: 0.9rem;
  outline: none;
}
.search-input::placeholder {
  color: var(--text-muted);
}
.search-input::-webkit-search-cancel-button {
  display: none;
}
.clear-btn {
  appearance: none;
  border: none;
  background: transparent;
  color: var(--text-muted);
  font-size: 1.15rem;
  line-height: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: 6px;
  padding: 0;
  cursor: pointer;
  transition: color 140ms ease, background-color 140ms ease;
}
.clear-btn:hover {
  color: var(--text-strong);
  background: var(--menu-hover);
}
.list-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  -webkit-overflow-scrolling: touch;
  overscroll-behavior: contain;
  padding: 0 14px 16px;
  scrollbar-width: thin;
  scrollbar-color: var(--scroll-thumb) transparent;
}
.list-scroll::-webkit-scrollbar {
  width: 6px;
}
.list-scroll::-webkit-scrollbar-track {
  background: transparent;
}
.list-scroll::-webkit-scrollbar-thumb {
  background: var(--scroll-thumb);
  border-radius: 999px;
}
.all-routes-btn {
  width: 100%;
  appearance: none;
  background: var(--bg-accent-card);
  border: 1px solid var(--border-subtle);
  border-radius: 14px;
  color: inherit;
  font: inherit;
  text-align: left;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  margin-bottom: 10px;
  box-sizing: border-box;
  cursor: pointer;
  transition: background-color 140ms ease, border-color 140ms ease;
}
.all-routes-btn:hover {
  background: var(--bg-accent-card-hover);
  border-color: var(--border-hover);
}
.transit-map-btn {
  width: 100%;
  background: #eab308;
  border: 1px solid transparent;
  border-radius: 14px;
  color: #0f172a;
  text-decoration: none;
  font: inherit;
  text-align: left;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  margin-bottom: 10px;
  box-sizing: border-box;
  cursor: pointer;
  transition: background-color 140ms ease;
}
.transit-map-btn:hover {
  background: #facc15;
}
.transit-map-btn .name {
  color: #0f172a;
  font-weight: 700;
}
.transit-map-btn .meta {
  color: rgba(15, 23, 42, 0.72);
}
.card-group {
  list-style: none;
  margin: 0;
  padding: 0;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: 14px;
  overflow: hidden;
}
.card-row + .card-row {
  border-top: 1px solid var(--border-subtle);
}
.corridor-item {
  width: 100%;
  appearance: none;
  border: none;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 11px 14px 11px 11px;
  cursor: pointer;
  transition: background-color 140ms ease;
}
.corridor-item:hover {
  background: var(--bg-surface-hover);
}
.route-block {
  min-width: 44px;
  height: 44px;
  padding: 0 6px;
  box-sizing: border-box;
  border-radius: 10px;
  font-size: 1.15rem;
  font-weight: 700;
  line-height: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.info {
  display: flex;
  flex-direction: column;
  gap: 3px;
  flex: 1;
  min-width: 0;
}
.name {
  font-size: 0.94rem;
  font-weight: 500;
  line-height: 1.3;
  color: var(--text-primary);
}
.via {
  font-size: 0.78rem;
  font-weight: 400;
  line-height: 1.25;
  color: var(--text-secondary);
}
.meta {
  font-size: 0.78rem;
  line-height: 1.25;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}
.section-label {
  margin: 14px 4px 8px;
  font-size: 0.76rem;
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.stop-match-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 10px 14px;
  transition: background-color 140ms ease;
}
.stop-match-row:hover {
  background: var(--bg-surface-hover);
}
.stop-match-btn {
  flex: 1;
  min-width: 0;
  appearance: none;
  border: none;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
  padding: 0;
  cursor: pointer;
}
.stop-match-name {
  font-size: 0.88rem;
  color: var(--text-primary);
}
.stop-match-badges {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  flex-shrink: 0;
}
.mini-badge {
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
  line-height: 1;
  cursor: pointer;
}
.empty-state {
  padding: 28px 12px;
  text-align: center;
  font-size: 0.86rem;
  color: var(--text-muted);
}
.load-err-msg {
  margin: 0 0 12px;
  color: var(--err-text);
  font-weight: 600;
}
.retry-btn {
  appearance: none;
  border: 1px solid var(--border-strong);
  background: var(--bg-surface);
  color: var(--text-strong);
  font-family: inherit;
  font-size: 0.84rem;
  font-weight: 700;
  padding: 8px 16px;
  border-radius: 8px;
  cursor: pointer;
}
.retry-btn:hover {
  background: var(--bg-surface-hover);
  border-color: var(--border-hover);
}
@media (max-width: 768px) {
  .search-box {
    height: 46px;
  }
  .search-input {
    font-size: 16px;
  }
  .clear-btn {
    width: 34px;
    height: 34px;
  }
}
</style>
