mod admin;
mod crypto;
mod public_api;
mod socket_bridge;

use admin::{
    apply_corridor_overrides, apply_leg_stop_overrides, apply_stop_overrides, extract_secret,
    extract_username, upstream_kor_for, verify_secret, AdminStore, CityPopup, CustomBus,
    CustomCorridor, CustomStop, MenuLink,
};
use axum::{
    body::Bytes,
    extract::{ConnectInfo, DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use crypto::AesKey;
use serde::Deserialize;
use serde_json::Value;
use socketioxide::SocketIo;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

const TOKEN_TTL: Duration = Duration::from_secs(5 * 60);
const CACHE_FRESH_TTL: Duration = Duration::from_secs(15 * 60);
const CACHE_STALE_TTL: Duration = Duration::from_secs(60 * 60);
const MAX_CACHE_ENTRIES: usize = 512;
const ADMIN_MAX_FAILS: u32 = 8;
const ADMIN_RATE_WINDOW: Duration = Duration::from_secs(15 * 60);
const ADMIN_LIMITER_MAX_KEYS: usize = 1024;

#[derive(Clone)]
struct CachedEntry {
    body: Bytes,
    fetched_at: Instant,
}

struct AdminRateLimiter {
    entries: StdMutex<HashMap<String, (u32, Instant)>>,
}

impl AdminRateLimiter {
    fn new() -> Self {
        Self {
            entries: StdMutex::new(HashMap::new()),
        }
    }

    fn is_blocked(&self, key: &str, now: Instant) -> bool {
        let mut guard = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        guard.retain(|_, (_, first)| now.saturating_duration_since(*first) < ADMIN_RATE_WINDOW);
        matches!(guard.get(key), Some((count, _)) if *count >= ADMIN_MAX_FAILS)
    }

    fn record_failure(&self, key: &str, now: Instant) {
        let mut guard = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        guard.retain(|_, (_, first)| now.saturating_duration_since(*first) < ADMIN_RATE_WINDOW);
        if !guard.contains_key(key) && guard.len() >= ADMIN_LIMITER_MAX_KEYS {
            if let Some(oldest) = guard
                .iter()
                .min_by_key(|(_, (_, ts))| *ts)
                .map(|(k, _)| k.clone())
            {
                guard.remove(&oldest);
            }
        }
        guard
            .entry(key.to_string())
            .and_modify(|(count, _)| *count = count.saturating_add(1))
            .or_insert((1, now));
    }

    fn clear(&self, key: &str) {
        let mut guard = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        guard.remove(key);
    }
}

pub(crate) struct AppState {
    http: reqwest::Client,
    gps_api_base: String,
    socket_api_base: String,
    gps_key: AesKey,
    default_pref: String,
    env_path: Option<PathBuf>,
    admin_username: Option<String>,
    admin_secret: Option<String>,
    pub(crate) admin_store: AdminStore,
    pub(crate) live: socket_bridge::LiveCache,
    pub(crate) upstream_health: socket_bridge::UpstreamHealth,
    io: SocketIo,
    token_cache: RwLock<Option<(String, Instant)>>,
    token_refresh_lock: Mutex<()>,
    response_cache: RwLock<HashMap<String, CachedEntry>>,
    in_flight: Mutex<HashSet<String>>,
    admin_limiter: AdminRateLimiter,
    pub(crate) public_limiter: public_api::PublicRateLimiter,
    pub(crate) trust_proxy: bool,
}

fn is_fresh(age: Duration) -> bool {
    age < CACHE_FRESH_TTL
}

fn is_stale(age: Duration) -> bool {
    age < CACHE_STALE_TTL
}

fn json_bytes_response(body: Bytes, cache_control: &'static str) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("application/json")),
            (header::CACHE_CONTROL, HeaderValue::from_static(cache_control)),
        ],
        body,
    )
        .into_response()
}

pub(crate) fn no_store_json(status: StatusCode, body: Value) -> Response {
    let bytes = Bytes::from(serde_json::to_vec(&body).unwrap_or_default());
    (
        status,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("application/json")),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
        ],
        bytes,
    )
        .into_response()
}

fn cache_insert(cache: &mut HashMap<String, CachedEntry>, key: String, entry: CachedEntry) {
    if cache.len() >= MAX_CACHE_ENTRIES && !cache.contains_key(&key) {
        cache.retain(|_, v| v.fetched_at.elapsed() < CACHE_STALE_TTL);
    }
    while cache.len() >= MAX_CACHE_ENTRIES && !cache.contains_key(&key) {
        let evict_key = cache
            .iter()
            .filter(|(k, _)| k.starts_with("route-corridor:"))
            .min_by_key(|(_, v)| v.fetched_at)
            .map(|(k, _)| k.clone())
            .or_else(|| {
                cache
                    .iter()
                    .min_by_key(|(_, v)| v.fetched_at)
                    .map(|(k, _)| k.clone())
            });
        match evict_key {
            Some(k) => {
                cache.remove(&k);
            }
            None => break,
        }
    }
    cache.insert(key, entry);
}

async fn warm_entry<F, Fut>(state: &AppState, key: String, fetcher: F)
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = anyhow::Result<Value>>,
{
    if let Ok(v) = fetcher().await {
        if v.get("data").is_some() {
            if let Ok(vec) = serde_json::to_vec(&v) {
                let mut guard = state.response_cache.write().await;
                cache_insert(
                    &mut guard,
                    key,
                    CachedEntry {
                        body: Bytes::from(vec),
                        fetched_at: Instant::now(),
                    },
                );
            }
        }
    }
}

fn parse_cors_origins(raw: &str) -> Vec<HeaderValue> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<HeaderValue>().ok())
        .collect()
}

