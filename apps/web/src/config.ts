export const API_BASE = String(import.meta.env.VITE_API_BASE ?? "")
  .trim()
  .replace(/\/+$/, "");

export const SOCKET_BASE = API_BASE || "/";

export const BASE_PATH = String(import.meta.env.BASE_URL ?? "/").replace(/\/+$/, "");

export function apiUrl(path: string): string {
  const normalized = path.startsWith("/") ? path : `/${path}`;
  return `${API_BASE}${normalized}`;
}

export function assetUrl(path: string): string {
  const clean = path.replace(/^\/+/, "");
  return `${BASE_PATH}/${clean}`;
}
