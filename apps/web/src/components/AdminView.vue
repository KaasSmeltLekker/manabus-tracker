<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { apiUrl } from "../config";
import { goHome } from "../router";
import {
  type CityPopup,
  type MenuLink,
  compareCorridorCodes,
  contrastText,
  corridorBadgeLabel,
  corridorDisplayVia,
  normalizeColor,
  useBusStore,
} from "../stores/buses";
import {
  decodePolyline,
  encodePolyline,
  type LatLng,
  projectStopToLine,
} from "../stores/polylineUtils";
import { theme, toggleTheme } from "../theme";

interface RawCorridorRow {
  id: string;
  kor?: string;
  kode?: string;
  corridor?: string;
  route_code?: string;
  source_kor?: string | null;
  route_name: string;
  via?: string | null;
  origin?: string;
  toward?: string;
  jam_operasional?: string | null;
  color: string;
  pref?: string;
  points_a?: string | null;
  points_b?: string | null;
}

interface RawStopRow {
  sh_id?: string | number;
  sh_name?: string;
  sh_lat?: string | number;
  sh_lng?: string | number;
  kor?: string;
  in_koridor?: string;
  color_koridor?: string;
  color?: string;
  toward?: string;
}

interface CustomBusRow {
  id: string;
  name: string;
  plate: string;
  corridor: string;
  toward: string;
  lat: number;
  lng: number;
  heading: number;
  speed: number;
  passengers: number;
}

interface CityOverrides {
  added_corridors: RawCorridorRow[];
  removed_corridors: string[];
  corridor_order: string[];
  added_stops: RawStopRow[];
  removed_stops: string[];
  added_buses: CustomBusRow[];
  welcome_popup?: CityPopup | null;
}

interface CorridorEntry {
  code: string;
  sourceCode: string;
  apiCode: string;
  name: string;
  via: string;
  origin: string;
  toward: string;
  hours: string;
  color: string;
  pointsA: string;
  pointsB: string;
  isUpstream: boolean;
  isCustom: boolean;
  isHidden: boolean;
}

interface StopEntry {
  id: string;
  name: string;
  lat: number;
  lng: number;
  codes: string[];
  colors: string[];
  primaryColor: string;
  toward: string;
  isUpstream: boolean;
  isCustom: boolean;
  isHidden: boolean;
}

const SESSION_KEY = "ndibise-admin-key";

const store = useBusStore();
const usernameInput = ref("");
const secretInput = ref("");
const showUsername = ref(true);
const showPassword = ref(false);
const activeUsername = ref("");
const activeSecret = ref("");
const unlocked = ref(false);
const checkingAuth = ref(true);
const authError = ref("");
const resetModalScope = ref<"all" | "routes" | "stops" | "buses" | null>(null);

const selectedPref = ref<"3501" | "1">("3501");
const activeTab = ref<"routes" | "stops" | "buses" | "site">("routes");
const loadingState = ref(false);
const busyAction = ref(false);
const notice = ref<{ type: "ok" | "err"; text: string } | null>(null);

const upstreamCorridors = ref<RawCorridorRow[]>([]);
const upstreamStops = ref<RawStopRow[]>([]);
const overrides = ref<CityOverrides>({
  added_corridors: [],
  removed_corridors: [],
  corridor_order: [],
  added_stops: [],
  removed_stops: [],
  added_buses: [],
});

const routeSearch = ref("");
const draggingCode = ref<string | null>(null);
const dragOverCode = ref<string | null>(null);
const stopSearch = ref("");
const selectedStopRoute = ref("");
const attachStopQuery = ref("");
const stopLimit = ref(60);
const busSearch = ref("");

const routeEditDir = ref<"a" | "b">("a");
const routeStopsModalCode = ref("");

const routeFormOpen = ref(false);
const routeForm = reactive({
  id: "",
  kor: "",
  source_kor: "",
  route_name: "",
  via: "",
  origin: "",
  toward: "",
  color: "#715d80",
  jam_operasional: "05:30 - 21:00",
  points_a: "",
  points_b: "",
});
const polyErrorA = ref("");
const polyErrorB = ref("");

const stopFormOpen = ref(false);
const stopForm = reactive({
  sh_id: "",
  sh_name: "",
  latText: "",
  lngText: "",
  corridorsText: "",
  toward: "",
});

const busFormOpen = ref(false);
const busForm = reactive({
  id: "",
  name: "",
  plate: "",
  corridor: "",
  toward: "",
  latText: "",
  lngText: "",
  speedText: "32",
  headingText: "0",
  passengersText: "12",
});

const menuLinksForm = ref<MenuLink[]>([]);

const popupForm = reactive<CityPopup>({
  enabled: false,
  title: "",
  text: "",
  image_url: "",
});

const popupFileInput = ref<HTMLInputElement | null>(null);

function normCode(raw: unknown): string {
  return String(raw ?? "")
    .trim()
    .toUpperCase()
    .replace(/\s+/g, "");
}

function rawRowCode(row: RawCorridorRow): string {
  return normCode(row.kor ?? row.kode ?? row.corridor ?? row.route_code ?? "");
}

function extractGeoJsonLine(parsed: unknown): LatLng[] | null {
  const toLatLng = (coords: unknown): LatLng[] | null => {
    if (!Array.isArray(coords) || coords.length < 2) return null;
    const pts: LatLng[] = [];
    for (const pair of coords) {
      if (!Array.isArray(pair) || pair.length < 2) continue;
      const lng = Number(pair[0]);
      const lat = Number(pair[1]);
      if (Number.isFinite(lat) && Number.isFinite(lng)) {
        pts.push([lat, lng]);
      }
    }
    return pts.length >= 2 ? pts : null;
  };

  if (!parsed || typeof parsed !== "object") return null;
  const obj = parsed as Record<string, unknown>;
  if (obj.type === "LineString") return toLatLng(obj.coordinates);
  if (obj.type === "MultiLineString" && Array.isArray(obj.coordinates) && obj.coordinates.length > 0) {
    return toLatLng(obj.coordinates[0]);
  }
  if (obj.type === "Feature") return extractGeoJsonLine(obj.geometry);
  if (obj.type === "FeatureCollection" && Array.isArray(obj.features)) {
    for (const feat of obj.features) {
      const pts = extractGeoJsonLine(feat);
      if (pts) return pts;
    }
  }
  return null;
}

function resolvePolylineField(raw: string): { encoded: string; count: number; error: string } {
  const trimmed = raw.trim();
  if (!trimmed) return { encoded: "", count: 0, error: "" };
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    try {
      const parsed = JSON.parse(trimmed);
      const pts = extractGeoJsonLine(parsed);
      if (!pts) {
        return {
          encoded: "",
          count: 0,
          error: "GeoJSON tidak memuat LineString dengan minimal 2 titik.",
        };
      }
      return { encoded: encodePolyline(pts), count: pts.length, error: "" };
    } catch {
      return { encoded: "", count: 0, error: "Format JSON/GeoJSON tidak valid." };
    }
  }
  const decoded = decodePolyline(trimmed);
  if (decoded.length < 2) {
    return {
      encoded: "",
      count: 0,
      error: "Polyline harus berupa encoded polyline atau GeoJSON LineString.",
    };
  }
  return { encoded: trimmed, count: decoded.length, error: "" };
}

const previewPolyA = computed(() => resolvePolylineField(routeForm.points_a));
const previewPolyB = computed(() => resolvePolylineField(routeForm.points_b));

let backdropDownTarget: EventTarget | null = null;

function onBackdropMouseDown(ev: MouseEvent) {
  backdropDownTarget = ev.target;
}

function onBackdropClick(ev: MouseEvent, close: () => void) {
  const startedOnBackdrop = backdropDownTarget === ev.currentTarget;
  backdropDownTarget = null;
  if (startedOnBackdrop && ev.target === ev.currentTarget) {
    close();
  }
}

async function onUploadGeoJson(dir: "a" | "b", ev: Event) {
  const input = ev.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;
  try {
    const text = await file.text();
    const res = resolvePolylineField(text);
    if (res.error) {
      if (dir === "a") polyErrorA.value = res.error;
      else polyErrorB.value = res.error;
      return;
    }
    if (dir === "a") {
      polyErrorA.value = "";
      routeForm.points_a = res.encoded;
    } else {
      polyErrorB.value = "";
      routeForm.points_b = res.encoded;
    }
  } catch {
    if (dir === "a") polyErrorA.value = "Gagal membaca berkas.";
    else polyErrorB.value = "Gagal membaca berkas.";
  }
}

function showNotice(type: "ok" | "err", text: string) {
  notice.value = { type, text };
}

function syncSiteForms(rawLinks: unknown, rawPopup: unknown) {
  const list = Array.isArray(rawLinks) ? rawLinks : [];
  menuLinksForm.value = list.map((raw) => {
    const item = raw && typeof raw === "object" ? (raw as Partial<MenuLink>) : undefined;
    return {
      enabled: Boolean(item?.enabled),
      label: String(item?.label ?? ""),
      url: String(item?.url ?? ""),
    };
  });

  const p = rawPopup && typeof rawPopup === "object" ? (rawPopup as Partial<CityPopup>) : null;
  popupForm.enabled = Boolean(p?.enabled);
  popupForm.title = String(p?.title ?? "");
  popupForm.text = String(p?.text ?? "");
  popupForm.image_url = String(p?.image_url ?? "");
}

function addMenuLinkSlot() {
  menuLinksForm.value.push({ enabled: true, label: "", url: "" });
}

function removeMenuLinkSlot(idx: number) {
  menuLinksForm.value.splice(idx, 1);
}

function onPopupImageFile(ev: Event) {
  const input = ev.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;
  if (!file.type.startsWith("image/")) {
    showNotice("err", "Pilih berkas gambar (PNG, JPG, WebP, atau GIF).");
    return;
  }
  if (file.size > 2 * 1024 * 1024) {
    showNotice("err", "Ukuran gambar maksimal 2 MB.");
    return;
  }
  const reader = new FileReader();
  reader.onload = () => {
    if (typeof reader.result === "string") {
      popupForm.image_url = reader.result;
    }
  };
  reader.onerror = () => {
    showNotice("err", "Gagal membaca berkas gambar.");
  };
  reader.readAsDataURL(file);
}

function clearPopupImage() {
  popupForm.image_url = "";
}

async function saveCityPopup() {
  const title = popupForm.title.trim();
  const text = popupForm.text.trim();
  const image_url = popupForm.image_url.trim();
  if (popupForm.enabled && !title && !text && !image_url) {
    showNotice("err", "Isi minimal teks atau gambar untuk mengaktifkan popup.");
    return;
  }
  busyAction.value = true;
  try {
    const resp = await adminFetch("/api/admin/popup", {
      method: "POST",
      body: JSON.stringify({
        pref: selectedPref.value,
        enabled: popupForm.enabled,
        title,
        text,
        image_url,
      }),
    });
    const json = await resp.json();
    if (!resp.ok) {
      showNotice("err", json.error || "Gagal menyimpan popup.");
      return;
    }
    const saved = json.welcome_popup as CityPopup;
    overrides.value.welcome_popup = saved;
    popupForm.enabled = Boolean(saved.enabled);
    popupForm.title = saved.title;
    popupForm.text = saved.text;
    popupForm.image_url = saved.image_url;
    const cityName = selectedPref.value === "1" ? "Yogyakarta" : "Kota Surabaya";
    showNotice("ok", `Popup ${cityName} berhasil disimpan.`);
    await store.invalidateCity(selectedPref.value);
  } finally {
    busyAction.value = false;
  }
}

async function saveMenuLinks() {
  busyAction.value = true;
  try {
    const resp = await adminFetch("/api/admin/menu-links", {
      method: "POST",
      body: JSON.stringify({
        links: menuLinksForm.value.map((item) => ({
          enabled: item.enabled,
          label: item.label.trim(),
          url: item.url.trim(),
        })),
      }),
    });
    const json = await resp.json();
    if (!resp.ok) {
      showNotice("err", json.error || "Gagal menyimpan tombol menu.");
      return;
    }
    syncSiteForms(json.menu_links, overrides.value.welcome_popup);
    showNotice("ok", "Tombol menu hamburger berhasil disimpan.");
    await Promise.all([store.invalidateCity("3501"), store.invalidateCity("1")]);
  } finally {
    busyAction.value = false;
  }
}

async function adminFetch(url: string, init: RequestInit = {}) {
  const headers = new Headers(init.headers);
  headers.set("X-Admin-Username", activeUsername.value);
  headers.set("X-Admin-Secret", activeSecret.value);
  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }
  const resp = await fetch(apiUrl(url), { ...init, headers, cache: "no-store" });
  if (resp.status === 401) {
    lockPanel("Sesi berakhir atau kredensial tidak valid.");
    throw new Error("Unauthorized");
  }
  return resp;
}

async function loadAdminState() {
  loadingState.value = true;
  try {
    const resp = await adminFetch(`/api/admin/state?pref=${encodeURIComponent(selectedPref.value)}`);
    const json = await resp.json();
    if (!resp.ok) {
      showNotice("err", json.error || "Gagal memuat data.");
      return;
    }
    upstreamCorridors.value = Array.isArray(json.upstream_corridors) ? json.upstream_corridors : [];
    upstreamStops.value = Array.isArray(json.upstream_stops) ? json.upstream_stops : [];
    overrides.value = {
      added_corridors: json.overrides?.added_corridors ?? [],
      removed_corridors: json.overrides?.removed_corridors ?? [],
      corridor_order: json.overrides?.corridor_order ?? [],
      added_stops: json.overrides?.added_stops ?? [],
      removed_stops: json.overrides?.removed_stops ?? [],
      added_buses: json.overrides?.added_buses ?? [],
      welcome_popup: json.overrides?.welcome_popup ?? null,
    };
    syncSiteForms(json.menu_links, json.overrides?.welcome_popup);
  } catch (e) {
    if (e instanceof Error && e.message !== "Unauthorized") {
      showNotice("err", "Gagal terhubung ke server.");
    }
  } finally {
    loadingState.value = false;
  }
}

