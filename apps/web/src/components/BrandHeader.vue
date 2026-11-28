<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { storeToRefs } from "pinia";
import { assetUrl } from "../config";
import { currentRoute, goAdmin, goHome } from "../router";
import { useBusStore } from "../stores/buses";
import { theme, toggleTheme } from "../theme";

const logoUrl = assetUrl("logo.png");

const store = useBusStore();
const { operator, menuLinks } = storeToRefs(store);
const menuOpen = ref(false);
const menuRoot = ref<HTMLElement | null>(null);
const secretTaps: number[] = [];

const visibleMenuLinks = computed(() =>
  menuLinks.value
    .filter((item) => {
      const label = item.label.trim();
      const url = item.url.trim();
      return Boolean(item.enabled && label && /^(https?:\/\/|\/)/i.test(url));
    })
    .map((item) => {
      const url = item.url.trim();
      return {
        label: item.label.trim(),
        url,
        external: /^https?:\/\//i.test(url),
      };
    }),
);

function onBrandClick() {
  if (currentRoute.value.name === "home") {
    const now = Date.now();
    secretTaps.push(now);
    while (secretTaps.length > 0 && now - secretTaps[0] > 1800) {
      secretTaps.shift();
    }
    if (secretTaps.length >= 5) {
      secretTaps.length = 0;
      goAdmin();
    }
    return;
  }
  secretTaps.length = 0;
  goHome();
}

function navigateAndClose(action: () => void) {
  menuOpen.value = false;
  action();
}

function onToggleTheme() {
  toggleTheme();
  menuOpen.value = false;
}

function onDocClick(ev: MouseEvent) {
  if (!menuOpen.value) return;
  const target = ev.target as Node | null;
  if (menuRoot.value && target && !menuRoot.value.contains(target)) {
    menuOpen.value = false;
  }
}

onMounted(() => {
  document.addEventListener("click", onDocClick);
});

onBeforeUnmount(() => {
  document.removeEventListener("click", onDocClick);
});
</script>

<template>
  <header
    class="brand-bar"
    :class="
      currentRoute.name === 'admin'
        ? 'mode-admin'
        : currentRoute.name !== 'home'
          ? `city-${operator.city}`
          : undefined
    "
  >
    <button type="button" class="brand-btn" @click="onBrandClick()">
      <img class="brand-logo" :src="logoUrl" width="38" height="38" alt="ManaBus?" />
      <span class="brand-text">
        <span class="brand-title">ManaBus?<span v-if="currentRoute.name !== 'home' && currentRoute.name !== 'admin'" class="brand-city"> ({{ operator.shortCode }})</span></span>
        <span class="brand-sub">{{
          currentRoute.name === "admin" ? "Admin Panel" : "Pelacak transum berbasis web"
        }}</span>
      </span>
    </button>

    <a
      v-if="currentRoute.name === 'home'"
      class="menu-btn"
      href="https://github.com/KaasSmeltLekker"
      target="_blank"
      rel="noopener noreferrer"
      aria-label="GitHub KaasSmeltLekker"
      title="GitHub"
    >
      <svg viewBox="0 0 24 24" width="22" height="22" fill="currentColor" aria-hidden="true">
        <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.53 1.032 1.53 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844a9.59 9.59 0 0 1 2.504.337c1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0 0 22 12.017C22 6.484 17.522 2 12 2Z" />
      </svg>
    </a>

    <div v-else-if="currentRoute.name !== 'admin'" ref="menuRoot" class="menu-wrap">
      <button
        type="button"
        class="menu-btn"
        aria-label="Menu navigasi"
        :aria-expanded="menuOpen"
        @click.stop="menuOpen = !menuOpen"
      >
        <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true">
          <path d="M4 7h16M4 12h16M4 17h16" />
        </svg>
      </button>

      <div v-if="menuOpen" class="menu-popover">
        <button type="button" class="menu-item" @click="navigateAndClose(goHome)">
          Pindah Kota
        </button>
        <a
          class="menu-item menu-item-row"
          href="https://github.com/KaasSmeltLekker"
          target="_blank"
          rel="noopener noreferrer"
          @click="menuOpen = false"
        >
          <span>GitHub</span>
          <svg class="menu-gh-icon" viewBox="0 0 24 24" width="16" height="16" fill="currentColor" aria-hidden="true">
            <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.53 1.032 1.53 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844a9.59 9.59 0 0 1 2.504.337c1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0 0 22 12.017C22 6.484 17.522 2 12 2Z" />
          </svg>
        </a>
        <a
          v-for="(link, idx) in visibleMenuLinks"
          :key="idx"
          class="menu-item"
          :href="link.url"
          :target="link.external ? '_blank' : undefined"
          :rel="link.external ? 'noopener noreferrer' : undefined"
          @click="menuOpen = false"
        >
          {{ link.label }}
        </a>
        <div class="menu-divider" role="separator"></div>
        <button
          type="button"
          class="menu-item menu-item-row"
          role="menuitemcheckbox"
          :aria-checked="theme === 'light'"
          @click="onToggleTheme()"
        >
          <span>Mode terang</span>
          <span
            class="theme-switch"
            :class="{ on: theme === 'light' }"
            aria-hidden="true"
          >
            <span class="theme-thumb"></span>
          </span>
        </button>
      </div>
    </div>
  </header>
