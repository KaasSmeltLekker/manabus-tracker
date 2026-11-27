# ManaBus? Public API (`v1`)

API JSON read-only dari `manabus-proxy`. Tidak perlu API key, CORS terbuka (`*`), dan datanya mentah langsung dari upstream (tidak terpengaruh `/atmin`).

- **Base URL**: `https://<domain-kamu>/api/v1` (lokal: `http://localhost:8080/api/v1`)
- **Rate limit**: `120` req/menit per IP (`429` + header `Retry-After` kalau lewat)

## Endpoint

| Method & Path | Query | Fungsi |
| --- | --- | --- |
| `GET /api/v1` | — | Info API & daftar endpoint |
| `GET /api/v1/status` | — | Status GPS & jumlah bus aktif |
| `GET /api/v1/cities` | — | Daftar semua kota + jumlah bus |
| `GET /api/v1/cities/:city/corridors` | `q` | Daftar koridor & polyline (`max-age=60`) |
| `GET /api/v1/cities/:city/corridors/:code/stops` | `direction=a\|b` | Urutan halte per koridor (`max-age=60`) |
| `GET /api/v1/cities/:city/stops` | `corridor`, `q` | Daftar halte di kota (`max-age=60`) |
| `GET /api/v1/cities/:city/buses` | `corridor`, `include_stale=true\|false` | Posisi & telemetri bus realtime (`no-store`) |

## Kode `:city`

Bisa pakai slug, singkatan, atau kode `pref`:

| Kota | Nilai `:city` |
| --- | --- |
| Surabaya | `surabaya`, `sby`, `3501` |
| Yogyakarta | `yogyakarta`, `jogja`, `yk`, `1` |
| Semarang | `semarang`, `smg`, `0` |
| Jawa Tengah | `jateng`, `jt`, `3` |
| Batam | `batam`, `btm`, `4` |
| Jawa Timur | `jatim`, `jtm`, `5` |
| Tangerang | `tangerang`, `tng`, `6` |
| Banjarmasin | `banjarmasin`, `bjm`, `7` |
| Aceh | `aceh`, `9` |
| Banjarbaru | `banjarbaru`, `bjb`, `10` |
| Donggala | `donggala`, `dgl`, `11` |
| Palu | `palu`, `plw`, `12` |
| Palembang | `palembang`, `plg`, `13` |
| Bali | `bali`, `15` |
| Pekanbaru | `pekanbaru`, `pku`, `70` |
| Medan | `medan`, `mdn`, `3502` |

## Contoh

```bash
curl "http://localhost:8080/api/v1/cities/surabaya/buses?corridor=R1&include_stale=false"
curl "http://localhost:8080/api/v1/cities/semarang/corridors"
```

```js
const res = await fetch("https://domain-kamu.example/api/v1/cities/bali/buses");
const { data: buses } = await res.json();
```
