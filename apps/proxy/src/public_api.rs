use crate::admin::norm_code;
use crate::socket_bridge::Bus;
use crate::{
    cached_json_value, client_ip_key, fetch_route_corridor_raw, fetch_with_pref, AppState,
    RouteCorridorQuery,
};
use axum::{
    body::Bytes,
    extract::{ConnectInfo, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PUBLIC_RATE_LIMIT: u32 = 120;
const PUBLIC_RATE_WINDOW: Duration = Duration::from_secs(60);
const PUBLIC_LIMITER_MAX_KEYS: usize = 4096;
const PARKED_AFTER_S: i64 = 10 * 60;

#[derive(Clone, Copy)]
struct CityMeta {
    slug: &'static str,
    pref: &'static str,
    name: &'static str,
    short_code: &'static str,
    center: [f64; 2],
}

const CITIES: &[CityMeta] = &[
    CityMeta {
        slug: "surabaya",
        pref: "3501",
        name: "Kota Surabaya",
        short_code: "SB",
        center: [-7.2575, 112.7521],
    },
    CityMeta {
        slug: "yogyakarta",
        pref: "1",
        name: "Yogyakarta",
        short_code: "YK",
        center: [-7.7956, 110.3695],
    },
];

fn resolve_city(raw: &str) -> Option<CityMeta> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "surabaya" | "sby" | "3501" => Some(CITIES[0]),
        "yogyakarta" | "jogja" | "yk" | "1" => Some(CITIES[1]),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RateDecision {
    pub allowed: bool,
    pub limit: u32,
    pub remaining: u32,
    pub retry_after_s: u64,
}

struct WindowEntry {
    count: u32,
    started_at: Instant,
}

pub(crate) struct PublicRateLimiter {
    entries: Mutex<HashMap<String, WindowEntry>>,
}

impl PublicRateLimiter {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn check(&self, key: &str, now: Instant) -> RateDecision {
        let mut guard = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        guard.retain(|_, v| now.saturating_duration_since(v.started_at) < PUBLIC_RATE_WINDOW);
        if !guard.contains_key(key) && guard.len() >= PUBLIC_LIMITER_MAX_KEYS {
            if let Some(oldest) = guard
                .iter()
                .min_by_key(|(_, v)| v.started_at)
                .map(|(k, _)| k.clone())
            {
                guard.remove(&oldest);
            }
        }
        let entry = guard.entry(key.to_string()).or_insert(WindowEntry {
            count: 0,
            started_at: now,
        });
        let elapsed = now.saturating_duration_since(entry.started_at);
        let retry_after_s = PUBLIC_RATE_WINDOW
            .saturating_sub(elapsed)
            .as_secs()
            .max(1);
        if entry.count >= PUBLIC_RATE_LIMIT {
            return RateDecision {
                allowed: false,
                limit: PUBLIC_RATE_LIMIT,
                remaining: 0,
                retry_after_s,
            };
        }
        entry.count = entry.count.saturating_add(1);
        RateDecision {
            allowed: true,
            limit: PUBLIC_RATE_LIMIT,
            remaining: PUBLIC_RATE_LIMIT.saturating_sub(entry.count),
            retry_after_s,
        }
    }
}

pub(crate) fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1", get(api_index))
        .route("/api/v1/", get(api_index))
        .route("/api/v1/status", get(api_status))
        .route("/api/v1/cities", get(api_cities))
        .route("/api/v1/cities/:city/corridors", get(api_corridors))
        .route(
            "/api/v1/cities/:city/corridors/:code/stops",
            get(api_corridor_stops),
        )
        .route("/api/v1/cities/:city/stops", get(api_stops))
        .route("/api/v1/cities/:city/buses", get(api_buses))
}

fn public_json(
    status: StatusCode,
    body: Value,
    cache_control: &'static str,
    rate: RateDecision,
) -> Response {
    let bytes = Bytes::from(serde_json::to_vec(&body).unwrap_or_default());
    let mut resp = (
        status,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static(cache_control)),
        ],
        bytes,
    )
        .into_response();
    let headers = resp.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&rate.limit.to_string()) {
        headers.insert("x-ratelimit-limit", v);
    }
    if let Ok(v) = HeaderValue::from_str(&rate.remaining.to_string()) {
        headers.insert("x-ratelimit-remaining", v);
    }
    if !rate.allowed {
        if let Ok(v) = HeaderValue::from_str(&rate.retry_after_s.to_string()) {
            headers.insert(header::RETRY_AFTER, v);
        }
    }
    resp
}