type AuthCheckResult = "ok" | "invalid" | "unconfigured" | "ratelimited" | "unreachable";

async function verifyKey(username: string, secret: string): Promise<AuthCheckResult> {
  try {
    const resp = await fetch(apiUrl("/api/admin/auth"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ username, secret }),
      cache: "no-store",
    });
    if (resp.ok) return "ok";
    if (resp.status === 401) return "invalid";
    if (resp.status === 429) return "ratelimited";
    if (resp.status === 503) return "unconfigured";
    return "unreachable";
  } catch {
    return "unreachable";
  }
}

async function submitUnlock() {
  const user = usernameInput.value.trim();
  const key = secretInput.value.trim();
  if (!user || !key) {
    authError.value = "Masukkan username dan password.";
    return;
  }
  checkingAuth.value = true;
  authError.value = "";
  try {
    const status = await verifyKey(user, key);
    if (status === "invalid") {
      authError.value = "Username atau password salah.";
      return;
    }
    if (status === "ratelimited") {
      authError.value = "Terlalu banyak percobaan gagal. Coba lagi dalam beberapa menit.";
      return;
    }
    if (status === "unconfigured") {
      authError.value = "ADMIN_SECRET belum dikonfigurasi di server.";
      return;
    }
    if (status === "unreachable") {
      authError.value = "Proxy server tidak aktif atau gagal terhubung.";
      return;
    }
    activeUsername.value = user;
    activeSecret.value = key;
    unlocked.value = true;
    secretInput.value = "";
    try {
      sessionStorage.setItem(SESSION_KEY, JSON.stringify({ username: user, secret: key }));
    } catch {
    }
    await loadAdminState();
  } finally {
    checkingAuth.value = false;
  }
}

function lockPanel(msg = "") {
  unlocked.value = false;
  activeUsername.value = "";
  activeSecret.value = "";
  authError.value = msg;
  try {
    sessionStorage.removeItem(SESSION_KEY);
  } catch {
  }
}

async function switchCity(pref: "3501" | "1") {
  if (selectedPref.value === pref) return;
  selectedPref.value = pref;
  notice.value = null;
  routeFormOpen.value = false;
  routeStopsModalCode.value = "";
  stopFormOpen.value = false;
  busFormOpen.value = false;
  selectedStopRoute.value = "";
  attachStopQuery.value = "";
  stopLimit.value = 60;
  await loadAdminState();
}

const routeEntries = computed<CorridorEntry[]>(() => {
  const hiddenSet = new Set(overrides.value.removed_corridors.map(normCode));
  const customByKey = new Map<string, RawCorridorRow>();
  for (const c of overrides.value.added_corridors) {
    const matchKey = normCode(c.source_kor || c.kor || c.kode || c.corridor || c.route_code || "");
    if (matchKey) customByKey.set(matchKey, c);
  }
  const matchedCustom = new Set<RawCorridorRow>();
  const seen = new Set<string>();
  const out: CorridorEntry[] = [];

  for (const row of upstreamCorridors.value) {
    const origCode = rawRowCode(row);
    if (!origCode || (selectedPref.value === "3501" && origCode === "SBT") || seen.has(origCode)) {
      continue;
    }
    seen.add(origCode);
    const custom = customByKey.get(origCode);
    if (custom) matchedCustom.add(custom);
    const active = custom ?? row;
    const code = rawRowCode(active) || origCode;
    out.push({
      code,
      sourceCode: origCode,
      apiCode: origCode,
      name: String(active.route_name ?? code).trim(),
      via:
        typeof active.via === "string"
          ? active.via.trim()
          : (corridorDisplayVia({
              id: active.id,
              kor: origCode,
              route_name: active.route_name,
              color: active.color,
              pref: selectedPref.value,
            }) ?? ""),
      origin: String(active.origin ?? "").trim(),
      toward: String(active.toward ?? "").trim(),
      hours: String(active.jam_operasional ?? "").trim(),
      color: normalizeColor(active.color) ?? normalizeColor(row.color) ?? "#0284c7",
      pointsA: String(custom?.points_a || row.points_a || ""),
      pointsB: String(custom?.points_b || row.points_b || ""),
      isUpstream: true,
      isCustom: Boolean(custom),
      isHidden: hiddenSet.has(origCode),
    });
  }

  for (const c of overrides.value.added_corridors) {
    if (matchedCustom.has(c)) continue;
    const code = rawRowCode(c);
    if (!code) continue;
    const apiCode = c.source_kor ? normCode(c.source_kor) : "";
    out.push({
      code,
      sourceCode: apiCode || code,
      apiCode,
      name: String(c.route_name ?? code).trim(),
      via: String(c.via ?? "").trim(),
      origin: String(c.origin ?? "").trim(),
      toward: String(c.toward ?? "").trim(),
      hours: String(c.jam_operasional ?? "").trim(),
      color: normalizeColor(c.color) ?? "#0284c7",
      pointsA: String(c.points_a ?? ""),
      pointsB: String(c.points_b ?? ""),
      isUpstream: false,
      isCustom: true,
      isHidden: false,
    });
  }

  const orderRanks = new Map<string, number>();
  overrides.value.corridor_order.forEach((c, idx) => {
    const key = normCode(c);
    if (key && !orderRanks.has(key)) orderRanks.set(key, idx);
  });

  out.sort((a, b) => {
    const ra = orderRanks.get(a.code) ?? orderRanks.get(a.sourceCode) ?? Number.MAX_SAFE_INTEGER;
    const rb = orderRanks.get(b.code) ?? orderRanks.get(b.sourceCode) ?? Number.MAX_SAFE_INTEGER;
    if (ra !== rb) return ra - rb;
    return compareCorridorCodes(a.code, b.code);
  });

  return out;
});

const canReorderRoutes = computed(() => routeSearch.value.trim() === "");

const activeCorridorOptions = computed(() =>
  routeEntries.value.filter((r) => !r.isHidden),
);

const filteredRoutes = computed(() => {
  const q = routeSearch.value.trim().toLowerCase();
  if (!q) return routeEntries.value;
  return routeEntries.value.filter(
    (r) =>
      r.code.toLowerCase().includes(q) ||
      r.apiCode.toLowerCase().includes(q) ||
      r.name.toLowerCase().includes(q) ||
      r.via.toLowerCase().includes(q) ||
      r.origin.toLowerCase().includes(q) ||
      r.toward.toLowerCase().includes(q),
  );
});

const stopEntries = computed<StopEntry[]>(() => {
  const hiddenSet = new Set(overrides.value.removed_stops.map((id) => String(id).trim()));
  const customMap = new Map<string, RawStopRow>();
  for (const s of overrides.value.added_stops) {
    const id = String(s.sh_id ?? "").trim();
    if (id) customMap.set(id, s);
  }
  const renameMap = new Map<string, string>();
  const colorByCode = new Map<string, string>();
  for (const r of routeEntries.value) {
    colorByCode.set(normCode(r.code), r.color);
    if (r.sourceCode !== r.code) {
      renameMap.set(normCode(r.sourceCode), r.code);
    }
  }
  const seen = new Set<string>();
  const out: StopEntry[] = [];

  const toEntry = (
    row: RawStopRow,
    isUpstream: boolean,
    isCustom: boolean,
    isHidden: boolean,
  ): StopEntry | null => {
    const lat = Number(row.sh_lat);
    const lng = Number(row.sh_lng);
    if (!Number.isFinite(lat) || !Number.isFinite(lng)) return null;
    const id = String(row.sh_id ?? `${lat},${lng}`).trim();
    const rawCodes = String(row.in_koridor || row.kor || "")
      .split(/[|,]/)
      .map((x) => x.trim().toUpperCase())
      .filter((x) => x && !(selectedPref.value === "3501" && x === "SBT"));
    const codes: string[] = [];
    for (const raw of rawCodes) {
      const mapped = renameMap.get(normCode(raw)) ?? raw;
      if (!codes.some((c) => normCode(c) === normCode(mapped))) {
        codes.push(mapped);
      }
    }
    const rawColors = String(row.color_koridor || row.color || "")
      .split(/[|,]/)
      .map((x) => normalizeColor(x) ?? "#64748b");
    const fallbackCode = renameMap.get(normCode(row.kor ?? "")) ?? String(row.kor ?? "-").trim().toUpperCase();
    const finalCodes = codes.length > 0 ? codes : [fallbackCode];
    const colors = finalCodes.map(
      (c, i) => colorByCode.get(normCode(c)) ?? rawColors[i] ?? normalizeColor(row.color) ?? "#64748b",
    );
    return {
      id,
      name: String(row.sh_name ?? "Halte").trim(),
      lat,
      lng,
      codes: finalCodes,
      colors,
      primaryColor: colors[0] ?? "#64748b",
      toward: String(row.toward ?? "").trim(),
      isUpstream,
      isCustom,
      isHidden,
    };
  };

  for (const row of upstreamStops.value) {
    const id = String(row.sh_id ?? "").trim();
    if (!id || seen.has(id)) continue;
    seen.add(id);
    const custom = customMap.get(id);
    const entry = toEntry(custom ?? row, true, Boolean(custom), hiddenSet.has(id));
    if (entry) out.push(entry);
  }

  for (const [id, s] of customMap) {
    if (seen.has(id)) continue;
    const entry = toEntry(s, false, true, false);
    if (entry) out.push(entry);
  }

  return out;
});

function stopMatchesRouteDir(
  stop: StopEntry,
  routeOrigin: string,
  routeToward: string,
  polyAEncoded: string,
  polyBEncoded: string,
  dir: "a" | "b",
): boolean {
  const stopTow = stop.toward.trim().toLowerCase();
  const orig = routeOrigin.trim().toLowerCase();
  const dest = routeToward.trim().toLowerCase();

  if (stopTow && orig && dest && orig !== dest) {
    const matchDest = stopTow === dest || stopTow.includes(dest) || dest.includes(stopTow);
    const matchOrig = stopTow === orig || stopTow.includes(orig) || orig.includes(stopTow);
    if (matchDest && !matchOrig) return dir === "a";
    if (matchOrig && !matchDest) return dir === "b";
  }

  const ptsA = decodePolyline(polyAEncoded);
  const ptsB = decodePolyline(polyBEncoded);
  if (ptsA.length >= 2 && ptsB.length >= 2) {
    const dA = projectStopToLine(stop.lat, stop.lng, ptsA).dist;
    const dB = projectStopToLine(stop.lat, stop.lng, ptsB).dist;
    return dir === "a" ? dA <= dB : dB < dA;
  }

  return true;
}

const filteredStops = computed(() => {
  const targetRoute = normCode(selectedStopRoute.value);
  const q = stopSearch.value.trim().toLowerCase();
  return stopEntries.value.filter((s) => {
    if (targetRoute && !s.codes.some((c) => normCode(c) === targetRoute)) {
      return false;
    }
    if (!q) return true;
    return (
      s.name.toLowerCase().includes(q) ||
      s.id.toLowerCase().includes(q) ||
      s.codes.some((c) => c.toLowerCase().includes(q)) ||
      s.toward.toLowerCase().includes(q)
    );
  });
});

const activeModalRouteCode = computed(() =>
  normCode(
    routeFormOpen.value
      ? routeForm.kor
      : routeStopsModalCode.value
        ? routeStopsModalCode.value
        : selectedStopRoute.value,
  ),
);

const routeStopsModalEntry = computed<CorridorEntry | null>(() => {
  const code = normCode(routeStopsModalCode.value);
  if (!code) return null;
  return routeEntries.value.find((r) => normCode(r.code) === code) ?? null;
});

const activeRouteDirOrigin = computed(() => {
  if (routeFormOpen.value) return routeForm.origin.trim();
  if (routeStopsModalEntry.value) return routeStopsModalEntry.value.origin.trim();
  const r = routeEntries.value.find((e) => normCode(e.code) === normCode(selectedStopRoute.value));
  return r?.origin.trim() ?? "";
});

const activeRouteDirToward = computed(() => {
  if (routeFormOpen.value) return routeForm.toward.trim();
  if (routeStopsModalEntry.value) return routeStopsModalEntry.value.toward.trim();
  const r = routeEntries.value.find((e) => normCode(e.code) === normCode(selectedStopRoute.value));
  return r?.toward.trim() ?? "";
});

const activeRouteDirDestination = computed(() =>
  routeEditDir.value === "a" ? activeRouteDirToward.value : activeRouteDirOrigin.value,
);

const routeFormStops = computed(() => {
  const code = normCode(routeForm.kor);
  if (!code) return [];
  const polyA = resolvePolylineField(routeForm.points_a).encoded;
  const polyB = resolvePolylineField(routeForm.points_b).encoded;
  return stopEntries.value.filter(
    (s) =>
      !s.isHidden &&
      s.codes.some((c) => normCode(c) === code) &&
      stopMatchesRouteDir(
        s,
        routeForm.origin,
        routeForm.toward,
        polyA,
        polyB,
        routeEditDir.value,
      ),
  );
});

const routeStopsModalList = computed(() => {
  const entry = routeStopsModalEntry.value;
  if (!entry) return [];
  const code = normCode(entry.code);
  return stopEntries.value.filter(
    (s) =>
      !s.isHidden &&
      s.codes.some((c) => normCode(c) === code) &&
      stopMatchesRouteDir(
        s,
        entry.origin,
        entry.toward,
        entry.pointsA,
        entry.pointsB,
        routeEditDir.value,
      ),
  );
});

const attachableStops = computed(() => {
  const targetCode = activeModalRouteCode.value;
  const q = attachStopQuery.value.trim().toLowerCase();
  if (!targetCode || !q) return [];
  const results: StopEntry[] = [];
  for (const s of stopEntries.value) {
    if (s.isHidden || s.codes.some((c) => normCode(c) === targetCode)) continue;
    if (
      s.name.toLowerCase().includes(q) ||
      s.id.toLowerCase().includes(q) ||
      s.toward.toLowerCase().includes(q)
    ) {
      results.push(s);
      if (results.length >= 8) break;
    }
  }
  return results;
});