</template>

<style scoped>
.brand-bar {
  position: relative;
  z-index: 1400;
  height: 58px;
  padding: 0 14px;
  padding-top: env(safe-area-inset-top);
  background: #715d80;
  color: #ffffff;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  flex-shrink: 0;
  transition: background-color 140ms ease, color 140ms ease;
}
.brand-bar.city-yogyakarta {
  background: #eab308;
  color: #0f172a;
}
.brand-bar.mode-admin {
  background: #7f1d1d;
  color: #ffffff;
  box-shadow: inset 0 -1px 0 rgba(255, 255, 255, 0.14);
}
.brand-bar.city-yogyakarta .brand-title {
  color: #0f172a;
}
.brand-bar.city-yogyakarta .brand-city {
  color: rgba(15, 23, 42, 0.78);
}
.brand-bar.city-yogyakarta .brand-sub {
  color: rgba(15, 23, 42, 0.75);
}
.brand-bar.city-yogyakarta .menu-btn {
  color: #0f172a;
}
.brand-bar.city-yogyakarta .menu-btn:hover {
  background: rgba(15, 23, 42, 0.1);
}
.brand-btn {
  appearance: none;
  border: none;
  background: transparent;
  color: inherit;
  font: inherit;
  padding: 0;
  display: inline-flex;
  align-items: center;
  gap: 10px;
  cursor: pointer;
  text-align: left;
  min-width: 0;
}
.brand-logo {
  width: 38px;
  height: 38px;
  flex-shrink: 0;
  border-radius: 50%;
  object-fit: cover;
  display: block;
}
.brand-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
  font-family: "Montserrat", "DM Sans", system-ui, sans-serif;
}
.brand-title {
  font-size: 1.08rem;
  font-weight: 700;
  line-height: 1.15;
  color: #ffffff;
}
.brand-city {
  font-weight: 600;
  font-size: 0.88em;
  color: rgba(255, 255, 255, 0.88);
}
.brand-sub {
  font-size: 0.72rem;
  font-weight: 400;
  line-height: 1.25;
  color: rgba(255, 255, 255, 0.85);
}
.menu-wrap {
  position: relative;
  flex-shrink: 0;
}
.menu-btn {
  appearance: none;
  border: none;
  background: transparent;
  color: #ffffff;
  text-decoration: none;
  width: 38px;
  height: 38px;
  border-radius: 10px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background-color 140ms ease;
}
.menu-btn:hover {
  background: rgba(255, 255, 255, 0.14);
}
.menu-popover {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  z-index: 1500;
  min-width: 196px;
  background: var(--bg-popup);
  border: 1px solid var(--border-popover);
  border-radius: 10px;
  padding: 6px;
  box-shadow: var(--shadow-dropdown);
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.menu-item {
  appearance: none;
  border: none;
  background: transparent;
  color: var(--text-primary);
  font: inherit;
  font-size: 0.86rem;
  font-weight: 500;
  text-align: left;
  text-decoration: none;
  padding: 9px 11px;
  border-radius: 8px;
  cursor: pointer;
  transition: background-color 140ms ease;
}
.menu-item:hover {
  background: var(--menu-hover);
}
.menu-gh-icon {
  color: var(--text-muted);
  flex-shrink: 0;
  display: block;
}
.menu-divider {
  height: 1px;
  margin: 4px 6px;
  background: var(--border-subtle);
}
.menu-item-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.theme-switch {
  width: 34px;
  height: 20px;
  border-radius: 999px;
  background: var(--switch-track);
  padding: 2px;
  box-sizing: border-box;
  display: inline-flex;
  align-items: center;
  transition: background-color 140ms ease;
  flex-shrink: 0;
}
.theme-switch.on {
  background: var(--city-accent);
}
.theme-thumb {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #ffffff;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
  transform: translateX(0);
  transition: transform 140ms ease;
}
.theme-switch.on .theme-thumb {
  transform: translateX(14px);
}
@media (max-width: 768px) {
  .brand-bar {
    height: 52px;
    padding-left: 12px;
    padding-right: 12px;
  }
  .brand-logo {
    width: 34px;
    height: 34px;
  }
}
</style>