pub(crate) fn rate_limited_response(rate: RateDecision) -> Response {
    public_json(
        StatusCode::TOO_MANY_REQUESTS,
        json!({
            "ok": false,
            "error": "rate limit exceeded",
            "retry_after_s": rate.retry_after_s,
        }),
        "no-store",
        rate,
    )
}

fn check_rate(
    state: &AppState,
    headers: &HeaderMap,
    addr: SocketAddr,
) -> Result<RateDecision, Box<Response>> {
    let key = client_ip_key(headers, Some(addr.ip()), state.trust_proxy);
    let rate = state.public_limiter.check(&key, Instant::now());
    if !rate.allowed {
        return Err(Box::new(rate_limited_response(rate)));
    }
    Ok(rate)
}

fn city_not_found(rate: RateDecision) -> Response {
    public_json(
        StatusCode::NOT_FOUND,
        json!({
            "ok": false,
            "error": "unknown city",
            "supported_cities": ["surabaya", "yogyakarta"],
        }),
        "no-store",
        rate,
    )
}

async fn api_index(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let rate = match check_rate(&state, &headers, addr) {
        Ok(r) => r,
        Err(resp) => return *resp,
    };
    let cities: Vec<Value> = CITIES
        .iter()
        .map(|c| {
            json!({
                "id": c.slug,
                "name": c.name,
                "short_code": c.short_code,
            })
        })
        .collect();
    public_json(
        StatusCode::OK,
        json!({
            "ok": true,
            "version": "v1",
            "rate_limit": {
                "requests_per_minute": PUBLIC_RATE_LIMIT,
            },
            "cities": cities,
            "endpoints": {
                "status": "/api/v1/status",
                "cities": "/api/v1/cities",
                "corridors": "/api/v1/cities/:city/corridors?q=",
                "corridor_stops": "/api/v1/cities/:city/corridors/:code/stops?direction=a|b",
                "stops": "/api/v1/cities/:city/stops?corridor=&q=",
                "buses": "/api/v1/cities/:city/buses?corridor=&include_stale=true|false",
            }
        }),
        "public, max-age=300",
        rate,
    )
}

async fn api_status(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let rate = match check_rate(&state, &headers, addr) {
        Ok(r) => r,
        Err(resp) => return *resp,
    };
    let now = crate::socket_bridge::now_unix();
    let counts = state.live.counts(now);
    public_json(
        StatusCode::OK,
        json!({
            "ok": true,
            "upstream_connected": state.upstream_health.is_connected(),
            "last_event_age_s": state.upstream_health.last_event_age_s(now),
            "uptime_s": state.upstream_health.uptime_s(),
            "buses": {
                "surabaya": counts.get("3501").copied().unwrap_or(0),
                "yogyakarta": counts.get("1").copied().unwrap_or(0),
            }
        }),
        "no-store",
        rate,
    )
}

async fn api_cities(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let rate = match check_rate(&state, &headers, addr) {
        Ok(r) => r,
        Err(resp) => return *resp,
    };
    let now = crate::socket_bridge::now_unix();
    let counts = state.live.counts(now);
    let data: Vec<Value> = CITIES
        .iter()
        .map(|c| {
            json!({
                "id": c.slug,
                "name": c.name,
                "short_code": c.short_code,
                "center": c.center,
                "live_buses": counts.get(c.pref).copied().unwrap_or(0),
            })
        })
        .collect();
    public_json(
        StatusCode::OK,
        json!({
            "ok": true,
            "count": data.len(),
            "data": data,
        }),
        "no-store",
        rate,
    )
}

#[derive(Deserialize)]
struct CorridorsQuery {
    q: Option<String>,
}