fn build_cors_layer(raw: &str) -> CorsLayer {
    let trimmed = raw.trim();
    let layer = if trimmed == "*" {
        CorsLayer::new().allow_origin(Any)
    } else {
        let origins = parse_cors_origins(trimmed);
        if origins.is_empty() {
            let fallback = HeaderValue::from_static("http://localhost:5173");
            CorsLayer::new().allow_origin(fallback)
        } else {
            CorsLayer::new().allow_origin(AllowOrigin::list(origins))
        }
    };
    layer.allow_methods(Any).allow_headers(Any)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        } else {
            std::future::pending::<()>().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let env_path = dotenvy::dotenv().ok().or_else(|| find_env_file(None));
    tracing_subscriber::fmt::init();

    let http = reqwest::Client::builder()
        .tcp_nodelay(true)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .pool_max_idle_per_host(8)
        .build()?;

    let admin_username = std::env::var("ADMIN_USERNAME")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let admin_secret = std::env::var("ADMIN_SECRET")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let admin_data_path = std::env::var("ADMIN_DATA_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("admin_overrides.json"));
    let admin_store = AdminStore::load(admin_data_path).await;

    let (socket_layer, io) = SocketIo::new_layer();
    let live = socket_bridge::LiveCache::default();
    let upstream_health = socket_bridge::UpstreamHealth::new();
    socket_bridge::register_namespace(&io, live.clone());
    socket_bridge::spawn_upstream(
        std::env::var("SOCKET_API_BASE")?,
        io.clone(),
        live.clone(),
        upstream_health.clone(),
    );

    let state = Arc::new(AppState {
        http,
        gps_api_base: std::env::var("GPS_API_BASE")?,
        socket_api_base: std::env::var("SOCKET_API_BASE")?,
        gps_key: AesKey::from_secret(&std::env::var("GPS_KEY")?)?,
        default_pref: std::env::var("DEFAULT_PREF").unwrap_or_else(|_| "3501".into()),
        env_path,
        admin_username,
        admin_secret,
        admin_store,
        live,
        upstream_health,
        io,
        token_cache: RwLock::new(None),
        token_refresh_lock: Mutex::new(()),
        response_cache: RwLock::new(HashMap::new()),
        in_flight: Mutex::new(HashSet::new()),
        admin_limiter: AdminRateLimiter::new(),
        public_limiter: public_api::PublicRateLimiter::new(),
        trust_proxy: std::env::var("TRUST_PROXY")
            .ok()
            .is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes")),
    });

    let allow_origin = std::env::var("ALLOW_ORIGIN").unwrap_or_else(|_| "http://localhost:5173".into());
    let cors = build_cors_layer(&allow_origin);
    let public_cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::OPTIONS])
        .allow_headers(Any);

    let fake_bus_state = state.clone();
    tokio::spawn(async move {
        loop {
            sync_fake_buses(&fake_bus_state).await;
            tokio::time::sleep(Duration::from_secs(20)).await;
        }
    });

    let warm_state = state.clone();
    tokio::spawn(async move {
        let _ = get_gps_token(&warm_state).await;
        tokio::join!(
            warm_entry(&warm_state, "corridors:3501".into(), || fetch_corridors_merged(&warm_state, "3501")),
            warm_entry(&warm_state, "routes:3501".into(), || fetch_routes_merged(&warm_state, "3501")),
            warm_entry(&warm_state, "corridors:1".into(), || fetch_corridors_merged(&warm_state, "1")),
            warm_entry(&warm_state, "routes:1".into(), || fetch_routes_merged(&warm_state, "1")),
        );
        tracing::info!("cache prewarmed");
    });

    let public_app = public_api::router().layer(public_cors);

    let internal_app = Router::new()
        .route("/api/health", get(health))
        .route("/api/status", get(get_status))
        .route("/api/trans", get(get_trans))
        .route("/api/corridors", get(get_corridors))
        .route("/api/routes", get(get_routes))
        .route("/api/route-corridor", get(get_route_corridor))
        .route("/api/site-config", get(get_site_config))
        .route(
            "/api/admin/auth",
            post(admin_auth).layer(DefaultBodyLimit::max(64 * 1024)),
        )
        .route("/api/admin/state", get(admin_state))
        .route("/api/admin/menu-links", post(admin_set_menu_links))
        .route(
            "/api/admin/popup",
            post(admin_set_popup).layer(DefaultBodyLimit::max(4 * 1024 * 1024)),
        )
        .route("/api/admin/corridors", post(admin_upsert_corridor))
        .route("/api/admin/corridors/order", post(admin_set_corridor_order))
        .route("/api/admin/corridors/:pref/:code", delete(admin_remove_corridor))
        .route("/api/admin/corridors/:pref/:code/restore", post(admin_restore_corridor))
        .route("/api/admin/stops", post(admin_upsert_stop))
        .route("/api/admin/stops/:pref/:id", delete(admin_remove_stop))
        .route("/api/admin/stops/:pref/:id/restore", post(admin_restore_stop))
        .route("/api/admin/buses", post(admin_upsert_bus))
        .route("/api/admin/buses/:pref/:id", delete(admin_remove_bus))
        .route("/api/admin/reset/:pref", post(admin_reset_city))
        .layer(cors);

    let app = Router::new()
        .merge(public_app)
        .merge(internal_app)
        .layer(CompressionLayer::new())
        .layer(socket_layer)
        .with_state(state);

    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
    tracing::info!("listening on {bind_addr}");
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

fn check_internal_rate(state: &AppState, headers: &HeaderMap, addr: SocketAddr) -> Option<Response> {
    let key = client_ip_key(headers, Some(addr.ip()), state.trust_proxy);
    let rate = state.public_limiter.check(&key, Instant::now());
    if !rate.allowed {
        return Some(public_api::rate_limited_response(rate));
    }
    None
}

async fn get_status(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    if let Some(resp) = check_internal_rate(&state, &headers, addr) {
        return resp;
    }
    let now = socket_bridge::now_unix();
    let buses = state.live.counts(now);
    let upstream_connected = state.upstream_health.is_connected();
    let last_event_age_s = state.upstream_health.last_event_age_s(now);
    let uptime_s = state.upstream_health.uptime_s();
    no_store_json(
        StatusCode::OK,
        serde_json::json!({
            "ok": true,
            "upstream_connected": upstream_connected,
            "last_event_age_s": last_event_age_s,
            "buses": buses,
            "uptime_s": uptime_s,
        }),
    )
}

async fn invalidate_gps_token(state: &AppState) {
    let mut guard = state.token_cache.write().await;
    *guard = None;
}

