# ManaBus? (`manabus-tracker`)

Realtime public transit tracker for **Surabaya** and **Yogyakarta**,
providing live vehicle positions, route polylines, stop lookups, and arrival ETAs.

Layout:

```
manabus-tracker/
├─ apps/
│  ├─ web/          # Vue 3 + Vite + TS frontend (Leaflet map)
│  └─ proxy/        # Rust: axum + socketioxide + rust_socketio
├─ ops/             # docker-compose, Dockerfiles
├─ docs/            # Public REST API documentation
└─ scripts/         # one-off helpers
```

## Public API

See `docs/public-api.md` for read-only `/api/v1` endpoints exposed without needing upstream keys.

**Never commit real secrets.** Copy `.env.example` to `.env` and fill
in the key material there.

## Local development

```bash
cp .env.example .env
pnpm install          # web deps
cargo fetch --manifest-path apps/proxy/Cargo.toml
pnpm dev               # runs web + proxy together, once wired up
```

## Status

- [x] Upstream API fully reverse-engineered & validated (crypto, auth, live feed)
- [x] Proxy: REST endpoints + crypto module
- [x] Proxy: Socket.IO bridge — typed, real-time `BRT-{pref}` forwarding (3501, 1)
- [x] Web: map + corridor list scaffolded
- [x] Confirm live-position event name/shape, switch bridge from generic forwarding to typed
- [ ] Polish + deploy