async fn api_corridors(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(city_raw): Path<String>,
    Query(q): Query<CorridorsQuery>,
) -> Response {
    let rate = match check_rate(&state, &headers, addr) {
        Ok(r) => r,
        Err(resp) => return *resp,
    };
    let Some(city) = resolve_city(&city_raw) else {
        return city_not_found(rate);
    };
    let pref = city.pref.to_string();
    let payload = match cached_json_value(
        state.clone(),
        format!("raw:getCorridor:{pref}"),
        move |st| async move { fetch_with_pref(&st, "getCorridor", &pref).await },
    )
    .await
    {
        Ok(v) => v,
        Err(status) => {
            return public_json(
                status,
                json!({ "ok": false, "error": "upstream unavailable" }),
                "no-store",
                rate,
            );
        }
    };
    let now = crate::socket_bridge::now_unix();
    let buses = state.live.snapshot(city.pref, now);
    let mut bus_counts: HashMap<String, usize> = HashMap::new();
    for b in &buses {
        if let Some(code) = b
            .corridor
            .as_deref()
            .map(|c| resolve_bus_corridor(c, city.pref))
            .filter(|c| !c.is_empty())
        {
            *bus_counts.entry(norm_code(&code)).or_insert(0) += 1;
        }
    }
    let query = q
        .q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase());
    let rows = payload
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut data = Vec::new();
    for row in &rows {
        let code = corridor_str(row, &["kor", "kode", "corridor", "route_code"]);
        if code.is_empty() || (city.pref == "3501" && norm_code(&code) == "SBT") {
            continue;
        }
        let name = corridor_str(row, &["route_name"]);
        let via = opt_str(row.get("via"));
        let origin = corridor_str(row, &["origin"]);
        let toward = corridor_str(row, &["toward"]);
        if let Some(needle) = &query {
            let matches = code.to_ascii_lowercase().contains(needle)
                || name.to_ascii_lowercase().contains(needle)
                || via
                    .as_deref()
                    .is_some_and(|v| v.to_ascii_lowercase().contains(needle))
                || origin.to_ascii_lowercase().contains(needle)
                || toward.to_ascii_lowercase().contains(needle);
            if !matches {
                continue;
            }
        }
        let active_buses = bus_counts.get(&norm_code(&code)).copied().unwrap_or(0);
        data.push(json!({
            "id": val_id(row.get("id")),
            "code": code,
            "name": if name.is_empty() { Value::String(code.clone()) } else { Value::String(name) },
            "via": via,
            "origin": if origin.is_empty() { Value::Null } else { Value::String(origin) },
            "toward": if toward.is_empty() { Value::Null } else { Value::String(toward) },
            "operating_hours": opt_str(row.get("jam_operasional")),
            "color": opt_str(row.get("color")),
            "polyline_a": opt_str(row.get("points_a")),
            "polyline_b": opt_str(row.get("points_b")),
            "active_buses": active_buses,
        }));
    }
    public_json(
        StatusCode::OK,
        json!({
            "ok": true,
            "city": city.slug,
            "count": data.len(),
            "data": data,
        }),
        "public, max-age=60",
        rate,
    )
}

#[derive(Deserialize)]
struct StopsQuery {
    corridor: Option<String>,
    q: Option<String>,
}

async fn api_stops(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(city_raw): Path<String>,
    Query(q): Query<StopsQuery>,
) -> Response {
    let rate = match check_rate(&state, &headers, addr) {
        Ok(r) => r,
        Err(resp) => return *resp,
    };
    let Some(city) = resolve_city(&city_raw) else {
        return city_not_found(rate);
    };
    let pref = city.pref.to_string();
    let payload = match cached_json_value(
        state.clone(),
        format!("raw:getRoutes:{pref}"),
        move |st| async move { fetch_with_pref(&st, "getRoutes", &pref).await },
    )
    .await
    {
        Ok(v) => v,
        Err(status) => {
            return public_json(
                status,
                json!({ "ok": false, "error": "upstream unavailable" }),
                "no-store",
                rate,
            );
        }
    };
    let filter_corridor = q
        .corridor
        .as_deref()
        .map(norm_code)
        .filter(|s| !s.is_empty());
    let query = q
        .q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase());
    let rows = payload
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut data = Vec::new();
    for row in &rows {
        let Some(stop) = normalize_stop_json(row, city.pref, None) else {
            continue;
        };
        if let Some(target) = &filter_corridor {
            let serves = stop["corridors"]
                .as_array()
                .is_some_and(|arr| arr.iter().any(|c| norm_code(c.as_str().unwrap_or("")) == *target))
                || stop["primary_corridor"]
                    .as_str()
                    .is_some_and(|c| norm_code(c) == *target);
            if !serves {
                continue;
            }
        }
        if let Some(needle) = &query {
            let name = stop["name"].as_str().unwrap_or("").to_ascii_lowercase();
            let id = stop["id"].as_str().unwrap_or("").to_ascii_lowercase();
            if !name.contains(needle) && !id.contains(needle) {
                continue;
            }
        }
        data.push(stop);
    }
    public_json(
        StatusCode::OK,
        json!({
            "ok": true,
            "city": city.slug,
            "count": data.len(),
            "data": data,
        }),
        "public, max-age=60",
        rate,
    )
}

