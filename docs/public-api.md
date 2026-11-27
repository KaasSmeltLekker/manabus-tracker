# ManaBus? Public REST API (`v1`)

API JSON read-only dari `manabus-proxy`. Enkripsi (`GPS_KEY`), bearer token, dan kredensial socket cuma hidup di server — web atau app luar bisa langsung pakai tanpa API key.

## Dasar

- **Base URL**: `https://<domain-kamu>/api/v1` (lokal: `http://localhost:8080/api/v1`)
- **Auth**: Tidak perlu (read-only)
- **Data**: Langsung dari sumber transit mentah, tidak terpengaruh editan panel `/atmin`
- **CORS**: Terbuka untuk `GET` dan `OPTIONS` dari origin mana saja (`Access-Control-Allow-Origin: *`)
- **Rate limit**: `120` request per IP tiap 60 detik
  - Header balasan: `X-RateLimit-Limit`, `X-RateLimit-Remaining`
  - Lewat batas bakal dapat HTTP `429`, header `Retry-After`, dan JSON `{ "ok": false, "error": "rate limit exceeded", "retry_after_s": <detik> }`

## Kode Kota

Parameter `:city` menerima salah satu nilai ini:

| Kota | Nilai `:city` |
| --- | --- |
| Kota Surabaya | `surabaya`, `sby`, `3501` |
| Yogyakarta | `yogyakarta`, `jogja`, `yk`, `1` |

## Endpoint

### `GET /api/v1`
Info versi API, batas rate limit, daftar kota, dan format URL tiap endpoint.

### `GET /api/v1/status`
Cek koneksi socket GPS, uptime server, dan jumlah bus yang sedang aktif di tiap kota.

```json
{
  "ok": true,
  "upstream_connected": true,
  "last_event_age_s": 1,
  "uptime_s": 342,
  "buses": {
    "surabaya": 143,
    "yogyakarta": 108
  }
}
```

### `GET /api/v1/cities`
Daftar kota beserta titik tengah peta (`center`) dan jumlah armada aktif.

### `GET /api/v1/cities/:city/corridors`
Daftar koridor di satu kota, termasuk polyline rute dan jumlah bus aktifnya.

- **Query**:
  - `q` (opsional): cari kode koridor, nama rute, `via`, `origin`, atau `toward` (tidak peka huruf besar/kecil).
- **Cache**: `Cache-Control: public, max-age=60`

### `GET /api/v1/cities/:city/corridors/:code/stops`
Urutan halte untuk satu koridor di arah tertentu.

- **Query**:
  - `direction` (opsional, default `a`): `a` (`origin -> toward`) atau `b` (`toward -> origin`).
- **Cache**: `Cache-Control: public, max-age=60`

### `GET /api/v1/cities/:city/stops`
Daftar halte di satu kota.

- **Query**:
  - `corridor` (opsional): filter halte berdasarkan kode koridor (misalnya `R1`, `1A`).
  - `q` (opsional): cari nama atau ID halte (tidak peka huruf besar/kecil).
- **Cache**: `Cache-Control: public, max-age=60`

### `GET /api/v1/cities/:city/buses`
Posisi bus realtime plus kecepatan, arah, jumlah penumpang, dan halte terdekat.

- **Query**:
  - `corridor` (opsional): filter per kode koridor (misalnya `R1`, `1A`, `FD2`).
  - `include_stale` (opsional, default `true`): ubah ke `false` kalau mau buang bus yang GPS-nya tidak update lebih dari 5 menit.
- **Cache**: `Cache-Control: no-store`

```json
{
  "ok": true,
  "city": "surabaya",
  "count": 1,
  "updated_at": 1795738000,
  "data": [
    {
      "id": "869066065363849",
      "name": "SB66",
      "plate": "L 1033 UB",
      "corridor": "R2",
      "toward": "Halte Balai Kota Surabaya",
      "lat": -7.291541,
      "lng": 112.717876,
      "heading": 99.0,
      "speed_kmh": 26.0,
      "next_stop_id": "0040409",
      "prev_stop_id": "0037941",
      "passengers": 14,
      "fix_age_s": 9,
      "stale": false,
      "parked": false,
      "stopped_s": 0
    }
  ]
}
```

## Contoh

```bash
curl "http://localhost:8080/api/v1/cities/surabaya/buses?corridor=R1&include_stale=false"
curl "http://localhost:8080/api/v1/cities/yogyakarta/corridors/1A/stops?direction=a"
```

```js
const res = await fetch("https://domain-kamu.example/api/v1/cities/surabaya/buses?corridor=R1");
const { data: buses } = await res.json();
```