const visibleStops = computed(() => filteredStops.value.slice(0, stopLimit.value));

const filteredBuses = computed(() => {
  const q = busSearch.value.trim().toLowerCase();
  const list = overrides.value.added_buses ?? [];
  if (!q) return list;
  return list.filter(
    (b) =>
      b.name.toLowerCase().includes(q) ||
      b.plate.toLowerCase().includes(q) ||
      b.corridor.toLowerCase().includes(q) ||
      b.toward.toLowerCase().includes(q),
  );
});

const summaryStats = computed(() => ({
  activeRoutes: routeEntries.value.filter((r) => !r.isHidden).length,
  activeStops: stopEntries.value.filter((s) => !s.isHidden).length,
}));

const hasRouteEdits = computed(
  () =>
    overrides.value.added_corridors.length > 0 ||
    overrides.value.removed_corridors.length > 0 ||
    overrides.value.corridor_order.length > 0,
);

const hasStopEdits = computed(
  () => overrides.value.added_stops.length > 0 || overrides.value.removed_stops.length > 0,
);

const hasBusEdits = computed(() => (overrides.value.added_buses?.length ?? 0) > 0);

const hasAnyEdits = computed(
  () => hasRouteEdits.value || hasStopEdits.value || hasBusEdits.value,
);

const resetModalCopy = computed(() => {
  const cityLabel = selectedPref.value === "1" ? "Yogyakarta" : "Surabaya";
  switch (resetModalScope.value) {
    case "routes":
      return {
        title: `Reset Semua Rute (${cityLabel})`,
        body: "Semua rute kustom, perubahan rute, dan urutan koridor untuk kota ini akan dihapus dan dikembalikan ke data asli dari API.",
        notice: "Semua perubahan rute dikembalikan ke bawaan",
      };
    case "stops":
      return {
        title: `Reset Semua Halte (${cityLabel})`,
        body: "Semua halte kustom, penyembunyian halte, dan perubahan rute halte untuk kota ini akan dihapus dan dikembalikan ke data asli dari API.",
        notice: "Semua perubahan halte dikembalikan ke bawaan",
      };
    case "buses":
      return {
        title: `Reset Semua Armada (${cityLabel})`,
        body: "Semua armada kustom untuk kota ini akan dihapus permanen dan langsung menghilang dari peta.",
        notice: "Semua armada kustom dihapus",
      };
    default:
      return {
        title: `Reset Semua Data (${cityLabel})`,
        body: "Semua perubahan rute, urutan koridor, halte, dan armada kustom untuk kota ini akan dihapus dan dikembalikan ke data asli dari API.",
        notice: "Semua data kota dikembalikan ke bawaan",
      };
  }
});

async function confirmResetScope() {
  const scope = resetModalScope.value;
  if (!scope) return;
  busyAction.value = true;
  try {
    const resp = await adminFetch(
      `/api/admin/reset/${encodeURIComponent(selectedPref.value)}?scope=${encodeURIComponent(scope)}`,
      { method: "POST" },
    );
    const json = await resp.json();
    if (!resp.ok) {
      showNotice("err", json.error || "Gagal mereset data.");
      return;
    }
    const okMsg = resetModalCopy.value.notice;
    resetModalScope.value = null;
    if (scope === "all" || scope === "stops") {
      selectedStopRoute.value = "";
      attachStopQuery.value = "";
    }
    showNotice("ok", okMsg);
    await Promise.all([loadAdminState(), store.invalidateCity(selectedPref.value)]);
  } finally {
    busyAction.value = false;
  }
}

function openNewRouteForm() {
  routeForm.id = "";
  routeForm.kor = "";
  routeForm.source_kor = "";
  routeForm.route_name = "";
  routeForm.via = "";
  routeForm.origin = "";
  routeForm.toward = "";
  routeForm.color = selectedPref.value === "1" ? "#eab308" : "#715d80";
  routeForm.jam_operasional = "05:30 - 21:00";
  routeForm.points_a = "";
  routeForm.points_b = "";
  polyErrorA.value = "";
  polyErrorB.value = "";
  routeEditDir.value = "a";
  attachStopQuery.value = "";
  routeFormOpen.value = true;
}

function editRouteEntry(entry: CorridorEntry) {
  routeForm.id = `edit-${selectedPref.value}-${entry.sourceCode.toLowerCase()}`;
  routeForm.kor = entry.code;
  routeForm.source_kor = entry.apiCode;
  routeForm.route_name = entry.name;
  routeForm.via = entry.via;
  routeForm.origin = entry.origin;
  routeForm.toward = entry.toward;
  routeForm.color = entry.color;
  routeForm.jam_operasional = entry.hours;
  routeForm.points_a = entry.pointsA;
  routeForm.points_b = entry.pointsB;
  polyErrorA.value = "";
  polyErrorB.value = "";
  routeEditDir.value = "a";
  attachStopQuery.value = "";
  routeFormOpen.value = true;
}

async function saveRouteForm() {
  polyErrorA.value = "";
  polyErrorB.value = "";
  const kor = routeForm.kor.trim().toUpperCase();
  const apiKor = routeForm.source_kor.trim().toUpperCase();
  const routeName = routeForm.route_name.trim();
  if (!kor || !routeName) {
    showNotice("err", "Kode rute dan nama rute wajib diisi.");
    return;
  }
  const polyA = resolvePolylineField(routeForm.points_a);
  if (polyA.error) {
    polyErrorA.value = polyA.error;
    routeEditDir.value = "a";
    return;
  }
  const polyB = resolvePolylineField(routeForm.points_b);
  if (polyB.error) {
    polyErrorB.value = polyB.error;
    routeEditDir.value = "b";
    return;
  }
  busyAction.value = true;
  try {
    const resp = await adminFetch("/api/admin/corridors", {
      method: "POST",
      body: JSON.stringify({
        id: routeForm.id,
        kor,
        source_kor: apiKor || null,
        route_name: routeName,
        via: routeForm.via.trim() || null,
        origin: routeForm.origin.trim(),
        toward: routeForm.toward.trim(),
        jam_operasional: routeForm.jam_operasional.trim() || null,
        color: routeForm.color.trim() || "#0284c7",
        pref: selectedPref.value,
        points_a: polyA.encoded || null,
        points_b: polyB.encoded || null,
      }),
    });
    const json = await resp.json();
    if (!resp.ok) {
      showNotice("err", json.error || "Gagal menyimpan rute.");
      return;
    }
    routeFormOpen.value = false;
    showNotice("ok", `Rute ${kor} berhasil disimpan.`);
    await Promise.all([loadAdminState(), store.invalidateCity(selectedPref.value)]);
  } finally {
    busyAction.value = false;
  }
}

async function persistRouteOrder(order: string[], msg: string) {
  busyAction.value = true;
  try {
    const resp = await adminFetch("/api/admin/corridors/order", {
      method: "POST",
      body: JSON.stringify({ pref: selectedPref.value, order }),
    });
    const json = await resp.json();
    if (!resp.ok) {
      showNotice("err", json.error || "Gagal menyimpan urutan rute.");
      return;
    }
    overrides.value.corridor_order = json.overrides?.corridor_order ?? order;
    showNotice("ok", msg);
    await store.invalidateCity(selectedPref.value);
  } finally {
    busyAction.value = false;
  }
}

function onRouteDragStart(code: string, ev: DragEvent) {
  if (!canReorderRoutes.value || busyAction.value) return;
  draggingCode.value = code;
  if (ev.dataTransfer) {
    ev.dataTransfer.effectAllowed = "move";
    ev.dataTransfer.setData("text/plain", code);
  }
}

function onRouteDragOver(code: string) {
  if (!draggingCode.value || draggingCode.value === code) return;
  dragOverCode.value = code;
}

async function onRouteDrop(targetCode: string) {
  const fromCode = draggingCode.value;
  draggingCode.value = null;
  dragOverCode.value = null;
  if (!fromCode || fromCode === targetCode || !canReorderRoutes.value) return;
  const next = routeEntries.value.map((r) => r.code);
  const fromIdx = next.indexOf(fromCode);
  const toIdx = next.indexOf(targetCode);
  if (fromIdx < 0 || toIdx < 0) return;
  const [item] = next.splice(fromIdx, 1);
  next.splice(toIdx, 0, item);
  await persistRouteOrder(next, "Urutan rute diperbarui.");
}

function onRouteDragEnd() {
  draggingCode.value = null;
  dragOverCode.value = null;
}

async function resetRouteOrder() {
  await persistRouteOrder([], "Urutan rute dikembalikan ke bawaan.");
}

async function removeRouteEntry(entry: CorridorEntry, hideUpstream: boolean) {
  busyAction.value = true;
  try {
    const url = `/api/admin/corridors/${encodeURIComponent(selectedPref.value)}/${encodeURIComponent(entry.sourceCode)}${hideUpstream ? "?upstream=true" : ""}`;
    const resp = await adminFetch(url, { method: "DELETE" });
    const json = await resp.json();
    if (!resp.ok) {
      showNotice("err", json.error || "Gagal menghapus rute.");
      return;
    }
    showNotice(
      "ok",
      hideUpstream
        ? `Rute ${entry.code} disembunyikan dari peta.`
        : `Perubahan rute ${entry.code} dihapus.`,
    );
    await Promise.all([loadAdminState(), store.invalidateCity(selectedPref.value)]);
  } finally {
    busyAction.value = false;
  }
}

async function restoreRouteEntry(entry: CorridorEntry) {
  busyAction.value = true;
  try {
    const resp = await adminFetch(
      `/api/admin/corridors/${encodeURIComponent(selectedPref.value)}/${encodeURIComponent(entry.sourceCode)}/restore`,
      { method: "POST" },
    );
    const json = await resp.json();
    if (!resp.ok) {
      showNotice("err", json.error || "Gagal memulihkan rute.");
      return;
    }
    showNotice("ok", `Rute ${entry.code} dipulihkan.`);
    await Promise.all([loadAdminState(), store.invalidateCity(selectedPref.value)]);
  } finally {
    busyAction.value = false;
  }
}

function selectStopRouteFilter(code: string) {
  selectedStopRoute.value = selectedStopRoute.value === code ? "" : code;
  attachStopQuery.value = "";
  stopLimit.value = 60;
}

function openRouteStopsModal(code: string) {
  routeStopsModalCode.value = code.trim().toUpperCase();
  routeEditDir.value = "a";
  attachStopQuery.value = "";
}

function closeRouteStopsModal() {
  routeStopsModalCode.value = "";
  attachStopQuery.value = "";
}

function openNewStopForm(prefillCode?: string, prefillToward?: string) {
  stopForm.sh_id = "";
  stopForm.sh_name = "";
  stopForm.latText = "";
  stopForm.lngText = "";
  stopForm.corridorsText = prefillCode ?? selectedStopRoute.value;
  stopForm.toward = prefillToward ?? "";
  stopFormOpen.value = true;
}

function editStopEntry(entry: StopEntry) {
  stopForm.sh_id = entry.id;
  stopForm.sh_name = entry.name;
  stopForm.latText = String(entry.lat);
  stopForm.lngText = String(entry.lng);
  stopForm.corridorsText = entry.codes.join(", ");
  stopForm.toward = entry.toward;
  stopFormOpen.value = true;
}

async function updateStopCorridors(
  entry: StopEntry,
  nextCodes: string[],
  successMsg: string,
  nextToward?: string,
) {
  if (nextCodes.length === 0) {
    await removeStopEntry(entry, entry.isUpstream);
    return;
  }
  const colorMap = new Map<string, string>();
  for (const r of routeEntries.value) {
    colorMap.set(normCode(r.code), r.color);
  }
  const colors = nextCodes.map((c) => colorMap.get(normCode(c)) ?? "#64748b");
  busyAction.value = true;
  try {
    const resp = await adminFetch("/api/admin/stops", {
      method: "POST",
      body: JSON.stringify({
        pref: selectedPref.value,
        sh_id: entry.id,
        sh_name: entry.name,
        sh_lat: entry.lat,
        sh_lng: entry.lng,
        kor: nextCodes[0],
        in_koridor: nextCodes.join("|"),
        color_koridor: colors.join("|"),
        color: colors[0],
        toward: nextToward !== undefined ? nextToward : entry.toward,
      }),
    });
    const json = await resp.json();
    if (!resp.ok) {
      showNotice("err", json.error || "Gagal memperbarui halte.");
      return;
    }
    showNotice("ok", successMsg);
    await Promise.all([loadAdminState(), store.invalidateCity(selectedPref.value)]);
  } finally {
    busyAction.value = false;
  }
}

async function attachStopToRoute(entry: StopEntry, rawRouteCode: string, dirDestination?: string) {
  const code = rawRouteCode.trim().toUpperCase();
  if (!code) return;
  if (entry.codes.some((c) => normCode(c) === normCode(code))) return;
  attachStopQuery.value = "";
  const toward = !entry.toward && dirDestination ? dirDestination : entry.toward;
  await updateStopCorridors(
    entry,
    [...entry.codes, code],
    `${entry.name} ditambahkan ke rute ${code}`,
    toward,
  );
}

async function detachStopFromRoute(entry: StopEntry, rawRouteCode: string) {
  const code = rawRouteCode.trim().toUpperCase();
  const next = entry.codes.filter((c) => normCode(c) !== normCode(code));
  await updateStopCorridors(
    entry,
    next,
    next.length > 0
      ? `${entry.name} dilepas dari rute ${code}`
      : `${entry.name} disembunyikan`,
  );
}

