<script setup lang="ts">
import { assetUrl } from "../config";
import { prefetchMapStyle } from "../mapStyle";
import { CITY_PREF, goAdmin, goCity, type CityId } from "../router";
import { useBusStore } from "../stores/buses";

const HOMEPAGE_NOTE = "Made by Keju 🧀 with love <3";
const logoUrl = assetUrl("logo.png");

const store = useBusStore();
const secretTaps: number[] = [];

function onBrandTap() {
  const now = Date.now();
  secretTaps.push(now);
  while (secretTaps.length > 0 && now - secretTaps[0] > 1800) {
    secretTaps.shift();
  }
  if (secretTaps.length >= 5) {
    secretTaps.length = 0;
    goAdmin();
  }
}

function warmCity(city: CityId) {
  store.prefetchCity(CITY_PREF[city]);
  if (typeof window !== "undefined" && window.matchMedia("(min-width: 769px)").matches) {
    prefetchMapStyle();
    void import("./MapView.vue");
  }
}
</script>

<template>
  <main class="home-view">
    <div class="home-content">
      <div class="home-head">
        <div class="home-brand" @click="onBrandTap()">
          <img class="home-logo" :src="logoUrl" width="38" height="38" alt="ManaBus?" />
          <span class="home-wordmark">ManaBus?</span>
        </div>
        <p class="home-sub">Pelacak transum berbasis web</p>
        <h1 class="home-title">Pilih Kota</h1>
      </div>

      <div class="city-grid">
        <button
          type="button"
          class="city-btn city-surabaya"
          @mouseenter="warmCity('surabaya')"
          @focus="warmCity('surabaya')"
          @touchstart.passive="warmCity('surabaya')"
          @click="goCity('surabaya')"
        >
          <span class="city-top">
            <span class="city-name">Kota Surabaya</span>
            <svg class="city-chevron" viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M7.5 5l5 5-5 5" />
            </svg>
          </span>
        </button>

        <button
          type="button"
          class="city-btn city-yogyakarta"
          @mouseenter="warmCity('yogyakarta')"
          @focus="warmCity('yogyakarta')"
          @touchstart.passive="warmCity('yogyakarta')"
          @click="goCity('yogyakarta')"
        >
          <span class="city-top">
            <span class="city-name">Yogyakarta</span>
            <svg class="city-chevron" viewBox="0 0 20 20" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M7.5 5l5 5-5 5" />
            </svg>
          </span>
        </button>

        <button type="button" class="city-btn city-semarang soon" disabled aria-disabled="true">
          <span class="city-name">Semarang</span>
          <span class="soon-badge">Segera hadir</span>
        </button>

        <button type="button" class="city-btn city-jatim soon" disabled aria-disabled="true">
          <span class="city-name">Jawa Timur</span>
          <span class="soon-badge">Segera hadir</span>
        </button>
      </div>

      <p v-if="HOMEPAGE_NOTE" class="home-note">{{ HOMEPAGE_NOTE }}</p>
    </div>
  </main>
</template>

<style scoped>
.home-view {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 28px 18px;
  box-sizing: border-box;
}
.home-content {
  width: 100%;
  max-width: 600px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 24px;
}
.home-head {
  text-align: center;
}
.home-brand {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  margin-bottom: 8px;
}
.home-logo {
  width: 38px;
  height: 38px;
  border-radius: 50%;
  object-fit: cover;
  display: block;
  flex-shrink: 0;
}
.home-wordmark {
  font-family: "Montserrat", "DM Sans", system-ui, sans-serif;
  font-size: 1.25rem;
  font-weight: 700;
  line-height: 1.15;
  color: var(--text-strong);
}
.home-sub {
  margin: 0 0 18px;
  font-size: 0.88rem;
  line-height: 1.4;
  color: var(--text-muted);
}
.home-title {
  margin: 0;
  font-size: 1.75rem;
  font-weight: 700;
  letter-spacing: -0.02em;
  color: var(--text-strong);
}
.city-grid {
  width: 100%;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}
.city-btn {
  appearance: none;
  box-sizing: border-box;
  width: 100%;
  min-height: 92px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: 14px;
  padding: 18px 20px;
  box-shadow: var(--shadow-chip);
  color: inherit;
  font: inherit;
  text-align: left;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: flex-start;
  gap: 8px;
  cursor: pointer;
  transition: background-color 140ms ease, border-color 140ms ease;
}
.city-btn:hover:not(:disabled) {
  background: var(--bg-surface-hover);
  border-color: var(--border-hover);
}
.city-btn.soon {
  background: #18202a;
  border-color: #263241;
  opacity: 0.6;
  cursor: not-allowed;
  box-shadow: none;
}
.city-btn.soon .city-name {
  color: #cbd5e1;
}
.city-btn.soon .soon-badge {
  background: #0f141a;
  border-color: #263241;
  color: #8b9db3;
}
.city-surabaya {
  background: #715d80;
  border-color: transparent;
}
.city-surabaya:hover:not(:disabled) {
  background: #826d93;
  border-color: transparent;
}
.city-surabaya .city-name {
  color: #ffffff;
}
.city-surabaya .city-chevron {
  color: rgba(255, 255, 255, 0.88);
}
.city-yogyakarta {
  background: #eab308;
  border-color: transparent;
}
.city-yogyakarta:hover:not(:disabled) {
  background: #facc15;
  border-color: transparent;
}
.city-yogyakarta .city-name {
  color: #0f172a;
}
.city-yogyakarta .city-chevron {
  color: rgba(15, 23, 42, 0.78);
}
.city-top {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}
.city-name {
  font-size: 1.25rem;
  font-weight: 700;
  line-height: 1.2;
  color: var(--text-strong);
}
.city-chevron {
  color: var(--text-muted);
  flex-shrink: 0;
}
.soon-badge {
  font-size: 0.68rem;
  font-weight: 600;
  line-height: 1;
  padding: 4px 8px;
  border-radius: 999px;
  background: var(--bg-control-soft);
  border: 1px solid var(--border-subtle);
  color: var(--text-muted);
  white-space: nowrap;
  flex-shrink: 0;
}
.home-note {
  margin: 4px 0 0;
  font-size: 0.84rem;
  line-height: 1.4;
  color: var(--text-muted);
  text-align: center;
}

@media (max-width: 560px) {
  .city-grid {
    grid-template-columns: 1fr;
    max-width: 360px;
  }
}
</style>