async fn get_gps_token(state: &AppState) -> anyhow::Result<String> {
    {
        let guard = state.token_cache.read().await;
        if let Some((tok, fetched_at)) = guard.as_ref() {
            if fetched_at.elapsed() < TOKEN_TTL {
                return Ok(tok.clone());
            }
        }
    }
    let _refresh_guard = state.token_refresh_lock.lock().await;
    {
        let guard = state.token_cache.read().await;
        if let Some((tok, fetched_at)) = guard.as_ref() {
            if fetched_at.elapsed() < TOKEN_TTL {
                return Ok(tok.clone());
            }
        }
    }
    let resp = state
        .http
        .get(format!("{}getToken", state.gps_api_base))
        .send()
        .await?;
    if !resp.status().is_success() {
        anyhow::bail!("getToken status {}", resp.status());
    }
    let body = resp.text().await?;
    let plaintext = state.gps_key.decrypt_from_b64(body.trim())?;
    let parsed: Value = serde_json::from_str(&plaintext)?;
    let token = parsed["data"]["token"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("no token in response"))?;
    let mut guard = state.token_cache.write().await;
    *guard = Some((token.clone(), Instant::now()));
    Ok(token)
}

#[allow(dead_code)]
async fn get_socket_auth_token(state: &AppState) -> anyhow::Result<String> {
    let plaintext = serde_json::json!({ "email": "user@mail.com", "id": "0" }).to_string();
    let encrypted = state.gps_key.encrypt_to_b64(&plaintext);
    let body = serde_json::json!({ "value": encrypted });

    let resp = state
        .http
        .post(format!("{}api/authToken", state.socket_api_base))
        .json(&body)
        .send()
        .await?
        .text()
        .await?;

    let decrypted = state.gps_key.decrypt_from_b64(resp.trim())?;
    let parsed: Value = serde_json::from_str(&decrypted)?;
    parsed["data"]["token"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("no token in authToken response"))
}

async fn cached_json_bytes<F, Fut>(
    state: Arc<AppState>,
    key: String,
    fetcher: F,
) -> Result<Bytes, StatusCode>
where
    F: FnOnce(Arc<AppState>) -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<Value>> + Send,
{
    let cached = {
        let guard = state.response_cache.read().await;
        guard.get(&key).cloned()
    };
    if let Some(ref entry) = cached {
        let age = entry.fetched_at.elapsed();
        if is_fresh(age) {
            return Ok(entry.body.clone());
        }
        if is_stale(age) {
            let should_refresh = {
                let mut inflight = state.in_flight.lock().await;
                inflight.insert(key.clone())
            };
            if should_refresh {
                let bg_state = state.clone();
                let bg_key = key.clone();
                tokio::spawn(async move {
                    if let Ok(v) = fetcher(bg_state.clone()).await {
                        if v.get("data").is_some() {
                            if let Ok(vec) = serde_json::to_vec(&v) {
                                let mut guard = bg_state.response_cache.write().await;
                                cache_insert(
                                    &mut guard,
                                    bg_key.clone(),
                                    CachedEntry {
                                        body: Bytes::from(vec),
                                        fetched_at: Instant::now(),
                                    },
                                );
                            }
                        }
                    }
                    let mut inflight = bg_state.in_flight.lock().await;
                    inflight.remove(&bg_key);
                });
            }
            return Ok(entry.body.clone());
        }
    }

    match fetcher(state.clone()).await {
        Ok(v) => {
            let bytes = Bytes::from(serde_json::to_vec(&v).unwrap_or_default());
            if v.get("data").is_some() {
                let mut guard = state.response_cache.write().await;
                cache_insert(
                    &mut guard,
                    key,
                    CachedEntry {
                        body: bytes.clone(),
                        fetched_at: Instant::now(),
                    },
                );
            }
            Ok(bytes)
        }
        Err(e) => {
            if let Some(entry) = cached {
                tracing::warn!(%key, "upstream fetch failed, serving stale cache: {e:#}");
                return Ok(entry.body);
            }
            tracing::warn!(%key, "upstream fetch failed: {e:#}");
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}

pub(crate) async fn cached_json_value<F, Fut>(
    state: Arc<AppState>,
    key: String,
    fetcher: F,
) -> Result<Value, StatusCode>
where
    F: FnOnce(Arc<AppState>) -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<Value>> + Send,
{
    let bytes = cached_json_bytes(state, key, fetcher).await?;
    serde_json::from_slice::<Value>(&bytes).map_err(|_| StatusCode::BAD_GATEWAY)
}

async fn serve_cached<F, Fut>(state: Arc<AppState>, key: String, fetcher: F) -> Response
where
    F: FnOnce(Arc<AppState>) -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<Value>> + Send,
{
    match cached_json_bytes(state, key, fetcher).await {
        Ok(bytes) => json_bytes_response(bytes, "no-store"),
        Err(status) => no_store_json(status, serde_json::json!({ "error": "upstream unavailable" })),
    }
}

async fn get_trans(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    if let Some(resp) = check_internal_rate(&state, &headers, addr) {
        return resp;
    }
    serve_cached(state, "trans".into(), |st| async move {
        fetch_trans(&st).await
    })
    .await
}

async fn fetch_trans(state: &AppState) -> anyhow::Result<Value> {
    let token = get_gps_token(state).await?;
    let resp = state
        .http
        .get(format!("{}getTrans", state.gps_api_base))
        .bearer_auth(token)
        .send()
        .await?;
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        invalidate_gps_token(state).await;
        anyhow::bail!("upstream auth status {status}");
    }
    if !status.is_success() {
        anyhow::bail!("upstream status {status}");
    }
    Ok(resp.json::<Value>().await?)
}

#[derive(Deserialize)]
struct PrefQuery {
    pref: Option<String>,
}

fn resolve_pref<'a>(requested: Option<&'a str>, default: &'a str) -> &'a str {
    match requested.map(str::trim) {
        Some(p) if !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric()) => p,
        _ => default,
    }
}

