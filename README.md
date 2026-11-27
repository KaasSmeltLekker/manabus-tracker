# ManaBus? (`manabus-tracker`)

Tracker bus kota realtime untuk **Surabaya** dan **Yogyakarta**. Posisi bus, jalur koridor, daftar halte, sampai perkiraan waktu tiba ada di satu peta.

```
manabus-tracker/
├─ apps/
│  ├─ web/          # Frontend Vue 3 + Vite + TS (Leaflet & MapLibre)
│  └─ proxy/        # Backend Rust (axum + socketioxide + rust_socketio)
├─ ops/             # docker-compose, Dockerfile, config Nginx
├─ docs/            # Dokumentasi Public REST API
└─ scripts/         # Skrip bantu
```

## Public API

Endpoint publik `/api/v1` (read-only, tanpa API key) didokumentasikan di [`docs/public-api.md`](docs/public-api.md).

Jangan pernah commit rahasia atau kredensial ke repo. Simpan semuanya di `.env` lokal atau environment variable server.

## Jalankan di Lokal

Buat file `.env` di root proyek, lalu jalankan:

```bash
pnpm install
cargo fetch --manifest-path apps/proxy/Cargo.toml
pnpm dev
```

Perintah `pnpm dev` menjalankan frontend web dan proxy Rust barengan.

## Deploy ke GitHub Pages

Frontend `apps/web` di-build jadi situs statis lewat `.github/workflows/pages.yml`, sementara `apps/proxy` di-host terpisah.

1. Di repo GitHub, buka **Settings → Pages → Source**, pilih **GitHub Actions**.
2. Di **Settings → Secrets and variables → Actions → Variables**, tambah variabel `VITE_API_BASE` berisi URL HTTPS server `manabus-proxy` (misalnya `https://api.example.com`).
3. Di server `manabus-proxy`, isi `ALLOW_ORIGIN` dengan origin GitHub Pages kamu (misalnya `https://<username>.github.io`).

## Status

- [x] Koneksi data upstream & live feed GPS (enkripsi, auth, stream)
- [x] Proxy Rust: endpoint REST, modul enkripsi, dan Public REST API `v1`
- [x] Bridge Socket.IO realtime (`BRT-{pref}` untuk Surabaya `3501` & Yogyakarta `1`)
- [x] Web: peta, daftar koridor, detail halte, dan panel admin `/atmin`
- [x] Build & workflow deploy GitHub Pages
- [ ] Deploy produksi
