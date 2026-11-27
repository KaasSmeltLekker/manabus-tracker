import { ref } from "vue";

export type ThemeMode = "dark" | "light";

const STORAGE_KEY = "ndibise-theme";

function readInitialTheme(): ThemeMode {
  if (typeof window === "undefined") return "dark";
  try {
    return window.localStorage.getItem(STORAGE_KEY) === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

function applyTheme(mode: ThemeMode) {
  if (typeof document === "undefined") return;
  document.documentElement.dataset.theme = mode;
  document.documentElement.style.colorScheme = mode;
}

export const theme = ref<ThemeMode>(readInitialTheme());
applyTheme(theme.value);

function setTheme(mode: ThemeMode) {
  theme.value = mode;
  applyTheme(mode);
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, mode);
  } catch {
  }
}

export function toggleTheme() {
  setTheme(theme.value === "dark" ? "light" : "dark");
}

