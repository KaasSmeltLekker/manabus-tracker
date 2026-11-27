use rust_socketio::{
    asynchronous::{Client, ClientBuilder},
    Event, Payload, TransportType,
};
use serde::{Deserialize, Deserializer, Serialize};
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const OPERATORS: &[(&str, i64)] = &[
    ("0", 7 * 3600),
    ("1", 7 * 3600),
    ("3", 7 * 3600),
    ("4", 7 * 3600),
    ("5", 7 * 3600),
    ("6", 7 * 3600),
    ("7", 8 * 3600),
    ("9", 7 * 3600),
    ("10", 8 * 3600),
    ("11", 8 * 3600),
    ("12", 8 * 3600),
    ("13", 7 * 3600),
    ("15", 8 * 3600),
    ("70", 7 * 3600),
    ("3501", 7 * 3600),
    ("3502", 7 * 3600),
];

pub const STALE_AFTER_S: i64 = 5 * 60;
pub const DROP_AFTER_S: i64 = 60 * 60;
const UPSTREAM_SILENCE_S: i64 = 90;
const UPSTREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const UPSTREAM_DISCONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Bus {
    pub id: String,
    pub pref: String,
    pub name: Option<String>,
    pub plate: Option<String>,
    pub lat: f64,
    pub lng: f64,
    pub heading: Option<f64>,
    pub speed: Option<f64>,
    pub corridor: Option<String>,
    pub toward: Option<String>,
    pub next_halte: Option<String>,
    pub prev_halte: Option<String>,
    pub passengers: Option<u32>,
    pub fix_age_s: i64,
    pub stale: bool,
    pub stopped_s: i64,
}

#[derive(Deserialize, Debug)]
struct RawBus {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    plate_number: Option<String>,
    #[serde(default, deserialize_with = "de_num")]
    lat: Option<f64>,
    #[serde(default, deserialize_with = "de_num")]
    lng: Option<f64>,
    #[serde(default, deserialize_with = "de_num")]
    angle: Option<f64>,
    #[serde(default, deserialize_with = "de_num")]
    speed: Option<f64>,
    #[serde(default)]
    kor: Option<String>,
    #[serde(default)]
    toward: Option<String>,
    #[serde(default)]
    dt_server: Option<String>,
    #[serde(default, deserialize_with = "de_num")]
    ago: Option<f64>,
    #[serde(default, deserialize_with = "de_num")]
    passenger: Option<f64>,
    #[serde(default)]
    new_shel_t: Option<String>,
    #[serde(default)]
    old_shel_t: Option<String>,
}

fn de_num<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(match Option::<serde_json::Value>::deserialize(d)? {
        Some(serde_json::Value::Number(n)) => n.as_f64(),
        Some(serde_json::Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    })
}

struct Entry {
    bus: Bus,
    fix_unix: i64,
    stopped_since: Option<i64>,
    dedupe_key: String,
}

#[derive(Clone)]
pub struct UpstreamHealth {
    connected: Arc<AtomicBool>,
    last_event_unix: Arc<AtomicI64>,
    started_at: Instant,
}

impl Default for UpstreamHealth {
    fn default() -> Self {
        Self::new()
    }
}

impl UpstreamHealth {
    pub fn new() -> Self {
        Self {
            connected: Arc::new(AtomicBool::new(false)),
            last_event_unix: Arc::new(AtomicI64::new(0)),
            started_at: Instant::now(),
        }
    }

    pub fn set_connected(&self, val: bool) {
        self.connected.store(val, Ordering::Relaxed);
    }

    pub fn mark_event(&self, now: i64) {
        self.last_event_unix.store(now, Ordering::Relaxed);
    }