function onLatInput(ev: Event) {
  const val = (ev.target as HTMLInputElement).value;
  const parts = val.split(",").map((s) => s.trim());
  if (parts.length === 2 && Number.isFinite(Number(parts[0])) && Number.isFinite(Number(parts[1]))) {
    stopForm.latText = parts[0];
    stopForm.lngText = parts[1];
  } else {
    stopForm.latText = val;
  }
}

function toggleStopCorridorChip(code: string) {
  const existing = stopForm.corridorsText
    .split(/[,|]/)
    .map((s) => s.trim().toUpperCase())
    .filter(Boolean);
  const idx = existing.indexOf(code);
  if (idx >= 0) existing.splice(idx, 1);
  else existing.push(code);
  stopForm.corridorsText = existing.join(", ");
}

function stopHasChip(code: string): boolean {
  return stopForm.corridorsText
    .split(/[,|]/)
    .map((s) => s.trim().toUpperCase())
    .includes(code);
}

async function saveStopForm() {
  const name = stopForm.sh_name.trim();
  const lat = Number(stopForm.latText.trim());
  const lng = Number(stopForm.lngText.trim());
  const codes = stopForm.corridorsText
    .split(/[,|]/)
    .map((s) => s.trim().toUpperCase())
    .filter(Boolean);

  if (!name || !Number.isFinite(lat) || !Number.isFinite(lng) || codes.length === 0) {
    showNotice("err", "Nama halte, koordinat valid, dan minimal satu koridor wajib diisi.");
    return;
  }

  const colorMap = new Map<string, string>();
  for (const r of routeEntries.value) {
    colorMap.set(r.code, r.color);
  }
  const colors = codes.map((c) => colorMap.get(normCode(c)) ?? "#64748b");

  busyAction.value = true;
  try {
    const resp = await adminFetch("/api/admin/stops", {
      method: "POST",
      body: JSON.stringify({
        pref: selectedPref.value,
        sh_id: stopForm.sh_id,
        sh_name: name,
        sh_lat: lat,
        sh_lng: lng,
        kor: codes[0],
        in_koridor: codes.join("|"),
        color_koridor: colors.join("|"),
        color: colors[0],
        toward: stopForm.toward.trim(),
      }),
    });
    const json = await resp.json();
    if (!resp.ok) {
      showNotice("err", json.error || "Gagal menyimpan halte.");
      return;
    }
    stopFormOpen.value = false;
    showNotice("ok", `${name} berhasil disimpan`);
    await Promise.all([loadAdminState(), store.invalidateCity(selectedPref.value)]);
  } finally {
    busyAction.value = false;
  }
}

async function removeStopEntry(entry: StopEntry, hideUpstream: boolean) {
  busyAction.value = true;
  try {
    const url = `/api/admin/stops/${encodeURIComponent(selectedPref.value)}/${encodeURIComponent(entry.id)}${hideUpstream ? "?upstream=true" : ""}`;
    const resp = await adminFetch(url, { method: "DELETE" });
    const json = await resp.json();
    if (!resp.ok) {
      showNotice("err", json.error || "Gagal menghapus halte.");
      return;
    }
    showNotice(
      "ok",
      hideUpstream
        ? `${entry.name} disembunyikan dari peta`
        : `${entry.name} dihapus`,
    );
    await Promise.all([loadAdminState(), store.invalidateCity(selectedPref.value)]);
  } finally {
    busyAction.value = false;
  }
}

async function restoreStopEntry(entry: StopEntry) {
  busyAction.value = true;
  try {
    const resp = await adminFetch(
      `/api/admin/stops/${encodeURIComponent(selectedPref.value)}/${encodeURIComponent(entry.id)}/restore`,
      { method: "POST" },
    );
    const json = await resp.json();
    if (!resp.ok) {
      showNotice("err", json.error || "Gagal memulihkan halte.");
      return;
    }
    showNotice("ok", `${entry.name} dipulihkan`);
    await Promise.all([loadAdminState(), store.invalidateCity(selectedPref.value)]);
  } finally {
    busyAction.value = false;
  }
}

const selectedBusRouteEntry = computed<CorridorEntry | null>(() => {
  const code = normCode(busForm.corridor);
  if (!code) return null;
  return routeEntries.value.find((r) => normCode(r.code) === code) ?? null;
});

function busColorForCode(code: string): string {
  const found = routeEntries.value.find((r) => normCode(r.code) === normCode(code));
  return found?.color ?? "#991b1b";
}

function defaultCityCenter(): [number, number] {
  return selectedPref.value === "1" ? [-7.7956, 110.3695] : [-7.2575, 112.7521];
}

function pickBusPointFromRoute(code: string) {
  const route = routeEntries.value.find((r) => normCode(r.code) === normCode(code));
  if (route) {
    const pts = decodePolyline(route.pointsA || route.pointsB);
    if (pts.length > 0) {
      const mid = pts[Math.floor(pts.length / 2)];
      busForm.latText = mid[0].toFixed(5);
      busForm.lngText = mid[1].toFixed(5);
      return;
    }
  }
  const [lat, lng] = defaultCityCenter();
  busForm.latText = lat.toFixed(5);
  busForm.lngText = lng.toFixed(5);
}

function selectBusCorridor(code: string) {
  busForm.corridor = code;
  const route = routeEntries.value.find((r) => normCode(r.code) === normCode(code));
  if (route && !busForm.toward.trim()) {
    busForm.toward = route.toward || route.origin;
  }
  if (!busForm.latText.trim() || !busForm.lngText.trim()) {
    pickBusPointFromRoute(code);
  }
}

function onBusLatInput(ev: Event) {
  const val = (ev.target as HTMLInputElement).value;
  const parts = val.split(",").map((s) => s.trim());
  if (parts.length === 2 && Number.isFinite(Number(parts[0])) && Number.isFinite(Number(parts[1]))) {
    busForm.latText = parts[0];
    busForm.lngText = parts[1];
  } else {
    busForm.latText = val;
  }
}

function openNewBusForm() {
  const firstRoute = activeCorridorOptions.value[0];
  busForm.id = "";
  busForm.name = "";
  busForm.plate = "";
  busForm.corridor = firstRoute?.code ?? "";
  busForm.toward = firstRoute?.toward || firstRoute?.origin || "";
  busForm.speedText = "32";
  busForm.headingText = "0";
  busForm.passengersText = "12";
  if (firstRoute) {
    pickBusPointFromRoute(firstRoute.code);
  } else {
    const [lat, lng] = defaultCityCenter();
    busForm.latText = lat.toFixed(5);
    busForm.lngText = lng.toFixed(5);
  }
  busFormOpen.value = true;
}

function editBusEntry(bus: CustomBusRow) {
  busForm.id = bus.id;
  busForm.name = bus.name;
  busForm.plate = bus.plate;
  busForm.corridor = bus.corridor;
  busForm.toward = bus.toward;
  busForm.latText = String(bus.lat);
  busForm.lngText = String(bus.lng);
  busForm.speedText = String(bus.speed);
  busForm.headingText = String(bus.heading);
  busForm.passengersText = String(bus.passengers);
  busFormOpen.value = true;
}

async function saveBusForm() {
  const name = busForm.name.trim();
  const corridor = busForm.corridor.trim().toUpperCase();
  const lat = Number(busForm.latText.trim());
  const lng = Number(busForm.lngText.trim());
  const speed = Number(busForm.speedText.trim() || "0");
  const heading = Number(busForm.headingText.trim() || "0");
  const passengers = Math.max(0, Math.round(Number(busForm.passengersText.trim() || "0")));

  if (!name || !corridor || !Number.isFinite(lat) || !Number.isFinite(lng)) {
    showNotice("err", "Nama armada, kode rute, dan koordinat valid wajib diisi.");
    return;
  }

  busyAction.value = true;
  try {
    const resp = await adminFetch("/api/admin/buses", {
      method: "POST",
      body: JSON.stringify({
        pref: selectedPref.value,
        id: busForm.id,
        name,
        plate: busForm.plate.trim(),
        corridor,
        toward: busForm.toward.trim(),
        lat,
        lng,
        speed: Number.isFinite(speed) ? speed : 0,
        heading: Number.isFinite(heading) ? heading : 0,
        passengers: Number.isFinite(passengers) ? passengers : 0,
      }),
    });
    const json = await resp.json();
    if (!resp.ok) {
      showNotice("err", json.error || "Gagal menyimpan armada.");
      return;
    }
    busFormOpen.value = false;
    showNotice("ok", `Armada ${name} berhasil disimpan dan disiarkan ke peta.`);
    await Promise.all([loadAdminState(), store.invalidateCity(selectedPref.value)]);
  } finally {
    busyAction.value = false;
  }
}

async function removeBusEntry(bus: CustomBusRow) {
  busyAction.value = true;
  try {
    const resp = await adminFetch(
      `/api/admin/buses/${encodeURIComponent(selectedPref.value)}/${encodeURIComponent(bus.id)}`,
      { method: "DELETE" },
    );
    const json = await resp.json();
    if (!resp.ok) {
      showNotice("err", json.error || "Gagal menghapus armada.");
      return;
    }
    showNotice("ok", `Armada ${bus.name} dihapus dari peta.`);
    await Promise.all([loadAdminState(), store.invalidateCity(selectedPref.value)]);
  } finally {
    busyAction.value = false;
  }
}

onMounted(async () => {
  let savedUser = "";
  let savedSecret = "";
  try {
    const raw = sessionStorage.getItem(SESSION_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as { username?: unknown; secret?: unknown };
      if (typeof parsed.username === "string" && typeof parsed.secret === "string") {
        savedUser = parsed.username;
        savedSecret = parsed.secret;
      }
    }
  } catch {
  }
  if (!savedUser || !savedSecret) {
    checkingAuth.value = false;
    return;
  }
  try {
    const status = await verifyKey(savedUser, savedSecret);
    if (status === "ok") {
      activeUsername.value = savedUser;
      activeSecret.value = savedSecret;
      unlocked.value = true;
      await loadAdminState();
    } else if (status === "invalid") {
      sessionStorage.removeItem(SESSION_KEY);
    }
  } finally {
    checkingAuth.value = false;
  }
});
</script>