async fn fetch_raw_cached(state: &AppState, endpoint: &str, pref: &str) -> anyhow::Result<Value> {
    let raw_key = format!("raw:{endpoint}:{pref}");
    let cached = {
        let guard = state.response_cache.read().await;
        guard.get(&raw_key).cloned()
    };
    if let Some(entry) = &cached {
        if is_fresh(entry.fetched_at.elapsed()) {
            if let Ok(parsed) = serde_json::from_slice::<Value>(&entry.body) {
                return Ok(parsed);
            }
        }
    }
    match fetch_with_pref(state, endpoint, pref).await {
        Ok(val) => {
            if val.get("data").is_some() {
                if let Ok(vec) = serde_json::to_vec(&val) {
                    let mut guard = state.response_cache.write().await;
                    cache_insert(
                        &mut guard,
                        raw_key,
                        CachedEntry {
                            body: Bytes::from(vec),
                            fetched_at: Instant::now(),
                        },
                    );
                }
            }
            Ok(val)
        }
        Err(e) => {
            if let Some(entry) = cached {
                if let Ok(parsed) = serde_json::from_slice::<Value>(&entry.body) {
                    tracing::warn!(%raw_key, "raw upstream fetch failed, serving stale cache: {e:#}");
                    return Ok(parsed);
                }
            }
            Err(e)
        }
    }
}

async fn fetch_corridors_merged(state: &AppState, pref: &str) -> anyhow::Result<Value> {
    let mut val = fetch_raw_cached(state, "getCorridor", pref).await?;
    let data = state.admin_store.snapshot().await;
    apply_corridor_overrides(&mut val, &data, pref);
    Ok(val)
}

async fn fetch_routes_merged(state: &AppState, pref: &str) -> anyhow::Result<Value> {
    let mut val = fetch_raw_cached(state, "getRoutes", pref).await?;
    let data = state.admin_store.snapshot().await;
    apply_stop_overrides(&mut val, &data, pref);
    Ok(val)
}

async fn invalidate_city_cache(state: &AppState, pref: &str) {
    let mut guard = state.response_cache.write().await;
    guard.remove(&format!("corridors:{pref}"));
    guard.remove(&format!("routes:{pref}"));
    let prefix = format!("route-corridor:{pref}:");
    guard.retain(|k, _| !k.starts_with(&prefix));
}

async fn get_corridors(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(q): Query<PrefQuery>,
) -> Response {
    if let Some(resp) = check_internal_rate(&state, &headers, addr) {
        return resp;
    }
    let pref = resolve_pref(q.pref.as_deref(), &state.default_pref).to_string();
    let key = format!("corridors:{pref}");
    serve_cached(state, key, move |st| async move {
        fetch_corridors_merged(&st, &pref).await
    })
    .await
}

async fn get_routes(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(q): Query<PrefQuery>,
) -> Response {
    if let Some(resp) = check_internal_rate(&state, &headers, addr) {
        return resp;
    }
    let pref = resolve_pref(q.pref.as_deref(), &state.default_pref).to_string();
    let key = format!("routes:{pref}");
    serve_cached(state, key, move |st| async move {
        fetch_routes_merged(&st, &pref).await
    })
    .await
}

#[derive(Deserialize)]
pub(crate) struct RouteCorridorQuery {
    pub(crate) pref: Option<String>,
    pub(crate) kor: String,
    pub(crate) origin: String,
    pub(crate) toward: String,
}

async fn get_route_corridor(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(q): Query<RouteCorridorQuery>,
) -> Response {
    if let Some(resp) = check_internal_rate(&state, &headers, addr) {
        return resp;
    }
    let kor = q.kor.trim();
    if kor.is_empty() || kor.len() > 32 || q.origin.len() > 120 || q.toward.len() > 120 {
        return no_store_json(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": "invalid corridor query" }),
        );
    }
    let pref = resolve_pref(q.pref.as_deref(), &state.default_pref).to_string();
    let key = format!("route-corridor:{pref}:{}:{}:{}", kor, q.origin, q.toward);
    serve_cached(state, key, move |st| async move {
        fetch_route_corridor(&st, &q, &pref).await
    })
    .await
}

fn encode_form_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'*' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

async fn fetch_route_corridor_upstream(
    state: &AppState,
    pref: &str,
    kor: &str,
    origin: &str,
    toward: &str,
) -> anyhow::Result<Value> {
    let token = get_gps_token(state).await?;
    let form = format!(
        "pref={}&kor={}&toward={}&origin={}",
        encode_form_component(pref),
        encode_form_component(kor),
        encode_form_component(toward),
        encode_form_component(origin),
    );
    let body = state.gps_key.encrypt_to_b64(&form);
    let resp = state
        .http
        .post(format!("{}getRouteCorridor", state.gps_api_base))
        .bearer_auth(token)
        .header("Content-Type", "text/plain")
        .body(body)
        .send()
        .await?;
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        invalidate_gps_token(state).await;
        anyhow::bail!("upstream auth status {status}");
    }
    if !status.is_success() {
        anyhow::bail!("upstream status {status}");
    }
    Ok(resp.json::<Value>().await?)
}

async fn fetch_route_corridor(
    state: &AppState,
    q: &RouteCorridorQuery,
    pref: &str,
) -> anyhow::Result<Value> {
    let data = state.admin_store.snapshot().await;
    let upstream_kor = upstream_kor_for(&data, pref, &q.kor);
    let mut parsed =
        fetch_route_corridor_upstream(state, pref, &upstream_kor, &q.origin, &q.toward).await?;
    apply_leg_stop_overrides(&mut parsed, &data, pref, &q.kor, &q.toward);
    Ok(parsed)
}

pub(crate) async fn fetch_route_corridor_raw(
    state: &AppState,
    q: &RouteCorridorQuery,
    pref: &str,
) -> anyhow::Result<Value> {
    fetch_route_corridor_upstream(state, pref, &q.kor, &q.origin, &q.toward).await
}

pub(crate) async fn fetch_with_pref(state: &AppState, endpoint: &str, pref: &str) -> anyhow::Result<Value> {
    let token = get_gps_token(state).await?;
    let body = state
        .gps_key
        .encrypt_to_b64(&format!("pref={}", encode_form_component(pref)));
    let resp = state
        .http
        .post(format!("{}{}", state.gps_api_base, endpoint))
        .bearer_auth(token)
        .header("Content-Type", "text/plain")
        .body(body)
        .send()
        .await?;
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        invalidate_gps_token(state).await;
        anyhow::bail!("upstream auth status {status}");
    }
    if !status.is_success() {
        anyhow::bail!("upstream status {status}");
    }
    Ok(resp.json::<Value>().await?)
}

