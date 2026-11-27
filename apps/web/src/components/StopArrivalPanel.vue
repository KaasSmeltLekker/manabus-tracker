<script setup lang="ts">
import { computed } from "vue";
import { storeToRefs } from "pinia";
import { goBackOrHome } from "../router";
import {
  computeStopArrivals,
  contrastText,
  corridorBadgeLabel,
  stopServingCorridors,
  useBusStore,
} from "../stores/buses";

const props = defineProps<{
  stopId: string;
  activeBusId?: string | null;
}>();

const emit = defineEmits<{
  focusBus: [id: string];
  recenterStop: [id: string];
}>();

const store = useBusStore();
const {
  pref,
  corridors,
  stops,
  legStops,
  buses,
  corridorColors,
  selectedStop,
  isSelectedStopLoading,
} = storeToRefs(store);

const stop = computed(() =>
  selectedStop.value?.sh_id === props.stopId ? selectedStop.value : null,
);

const servingBadges = computed(() =>
  stop.value
    ? stopServingCorridors(stop.value, stops.value, corridorColors.value, pref.value)
    : [],
);

const arrivals = computed(() =>
  stop.value
    ? computeStopArrivals(
        stop.value,
        corridors.value,
        stops.value,
        legStops.value,
        buses.value,
        corridorColors.value,
        pref.value,
      )
    : [],
);

function formatDistance(meters: number): string {
  if (meters < 1000) {
    return `${Math.max(10, Math.round(meters / 10) * 10)} m`;
  }
  return `${(meters / 1000).toFixed(1).replace(".", ",")} km`;
}
</script>

<template>
  <section class="stop-panel">
    <div class="panel-top">
      <div class="top-actions">
        <button type="button" class="back-btn" @click="goBackOrHome()">
          <svg viewBox="0 0 20 20" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M15.5 10H4.5M9.5 5l-5 5 5 5" />
          </svg>
          <span>Kembali</span>
        </button>

        <button
          type="button"
          class="recenter-btn"
          aria-label="Fokus ke halte"
          title="Fokus ke halte"
          @click="emit('recenterStop', stopId)"
        >
          <svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor" aria-hidden="true">
            <path d="M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8zm8.94 3A8.994 8.994 0 0 0 13 3.06V1h-2v2.06A8.994 8.994 0 0 0 3.06 11H1v2h2.06A8.994 8.994 0 0 0 11 20.94V23h2v-2.06A8.994 8.994 0 0 0 20.94 13H23v-2h-2.06zM12 19a7 7 0 1 1 0-14 7 7 0 0 1 0 14z" />
          </svg>
        </button>
      </div>

      <div class="stop-heading">
        <div class="stop-eyebrow">Kedatangan halte</div>
        <h2 class="stop-title">{{ stop?.sh_name ?? "Memuat halte…" }}</h2>
      </div>

      <div v-if="servingBadges.length > 0" class="stop-badges-row">
        <div class="badge-list">
          <span
            v-for="b in servingBadges"
            :key="b.code"
            class="route-pill"
            :style="{ background: b.color, color: contrastText(b.color) }"
          >
            {{ corridorBadgeLabel(b.code) }}
          </span>
        </div>
      </div>
    </div>

    <div class="arrival-scroll">
      <p class="eta-disclaimer">
        *) Estimasi waktu tiba mungkin bisa tidak akurat, tergantung kondisi lalu lintas. Data didapat dari API
      </p>

      <div
        v-if="!stop || (arrivals.length === 0 && isSelectedStopLoading)"
        class="state-box"
      >
        Memuat estimasi kedatangan…
      </div>

      <div v-else-if="arrivals.length === 0" class="state-box">
        <p class="state-title">Belum ada bus terdekat menuju halte ini</p>
        <p class="state-sub">
          Estimasi muncul saat armada aktif berada dalam jarak 5 km menuju halte ini.
        </p>
      </div>

      <ul v-else class="arrival-list">
        <li v-for="item in arrivals" :key="item.busId" class="arrival-row">
          <button
            type="button"
            class="arrival-card"
            :class="{ active: item.busId === activeBusId }"
            @click="emit('focusBus', item.busId)"
          >
            <span
              class="arrival-bar"
              :style="{
                background: item.corridorColor,
                color: contrastText(item.corridorColor),
              }"
            >
              {{ corridorBadgeLabel(item.corridor) }}
            </span>

            <span class="arrival-eta">
              <span class="eta-value">
                {{ item.isArrivingNow ? "< 1" : item.etaMinutes }}
                <span class="eta-unit">mnt</span>
              </span>
              <span class="fleet-tag">{{ item.busName }}</span>
            </span>

            <span class="arrival-info">
              <span class="info-top">
                <span v-if="item.segmentText" class="info-seg" :title="item.segmentText">
                  {{ item.segmentText }}
                </span>
                <span class="dist-tag">{{ formatDistance(item.distanceMeters) }}</span>
              </span>
              <span v-if="item.destination" class="info-dest" :title="item.destination">
                Arah {{ item.destination }}
              </span>
            </span>
          </button>
        </li>
      </ul>
    </div>
  </section>