#[derive(Deserialize)]
struct BusesQuery {
    corridor: Option<String>,
    include_stale: Option<bool>,
}

async fn api_buses(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(city_raw): Path<String>,
    Query(q): Query<BusesQuery>,
) -> Response {
    let rate = match check_rate(&state, &headers, addr) {
        Ok(r) => r,
        Err(resp) => return *resp,
    };
    let Some(city) = resolve_city(&city_raw) else {
        return city_not_found(rate);
    };
    let include_stale = q.include_stale.unwrap_or(true);
    let filter_corridor = q
        .corridor
        .as_deref()
        .map(norm_code)
        .filter(|s| !s.is_empty());
    let now = crate::socket_bridge::now_unix();
    let list = state.live.snapshot(city.pref, now);
    let mut data: Vec<Value> = list
        .iter()
        .filter(|b| include_stale || !b.stale)
        .filter_map(|b| {
            let bus_json = normalize_bus_json(b, city.pref);
            if let Some(target) = &filter_corridor {
                let code = bus_json["corridor"].as_str().unwrap_or("");
                if norm_code(code) != *target {
                    return None;
                }
            }
            Some(bus_json)
        })
        .collect();
    data.sort_by(|a, b| {
        a["id"]
            .as_str()
            .unwrap_or("")
            .cmp(b["id"].as_str().unwrap_or(""))
    });
    public_json(
        StatusCode::OK,
        json!({
            "ok": true,
            "city": city.slug,
            "count": data.len(),
            "updated_at": now,
            "data": data,
        }),
        "no-store",
        rate,
    )
}

#[derive(Deserialize)]
struct DirectionQuery {
    direction: Option<String>,
}

async fn api_corridor_stops(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path((city_raw, code_raw)): Path<(String, String)>,
    Query(q): Query<DirectionQuery>,
) -> Response {
    let rate = match check_rate(&state, &headers, addr) {
        Ok(r) => r,
        Err(resp) => return *resp,
    };
    let Some(city) = resolve_city(&city_raw) else {
        return city_not_found(rate);
    };
    let code_trim = code_raw.trim();
    if code_trim.is_empty() || code_trim.len() > 32 {
        return public_json(
            StatusCode::BAD_REQUEST,
            json!({ "ok": false, "error": "invalid corridor code" }),
            "no-store",
            rate,
        );
    }
    let dir = match q
        .direction
        .as_deref()
        .map(|s| s.trim().to_ascii_lowercase())
        .as_deref()
    {
        None | Some("") | Some("a") => "a",
        Some("b") => "b",
        Some(_) => {
            return public_json(
                StatusCode::BAD_REQUEST,
                json!({ "ok": false, "error": "direction must be 'a' or 'b'" }),
                "no-store",
                rate,
            );
        }
    };
    let pref = city.pref.to_string();
    let corridors_payload = match cached_json_value(
        state.clone(),
        format!("raw:getCorridor:{pref}"),
        move |st| async move { fetch_with_pref(&st, "getCorridor", &pref).await },
    )
    .await
    {
        Ok(v) => v,
        Err(status) => {
            return public_json(
                status,
                json!({ "ok": false, "error": "upstream unavailable" }),
                "no-store",
                rate,
            );
        }
    };
    let target_norm = norm_code(code_trim);
    let corridors = corridors_payload
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let found = corridors.iter().find(|row| {
        let code = corridor_str(row, &["kor", "kode", "corridor", "route_code"]);
        norm_code(&code) == target_norm
    });
    let Some(corridor_row) = found else {
        return public_json(
            StatusCode::NOT_FOUND,
            json!({ "ok": false, "error": "corridor not found" }),
            "no-store",
            rate,
        );
    };
    let canonical_code = corridor_str(corridor_row, &["kor", "kode", "corridor", "route_code"]);
    let raw_origin = corridor_str(corridor_row, &["origin"]);
    let raw_toward = corridor_str(corridor_row, &["toward"]);
    if raw_origin.is_empty() || raw_toward.is_empty() {
        return public_json(
            StatusCode::OK,
            json!({
                "ok": true,
                "city": city.slug,
                "corridor": canonical_code,
                "direction": dir,
                "origin": Value::Null,
                "toward": Value::Null,
                "count": 0,
                "data": [],
            }),
            "public, max-age=60",
            rate,
        );
    }
    let (origin, toward) = if dir == "a" {
        (raw_origin, raw_toward)
    } else {
        (raw_toward, raw_origin)
    };
    let pref_clone = city.pref.to_string();
    let leg_key = format!("raw:route-corridor:{pref_clone}:{canonical_code}:{origin}:{toward}");
    let query_obj = RouteCorridorQuery {
        pref: Some(pref_clone.clone()),
        kor: canonical_code.clone(),
        origin: origin.clone(),
        toward: toward.clone(),
    };
    let stops_payload = match cached_json_value(state.clone(), leg_key, move |st| async move {
        fetch_route_corridor_raw(&st, &query_obj, &pref_clone).await
    })
    .await
    {
        Ok(v) => v,
        Err(status) => {
            return public_json(
                status,
                json!({ "ok": false, "error": "upstream unavailable" }),
                "no-store",
                rate,
            );
        }
    };
    let rows = stops_payload
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut data = Vec::new();
    for (idx, row) in rows.iter().enumerate() {
        if let Some(stop) = normalize_stop_json(row, city.pref, Some(idx + 1)) {
            data.push(stop);
        }
    }
    public_json(
        StatusCode::OK,
        json!({
            "ok": true,
            "city": city.slug,
            "corridor": canonical_code,
            "direction": dir,
            "origin": origin,
            "toward": toward,
            "count": data.len(),
            "data": data,
        }),
        "public, max-age=60",
        rate,
    )
}