<template>
  <main class="admin-page">
    <section v-if="!unlocked" class="gate-wrap">
      <form class="gate-card" @submit.prevent="submitUnlock">
        <h1 class="gate-title">Admin Panel</h1>
        <div class="input-with-toggle">
          <input
            v-model="usernameInput"
            :type="showUsername ? 'text' : 'password'"
            class="field-input has-toggle"
            placeholder="Username"
            autocomplete="username"
            autofocus
          />
          <button
            type="button"
            class="vis-toggle-btn"
            :aria-label="showUsername ? 'Sembunyikan username' : 'Tampilkan username'"
            :title="showUsername ? 'Sembunyikan username' : 'Tampilkan username'"
            @click="showUsername = !showUsername"
          >
            <svg v-if="showUsername" viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
              <circle cx="12" cy="12" r="3" />
            </svg>
            <svg v-else viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" />
              <line x1="1" y1="1" x2="23" y2="23" />
            </svg>
          </button>
        </div>
        <div class="input-with-toggle">
          <input
            v-model="secretInput"
            :type="showPassword ? 'text' : 'password'"
            class="field-input has-toggle"
            placeholder="Password"
            autocomplete="current-password"
          />
          <button
            type="button"
            class="vis-toggle-btn"
            :aria-label="showPassword ? 'Sembunyikan password' : 'Tampilkan password'"
            :title="showPassword ? 'Sembunyikan password' : 'Tampilkan password'"
            @click="showPassword = !showPassword"
          >
            <svg v-if="showPassword" viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
              <circle cx="12" cy="12" r="3" />
            </svg>
            <svg v-else viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" />
              <line x1="1" y1="1" x2="23" y2="23" />
            </svg>
          </button>
        </div>
        <p v-if="authError" class="inline-err">{{ authError }}</p>
        <button type="submit" class="btn-primary gate-submit" :disabled="checkingAuth">
          {{ checkingAuth ? "Memeriksa…" : "Masuk" }}
        </button>
        <button type="button" class="btn-ghost" @click="goHome()">
          Kembali ke Beranda
        </button>
      </form>
    </section>

    <div v-else class="dash-wrap">
      <div class="top-bar">
        <div class="city-switcher" role="tablist" aria-label="Pilih kota">
          <button
            type="button"
            class="city-pill"
            :class="{ active: selectedPref === '3501', sb: selectedPref === '3501' }"
            @click="switchCity('3501')"
          >
            Kota Surabaya (SB)
          </button>
          <button
            type="button"
            class="city-pill"
            :class="{ active: selectedPref === '1', yk: selectedPref === '1' }"
            @click="switchCity('1')"
          >
            Yogyakarta (YK)
          </button>
        </div>
        <div class="top-actions">
          <button
            v-if="hasAnyEdits"
            type="button"
            class="btn-danger"
            :disabled="busyAction"
            @click="resetModalScope = 'all'"
          >
            Reset Semua Data
          </button>
          <button type="button" class="btn-soft" @click="goHome()">Homepage</button>
          <button type="button" class="btn-soft" @click="toggleTheme()">
            {{ theme === "light" ? "Mode Gelap" : "Mode Terang" }}
          </button>
          <button type="button" class="btn-soft" @click="lockPanel()">Logout</button>
        </div>
      </div>

      <section class="stats-grid">
        <div class="stat-card">
          <span class="stat-label">Total armada</span>
          <strong class="stat-value">{{ summaryStats.activeRoutes }}</strong>
        </div>
        <div class="stat-card">
          <span class="stat-label">Total halte</span>
          <strong class="stat-value">{{ summaryStats.activeStops }}</strong>
        </div>
      </section>

      <div v-if="notice" class="notice-bar" :class="notice.type">
        <span>{{ notice.text }}</span>
        <button type="button" class="notice-close" @click="notice = null">×</button>
      </div>

      <div class="tab-bar">
        <button
          type="button"
          class="tab-btn"
          :class="{ active: activeTab === 'routes' }"
          @click="activeTab = 'routes'"
        >
          Rute &amp; Koridor
        </button>
        <button
          type="button"
          class="tab-btn"
          :class="{ active: activeTab === 'stops' }"
          @click="activeTab = 'stops'"
        >
          Halte
        </button>
        <button
          type="button"
          class="tab-btn"
          :class="{ active: activeTab === 'buses' }"
          @click="activeTab = 'buses'"
        >
          Armada
        </button>
        <button
          type="button"
          class="tab-btn"
          :class="{ active: activeTab === 'site' }"
          @click="activeTab = 'site'"
        >
          Menu &amp; Popup
        </button>
      </div>

      <section v-if="activeTab === 'routes'" class="panel-section">
        <div class="toolbar">
          <div class="search-field">
            <svg class="search-icon" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <circle cx="11" cy="11" r="7" />
              <path d="M20 20l-3.5-3.5" />
            </svg>
            <input
              v-model="routeSearch"
              type="search"
              class="search-box"
              placeholder="Cari kode, nama rute, atau asal/tujuan"
            />
            <button
              v-if="routeSearch"
              type="button"
              class="search-clear"
              aria-label="Hapus pencarian"
              @click="routeSearch = ''"
            >
              ×
            </button>
          </div>
          <button
            v-if="hasRouteEdits"
            type="button"
            class="btn-danger"
            :disabled="busyAction"
            @click="resetModalScope = 'routes'"
          >
            Reset Semua Rute
          </button>
          <button
            v-if="overrides.corridor_order.length > 0"
            type="button"
            class="btn-soft"
            :disabled="busyAction"
            @click="resetRouteOrder()"
          >
            Reset Urutan
          </button>
          <button type="button" class="btn-primary" @click="openNewRouteForm()">
            + Tambah Rute
          </button>
        </div>

        <div v-if="loadingState" class="empty-card">Memuat data koridor…</div>
        <div v-else-if="filteredRoutes.length === 0" class="empty-card">Tidak ada rute yang cocok.</div>
        <ul v-else class="list-card">
          <li
            v-for="entry in filteredRoutes"
            :key="entry.sourceCode + '-' + entry.code"
            class="list-row"
            :class="{
              dragging: draggingCode === entry.code,
              'drag-over': dragOverCode === entry.code,
              'is-hidden': entry.isHidden,
            }"
            :draggable="canReorderRoutes && !busyAction"
            @dragstart="onRouteDragStart(entry.code, $event)"
            @dragover.prevent="onRouteDragOver(entry.code)"
            @drop.prevent="onRouteDrop(entry.code)"
            @dragend="onRouteDragEnd"
          >
            <div class="row-main">
              <span
                v-if="canReorderRoutes"
                class="drag-handle"
                title="Seret untuk ubah urutan"
                aria-hidden="true"
              >
                ⋮⋮
              </span>
              <span
                class="row-badge"
                :style="{ background: entry.color, color: contrastText(entry.color) }"
              >
                {{ corridorBadgeLabel(entry.code) }}
              </span>
              <div class="row-info">
                <div class="row-title-line">
                  <span class="row-name">{{ entry.name }}</span>
                  <span v-if="entry.isHidden" class="status-tag hidden">Disembunyikan</span>
                </div>
                <span v-if="entry.via" class="row-via">{{ entry.via }}</span>
                <span class="row-meta">
                  {{ entry.origin || "-" }} - {{ entry.toward || "-" }}
                  <template v-if="entry.hours"> · {{ entry.hours }}</template>
                </span>
              </div>
            </div>
            <div class="row-actions">
              <template v-if="entry.isHidden">
                <button
                  type="button"
                  class="row-action-btn"
                  :disabled="busyAction"
                  @click="restoreRouteEntry(entry)"
                >
                  Pulihkan
                </button>
              </template>
              <template v-else>
                <button
                  type="button"
                  class="row-action-btn"
                  :disabled="busyAction"
                  @click="openRouteStopsModal(entry.code)"
                >
                  Halte
                </button>
                <button
                  type="button"
                  class="row-action-btn"
                  :disabled="busyAction"
                  @click="editRouteEntry(entry)"
                >
                  Edit
                </button>
                <button
                  v-if="entry.isCustom && entry.isUpstream"
                  type="button"
                  class="row-action-btn"
                  :disabled="busyAction"
                  @click="removeRouteEntry(entry, false)"
                >
                  Reset
                </button>
                <button
                  type="button"
                  class="row-action-btn danger"
                  :disabled="busyAction"
                  @click="removeRouteEntry(entry, entry.isUpstream)"
                >
                  {{ entry.isUpstream ? "Sembunyikan" : "Hapus" }}
                </button>
              </template>
            </div>
          </li>
        </ul>
      </section>

      <section v-if="activeTab === 'stops'" class="panel-section">
        <div v-if="activeCorridorOptions.length > 0" class="route-filter-bar">
          <button
            type="button"
            class="quick-chip"
            :class="{ selected: selectedStopRoute === '' }"
            @click="selectedStopRoute = ''; attachStopQuery = ''; stopLimit = 60"
          >
            Semua Rute
          </button>
          <button
            v-for="c in activeCorridorOptions"
            :key="c.code"
            type="button"
            class="quick-chip"
            :class="{ selected: selectedStopRoute === c.code }"
            @click="selectStopRouteFilter(c.code)"
          >
            <span class="quick-dot" :style="{ background: c.color }"></span>
            {{ corridorBadgeLabel(c.code) }}
          </button>
        </div>

        <div class="toolbar">
          <div class="search-field">
            <svg class="search-icon" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <circle cx="11" cy="11" r="7" />
              <path d="M20 20l-3.5-3.5" />
            </svg>
            <input
              v-model="stopSearch"
              type="search"
              class="search-box"
              :placeholder="selectedStopRoute ? `Cari halte di rute ${corridorBadgeLabel(selectedStopRoute)}` : 'Cari nama halte, ID, atau koridor'"
            />
            <button
              v-if="stopSearch"
              type="button"
              class="search-clear"
              aria-label="Hapus pencarian"
              @click="stopSearch = ''"
            >
              ×
            </button>
          </div>
          <button
            v-if="hasStopEdits"
            type="button"
            class="btn-danger"
            :disabled="busyAction"
            @click="resetModalScope = 'stops'"
          >
            Reset Semua Halte
          </button>
          <button
            v-if="selectedStopRoute"
            type="button"
            class="btn-soft"
            @click="openRouteStopsModal(selectedStopRoute)"
          >
            Kelola A &gt; B / B &gt; A
          </button>
          <button type="button" class="btn-primary" @click="openNewStopForm()">
            + Tambah Halte
          </button>
        </div>

        <div v-if="loadingState" class="empty-card">Memuat data halte…</div>
        <div v-else-if="filteredStops.length === 0" class="empty-card">Tidak ada halte yang cocok.</div>
        <template v-else>
          <ul class="list-card">
            <li
              v-for="entry in visibleStops"
              :key="entry.id"
              class="list-row"
              :class="{ 'is-hidden': entry.isHidden }"
            >
              <div class="row-main">
                <div class="row-info">
                  <div class="row-title-line">
                    <span class="row-name">{{ entry.name }}</span>
                    <span v-if="entry.isHidden" class="status-tag hidden">Disembunyikan</span>
                  </div>
                  <div class="stop-badges">
                    <span
                      v-for="(code, idx) in entry.codes"
                      :key="`${entry.id}-${code}`"
                      class="mini-badge"
                      :style="{
                        background: entry.colors[idx] ?? entry.primaryColor,
                        color: contrastText(entry.colors[idx] ?? entry.primaryColor),
                      }"
                    >
                      {{ corridorBadgeLabel(code) }}
                    </span>
                    <span class="row-meta">
                      {{ entry.lat.toFixed(5) }}, {{ entry.lng.toFixed(5) }}
                      <template v-if="entry.toward"> · {{ entry.toward }}</template>
                    </span>
                  </div>
                </div>
              </div>
              <div class="row-actions">
                <button
                  v-if="entry.isHidden"
                  type="button"
                  class="row-action-btn"
                  :disabled="busyAction"
                  @click="restoreStopEntry(entry)"
                >
                  Pulihkan
                </button>
                <template v-else>
                  <button
                    type="button"
                    class="row-action-btn"
                    :disabled="busyAction"
                    @click="editStopEntry(entry)"
                  >
                    Edit
                  </button>
                  <button
                    v-if="entry.isCustom && entry.isUpstream"
                    type="button"
                    class="row-action-btn"
                    :disabled="busyAction"
                    @click="removeStopEntry(entry, false)"
                  >
                    Reset
                  </button>
                  <button
                    v-if="selectedStopRoute && entry.codes.length > 1"
                    type="button"
                    class="row-action-btn"
                    :disabled="busyAction"
                    @click="detachStopFromRoute(entry, selectedStopRoute)"
                  >
                    Lepas
                  </button>
                  <button
                    type="button"
                    class="row-action-btn danger"
                    :disabled="busyAction"
                    @click="removeStopEntry(entry, entry.isUpstream)"
                  >
                    {{ entry.isUpstream ? "Sembunyikan" : "Hapus" }}
                  </button>
                </template>
              </div>
            </li>
          </ul>
          <div v-if="filteredStops.length > visibleStops.length" class="more-wrap">
            <button type="button" class="btn-soft" @click="stopLimit += 60">
              Tampilkan lebih banyak ({{ filteredStops.length - visibleStops.length }} tersisa)
            </button>
          </div>
        </template>
      </section>

      <section v-if="activeTab === 'buses'" class="panel-section">
        <div class="toolbar">
          <div class="search-field">
            <svg class="search-icon" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <circle cx="11" cy="11" r="7" />
              <path d="M20 20l-3.5-3.5" />
            </svg>
            <input
              v-model="busSearch"
              type="search"
              class="search-box"
              placeholder="Cari nama armada, pelat nomor, atau rute"
            />
            <button
              v-if="busSearch"
              type="button"
              class="search-clear"
              aria-label="Hapus pencarian"
              @click="busSearch = ''"
            >
              ×
            </button>
          </div>
          <button
            v-if="hasBusEdits"
            type="button"
            class="btn-danger"
            :disabled="busyAction"
            @click="resetModalScope = 'buses'"
          >
            Reset Semua Armada
          </button>
          <button type="button" class="btn-primary" @click="openNewBusForm()">
            + Tambah Armada
          </button>
        </div>

        <div v-if="loadingState" class="empty-card">Memuat data armada…</div>
        <div v-else-if="filteredBuses.length === 0" class="empty-card">
          Belum ada armada kustom. Tambahkan armada untuk menampilkannya langsung di peta.
        </div>
        <ul v-else class="list-card">
          <li v-for="bus in filteredBuses" :key="bus.id" class="list-row">
            <div class="row-main">
              <span
                class="row-badge"
                :style="{
                  background: busColorForCode(bus.corridor),
                  color: contrastText(busColorForCode(bus.corridor)),
                }"
              >
                {{ corridorBadgeLabel(bus.corridor) }}
              </span>
              <div class="row-info">
                <div class="row-title-line">
                  <span class="row-name">{{ bus.name }}</span>
                  <span v-if="bus.plate" class="row-via">{{ bus.plate }}</span>
                </div>
                <span class="row-meta">
                  {{ bus.lat.toFixed(5) }}, {{ bus.lng.toFixed(5) }}
                  <template v-if="bus.toward"> · Arah {{ bus.toward }}</template>
                  · {{ Math.round(bus.speed) }} km/jam
                </span>
              </div>
            </div>
            <div class="row-actions">
              <button
                type="button"
                class="row-action-btn"
                :disabled="busyAction"
                @click="editBusEntry(bus)"
              >
                Edit
              </button>
              <button
                type="button"
                class="row-action-btn danger"
                :disabled="busyAction"
                @click="removeBusEntry(bus)"
              >
                Hapus
              </button>
            </div>
          </li>
        </ul>
      </section>

      <section v-if="activeTab === 'site'" class="panel-section">
        <form class="site-card" @submit.prevent="saveCityPopup">
          <div class="site-card-head">
            <div>
              <h2 class="site-card-title">Popup</h2>
              <p class="site-card-sub">
                Muncul saat pengguna membuka tracker
              </p>
            </div>
            <div class="site-city-pills" role="group" aria-label="Pilih kota popup">
              <button
                type="button"
                class="quick-chip"
                :class="{ selected: selectedPref === '3501' }"
                :disabled="busyAction || loadingState"
                @click="switchCity('3501')"
              >
                Kota Surabaya (SB)
              </button>
              <button
                type="button"
                class="quick-chip"
                :class="{ selected: selectedPref === '1' }"
                :disabled="busyAction || loadingState"
                @click="switchCity('1')"
              >
                Yogyakarta (YK)
              </button>
            </div>
          </div>

          <label class="toggle-row">
            <input v-model="popupForm.enabled" type="checkbox" class="sr-only-check" />
            <span class="toggle-copy">
              Aktifkan popup untuk {{ selectedPref === "3501" ? "Kota Surabaya" : "Yogyakarta" }}
            </span>
            <span class="toggle-switch" :class="{ on: popupForm.enabled }" aria-hidden="true">
              <span class="toggle-thumb"></span>
            </span>
          </label>

          <div class="form-grid">
            <label class="field field-wide">
              <span class="field-label">Judul (opsional)</span>
              <input
                v-model="popupForm.title"
                type="text"
                class="field-input"
                placeholder="cth. Info Operasional Hari Ini"
              />
            </label>

            <label class="field field-wide">
              <span class="field-label">Teks Popup (opsional)</span>
              <textarea
                v-model="popupForm.text"
                rows="4"
                class="field-textarea field-textarea-prose"
                placeholder="Tulis pengumuman atau pesan untuk pengguna kota ini"
              ></textarea>
            </label>

            <div class="field field-wide">
              <div class="poly-label-row">
                <span class="field-label">Gambar (opsional)</span>
                <div class="poly-actions">
                  <button
                    type="button"
                    class="link-btn"
                    @click="popupFileInput?.click()"
                  >
                    Unggah Gambar
                  </button>
                  <button
                    v-if="popupForm.image_url"
                    type="button"
                    class="link-btn danger"
                    @click="clearPopupImage()"
                  >
                    Hapus Gambar
                  </button>
                </div>
              </div>
              <input
                ref="popupFileInput"
                type="file"
                accept="image/png,image/jpeg,image/webp,image/gif"
                class="hidden-file"
                @change="onPopupImageFile"
              />
              <input
                v-model="popupForm.image_url"
                type="text"
                class="field-input"
                placeholder="Tempel URL gambar (https://...) atau klik Unggah Gambar"
              />
              <div v-if="popupForm.image_url" class="popup-preview-box">
                <img :src="popupForm.image_url" alt="Pratinjau gambar popup" class="popup-preview-img" />
              </div>
            </div>
          </div>

          <div class="site-card-foot">
            <button type="submit" class="btn-primary" :disabled="busyAction || loadingState">
              Simpan Popup ({{ selectedPref === "3501" ? "SB" : "YK" }})
            </button>
          </div>
        </form>

        <form class="site-card" @submit.prevent="saveMenuLinks">
          <div class="site-card-head">
            <h2 class="site-card-title">Hamburger menu</h2>
            <button
              type="button"
              class="btn-soft sm"
              :disabled="busyAction || loadingState"
              @click="addMenuLinkSlot()"
            >
              + Tambah Tombol
            </button>
          </div>

          <p v-if="menuLinksForm.length === 0" class="menu-empty-note">
            Belum ada tombol tambahan. Klik &ldquo;+ Tambah Tombol&rdquo; untuk menambahkan.
          </p>

          <div v-else class="menu-slots-grid">
            <div v-for="(slot, idx) in menuLinksForm" :key="idx" class="menu-slot-card">
              <div class="menu-slot-head">
                <span class="menu-slot-title">Tombol {{ idx + 1 }}</span>
                <div class="menu-slot-actions">
                  <label class="toggle-inline">
                    <input v-model="slot.enabled" type="checkbox" class="sr-only-check" />
                    <span class="field-label">{{ slot.enabled ? "Aktif" : "Nonaktif" }}</span>
                    <span class="toggle-switch" :class="{ on: slot.enabled }" aria-hidden="true">
                      <span class="toggle-thumb"></span>
                    </span>
                  </label>
                  <button
                    type="button"
                    class="link-btn danger"
                    @click="removeMenuLinkSlot(idx)"
                  >
                    Hapus
                  </button>
                </div>
              </div>
              <label class="field">
                <span class="field-label">Teks Tombol</span>
                <input
                  v-model="slot.label"
                  type="text"
                  class="field-input"
                  placeholder="cth. Info Tarif / Lapor Masalah"
                />
              </label>
              <label class="field">
                <span class="field-label">Link Tujuan (URL)</span>
                <input
                  v-model="slot.url"
                  type="text"
                  class="field-input"
                  placeholder="https://..."
                />
              </label>
            </div>
          </div>

          <div class="site-card-foot">
            <button type="submit" class="btn-primary" :disabled="busyAction || loadingState">
              Simpan Tombol Menu
            </button>
          </div>
        </form>
      </section>
    </div>

    <div
      v-if="routeFormOpen"
      class="modal-backdrop"
      @mousedown="onBackdropMouseDown"
      @click="onBackdropClick($event, () => (routeFormOpen = false))"
    >
      <form class="modal-card modal-lg" @submit.prevent="saveRouteForm">
        <div class="modal-head">
          <div class="modal-head-title">
            <h2 class="editor-title">
              {{ routeForm.id ? `Edit Rute ${routeForm.kor}` : "Tambah Rute Baru" }}
            </h2>
            <span
              class="route-preview-badge"
              :style="{
                background: normalizeColor(routeForm.color) ?? '#715d80',
                color: contrastText(normalizeColor(routeForm.color) ?? '#715d80'),
              }"
            >
              {{ corridorBadgeLabel(routeForm.kor || "KODE") }}
            </span>
          </div>
          <button
            type="button"
            class="modal-close"
            aria-label="Tutup"
            @click="routeFormOpen = false"
          >
            ×
          </button>
        </div>

        <div class="modal-body">
          <div class="form-grid">
            <label class="field">
              <span class="field-label">Kode Rute</span>
              <input
                v-model="routeForm.kor"
                type="text"
                class="field-input"
                placeholder="cth. R1 / 4 / AF1"
                required
              />
            </label>
            <label class="field">
              <span class="field-label">Kode Rute (API) (opsional)</span>
              <input
                v-model="routeForm.source_kor"
                type="text"
                class="field-input"
                placeholder="Kosongkan untuk rute kustom / April Fools"
              />
            </label>
            <label class="field field-wide">
              <span class="field-label">Nama Rute</span>
              <input
                v-model="routeForm.route_name"
                type="text"
                class="field-input"
                placeholder="cth. Terminal Purabaya - Tanjung Perak"
                required
              />
            </label>
            <label class="field field-wide">
              <span class="field-label">Via (opsional)</span>
              <input
                v-model="routeForm.via"
                type="text"
                class="field-input"
                placeholder="cth. via Malioboro / via Tunjungan"
              />
            </label>
            <label class="field">
              <span class="field-label">Titik A (Asal)</span>
              <input
                v-model="routeForm.origin"
                type="text"
                class="field-input"
                placeholder="cth. Terminal Purabaya"
              />
            </label>
            <label class="field">
              <span class="field-label">Titik B (Tujuan)</span>
              <input
                v-model="routeForm.toward"
                type="text"
                class="field-input"
                placeholder="cth. Tanjung Perak"
              />
            </label>
            <label class="field">
              <span class="field-label">Warna Koridor</span>
              <div class="color-row">
                <input v-model="routeForm.color" type="color" class="color-picker" />
                <input
                  v-model="routeForm.color"
                  type="text"
                  class="field-input"
                  placeholder="#715d80"
                />
              </div>
            </label>
            <label class="field">
              <span class="field-label">Jam Operasional</span>
              <input
                v-model="routeForm.jam_operasional"
                type="text"
                class="field-input"
                placeholder="05:30 - 21:00"
              />
            </label>
          </div>

          <div class="dir-switch-bar">
            <span class="field-label">Arah Rute &amp; Halte</span>
            <div class="dir-toggle" role="tablist" aria-label="Pilih arah rute">
              <button
                type="button"
                class="dir-btn"
                :class="{ active: routeEditDir === 'a' }"
                @click="routeEditDir = 'a'"
              >
                A &gt; B
                <span v-if="routeForm.origin || routeForm.toward" class="dir-sub">
                  ({{ routeForm.origin || "A" }} &gt; {{ routeForm.toward || "B" }})
                </span>
              </button>
              <button
                type="button"
                class="dir-btn"
                :class="{ active: routeEditDir === 'b' }"
                @click="routeEditDir = 'b'"
              >
                B &gt; A
                <span v-if="routeForm.origin || routeForm.toward" class="dir-sub">
                  ({{ routeForm.toward || "B" }} &gt; {{ routeForm.origin || "A" }})
                </span>
              </button>
            </div>
          </div>

          <div v-if="routeEditDir === 'a'" class="field">
            <div class="poly-label-row">
              <span class="field-label">
                Polyline / GeoJSON (A &gt; B)
                <span v-if="previewPolyA.count > 0" class="pt-pill">{{ previewPolyA.count }} titik</span>
              </span>
              <span class="poly-actions">
                <button
                  v-if="routeForm.points_a.trim()"
                  type="button"
                  class="link-btn danger"
                  @click="routeForm.points_a = ''; polyErrorA = ''"
                >
                  Hapus
                </button>
                <label class="upload-btn">
                  Unggah .geojson
                  <input type="file" accept=".geojson,.json" hidden @change="onUploadGeoJson('a', $event)" />
                </label>
              </span>
            </div>
            <textarea
              v-model="routeForm.points_a"
              rows="3"
              class="field-textarea"
              placeholder="Tempel GeoJSON LineString/Feature atau encoded polyline untuk arah A > B"
            ></textarea>
            <span v-if="polyErrorA" class="inline-err">{{ polyErrorA }}</span>
          </div>

          <div v-else class="field">
            <div class="poly-label-row">
              <span class="field-label">
                Polyline / GeoJSON (B &gt; A)
                <span v-if="previewPolyB.count > 0" class="pt-pill">{{ previewPolyB.count }} titik</span>
              </span>
              <span class="poly-actions">
                <button
                  v-if="routeForm.points_b.trim()"
                  type="button"
                  class="link-btn danger"
                  @click="routeForm.points_b = ''; polyErrorB = ''"
                >
                  Hapus
                </button>
                <label class="upload-btn">
                  Unggah .geojson
                  <input type="file" accept=".geojson,.json" hidden @change="onUploadGeoJson('b', $event)" />
                </label>
              </span>
            </div>
            <textarea
              v-model="routeForm.points_b"
              rows="3"
              class="field-textarea"
              placeholder="Tempel GeoJSON LineString/Feature atau encoded polyline untuk arah B > A"
            ></textarea>
            <span v-if="polyErrorB" class="inline-err">{{ polyErrorB }}</span>
          </div>

          <div v-if="routeForm.kor.trim()" class="route-stops-box">
            <div class="route-stops-head">
              <span class="field-label">
                Halte {{ corridorBadgeLabel(routeForm.kor) }} ·
                {{ routeEditDir === "a" ? "A > B" : "B > A" }}
                ({{ routeFormStops.length }})
              </span>
              <div class="route-stops-head-actions">
                <button
                  type="button"
                  class="btn-soft sm"
                  @click="
                    openNewStopForm(
                      routeForm.kor.trim().toUpperCase(),
                      activeRouteDirDestination,
                    )
                  "
                >
                  + Buat Halte Baru
                </button>
              </div>
            </div>

            <div class="attach-box">
              <input
                v-model="attachStopQuery"
                type="search"
                class="field-input"
                placeholder="Cari halte yang sudah ada untuk ditambahkan ke arah ini…"
              />
              <ul v-if="attachableStops.length > 0" class="attach-results">
                <li v-for="s in attachableStops" :key="s.id" class="attach-row">
                  <div class="attach-info">
                    <span class="attach-name">{{ s.name }}</span>
                    <span class="row-meta">
                      {{ s.codes.join(", ") }}
                      <template v-if="s.toward"> · {{ s.toward }}</template>
                    </span>
                  </div>
                  <button
                    type="button"
                    class="btn-soft sm"
                    :disabled="busyAction"
                    @click="attachStopToRoute(s, routeForm.kor, activeRouteDirDestination)"
                  >
                    + Tambah
                  </button>
                </li>
              </ul>
            </div>

            <div v-if="routeFormStops.length === 0" class="route-stops-empty">
              Belum ada halte untuk arah {{ routeEditDir === "a" ? "A > B" : "B > A" }}.
            </div>
            <ul v-else class="route-stops-list">
              <li v-for="s in routeFormStops" :key="s.id" class="route-stop-item">
                <div class="route-stop-info">
                  <span class="route-stop-name">{{ s.name }}</span>
                  <span class="row-meta">
                    {{ s.lat.toFixed(5) }}, {{ s.lng.toFixed(5) }}
                    <template v-if="s.toward"> · {{ s.toward }}</template>
                  </span>
                </div>
                <div class="row-actions">
                  <button
                    type="button"
                    class="btn-soft sm"
                    :disabled="busyAction"
                    @click="editStopEntry(s)"
                  >
                    Edit
                  </button>
                  <button
                    type="button"
                    class="btn-danger sm"
                    :disabled="busyAction"
                    @click="detachStopFromRoute(s, routeForm.kor)"
                  >
                    Lepas
                  </button>
                </div>
              </li>
            </ul>
          </div>
        </div>

        <div class="modal-foot">
          <button type="button" class="btn-soft" @click="routeFormOpen = false">Batal</button>
          <button type="submit" class="btn-primary" :disabled="busyAction">Simpan Rute</button>
        </div>
      </form>
    </div>

    <div
      v-if="routeStopsModalEntry"
      class="modal-backdrop"
      @mousedown="onBackdropMouseDown"
      @click="onBackdropClick($event, closeRouteStopsModal)"
    >
      <div class="modal-card modal-lg">
        <div class="modal-head">
          <div class="modal-head-title">
            <h2 class="editor-title">Halte Rute {{ routeStopsModalEntry.name }}</h2>
            <span
              class="route-preview-badge"
              :style="{
                background: routeStopsModalEntry.color,
                color: contrastText(routeStopsModalEntry.color),
              }"
            >
              {{ corridorBadgeLabel(routeStopsModalEntry.code) }}
            </span>
          </div>
          <button
            type="button"
            class="modal-close"
            aria-label="Tutup"
            @click="closeRouteStopsModal()"
          >
            ×
          </button>
        </div>

        <div class="modal-body">
          <div class="dir-switch-bar">
            <span class="field-label">Pilih Arah Halte</span>
            <div class="dir-toggle" role="tablist" aria-label="Pilih arah halte">
              <button
                type="button"
                class="dir-btn"
                :class="{ active: routeEditDir === 'a' }"
                @click="routeEditDir = 'a'"
              >
                A &gt; B
                <span v-if="routeStopsModalEntry.origin || routeStopsModalEntry.toward" class="dir-sub">
                  ({{ routeStopsModalEntry.origin || "A" }} &gt; {{ routeStopsModalEntry.toward || "B" }})
                </span>
              </button>
              <button
                type="button"
                class="dir-btn"
                :class="{ active: routeEditDir === 'b' }"
                @click="routeEditDir = 'b'"
              >
                B &gt; A
                <span v-if="routeStopsModalEntry.origin || routeStopsModalEntry.toward" class="dir-sub">
                  ({{ routeStopsModalEntry.toward || "B" }} &gt; {{ routeStopsModalEntry.origin || "A" }})
                </span>
              </button>
            </div>
          </div>

          <div class="route-stops-box">
            <div class="route-stops-head">
              <span class="field-label">
                Daftar Halte {{ routeEditDir === "a" ? "A > B" : "B > A" }}
                ({{ routeStopsModalList.length }})
              </span>
              <div class="route-stops-head-actions">
                <button
                  type="button"
                  class="btn-primary sm"
                  @click="
                    openNewStopForm(
                      routeStopsModalEntry.code,
                      activeRouteDirDestination,
                    )
                  "
                >
                  + Buat Halte Baru
                </button>
              </div>
            </div>

            <div class="attach-box">
              <input
                v-model="attachStopQuery"
                type="search"
                class="field-input"
                placeholder="Cari halte yang sudah ada untuk ditambahkan ke arah ini…"
              />
              <ul v-if="attachableStops.length > 0" class="attach-results">
                <li v-for="s in attachableStops" :key="s.id" class="attach-row">
                  <div class="attach-info">
                    <span class="attach-name">{{ s.name }}</span>
                    <span class="row-meta">
                      {{ s.codes.join(", ") }}
                      <template v-if="s.toward"> · {{ s.toward }}</template>
                    </span>
                  </div>
                  <button
                    type="button"
                    class="btn-soft sm"
                    :disabled="busyAction"
                    @click="
                      attachStopToRoute(
                        s,
                        routeStopsModalEntry.code,
                        activeRouteDirDestination,
                      )
                    "
                  >
                    + Tambah
                  </button>
                </li>
              </ul>
            </div>

            <div v-if="routeStopsModalList.length === 0" class="route-stops-empty">
              Belum ada halte untuk arah {{ routeEditDir === "a" ? "A > B" : "B > A" }}.
            </div>
            <ul v-else class="route-stops-list modal-stops-tall">
              <li v-for="s in routeStopsModalList" :key="s.id" class="route-stop-item">
                <div class="route-stop-info">
                  <span class="route-stop-name">{{ s.name }}</span>
                  <span class="row-meta">
                    {{ s.lat.toFixed(5) }}, {{ s.lng.toFixed(5) }}
                    <template v-if="s.toward"> · {{ s.toward }}</template>
                  </span>
                </div>
                <div class="row-actions">
                  <button
                    type="button"
                    class="btn-soft sm"
                    :disabled="busyAction"
                    @click="editStopEntry(s)"
                  >
                    Edit
                  </button>
                  <button
                    type="button"
                    class="btn-danger sm"
                    :disabled="busyAction"
                    @click="detachStopFromRoute(s, routeStopsModalEntry.code)"
                  >
                    Lepas
                  </button>
                </div>
              </li>
            </ul>
          </div>
        </div>

        <div class="modal-foot">
          <button type="button" class="btn-soft" @click="closeRouteStopsModal()">Selesai</button>
        </div>
      </div>
    </div>

    <div
      v-if="stopFormOpen"
      class="modal-backdrop modal-layer-top"
      @mousedown="onBackdropMouseDown"
      @click="onBackdropClick($event, () => (stopFormOpen = false))"
    >
      <form class="modal-card" @submit.prevent="saveStopForm">
        <div class="modal-head">
          <h2 class="editor-title">
            {{ stopForm.sh_id ? `Edit Halte (${stopForm.sh_name || stopForm.sh_id})` : "Tambah Halte Baru" }}
          </h2>
          <button
            type="button"
            class="modal-close"
            aria-label="Tutup"
            @click="stopFormOpen = false"
          >
            ×
          </button>
        </div>

        <div class="modal-body">
          <div class="form-grid">
            <label class="field">
              <span class="field-label">Nama Halte</span>
              <input
                v-model="stopForm.sh_name"
                type="text"
                class="field-input"
                placeholder="cth. Halte Kampus Baru"
                required
              />
            </label>
            <label class="field">
              <span class="field-label">Arah / Tujuan</span>
              <input
                v-model="stopForm.toward"
                type="text"
                class="field-input"
                placeholder="cth. Tanjung Perak"
              />
              <div
                v-if="activeRouteDirOrigin || activeRouteDirToward"
                class="quick-chips"
              >
                <button
                  v-if="activeRouteDirToward"
                  type="button"
                  class="quick-chip"
                  :class="{ selected: stopForm.toward === activeRouteDirToward }"
                  @click="stopForm.toward = activeRouteDirToward"
                >
                  A &gt; B ({{ activeRouteDirToward }})
                </button>
                <button
                  v-if="activeRouteDirOrigin"
                  type="button"
                  class="quick-chip"
                  :class="{ selected: stopForm.toward === activeRouteDirOrigin }"
                  @click="stopForm.toward = activeRouteDirOrigin"
                >
                  B &gt; A ({{ activeRouteDirOrigin }})
                </button>
              </div>
            </label>
            <label class="field">
              <span class="field-label">Latitude (bisa tempel lat, lng)</span>
              <input
                :value="stopForm.latText"
                type="text"
                class="field-input"
                placeholder="-7.2575"
                required
                @input="onLatInput"
              />
            </label>
            <label class="field">
              <span class="field-label">Longitude</span>
              <input
                v-model="stopForm.lngText"
                type="text"
                class="field-input"
                placeholder="112.7521"
                required
              />
            </label>
          </div>

          <div class="field">
            <span class="field-label">Koridor yang Melayani (pisahkan koma)</span>
            <input
              v-model="stopForm.corridorsText"
              type="text"
              class="field-input"
              placeholder="cth. R1, FD3"
              required
            />
            <div v-if="activeCorridorOptions.length > 0" class="quick-chips">
              <button
                v-for="c in activeCorridorOptions"
                :key="c.code"
                type="button"
                class="quick-chip"
                :class="{ selected: stopHasChip(c.code) }"
                @click="toggleStopCorridorChip(c.code)"
              >
                <span class="quick-dot" :style="{ background: c.color }"></span>
                {{ corridorBadgeLabel(c.code) }}
              </button>
            </div>
          </div>
        </div>

        <div class="modal-foot">
          <button type="button" class="btn-soft" @click="stopFormOpen = false">Batal</button>
          <button type="submit" class="btn-primary" :disabled="busyAction">Simpan Halte</button>
        </div>
      </form>
    </div>

    <div
      v-if="busFormOpen"
      class="modal-backdrop modal-layer-top"
      @mousedown="onBackdropMouseDown"
      @click="onBackdropClick($event, () => (busFormOpen = false))"
    >
      <form class="modal-card" @submit.prevent="saveBusForm">
        <div class="modal-head">
          <h2 class="editor-title">
            {{ busForm.id ? `Edit Armada (${busForm.name || busForm.id})` : "Tambah Armada Kustom" }}
          </h2>
          <button
            type="button"
            class="modal-close"
            aria-label="Tutup"
            @click="busFormOpen = false"
          >
            ×
          </button>
        </div>

        <div class="modal-body">
          <div class="form-grid">
            <label class="field">
              <span class="field-label">Nama / Kode Lambung Armada</span>
              <input
                v-model="busForm.name"
                type="text"
                class="field-input"
                placeholder="cth. Suroboyo Express 404"
                required
              />
            </label>
            <label class="field">
              <span class="field-label">Pelat Nomor (opsional)</span>
              <input
                v-model="busForm.plate"
                type="text"
                class="field-input"
                placeholder="cth. L 1 APR"
              />
            </label>
            <label class="field">
              <span class="field-label">Kode Rute</span>
              <input
                v-model="busForm.corridor"
                type="text"
                class="field-input"
                placeholder="cth. R1 / AF1"
                required
              />
            </label>
            <label class="field">
              <span class="field-label">Arah / Tujuan</span>
              <input
                v-model="busForm.toward"
                type="text"
                class="field-input"
                placeholder="cth. Tanjung Perak"
              />
              <div
                v-if="selectedBusRouteEntry && (selectedBusRouteEntry.origin || selectedBusRouteEntry.toward)"
                class="quick-chips"
              >
                <button
                  v-if="selectedBusRouteEntry.toward"
                  type="button"
                  class="quick-chip"
                  :class="{ selected: busForm.toward === selectedBusRouteEntry.toward }"
                  @click="busForm.toward = selectedBusRouteEntry.toward"
                >
                  A &gt; B ({{ selectedBusRouteEntry.toward }})
                </button>
                <button
                  v-if="selectedBusRouteEntry.origin"
                  type="button"
                  class="quick-chip"
                  :class="{ selected: busForm.toward === selectedBusRouteEntry.origin }"
                  @click="busForm.toward = selectedBusRouteEntry.origin"
                >
                  B &gt; A ({{ selectedBusRouteEntry.origin }})
                </button>
              </div>
            </label>
          </div>

          <div v-if="activeCorridorOptions.length > 0" class="field">
            <span class="field-label">Pilih Cepat Rute</span>
            <div class="quick-chips">
              <button
                v-for="c in activeCorridorOptions"
                :key="c.code"
                type="button"
                class="quick-chip"
                :class="{ selected: normCode(busForm.corridor) === normCode(c.code) }"
                @click="selectBusCorridor(c.code)"
              >
                <span class="quick-dot" :style="{ background: c.color }"></span>
                {{ corridorBadgeLabel(c.code) }}
              </button>
            </div>
          </div>

          <div class="form-grid">
            <label class="field">
              <div class="poly-label-row">
                <span class="field-label">Latitude (bisa tempel lat, lng)</span>
                <button
                  v-if="busForm.corridor.trim()"
                  type="button"
                  class="link-btn"
                  @click="pickBusPointFromRoute(busForm.corridor)"
                >
                  Ambil titik rute
                </button>
              </div>
              <input
                :value="busForm.latText"
                type="text"
                class="field-input"
                placeholder="-7.2575"
                required
                @input="onBusLatInput"
              />
            </label>
            <label class="field">
              <span class="field-label">Longitude</span>
              <input
                v-model="busForm.lngText"
                type="text"
                class="field-input"
                placeholder="112.7521"
                required
              />
            </label>
            <label class="field">
              <span class="field-label">Kecepatan (km/jam)</span>
              <input
                v-model="busForm.speedText"
                type="number"
                step="any"
                class="field-input"
                placeholder="32"
              />
            </label>
            <label class="field">
              <span class="field-label">Arah Kompas / Heading (0 - 360°)</span>
              <input
                v-model="busForm.headingText"
                type="number"
                step="any"
                class="field-input"
                placeholder="0"
              />
            </label>
          </div>
        </div>

        <div class="modal-foot">
          <button type="button" class="btn-soft" @click="busFormOpen = false">Batal</button>
          <button type="submit" class="btn-primary" :disabled="busyAction">Simpan Armada</button>
        </div>
      </form>
    </div>

    <div
      v-if="resetModalScope"
      class="modal-backdrop modal-layer-top"
      @mousedown="onBackdropMouseDown"
      @click="onBackdropClick($event, () => (resetModalScope = null))"
    >
      <div class="modal-card">
        <div class="modal-head">
          <h2 class="editor-title">{{ resetModalCopy.title }}</h2>
          <button
            type="button"
            class="modal-close"
            aria-label="Tutup"
            @click="resetModalScope = null"
          >
            ×
          </button>
        </div>
        <div class="modal-body">
          <p class="confirm-text">{{ resetModalCopy.body }}</p>
        </div>
        <div class="modal-foot">
          <button type="button" class="btn-soft" @click="resetModalScope = null">Batal</button>
          <button
            type="button"
            class="btn-primary"
            :disabled="busyAction"
            @click="confirmResetScope()"
          >
            Ya, Reset
          </button>
        </div>
      </div>
    </div>
  </main>