fn find_env_file(initial: Option<&FsPath>) -> Option<PathBuf> {
    if let Some(p) = initial {
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        for dir in cwd.ancestors() {
            let candidate = dir.join(".env");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            for dir in parent.ancestors() {
                let candidate = dir.join(".env");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn parse_env_file(path: &FsPath) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return map;
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let body = trimmed
            .strip_prefix("export ")
            .map(str::trim_start)
            .unwrap_or(trimmed);
        let Some((raw_key, raw_val)) = body.split_once('=') else {
            continue;
        };
        let key = raw_key.trim();
        if key.is_empty() {
            continue;
        }
        let val = raw_val.trim();
        let unquoted = if (val.starts_with('"') && val.ends_with('"') && val.len() >= 2)
            || (val.starts_with('\'') && val.ends_with('\'') && val.len() >= 2)
        {
            &val[1..val.len() - 1]
        } else {
            val
        };
        map.insert(key.to_string(), unquoted.to_string());
    }
    map
}

fn push_admin_account(accounts: &mut Vec<(String, String)>, user: &str, secret: &str) {
    let u = user.trim();
    let s = secret.trim();
    if u.is_empty() || s.is_empty() {
        return;
    }
    if !accounts.iter().any(|(eu, es)| eu == u && es == s) {
        accounts.push((u.to_string(), s.to_string()));
    }
}

fn parse_admin_accounts_from_map(vars: &HashMap<String, String>) -> Vec<(String, String)> {
    let mut accounts = Vec::new();
    if let Some(secret) = vars
        .get("ADMIN_SECRET")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let user = vars
            .get("ADMIN_USERNAME")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("admin");
        push_admin_account(&mut accounts, user, secret);
    }
    if let Some(raw_list) = vars.get("ADMIN_ACCOUNTS") {
        for entry in raw_list.split([',', ';']) {
            if let Some((u, s)) = entry.trim().split_once(':') {
                push_admin_account(&mut accounts, u, s);
            }
        }
    }
    let mut suffixed_keys: Vec<(&String, &String)> = vars
        .iter()
        .filter(|(k, _)| k.starts_with("ADMIN_SECRET_"))
        .collect();
    suffixed_keys.sort_by(|a, b| a.0.cmp(b.0));
    for (k, secret_val) in suffixed_keys {
        let Some(suffix) = k.strip_prefix("ADMIN_SECRET_") else {
            continue;
        };
        let secret = secret_val.trim();
        if secret.is_empty() {
            continue;
        }
        let user_key = format!("ADMIN_USERNAME_{suffix}");
        if let Some(user) = vars
            .get(&user_key)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            push_admin_account(&mut accounts, user, secret);
        }
    }
    accounts
}

fn current_admin_accounts(state: &AppState) -> Vec<(String, String)> {
    let mut vars: HashMap<String, String> = std::env::vars()
        .filter(|(k, _)| k.starts_with("ADMIN_"))
        .collect();
    if let Some(u) = &state.admin_username {
        vars.entry("ADMIN_USERNAME".to_string())
            .or_insert_with(|| u.clone());
    }
    if let Some(s) = &state.admin_secret {
        vars.entry("ADMIN_SECRET".to_string())
            .or_insert_with(|| s.clone());
    }
    if let Some(path) = find_env_file(state.env_path.as_deref()) {
        for (k, v) in parse_env_file(&path) {
            if k.starts_with("ADMIN_") {
                vars.insert(k, v);
            }
        }
    }
    parse_admin_accounts_from_map(&vars)
}

fn check_admin_accounts(
    accounts: &[(String, String)],
    provided_user: &str,
    provided_secret: &str,
) -> bool {
    let mut matched = false;
    for (expected_user, expected_secret) in accounts {
        let user_ok = verify_secret(&Some(expected_user.clone()), provided_user);
        let secret_ok = verify_secret(&Some(expected_secret.clone()), provided_secret);
        matched |= user_ok && secret_ok;
    }
    matched
}

pub(crate) fn client_ip_key(
    headers: &HeaderMap,
    peer: Option<IpAddr>,
    trust_proxy: bool,
) -> String {
    if trust_proxy {
        if let Some(cf) = headers
            .get("cf-connecting-ip")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return cf.to_string();
        }
        if let Some(xri) = headers
            .get("x-real-ip")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return xri.to_string();
        }
        if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            if let Some(last) = xff.rsplit(',').next().map(str::trim).filter(|s| !s.is_empty()) {
                return last.to_string();
            }
        }
    }
    peer.map(|ip| ip.to_string())
        .unwrap_or_else(|| "unknown".into())
}

fn require_admin(state: &AppState, headers: &HeaderMap, peer: Option<IpAddr>) -> Option<Response> {
    let client_key = client_ip_key(headers, peer, state.trust_proxy);
    let now = Instant::now();
    if state.admin_limiter.is_blocked(&client_key, now) {
        return Some(no_store_json(
            StatusCode::TOO_MANY_REQUESTS,
            serde_json::json!({ "error": "terlalu banyak percobaan gagal, coba lagi nanti" }),
        ));
    }
    let accounts = current_admin_accounts(state);
    if accounts.is_empty() {
        return Some(no_store_json(
            StatusCode::SERVICE_UNAVAILABLE,
            serde_json::json!({ "error": "admin secret not configured" }),
        ));
    }
    let Some(provided_user) = extract_username(headers) else {
        state.admin_limiter.record_failure(&client_key, now);
        return Some(no_store_json(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({ "error": "unauthorized" }),
        ));
    };
    let Some(provided_secret) = extract_secret(headers) else {
        state.admin_limiter.record_failure(&client_key, now);
        return Some(no_store_json(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({ "error": "unauthorized" }),
        ));
    };
    if !check_admin_accounts(&accounts, provided_user, provided_secret) {
        state.admin_limiter.record_failure(&client_key, now);
        return Some(no_store_json(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({ "error": "unauthorized" }),
        ));
    }
    state.admin_limiter.clear(&client_key);
    None
}