fn corridor_str(row: &Value, keys: &[&str]) -> String {
    for k in keys {
        if let Some(val) = row.get(*k) {
            if let Some(s) = val.as_str() {
                let t = s.trim();
                if !t.is_empty() {
                    return t.to_string();
                }
            } else if let Some(n) = val.as_i64() {
                return n.to_string();
            }
        }
    }
    String::new()
}

fn opt_str(val: Option<&Value>) -> Option<String> {
    let s = val?.as_str()?.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn val_id(val: Option<&Value>) -> Value {
    match val {
        Some(Value::String(s)) => Value::String(s.trim().to_string()),
        Some(Value::Number(n)) => Value::String(n.to_string()),
        _ => Value::String(String::new()),
    }
}

fn parse_f64_field(val: Option<&Value>) -> Option<f64> {
    match val? {
        Value::Number(n) => n.as_f64().filter(|x| x.is_finite()),
        Value::String(s) => s.trim().parse::<f64>().ok().filter(|x| x.is_finite()),
        _ => None,
    }
}

fn normalize_stop_json(row: &Value, pref: &str, order: Option<usize>) -> Option<Value> {
    let lat = parse_f64_field(row.get("sh_lat"))?;
    let lng = parse_f64_field(row.get("sh_lng"))?;
    if lat == 0.0 && lng == 0.0 {
        return None;
    }
    let is_surabaya = pref == "3501";
    let primary_raw = corridor_str(row, &["kor"]);
    let primary = if is_surabaya && norm_code(&primary_raw) == "SBT" {
        String::new()
    } else {
        primary_raw.to_uppercase()
    };
    let in_kor = corridor_str(row, &["in_koridor", "kor"]);
    let mut corridors = Vec::new();
    for piece in in_kor.split('|') {
        let code = piece.trim().to_uppercase();
        if code.is_empty() || (is_surabaya && norm_code(&code) == "SBT") {
            continue;
        }
        if !corridors.contains(&code) {
            corridors.push(code);
        }
    }
    if corridors.is_empty() && !primary.is_empty() {
        corridors.push(primary.clone());
    }
    let id = match row.get("sh_id") {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    let name = corridor_str(row, &["sh_name"]);
    let toward = opt_str(row.get("toward"));
    let primary_val = if primary.is_empty() {
        corridors
            .first()
            .cloned()
            .map(Value::String)
            .unwrap_or(Value::Null)
    } else {
        Value::String(primary)
    };
    let mut obj = serde_json::Map::new();
    if let Some(pos) = order {
        obj.insert("order".into(), json!(pos));
    }
    obj.insert("id".into(), Value::String(id));
    obj.insert("name".into(), Value::String(name));
    obj.insert("lat".into(), json!(lat));
    obj.insert("lng".into(), json!(lng));
    obj.insert("primary_corridor".into(), primary_val);
    obj.insert("corridors".into(), json!(corridors));
    obj.insert("toward".into(), json!(toward));
    Some(Value::Object(obj))
}

fn resolve_bus_corridor(raw: &str, pref: &str) -> String {
    let trimmed = raw.trim().to_uppercase();
    let tracked = if pref == "3501" && norm_code(&trimmed) == "SBT" {
        "R1".to_string()
    } else {
        trimmed
    };
    if norm_code(&tracked).is_empty() {
        return String::new();
    }
    tracked
}

fn normalize_bus_json(bus: &Bus, pref: &str) -> Value {
    let corridor = bus
        .corridor
        .as_deref()
        .map(|c| resolve_bus_corridor(c, pref))
        .filter(|c| !c.is_empty());
    let parked = bus.speed.is_some_and(|s| s.round() == 0.0) && bus.stopped_s >= PARKED_AFTER_S;
    json!({
        "id": bus.id,
        "name": bus.name,
        "plate": bus.plate,
        "corridor": corridor,
        "toward": bus.toward,
        "lat": bus.lat,
        "lng": bus.lng,
        "heading": bus.heading,
        "speed_kmh": bus.speed,
        "next_stop_id": bus.next_halte,
        "prev_stop_id": bus.prev_halte,
        "passengers": bus.passengers,
        "fix_age_s": bus.fix_age_s,
        "stale": bus.stale,
        "parked": parked,
        "stopped_s": bus.stopped_s,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_bus_json, resolve_city, PublicRateLimiter, PARKED_AFTER_S, PUBLIC_RATE_LIMIT,
    };
    use crate::socket_bridge::Bus;
    use std::time::Instant;

    #[test]
    fn resolves_city_slugs_and_rejects_unknown() {
        assert_eq!(resolve_city("surabaya").unwrap().pref, "3501");
        assert_eq!(resolve_city("sby").unwrap().pref, "3501");
        assert_eq!(resolve_city("3501").unwrap().slug, "surabaya");
        assert_eq!(resolve_city("yogyakarta").unwrap().pref, "1");
        assert_eq!(resolve_city("jogja").unwrap().pref, "1");
        assert_eq!(resolve_city("yk").unwrap().pref, "1");
        assert!(resolve_city("semarang").is_none());
    }

    #[test]
    fn public_rate_limiter_enforces_limit_and_isolates_clients() {
        let limiter = PublicRateLimiter::new();
        let now = Instant::now();
        for i in 0..PUBLIC_RATE_LIMIT {
            let d = limiter.check("10.0.0.1", now);
            assert!(d.allowed);
            assert_eq!(d.remaining, PUBLIC_RATE_LIMIT - i - 1);
        }
        let blocked = limiter.check("10.0.0.1", now);
        assert!(!blocked.allowed);
        assert_eq!(blocked.remaining, 0);
        let other = limiter.check("10.0.0.2", now);
        assert!(other.allowed);
    }

    #[test]
    fn normalizes_bus_parked_and_keeps_upstream_corridor() {
        let bus = Bus {
            id: "b1".into(),
            pref: "1".into(),
            name: Some("Bus 01".into()),
            plate: Some("AB 1234 XY".into()),
            lat: -7.79,
            lng: 110.36,
            heading: Some(90.0),
            speed: Some(0.0),
            corridor: Some("1A".into()),
            toward: Some("Prambanan".into()),
            next_halte: None,
            prev_halte: None,
            passengers: Some(12),
            fix_age_s: 20,
            stale: false,
            stopped_s: PARKED_AFTER_S,
        };
        let out = normalize_bus_json(&bus, "1");
        assert_eq!(out["corridor"], "1A");
        assert_eq!(out["parked"], true);
        assert_eq!(out["speed_kmh"], 0.0);

        let sbt_bus = Bus {
            id: "b2".into(),
            pref: "3501".into(),
            corridor: Some("SBT".into()),
            ..bus
        };
        let sbt_out = normalize_bus_json(&sbt_bus, "3501");
        assert_eq!(sbt_out["corridor"], "R1");
    }
}
