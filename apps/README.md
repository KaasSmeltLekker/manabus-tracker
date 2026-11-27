# Aplikasi ManaBus? (`manabus-tracker`)

Folder ini menampung dua app utama ManaBus?:

```
apps/
├─ web/          # Frontend Vue 3 + Vite + TS (Leaflet & MapLibre)
└─ proxy/        # Backend Rust (axum + socketioxide + rust_socketio)
```

Endpoint publik `/api/v1` (read-only) bisa dilihat di [`../docs/public-api.md`](../docs/public-api.md). JANGAN PERNAH COMMIT CREDENTIALS! Save semuanya di `.env` lokal D:

## Jalanin di localhost

Dari root repo:

```bash
pnpm install
cargo fetch --manifest-path apps/proxy/Cargo.toml
pnpm dev
```