#[derive(Deserialize)]
struct AuthPayload {
    username: String,
    secret: String,
}

async fn admin_auth(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<AuthPayload>,
) -> Response {
    let client_key = client_ip_key(&headers, Some(addr.ip()), state.trust_proxy);
    let now = Instant::now();
    if state.admin_limiter.is_blocked(&client_key, now) {
        return no_store_json(
            StatusCode::TOO_MANY_REQUESTS,
            serde_json::json!({ "error": "terlalu banyak percobaan gagal, coba lagi nanti" }),
        );
    }
    let accounts = current_admin_accounts(&state);
    if accounts.is_empty() {
        return no_store_json(
            StatusCode::SERVICE_UNAVAILABLE,
            serde_json::json!({ "error": "admin secret not configured" }),
        );
    }
    if check_admin_accounts(&accounts, payload.username.trim(), payload.secret.trim()) {
        state.admin_limiter.clear(&client_key);
        no_store_json(StatusCode::OK, serde_json::json!({ "ok": true }))
    } else {
        state.admin_limiter.record_failure(&client_key, now);
        no_store_json(
            StatusCode::UNAUTHORIZED,
            serde_json::json!({ "error": "username atau password salah" }),
        )
    }
}

async fn admin_state(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(q): Query<PrefQuery>,
) -> Response {
    if let Some(resp) = require_admin(&state, &headers, Some(addr.ip())) {
        return resp;
    }
    let pref = resolve_pref(q.pref.as_deref(), &state.default_pref).to_string();
    let (raw_corridors, raw_stops, overrides, menu_links) = tokio::join!(
        fetch_raw_cached(&state, "getCorridor", &pref),
        fetch_raw_cached(&state, "getRoutes", &pref),
        state.admin_store.city_overrides(&pref),
        state.admin_store.menu_links(),
    );
    let upstream_corridors = raw_corridors
        .ok()
        .and_then(|v| v.get("data").cloned())
        .unwrap_or_else(|| Value::Array(vec![]));
    let upstream_stops = raw_stops
        .ok()
        .and_then(|v| v.get("data").cloned())
        .unwrap_or_else(|| Value::Array(vec![]));
    no_store_json(
        StatusCode::OK,
        serde_json::json!({
            "ok": true,
            "pref": pref,
            "upstream_corridors": upstream_corridors,
            "upstream_stops": upstream_stops,
            "overrides": overrides,
            "menu_links": menu_links,
        }),
    )
}

async fn get_site_config(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(q): Query<PrefQuery>,
) -> Response {
    if let Some(resp) = check_internal_rate(&state, &headers, addr) {
        return resp;
    }
    let pref = resolve_pref(q.pref.as_deref(), &state.default_pref).to_string();
    let (menu_links, welcome_popup) = tokio::join!(
        state.admin_store.menu_links(),
        state.admin_store.city_popup(&pref),
    );
    no_store_json(
        StatusCode::OK,
        serde_json::json!({
            "ok": true,
            "pref": pref,
            "menu_links": menu_links,
            "welcome_popup": welcome_popup,
        }),
    )
}

#[derive(Deserialize)]
struct MenuLinksPayload {
    links: Vec<MenuLink>,
}