    pub fn reset_event(&self) {
        self.last_event_unix.store(0, Ordering::Relaxed);
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn last_event_age_s(&self, now: i64) -> Option<i64> {
        let ts = self.last_event_unix.load(Ordering::Relaxed);
        if ts <= 0 {
            None
        } else {
            Some((now - ts).max(0))
        }
    }

    pub fn uptime_s(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}

#[derive(Clone, Default)]
pub struct LiveCache(Arc<Mutex<HashMap<String, HashMap<String, Entry>>>>);

impl LiveCache {
    pub fn counts(&self, now: i64) -> HashMap<String, usize> {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = HashMap::new();
        for &(pref, _) in OPERATORS {
            let count = match map.get_mut(pref) {
                Some(buses) => {
                    buses.retain(|_, e| now - e.fix_unix <= DROP_AFTER_S);
                    buses.len()
                }
                None => 0,
            };
            out.insert(pref.to_string(), count);
        }
        out
    }

    pub fn snapshot(&self, pref: &str, now: i64) -> Vec<Bus> {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let Some(buses) = map.get_mut(pref) else {
            return Vec::new();
        };
        buses.retain(|_, e| now - e.fix_unix <= DROP_AFTER_S);
        buses
            .values()
            .map(|e| with_age(e.bus.clone(), e.fix_unix, e.stopped_since, now))
            .collect()
    }

    fn ingest(&self, pref: &str, raw: RawBus, now: i64) -> Option<Bus> {
        let offset = operator_offset(pref)?;
        let lat = raw.lat?;
        let lng = raw.lng?;
        if lat == 0.0 && lng == 0.0 {
            return None;
        }

        let fix_unix = raw
            .dt_server
            .as_deref()
            .and_then(|s| parse_local_datetime(s, offset))
            .unwrap_or_else(|| now - raw.ago.unwrap_or(0.0) as i64);
        if now - fix_unix > DROP_AFTER_S {
            return None;
        }

        let bus = Bus {
            id: raw.id,
            pref: pref.to_string(),
            name: raw.name.map(|s| s.trim().to_string()),
            plate: raw.plate_number.map(|s| s.trim().to_string()),
            lat,
            lng,
            heading: raw.angle,
            speed: raw.speed,
            corridor: raw.kor,
            toward: raw.toward.map(|s| s.trim().to_string()),
            next_halte: raw.new_shel_t,
            prev_halte: raw.old_shel_t,
            passengers: raw.passenger.map(|p| p.max(0.0) as u32),
            fix_age_s: 0,
            stale: false,
            stopped_s: 0,
        };
        let dedupe_key = format!(
            "{}|{}|{:?}|{:?}|{:?}|{:?}|{}",
            bus.lat, bus.lng, bus.heading, bus.speed, bus.passengers, bus.next_halte, fix_unix
        );

        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let buses = map.entry(pref.to_string()).or_default();
        let prev = buses.get(&bus.id);
        if prev.is_some_and(|e| e.dedupe_key == dedupe_key) {
            return None;
        }
        let is_moving = bus.speed.is_some_and(|s| s.round() > 0.0)
            || prev.is_some_and(|p| approx_moved_m(p.bus.lat, p.bus.lng, bus.lat, bus.lng) >= 15.0);
        let stopped_since = if is_moving {
            None
        } else {
            prev.and_then(|p| p.stopped_since).or(Some(fix_unix))
        };
        buses.insert(
            bus.id.clone(),
            Entry {
                bus: bus.clone(),
                fix_unix,
                stopped_since,
                dedupe_key,
            },
        );
        Some(with_age(bus, fix_unix, stopped_since, now))
    }

    pub fn upsert_custom(&self, mut bus: Bus, now: i64) -> Bus {
        bus.fix_age_s = 0;
        bus.stale = false;
        bus.stopped_s = 0;
        let stopped_since = if bus.speed.is_some_and(|s| s.round() > 0.0) {
            None
        } else {
            Some(now)
        };
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let buses = map.entry(bus.pref.clone()).or_default();
        buses.insert(
            bus.id.clone(),
            Entry {
                bus: bus.clone(),
                fix_unix: now,
                stopped_since,
                dedupe_key: String::from("custom"),
            },
        );
        bus
    }

    pub fn remove_custom(&self, pref: &str, id: &str) -> Option<Bus> {
        let mut map = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let buses = map.get_mut(pref)?;
        let mut entry = buses.remove(id)?;
        entry.bus.fix_age_s = DROP_AFTER_S + 10;
        entry.bus.stale = true;
        Some(entry.bus)
    }
}

pub fn broadcast_bus(io: &SocketIo, bus: &Bus) {
    let _ = io.to(room(&bus.pref)).emit("bus", bus);
}

fn approx_moved_m(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let cos_lat = ((lat1 + lat2) * 0.5).to_radians().cos();
    (lat2 - lat1).hypot((lng2 - lng1) * cos_lat) * 111_320.0
}

fn with_age(mut bus: Bus, fix_unix: i64, stopped_since: Option<i64>, now: i64) -> Bus {
    bus.fix_age_s = (now - fix_unix).max(0);
    bus.stale = bus.fix_age_s > STALE_AFTER_S;
    bus.stopped_s = stopped_since.map(|t| (now - t).max(0)).unwrap_or(0);
    bus
}

fn operator_offset(pref: &str) -> Option<i64> {
    OPERATORS.iter().find(|(p, _)| *p == pref).map(|(_, o)| *o)
}

fn room(pref: &str) -> String {
    format!("pref:{pref}")
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn parse_local_datetime(s: &str, utc_offset_s: i64) -> Option<i64> {
    let s = s.trim();
    let n = |a: usize, b: usize| -> Option<i64> { s.get(a..b)?.parse().ok() };
    let (y, mo, d) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (h, mi, se) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se - utc_offset_s)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SubscribeMsg {
    Bare(String),
    Obj { pref: String },
}

pub fn register_namespace(io: &SocketIo, cache: LiveCache) {
    io.ns("/", move |s: SocketRef| {
        tracing::debug!(sid = %s.id, "browser connected");
        let cache = cache.clone();
        s.on("subscribe", move |s: SocketRef, Data::<SubscribeMsg>(msg)| {
            let pref = match msg {
                SubscribeMsg::Bare(p) | SubscribeMsg::Obj { pref: p } => p,
            };
            if operator_offset(&pref).is_none() {
                tracing::debug!(%pref, "rejecting unknown pref");
                return;
            }
            let _ = s.leave_all();
            let _ = s.join(room(&pref));
            let snap = cache.snapshot(&pref, now_unix());
            tracing::debug!(sid = %s.id, %pref, buses = snap.len(), "subscribed");
            let _ = s.emit("snapshot", &[snap]);
        });
    });
}

pub fn spawn_upstream(url: String, io: SocketIo, cache: LiveCache, health: UpstreamHealth) {
    tokio::spawn(async move {
        let last_event = Arc::new(AtomicI64::new(now_unix()));
        let mut retry_delay_s: u64 = 2;
        loop {
            match tokio::time::timeout(
                UPSTREAM_CONNECT_TIMEOUT,
                connect_upstream(&url, io.clone(), cache.clone(), last_event.clone(), health.clone()),
            )
            .await
            {
                Ok(Ok(client)) => {
                    retry_delay_s = 2;
                    last_event.store(now_unix(), Ordering::Relaxed);
                    loop {
                        tokio::time::sleep(Duration::from_secs(15)).await;
                        let silent = now_unix() - last_event.load(Ordering::Relaxed);
                        if silent > UPSTREAM_SILENCE_S {
                            tracing::warn!(silent, "upstream silent, reconnecting");
                            health.set_connected(false);
                            let _ = tokio::time::timeout(
                                UPSTREAM_DISCONNECT_TIMEOUT,
                                client.disconnect(),
                            )
                            .await;
                            break;
                        }
                    }
                }
                Ok(Err(e)) => {
                    health.set_connected(false);
                    tracing::error!("upstream connect failed: {e:#}, retrying in {retry_delay_s}s");
                    tokio::time::sleep(Duration::from_secs(retry_delay_s)).await;
                    retry_delay_s = (retry_delay_s * 2).min(30);
                }
                Err(_) => {
                    health.set_connected(false);
                    tracing::warn!("upstream connect timed out, retrying in {retry_delay_s}s");
                    tokio::time::sleep(Duration::from_secs(retry_delay_s)).await;
                    retry_delay_s = (retry_delay_s * 2).min(30);
                }
            }
        }
    });
}

async fn connect_upstream(
    url: &str,
    io: SocketIo,
    cache: LiveCache,
    last_event: Arc<AtomicI64>,
    health: UpstreamHealth,
) -> anyhow::Result<Client> {
    let close_last_event = last_event.clone();
    let connect_health = health.clone();
    let close_health = health.clone();
    let event_health = health;
    let client = ClientBuilder::new(url)
        .transport_type(TransportType::Websocket)
        .reconnect(true)
        .reconnect_on_disconnect(true)
        .on(Event::Connect, move |_, _| {
            let connect_health = connect_health.clone();
            Box::pin(async move {
                connect_health.set_connected(true);
                tracing::info!("upstream socket connected");
            })
        })
        .on(Event::Close, move |_, _| {
            let close_last_event = close_last_event.clone();
            let close_health = close_health.clone();
            Box::pin(async move {
                close_last_event.store(0, Ordering::Relaxed);
                close_health.set_connected(false);
                close_health.reset_event();
                tracing::warn!("upstream socket closed");
            })
        })
        .on(Event::Error, |p, _| {
            Box::pin(async move { tracing::warn!(?p, "upstream socket error") })
        })
        .on_any(move |event, payload, _| {
            let (io, cache, last_event, event_health) = (
                io.clone(),
                cache.clone(),
                last_event.clone(),
                event_health.clone(),
            );
            Box::pin(async move {
                let name = event.as_str();
                let Some(pref) = name.strip_prefix("BRT-") else { return };
                let now = now_unix();
                last_event.store(now, Ordering::Relaxed);
                event_health.mark_event(now);
                event_health.set_connected(true);
                if operator_offset(pref).is_none() {
                    return;
                }
                let Payload::Text(values) = payload else { return };
                for v in values {
                    match serde_json::from_value::<RawBus>(v) {
                        Ok(raw) => {
                            if let Some(bus) = cache.ingest(pref, raw, now) {
                                let _ = io.to(room(pref)).emit("bus", &bus);
                            }
                        }
                        Err(e) => tracing::debug!(%pref, "unparseable bus: {e}"),
                    }
                }
            })
        })
        .connect()
        .await?;
    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"ago":"1","angle":99,"dt_server":"2026-11-19 19:41:19","dt_tracker":"2026-11-19 12:41:17","gap":"3","id":"869066065363849","imei":"869066065363849","kor":"FD2","lat":-7.291541,"lng":112.717876,"name":"WW84","new_shel_t":"0040409","old_shel_t":"0037941","plate_number":"L 1033 UB","pref":"3501","speed":26,"toward":"Halte Balai Kota Surabaya"}"#;

    fn sample() -> RawBus {
        serde_json::from_str(SAMPLE).unwrap()
    }

    #[test]
    fn parses_wib_datetime() {
        assert_eq!(parse_local_datetime("1970-01-01 07:00:00", 7 * 3600), Some(0));
        assert_eq!(parse_local_datetime("2026-11-19 19:41:19", 7 * 3600), Some(1_795_092_079));
        assert_eq!(parse_local_datetime("garbage", 0), None);
    }

    #[test]
    fn ingests_and_dedupes() {
        let cache = LiveCache::default();
        let now = 1_795_092_081;
        let bus = cache.ingest("3501", sample(), now).expect("first update forwarded");
        assert_eq!(bus.name.as_deref(), Some("WW84"));
        assert_eq!(bus.fix_age_s, 2);
        assert!(!bus.stale);
        assert!(cache.ingest("3501", sample(), now + 1).is_none(), "duplicate dropped");
        assert_eq!(cache.snapshot("3501", now).len(), 1);
    }

    #[test]
    fn drops_old_and_unknown() {
        let cache = LiveCache::default();
        let fix = 1_795_092_079;
        assert!(cache.ingest("3501", sample(), fix + DROP_AFTER_S + 1).is_none());
        assert!(cache.ingest("99999", sample(), fix).is_none());
        let b = cache.ingest("3501", sample(), fix + STALE_AFTER_S + 1).unwrap();
        assert!(b.stale);
        assert!(cache.snapshot("3501", fix + DROP_AFTER_S + 1).is_empty());
    }

    #[test]
    fn tracks_stopped_duration_across_fixes() {
        let cache = LiveCache::default();
        let fix = 1_795_092_079;
        let mut s0 = sample();
        s0.speed = Some(0.0);
        let b0 = cache.ingest("3501", s0, fix + 1).unwrap();
        assert_eq!(b0.stopped_s, 1);

        let mut s1 = sample();
        s1.speed = Some(0.0);
        s1.dt_server = Some("2026-11-19 19:51:29".into());
        let b1 = cache.ingest("3501", s1, fix + 610).unwrap();
        assert_eq!(b1.stopped_s, 610);

        let mut s2 = sample();
        s2.speed = Some(0.0);
        s2.lat = s2.lat.map(|v| v + 0.0005);
        s2.dt_server = Some("2026-11-19 19:51:49".into());
        let b2 = cache.ingest("3501", s2, fix + 630).unwrap();
        assert_eq!(b2.stopped_s, 0);
    }
}
