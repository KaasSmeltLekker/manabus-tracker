# ManaBus? Public REST API (`v1`)

Read-only public JSON API backed by `manabus-proxy`. All upstream encryption (`GPS_KEY`), bearer tokens, and socket credentials stay strictly on the server — external apps and websites never need an API key or cryptographic secret.

## Basics

- **Base URL**: `https://<your-domain>/api/v1` (or `http://localhost:8080/api/v1` locally)
- **Authentication**: None required (read-only)
- **Data Source**: Serves raw upstream transit data directly; unaffected by `/atmin` admin overrides
- **CORS**: Enabled for `GET` / `OPTIONS` from any origin (`Access-Control-Allow-Origin: *`)
- **Rate Limit**: `120` requests per IP per 60 seconds
  - Response headers: `X-RateLimit-Limit`, `X-RateLimit-Remaining`
  - Exceeding the limit returns HTTP `429` with `Retry-After` and `{ "ok": false, "error": "rate limit exceeded", "retry_after_s": <seconds> }`

## Supported Cities

Use any of these values for `:city`:

| City | Accepted `:city` values |
| --- | --- |
| Kota Surabaya | `surabaya`, `sby`, `3501` |
| Yogyakarta | `yogyakarta`, `jogja`, `yk`, `1` |

## Endpoints

### `GET /api/v1`
Returns API metadata, rate-limit settings, supported cities, and endpoint templates.

### `GET /api/v1/status`
Returns upstream GPS socket health, uptime, and active bus counts per city.

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
Lists all supported cities with coordinates (`center`) and live fleet counts.

### `GET /api/v1/cities/:city/corridors`
Lists routes/corridors for the given city, including route polylines and live bus counts.

- **Query params**:
  - `q` (optional): Case-insensitive filter on corridor code, route name, `via`, `origin`, or `toward`.
- **Caching**: `Cache-Control: public, max-age=60`

### `GET /api/v1/cities/:city/corridors/:code/stops`
Returns the ordered list of stops along a specific corridor and direction.

- **Query params**:
  - `direction` (optional, default `a`): `a` (`origin -> toward`) or `b` (`toward -> origin`).
- **Caching**: `Cache-Control: public, max-age=60`

### `GET /api/v1/cities/:city/stops`
Lists stops in the city.

- **Query params**:
  - `corridor` (optional): Filter stops serving a specific corridor code (e.g. `R1`, `1A`).
  - `q` (optional): Case-insensitive search by stop name or ID.
- **Caching**: `Cache-Control: public, max-age=60`

### `GET /api/v1/cities/:city/buses`
Returns live bus positions and telemetry.

- **Query params**:
  - `corridor` (optional): Filter by corridor code (e.g. `R1`, `1A`, `FD2`).
  - `include_stale` (optional, default `true`): Set `false` to exclude vehicles with no GPS fix in over 5 minutes.
- **Caching**: `Cache-Control: no-store`

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

## Examples

```bash
curl "http://localhost:8080/api/v1/cities/surabaya/buses?corridor=R1&include_stale=false"
curl "http://localhost:8080/api/v1/cities/yogyakarta/corridors/1A/stops?direction=a"
```

```js
const res = await fetch("https://your-domain.example/api/v1/cities/surabaya/buses?corridor=R1");
const { data: buses } = await res.json();
```