async fn admin_set_menu_links(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<MenuLinksPayload>,
) -> Response {
    if let Some(resp) = require_admin(&state, &headers, Some(addr.ip())) {
        return resp;
    }
    match state.admin_store.set_menu_links(payload.links).await {
        Ok(menu_links) => no_store_json(
            StatusCode::OK,
            serde_json::json!({ "ok": true, "menu_links": menu_links }),
        ),
        Err(e) => no_store_json(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

#[derive(Deserialize)]
struct CityPopupPayload {
    pref: Option<String>,
    #[serde(flatten)]
    popup: CityPopup,
}

async fn admin_set_popup(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<CityPopupPayload>,
) -> Response {
    if let Some(resp) = require_admin(&state, &headers, Some(addr.ip())) {
        return resp;
    }
    let pref = resolve_pref(payload.pref.as_deref(), &state.default_pref).to_string();
    match state.admin_store.set_city_popup(&pref, payload.popup).await {
        Ok(welcome_popup) => no_store_json(
            StatusCode::OK,
            serde_json::json!({ "ok": true, "pref": pref, "welcome_popup": welcome_popup }),
        ),
        Err(e) => no_store_json(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

#[derive(Deserialize)]
struct RemoveQuery {
    upstream: Option<bool>,
}

async fn admin_upsert_corridor(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(corridor): Json<CustomCorridor>,
) -> Response {
    if let Some(resp) = require_admin(&state, &headers, Some(addr.ip())) {
        return resp;
    }
    let pref = resolve_pref(Some(&corridor.pref), &state.default_pref).to_string();
    match state.admin_store.upsert_corridor(&pref, corridor).await {
        Ok(overrides) => {
            invalidate_city_cache(&state, &pref).await;
            no_store_json(
                StatusCode::OK,
                serde_json::json!({ "ok": true, "overrides": overrides }),
            )
        }
        Err(e) => no_store_json(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

#[derive(Deserialize)]
struct OrderPayload {
    pref: Option<String>,
    order: Vec<String>,
}

async fn admin_set_corridor_order(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<OrderPayload>,
) -> Response {
    if let Some(resp) = require_admin(&state, &headers, Some(addr.ip())) {
        return resp;
    }
    let pref = resolve_pref(payload.pref.as_deref(), &state.default_pref).to_string();
    match state.admin_store.set_corridor_order(&pref, payload.order).await {
        Ok(overrides) => {
            invalidate_city_cache(&state, &pref).await;
            no_store_json(
                StatusCode::OK,
                serde_json::json!({ "ok": true, "overrides": overrides }),
            )
        }
        Err(e) => no_store_json(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

async fn admin_remove_corridor(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path((pref_raw, code)): Path<(String, String)>,
    Query(q): Query<RemoveQuery>,
) -> Response {
    if let Some(resp) = require_admin(&state, &headers, Some(addr.ip())) {
        return resp;
    }
    let pref = resolve_pref(Some(&pref_raw), &state.default_pref).to_string();
    match state
        .admin_store
        .remove_corridor(&pref, &code, q.upstream.unwrap_or(false))
        .await
    {
        Ok(overrides) => {
            invalidate_city_cache(&state, &pref).await;
            no_store_json(
                StatusCode::OK,
                serde_json::json!({ "ok": true, "overrides": overrides }),
            )
        }
        Err(e) => no_store_json(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

async fn admin_restore_corridor(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path((pref_raw, code)): Path<(String, String)>,
) -> Response {
    if let Some(resp) = require_admin(&state, &headers, Some(addr.ip())) {
        return resp;
    }
    let pref = resolve_pref(Some(&pref_raw), &state.default_pref).to_string();
    match state.admin_store.restore_corridor(&pref, &code).await {
        Ok(overrides) => {
            invalidate_city_cache(&state, &pref).await;
            no_store_json(
                StatusCode::OK,
                serde_json::json!({ "ok": true, "overrides": overrides }),
            )
        }
        Err(e) => no_store_json(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

#[derive(Deserialize)]
struct UpsertStopPayload {
    pref: Option<String>,
    #[serde(flatten)]
    stop: CustomStop,
}

async fn admin_upsert_stop(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<UpsertStopPayload>,
) -> Response {
    if let Some(resp) = require_admin(&state, &headers, Some(addr.ip())) {
        return resp;
    }
    let pref = resolve_pref(payload.pref.as_deref(), &state.default_pref).to_string();
    match state.admin_store.upsert_stop(&pref, payload.stop).await {
        Ok(overrides) => {
            invalidate_city_cache(&state, &pref).await;
            no_store_json(
                StatusCode::OK,
                serde_json::json!({ "ok": true, "overrides": overrides }),
            )
        }
        Err(e) => no_store_json(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

async fn admin_remove_stop(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path((pref_raw, id)): Path<(String, String)>,
    Query(q): Query<RemoveQuery>,
) -> Response {
    if let Some(resp) = require_admin(&state, &headers, Some(addr.ip())) {
        return resp;
    }
    let pref = resolve_pref(Some(&pref_raw), &state.default_pref).to_string();
    match state
        .admin_store
        .remove_stop(&pref, &id, q.upstream.unwrap_or(false))
        .await
    {
        Ok(overrides) => {
            invalidate_city_cache(&state, &pref).await;
            no_store_json(
                StatusCode::OK,
                serde_json::json!({ "ok": true, "overrides": overrides }),
            )
        }
        Err(e) => no_store_json(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

async fn admin_restore_stop(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path((pref_raw, id)): Path<(String, String)>,
) -> Response {
    if let Some(resp) = require_admin(&state, &headers, Some(addr.ip())) {
        return resp;
    }
    let pref = resolve_pref(Some(&pref_raw), &state.default_pref).to_string();
    match state.admin_store.restore_stop(&pref, &id).await {
        Ok(overrides) => {
            invalidate_city_cache(&state, &pref).await;
            no_store_json(
                StatusCode::OK,
                serde_json::json!({ "ok": true, "overrides": overrides }),
            )
        }
        Err(e) => no_store_json(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

fn to_live_bus(pref: &str, custom: &CustomBus) -> socket_bridge::Bus {
    socket_bridge::Bus {
        id: custom.id.clone(),
        pref: pref.to_string(),
        name: Some(custom.name.clone()),
        plate: custom.plate.clone(),
        lat: custom.lat,
        lng: custom.lng,
        heading: custom.heading,
        speed: custom.speed,
        corridor: Some(custom.corridor.clone()),
        toward: custom.toward.clone(),
        next_halte: None,
        prev_halte: None,
        passengers: custom.passengers,
        fix_age_s: 0,
        stale: false,
        stopped_s: 0,
    }
}

async fn sync_fake_buses(state: &AppState) {
    let data = state.admin_store.snapshot().await;
    let now = socket_bridge::now_unix();
    for (pref, city) in &data.cities {
        for custom in &city.added_buses {
            let live_bus = state.live.upsert_custom(to_live_bus(pref, custom), now);
            socket_bridge::broadcast_bus(&state.io, &live_bus);
        }
    }
}

#[derive(Deserialize)]
struct UpsertBusPayload {
    pref: Option<String>,
    #[serde(flatten)]
    bus: CustomBus,
}

async fn admin_upsert_bus(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<UpsertBusPayload>,
) -> Response {
    if let Some(resp) = require_admin(&state, &headers, Some(addr.ip())) {
        return resp;
    }
    let pref = resolve_pref(payload.pref.as_deref(), &state.default_pref).to_string();
    match state.admin_store.upsert_bus(&pref, payload.bus).await {
        Ok((saved, overrides)) => {
            let now = socket_bridge::now_unix();
            let live_bus = state.live.upsert_custom(to_live_bus(&pref, &saved), now);
            socket_bridge::broadcast_bus(&state.io, &live_bus);
            no_store_json(
                StatusCode::OK,
                serde_json::json!({ "ok": true, "overrides": overrides }),
            )
        }
        Err(e) => no_store_json(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

async fn admin_remove_bus(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path((pref_raw, id)): Path<(String, String)>,
) -> Response {
    if let Some(resp) = require_admin(&state, &headers, Some(addr.ip())) {
        return resp;
    }
    let pref = resolve_pref(Some(&pref_raw), &state.default_pref).to_string();
    match state.admin_store.remove_bus(&pref, &id).await {
        Ok(overrides) => {
            if let Some(removed) = state.live.remove_custom(&pref, id.trim()) {
                socket_bridge::broadcast_bus(&state.io, &removed);
            }
            no_store_json(
                StatusCode::OK,
                serde_json::json!({ "ok": true, "overrides": overrides }),
            )
        }
        Err(e) => no_store_json(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

#[derive(Deserialize)]
struct ResetQuery {
    scope: Option<String>,
}

async fn admin_reset_city(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(pref_raw): Path<String>,
    Query(q): Query<ResetQuery>,
) -> Response {
    if let Some(resp) = require_admin(&state, &headers, Some(addr.ip())) {
        return resp;
    }
    let pref = resolve_pref(Some(&pref_raw), &state.default_pref).to_string();
    let scope = q
        .scope
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("all");
    match state.admin_store.reset_city_scope(&pref, scope).await {
        Ok((overrides, removed_bus_ids)) => {
            if matches!(scope, "all" | "routes" | "stops") {
                invalidate_city_cache(&state, &pref).await;
            }
            for bus_id in removed_bus_ids {
                if let Some(removed) = state.live.remove_custom(&pref, &bus_id) {
                    socket_bridge::broadcast_bus(&state.io, &removed);
                }
            }
            no_store_json(
                StatusCode::OK,
                serde_json::json!({ "ok": true, "overrides": overrides }),
            )
        }
        Err(e) => no_store_json(
            StatusCode::BAD_REQUEST,
            serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cache_insert, client_ip_key, is_fresh, is_stale, parse_admin_accounts_from_map,
        parse_cors_origins, parse_env_file, resolve_pref, AdminRateLimiter, CachedEntry,
        ADMIN_MAX_FAILS, CACHE_FRESH_TTL, CACHE_STALE_TTL, MAX_CACHE_ENTRIES,
    };
    use axum::body::Bytes;
    use axum::http::{HeaderMap, HeaderValue};
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    #[test]
    fn resolves_valid_or_default_pref() {
        assert_eq!(resolve_pref(None, "3501"), "3501");
        assert_eq!(resolve_pref(Some("1"), "3501"), "1");
        assert_eq!(resolve_pref(Some(" 3501 "), "1"), "3501");
        assert_eq!(resolve_pref(Some(""), "3501"), "3501");
        assert_eq!(resolve_pref(Some("   "), "3501"), "3501");
        assert_eq!(resolve_pref(Some("1&x=2"), "3501"), "3501");
    }

    #[test]
    fn cache_freshness_windows() {
        assert!(is_fresh(Duration::from_secs(0)));
        assert!(is_fresh(CACHE_FRESH_TTL - Duration::from_secs(1)));
        assert!(!is_fresh(CACHE_FRESH_TTL));
        assert!(is_stale(CACHE_FRESH_TTL));
        assert!(is_stale(CACHE_STALE_TTL - Duration::from_secs(1)));
        assert!(!is_stale(CACHE_STALE_TTL));
    }

    #[test]
    fn parses_env_file_resiliently() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("suroboyo-env-test-{nanos}.env"));
        let content = "# comment\n\nMALFORMED_LINE\nexport ADMIN_USERNAME='alice'\nADMIN_SECRET=\"secret123\"";
        std::fs::write(&path, content).unwrap();
        let parsed = parse_env_file(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(parsed.get("ADMIN_USERNAME").map(String::as_str), Some("alice"));
        assert_eq!(parsed.get("ADMIN_SECRET").map(String::as_str), Some("secret123"));
        assert!(!parsed.contains_key("MALFORMED_LINE"));
    }

    #[test]
    fn parses_single_list_and_numbered_admin_accounts() {
        let mut vars = HashMap::new();
        vars.insert("ADMIN_USERNAME".into(), "alice".into());
        vars.insert("ADMIN_SECRET".into(), "secret123".into());
        vars.insert("ADMIN_ACCOUNTS".into(), "editor:pass:with:colons; viewer : view123".into());
        vars.insert("ADMIN_USERNAME_2".into(), "backup".into());
        vars.insert("ADMIN_SECRET_2".into(), "backupSecret".into());

        let accounts = parse_admin_accounts_from_map(&vars);
        assert_eq!(accounts.len(), 4);
        assert!(accounts.contains(&("alice".into(), "secret123".into())));
        assert!(accounts.contains(&("editor".into(), "pass:with:colons".into())));
        assert!(accounts.contains(&("viewer".into(), "view123".into())));
        assert!(accounts.contains(&("backup".into(), "backupSecret".into())));
    }

    #[test]
    fn rate_limiter_blocks_and_resets() {
        let limiter = AdminRateLimiter::new();
        let now = Instant::now();
        for _ in 0..ADMIN_MAX_FAILS {
            assert!(!limiter.is_blocked("1.2.3.4", now));
            limiter.record_failure("1.2.3.4", now);
        }
        assert!(limiter.is_blocked("1.2.3.4", now));
        assert!(!limiter.is_blocked("5.6.7.8", now));
        limiter.clear("1.2.3.4");
        assert!(!limiter.is_blocked("1.2.3.4", now));
    }

    #[test]
    fn cache_evicts_route_corridor_entries_first_at_limit() {
        let mut cache = HashMap::new();
        let now = Instant::now();
        for i in 0..MAX_CACHE_ENTRIES {
            let key = if i == 0 {
                "corridors:3501".to_string()
            } else {
                format!("route-corridor:3501:{i}")
            };
            cache_insert(
                &mut cache,
                key,
                CachedEntry {
                    body: Bytes::from_static(b"{}"),
                    fetched_at: now,
                },
            );
        }
        assert_eq!(cache.len(), MAX_CACHE_ENTRIES);
        cache_insert(
            &mut cache,
            "corridors:1".to_string(),
            CachedEntry {
                body: Bytes::from_static(b"{}"),
                fetched_at: now,
            },
        );
        assert_eq!(cache.len(), MAX_CACHE_ENTRIES);
        assert!(cache.contains_key("corridors:3501"));
        assert!(cache.contains_key("corridors:1"));
    }

    #[test]
    fn parses_multiple_cors_origins() {
        let parsed = parse_cors_origins("https://manabus.id, http://localhost:5173 , ");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], "https://manabus.id");
        assert_eq!(parsed[1], "http://localhost:5173");
    }

    #[test]
    fn client_ip_key_ignores_spoofed_headers_unless_trusted() {
        let peer = Some(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 9)));
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("1.2.3.4, 203.0.113.5"),
        );
        assert_eq!(client_ip_key(&headers, peer, false), "198.51.100.9");
        assert_eq!(client_ip_key(&headers, peer, true), "203.0.113.5");
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.8"));
        assert_eq!(client_ip_key(&headers, peer, true), "203.0.113.8");
        headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.12"));
        assert_eq!(client_ip_key(&headers, peer, true), "203.0.113.12");
    }
}