</template>

<style scoped>
.admin-page {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  background: var(--bg-app);
  color: var(--text-primary);
  padding: 24px 16px calc(36px + env(safe-area-inset-bottom));
  box-sizing: border-box;
}
.gate-wrap {
  min-height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}
.gate-card {
  width: 100%;
  max-width: 390px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: 16px;
  padding: 24px;
  box-shadow: var(--shadow-chip);
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.gate-title {
  margin: 0 0 2px;
  font-size: 1.15rem;
  font-weight: 700;
  color: var(--text-strong);
}
.input-with-toggle {
  position: relative;
  display: flex;
  align-items: center;
}
.field-input.has-toggle {
  padding-right: 38px;
}
.vis-toggle-btn {
  position: absolute;
  right: 8px;
  appearance: none;
  border: none;
  background: transparent;
  color: var(--text-muted);
  width: 28px;
  height: 28px;
  border-radius: 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  padding: 0;
}
.vis-toggle-btn:hover {
  color: var(--text-strong);
}
.confirm-text {
  margin: 0;
  font-size: 0.88rem;
  line-height: 1.5;
  color: var(--text-secondary);
}
.gate-submit {
  width: 100%;
}
.dash-wrap {
  width: 100%;
  max-width: 1020px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.top-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.city-switcher {
  display: inline-flex;
  padding: 4px;
  border-radius: 12px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  gap: 4px;
}
.city-pill {
  appearance: none;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: 0.84rem;
  font-weight: 700;
  padding: 8px 14px;
  border-radius: 9px;
  cursor: pointer;
  transition: background-color 140ms ease, color 140ms ease;
}
.city-pill.active.sb {
  background: #715d80;
  color: #ffffff;
}
.city-pill.active.yk {
  background: #eab308;
  color: #0f172a;
}
.top-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.stats-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}
.stat-card {
  background: var(--bg-surface);
  border: 1px solid var(--border-hairline);
  border-radius: 8px;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.stat-label {
  font-size: 0.76rem;
  font-weight: 600;
  color: var(--text-muted);
}
.stat-value {
  font-size: 1.5rem;
  font-weight: 700;
  line-height: 1.15;
  color: var(--text-strong);
  font-variant-numeric: tabular-nums;
}
.notice-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 14px;
  border-radius: 12px;
  font-size: 0.85rem;
  font-weight: 600;
  border: 1px solid var(--border-strong);
  background: var(--bg-surface);
}
.notice-bar.ok {
  border-color: var(--dot-live);
  color: var(--text-strong);
}
.notice-bar.err {
  background: var(--err-bg);
  border-color: var(--err-border);
  color: var(--err-text);
}
.notice-close {
  appearance: none;
  border: none;
  background: transparent;
  color: inherit;
  font-size: 1.1rem;
  cursor: pointer;
  padding: 0 4px;
}
.tab-bar {
  display: inline-flex;
  align-self: flex-start;
  padding: 4px;
  border-radius: 12px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  gap: 4px;
}
.tab-btn {
  appearance: none;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: 0.84rem;
  font-weight: 700;
  padding: 8px 16px;
  border-radius: 9px;
  cursor: pointer;
  transition: background-color 140ms ease, color 140ms ease;
}
.tab-btn.active {
  background: #991b1b;
  color: #ffffff;
}
.panel-section {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}
.toolbar .btn-soft,
.toolbar .btn-primary {
  height: 40px;
  box-sizing: border-box;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.search-field {
  flex: 1 1 240px;
  min-width: 0;
  height: 40px;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: 10px;
  transition: border-color 140ms ease;
}
.search-field:focus-within {
  border-color: var(--border-hover);
}
.search-icon {
  color: var(--text-muted);
  flex-shrink: 0;
}
.search-box {
  flex: 1;
  min-width: 0;
  appearance: none;
  background: transparent;
  border: none;
  color: var(--text-strong);
  font-family: inherit;
  font-size: 0.86rem;
  padding: 0;
  outline: none;
}
.search-box::placeholder {
  color: var(--text-muted);
}
.search-box::-webkit-search-cancel-button {
  display: none;
}
.search-clear {
  appearance: none;
  border: none;
  background: transparent;
  color: var(--text-muted);
  font-size: 1.05rem;
  line-height: 1;
  padding: 2px 4px;
  cursor: pointer;
}
.search-clear:hover {
  color: var(--text-primary);
}
.field-input:focus,
.field-textarea:focus {
  border-color: var(--border-hover);
}
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1200;
  background: rgba(2, 6, 23, 0.72);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  box-sizing: border-box;
}
.modal-backdrop.modal-layer-top {
  z-index: 1300;
}
.modal-card {
  width: 100%;
  max-width: 620px;
  max-height: calc(100dvh - 32px);
  background: var(--bg-surface);
  border: 1px solid var(--border-strong);
  border-radius: 16px;
  box-shadow: 0 22px 48px rgba(0, 0, 0, 0.38);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.modal-card.modal-lg {
  max-width: 780px;
}
.modal-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}
.modal-head-title {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  min-width: 0;
}
.modal-close {
  appearance: none;
  border: 1px solid var(--border-subtle);
  background: var(--bg-app);
  color: var(--text-secondary);
  width: 32px;
  height: 32px;
  border-radius: 8px;
  font-size: 1.15rem;
  line-height: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
}
.modal-close:hover {
  color: var(--text-strong);
  border-color: var(--border-hover);
}
.modal-body {
  padding: 18px 20px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.modal-foot {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  padding: 14px 20px;
  border-top: 1px solid var(--border-subtle);
  background: var(--bg-surface);
  flex-shrink: 0;
}
.editor-title {
  margin: 0;
  font-size: 1.02rem;
  font-weight: 700;
  color: var(--text-strong);
}
.route-preview-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 44px;
  height: 28px;
  padding: 0 10px;
  border-radius: 8px;
  font-size: 0.82rem;
  font-weight: 700;
}
.form-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.field-wide {
  grid-column: 1 / -1;
}
.field-label {
  font-size: 0.78rem;
  font-weight: 600;
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.field-input,
.field-textarea {
  appearance: none;
  width: 100%;
  box-sizing: border-box;
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  color: var(--text-strong);
  font-family: inherit;
  font-size: 0.86rem;
  padding: 9px 11px;
  border-radius: 10px;
  outline: none;
}
.field-textarea {
  resize: vertical;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 0.78rem;
}
.color-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.color-picker {
  width: 42px;
  height: 36px;
  padding: 2px;
  border-radius: 8px;
  border: 1px solid var(--border-subtle);
  background: var(--bg-app);
  cursor: pointer;
  flex-shrink: 0;
}
.dir-switch-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 12px;
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
}
.dir-toggle {
  display: inline-flex;
  flex-wrap: wrap;
  padding: 3px;
  border-radius: 10px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  gap: 4px;
}
.dir-btn {
  appearance: none;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-family: inherit;
  font-size: 0.8rem;
  font-weight: 700;
  padding: 7px 12px;
  border-radius: 8px;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  transition: background-color 140ms ease, color 140ms ease;
}
.dir-btn.active {
  background: #991b1b;
  color: #ffffff;
}
.dir-sub {
  font-weight: 600;
  opacity: 0.9;
}
.poly-label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.pt-pill {
  padding: 2px 7px;
  border-radius: 999px;
  font-size: 0.7rem;
  background: var(--bg-control-soft);
  color: var(--text-strong);
}
.poly-actions {
  display: inline-flex;
  align-items: center;
  gap: 12px;
}
.upload-btn,
.link-btn {
  appearance: none;
  border: none;
  background: transparent;
  padding: 0;
  font-family: inherit;
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--text-link);
  cursor: pointer;
}
.link-btn.danger {
  color: var(--err-text);
}
.quick-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 4px;
}
.quick-chip {
  appearance: none;
  border: 1px solid var(--border-subtle);
  background: var(--bg-app);
  color: var(--text-secondary);
  font-size: 0.75rem;
  font-weight: 700;
  padding: 5px 9px;
  border-radius: 8px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}