</template>

<style scoped>
.stop-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  color: var(--text-primary);
}
.panel-top {
  padding: 12px 14px 12px;
  flex-shrink: 0;
  border-bottom: 1px solid var(--border-subtle);
}
.top-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}
.back-btn {
  appearance: none;
  border: none;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: 0.84rem;
  font-weight: 500;
  padding: 4px 0;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  transition: color 140ms ease;
}
.back-btn:hover {
  color: var(--text-strong);
}
.recenter-btn {
  appearance: none;
  box-sizing: border-box;
  width: 30px;
  height: 30px;
  border-radius: 9px;
  border: 1px solid var(--border-strong);
  background: var(--bg-recenter);
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background-color 140ms ease, color 140ms ease, border-color 140ms ease;
}
.recenter-btn:hover {
  background: var(--bg-recenter-hover);
  color: var(--text-strong);
  border-color: var(--border-hover);
}
.stop-heading {
  display: flex;
  flex-direction: column;
  gap: 3px;
}
.stop-eyebrow {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 0.76rem;
  font-weight: 600;
  color: var(--text-muted);
}
.stop-title {
  margin: 0;
  font-size: 1.08rem;
  font-weight: 700;
  line-height: 1.3;
  color: var(--text-strong);
}
.stop-badges-row {
  margin-top: 10px;
  display: flex;
  align-items: flex-start;
  gap: 8px;
}
.badge-list {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  min-width: 0;
}
.route-pill {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  min-width: 26px;
  height: 22px;
  padding: 0 7px;
  border-radius: 6px;
  font-size: 0.72rem;
  font-weight: 700;
  line-height: 1;
}
.arrival-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  -webkit-overflow-scrolling: touch;
  overscroll-behavior: contain;
  padding: 10px 14px 18px;
  scrollbar-width: thin;
  scrollbar-color: var(--scroll-thumb) transparent;
}
.arrival-scroll::-webkit-scrollbar {
  width: 6px;
}
.arrival-scroll::-webkit-scrollbar-track {
  background: transparent;
}
.arrival-scroll::-webkit-scrollbar-thumb {
  background: var(--scroll-thumb);
  border-radius: 999px;
}
.eta-disclaimer {
  margin: 0 0 10px;
  font-size: 0.72rem;
  line-height: 1.4;
  color: var(--text-muted);
}
.state-box {
  padding: 28px 14px;
  text-align: center;
  color: var(--text-muted);
  font-size: 0.86rem;
}
.state-title {
  margin: 0;
  font-size: 0.9rem;
  font-weight: 600;
  color: var(--text-secondary);
}
.state-sub {
  margin: 6px 0 0;
  font-size: 0.78rem;
  line-height: 1.45;
  color: var(--text-muted);
}
.arrival-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.arrival-row {
  margin: 0;
  padding: 0;
}
.arrival-card {
  width: 100%;
  appearance: none;
  box-sizing: border-box;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: 14px;
  padding: 9px 11px 9px 9px;
  color: inherit;
  font: inherit;
  text-align: left;
  display: flex;
  align-items: stretch;
  gap: 11px;
  cursor: pointer;
  transition: background-color 140ms ease, border-color 140ms ease;
}
.arrival-card:hover {
  background: var(--bg-surface-hover);
}
.arrival-card.active {
  background: var(--bg-surface-active);
  border-color: rgba(113, 93, 128, 0.7);
}
.arrival-bar {
  width: 44px;
  min-height: 56px;
  align-self: stretch;
  border-radius: 10px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 0.86rem;
  font-weight: 700;
  line-height: 1;
  flex-shrink: 0;
  padding: 0 4px;
  box-sizing: border-box;
}
.arrival-eta {
  width: 68px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 5px;
}
.eta-value {
  font-size: 1.15rem;
  font-weight: 700;
  line-height: 1.1;
  color: var(--text-strong);
  font-variant-numeric: tabular-nums;
}
.eta-unit {
  font-size: 0.76rem;
  font-weight: 500;
  color: var(--text-secondary);
  margin-left: 2px;
}
.fleet-tag {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 0.76rem;
  font-weight: 600;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}
.arrival-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 2px;
}
.info-top {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--text-strong);
  font-variant-numeric: tabular-nums;
}
.dist-tag {
  margin-left: auto;
  flex-shrink: 0;
  font-size: 0.74rem;
  font-weight: 600;
  color: var(--text-muted);
}
.info-seg,
.info-dest {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.info-seg {
  font-size: 0.78rem;
  color: var(--text-primary);
  line-height: 1.3;
}
.info-dest {
  font-size: 0.75rem;
  color: var(--text-muted);
  line-height: 1.3;
}

@media (max-width: 768px) {
  .panel-top {
    padding-top: 6px;
  }
}
</style>

