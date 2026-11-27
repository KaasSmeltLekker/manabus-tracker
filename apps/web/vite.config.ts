import { copyFileSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import { defineConfig, loadEnv, type Plugin } from "vite";
import vue from "@vitejs/plugin-vue";

function normalizeBasePath(raw?: string): string {
  const trimmed = String(raw ?? "").trim();
  if (!trimmed || trimmed === "/") return "/";
  const withLead = trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
  return withLead.endsWith("/") ? withLead : `${withLead}/`;
}

function maplibreWorkerAssets(): Plugin {
  const require = createRequire(import.meta.url);
  return {
    name: "maplibre-worker-assets",
    apply: "build",
    generateBundle() {
      for (const name of ["maplibre-gl-worker.mjs", "maplibre-gl-shared.mjs"]) {
        this.emitFile({
          type: "asset",
          fileName: name,
          source: readFileSync(require.resolve(`maplibre-gl/dist/${name}`)),
        });
      }
    },
  };
}

function githubPagesAssets(): Plugin {
  let outDir = "dist";
  return {
    name: "github-pages-assets",
    apply: "build",
    configResolved(cfg) {
      outDir = cfg.build.outDir;
    },
    closeBundle() {
      if (process.env.CF_PAGES === "1") return;
      const target = resolve(outDir);
      copyFileSync(resolve(target, "index.html"), resolve(target, "404.html"));
      writeFileSync(resolve(target, ".nojekyll"), "");
    },
  };
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const target = env.VITE_PROXY_TARGET || "http://localhost:8080";

  return {
    base: normalizeBasePath(env.VITE_BASE_PATH),
    plugins: [vue(), maplibreWorkerAssets(), githubPagesAssets()],
    optimizeDeps: {
      exclude: ["maplibre-gl"],
    },
    build: {
      rollupOptions: {
        output: {
          manualChunks(id) {
            if (id.includes("node_modules/maplibre-gl")) return "maplibre";
            if (id.includes("node_modules/leaflet")) return "leaflet";
          },
        },
      },
    },
    server: {
      allowedHosts: [".ngrok-free.dev", ".ngrok-free.app", ".trycloudflare.com"],
      proxy: {
        "/api": { target, changeOrigin: true },
        "/socket.io": { target, ws: true, changeOrigin: true },
      },
    },
  };
});