.quick-chip.selected {
  border-color: #dc2626;
  background: rgba(153, 27, 27, 0.16);
  color: var(--text-strong);
}
.quick-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}
.route-filter-bar {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.route-stops-box {
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  border-radius: 12px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.route-stops-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.route-stops-head-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.attach-box {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.attach-results,
.route-stops-list {
  list-style: none;
  margin: 0;
  padding: 0;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: 10px;
  max-height: 220px;
  overflow-y: auto;
}
.route-stops-list.modal-stops-tall {
  max-height: 360px;
}
.attach-row,
.route-stop-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border-subtle);
}
.attach-row:last-child,
.route-stop-item:last-child {
  border-bottom: none;
}
.attach-info,
.route-stop-info {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.attach-name,
.route-stop-name {
  font-size: 0.84rem;
  font-weight: 600;
  color: var(--text-strong);
}
.route-stops-empty {
  font-size: 0.82rem;
  color: var(--text-muted);
  padding: 8px 4px;
}
.list-card {
  list-style: none;
  margin: 0;
  padding: 0;
  background: var(--bg-surface);
  border-top: 1px solid var(--border-hairline);
  border-bottom: 1px solid var(--border-hairline);
}
.list-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border-hairline);
  transition: background-color 120ms ease, opacity 120ms ease;
}
.list-row:hover {
  background: var(--bg-surface-hover);
}
.list-row.is-hidden {
  opacity: 0.65;
}
.list-row.dragging {
  opacity: 0.45;
}
.list-row.drag-over {
  background: var(--bg-surface-hover);
  box-shadow: inset 0 2px 0 #dc2626;
}
.drag-handle {
  color: var(--text-muted);
  font-size: 0.9rem;
  letter-spacing: -1px;
  cursor: grab;
  user-select: none;
  padding: 4px 2px;
  flex-shrink: 0;
}
.drag-handle:active {
  cursor: grabbing;
}
.list-row:last-child {
  border-bottom: none;
}
.row-main {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  flex: 1;
}
.row-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 44px;
  height: 32px;
  padding: 0 8px;
  box-sizing: border-box;
  border-radius: 6px;
  font-size: 0.86rem;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  line-height: 1;
  flex-shrink: 0;
}
.row-info {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.row-title-line {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}
.row-name {
  font-size: 0.9rem;
  font-weight: 600;
  line-height: 1.3;
  color: var(--text-strong);
}
.row-via {
  font-size: 0.78rem;
  font-weight: 400;
  line-height: 1.25;
  color: var(--text-secondary);
}
.row-meta {
  font-size: 0.78rem;
  line-height: 1.25;
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}
.stop-badges {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 5px;
}
.mini-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 24px;
  height: 20px;
  padding: 0 6px;
  border-radius: 5px;
  font-size: 0.7rem;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  line-height: 1;
}
.status-tag {
  font-size: 0.68rem;
  font-weight: 700;
  padding: 2px 7px;
  border-radius: 999px;
  border: 1px solid var(--border-subtle);
}
.status-tag.hidden {
  color: var(--err-text);
  border-color: var(--err-border);
}
.row-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}
.row-action-btn {
  appearance: none;
  border: 1px solid transparent;
  background: transparent;
  color: var(--text-muted);
  font-family: inherit;
  font-size: 0.76rem;
  font-weight: 600;
  padding: 5px 8px;
  border-radius: 6px;
  cursor: pointer;
  transition: background-color 120ms ease, color 120ms ease, border-color 120ms ease;
}
.list-row:hover .row-action-btn {
  color: var(--text-primary);
  border-color: var(--border-hairline);
  background: var(--bg-control-soft);
}
.row-action-btn:hover:not(:disabled) {
  color: var(--text-strong);
  border-color: var(--border-hover);
  background: var(--bg-surface-active);
}
.row-action-btn.danger {
  color: var(--text-muted);
}
.list-row:hover .row-action-btn.danger {
  color: var(--err-text);
  border-color: var(--err-border);
  background: transparent;
}
.row-action-btn.danger:hover:not(:disabled) {
  background: var(--err-bg);
  color: var(--err-text);
  border-color: var(--err-border);
}
.btn-primary,
.btn-soft,
.btn-danger,
.btn-ghost {
  appearance: none;
  border-radius: 10px;
  font-family: inherit;
  font-size: 0.84rem;
  font-weight: 700;
  padding: 9px 14px;
  cursor: pointer;
  transition: background-color 140ms ease, border-color 140ms ease, opacity 140ms ease;
}
.btn-primary {
  border: none;
  background: #991b1b;
  color: #ffffff;
}
.btn-primary:hover:not(:disabled) {
  background: #b91c1c;
}
.btn-soft {
  border: 1px solid var(--border-subtle);
  background: var(--bg-surface);
  color: var(--text-primary);
}
.btn-soft:hover:not(:disabled) {
  background: var(--bg-surface-hover);
  border-color: var(--border-hover);
}
.btn-danger {
  border: 1px solid var(--err-border);
  background: transparent;
  color: var(--err-text);
}
.btn-danger:hover:not(:disabled) {
  background: var(--err-bg);
}
.btn-ghost {
  border: none;
  background: transparent;
  color: var(--text-muted);
}
.btn-ghost:hover {
  color: var(--text-primary);
}
.sm {
  font-size: 0.76rem;
  padding: 6px 10px;
  border-radius: 8px;
}
button:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
.inline-err {
  margin: 0;
  font-size: 0.78rem;
  color: var(--err-text);
  font-weight: 600;
}
.empty-card {
  background: var(--bg-surface);
  border: 1px solid var(--border-hairline);
  border-radius: 8px;
  padding: 28px 16px;
  text-align: center;
  font-size: 0.88rem;
  color: var(--text-muted);
}
.more-wrap {
  display: flex;
  justify-content: center;
}
@media (max-width: 768px) {
  .stats-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .form-grid {
    grid-template-columns: minmax(0, 1fr);
  }
  .list-row {
    flex-direction: column;
    align-items: flex-start;
  }
  .row-actions {
    width: 100%;
    justify-content: flex-end;
  }
  .row-action-btn {
    border-color: var(--border-hairline);
    color: var(--text-secondary);
  }
  .row-action-btn.danger {
    color: var(--err-text);
    border-color: var(--err-border);
  }
}

.site-card {
  background: var(--bg-surface);
  border: 1px solid var(--border-hairline);
  border-radius: 10px;
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.site-card-head {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border-hairline);
}
.site-card-title {
  margin: 0;
  font-size: 0.98rem;
  font-weight: 700;
  color: var(--text-strong);
}
.site-card-sub {
  margin: 4px 0 0;
  font-size: 0.8rem;
  color: var(--text-muted);
}
.site-city-pills {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.sr-only-check {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  border: 0;
}
.toggle-row {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  border-radius: 10px;
  cursor: pointer;
}
.toggle-copy {
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--text-strong);
}
.toggle-inline {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
}
.toggle-switch {
  width: 36px;
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
.toggle-switch.on {
  background: #991b1b;
}
.toggle-thumb {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #ffffff;
  transform: translateX(0);
  transition: transform 140ms ease;
}
.toggle-switch.on .toggle-thumb {
  transform: translateX(16px);
}
.field-textarea-prose {
  font-family: inherit;
  font-size: 0.86rem;
  line-height: 1.45;
}
.hidden-file {
  display: none;
}
.popup-preview-box {
  margin-top: 6px;
  padding: 10px;
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  border-radius: 10px;
  display: flex;
  justify-content: center;
}
.popup-preview-img {
  max-width: 100%;
  max-height: 220px;
  object-fit: contain;
  border-radius: 6px;
  display: block;
}
.menu-slots-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}
.menu-slot-card {
  background: var(--bg-app);
  border: 1px solid var(--border-subtle);
  border-radius: 10px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.menu-slot-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.menu-slot-title {
  font-size: 0.84rem;
  font-weight: 700;
  color: var(--text-strong);
}
.menu-slot-actions {
  display: inline-flex;
  align-items: center;
  gap: 10px;
}
.menu-empty-note {
  margin: 0;
  font-size: 0.82rem;
  color: var(--text-muted);
}
.site-card-foot {
  display: flex;
  justify-content: flex-end;
  padding-top: 4px;
}
@media (max-width: 768px) {
  .menu-slots-grid {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
