use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct MenuLink {
    pub enabled: bool,
    pub label: String,
    pub url: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CityPopup {
    pub enabled: bool,
    pub title: String,
    pub text: String,
    pub image_url: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AdminData {
    pub cities: HashMap<String, CityOverrides>,
    pub menu_links: Vec<MenuLink>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CityOverrides {
    pub added_corridors: Vec<CustomCorridor>,
    pub removed_corridors: Vec<String>,
    pub corridor_order: Vec<String>,
    pub added_stops: Vec<CustomStop>,
    pub removed_stops: Vec<String>,
    pub added_buses: Vec<CustomBus>,
    pub welcome_popup: Option<CityPopup>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomCorridor {
    pub id: String,
    pub kor: String,
    #[serde(default)]
    pub source_kor: Option<String>,
    pub route_name: String,
    #[serde(default)]
    pub via: Option<String>,
    #[serde(default)]
    pub origin: String,
    #[serde(default)]
    pub toward: String,
    #[serde(default)]
    pub jam_operasional: Option<String>,
    #[serde(default = "default_corridor_color")]
    pub color: String,
    pub pref: String,
    #[serde(default)]
    pub points_a: Option<String>,
    #[serde(default)]
    pub points_b: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomStop {
    #[serde(default)]
    pub sh_id: String,
    pub sh_name: String,
    pub sh_lat: f64,
    pub sh_lng: f64,
    #[serde(default)]
    pub kor: String,
    #[serde(default)]
    pub in_koridor: String,
    #[serde(default)]
    pub color_koridor: String,
    #[serde(default = "default_stop_color")]
    pub color: String,
    #[serde(default)]
    pub toward: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomBus {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub plate: Option<String>,
    pub corridor: String,
    #[serde(default)]
    pub toward: Option<String>,
    pub lat: f64,
    pub lng: f64,
    #[serde(default)]
    pub heading: Option<f64>,
    #[serde(default)]
    pub speed: Option<f64>,
    #[serde(default)]
    pub passengers: Option<u32>,
}

fn default_corridor_color() -> String {
    "#0284c7".to_string()
}

fn default_stop_color() -> String {
    "#64748b".to_string()
}

#[derive(Clone)]
pub struct GistConfig {
    http: reqwest::Client,
    id: String,
    token: String,
    file: String,
}

#[derive(Debug, PartialEq, Eq)]
enum GistContent {
    Inline(String),
    RawUrl(String),
}

fn extract_gist_content(payload: &Value, file: &str) -> Option<GistContent> {
    let entry = payload.get("files")?.get(file)?;
    let truncated = entry
        .get("truncated")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if truncated {
        if let Some(url) = entry
            .get("raw_url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Some(GistContent::RawUrl(url.to_string()));
        }
    }
    let content = entry
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Some(GistContent::Inline(content))
}

impl GistConfig {
    pub fn from_env(http: reqwest::Client) -> Option<Self> {
        let id = std::env::var("ADMIN_GIST_ID")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())?;
        let token = std::env::var("ADMIN_GIST_TOKEN")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())?;
        let file = std::env::var("ADMIN_GIST_FILE")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "admin_overrides.json".to_string());
        Some(Self {
            http,
            id,
            token,
            file,
        })
    }

    async fn load(&self) -> anyhow::Result<Option<AdminData>> {
        let resp = self
            .http
            .get(format!("https://api.github.com/gists/{}", self.id))
            .bearer_auth(&self.token)
            .header("User-Agent", "manabus-proxy")
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("GitHub Gist fetch failed with status {status}");
        }
        let payload = resp.json::<Value>().await?;
        let raw = match extract_gist_content(&payload, &self.file) {
            None => return Ok(None),
            Some(GistContent::Inline(text)) => text,
            Some(GistContent::RawUrl(url)) => {
                let raw_resp = self
                    .http
                    .get(url)
                    .bearer_auth(&self.token)
                    .header("User-Agent", "manabus-proxy")
                    .send()
                    .await?;
                let raw_status = raw_resp.status();
                if !raw_status.is_success() {
                    anyhow::bail!("GitHub Gist raw download failed with status {raw_status}");
                }
                raw_resp.text().await?
            }
        };
        if raw.trim().is_empty() {
            return Ok(Some(AdminData::default()));
        }
        let parsed = serde_json::from_str::<AdminData>(&raw)?;
        Ok(Some(parsed))
    }

    async fn save(&self, body: &str) -> anyhow::Result<()> {
        let mut files = Map::new();
        let mut file_obj = Map::new();
        file_obj.insert("content".into(), Value::String(body.to_string()));
        files.insert(self.file.clone(), Value::Object(file_obj));
        let payload = serde_json::json!({ "files": Value::Object(files) });
        let resp = self
            .http
            .patch(format!("https://api.github.com/gists/{}", self.id))
            .bearer_auth(&self.token)
            .header("User-Agent", "manabus-proxy")
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&payload)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            anyhow::bail!("GitHub Gist save failed with status {status}");
        }
        Ok(())
    }
}

pub struct AdminStore {
    path: PathBuf,
    gist: Option<GistConfig>,
    data: RwLock<AdminData>,
}

impl AdminStore {
    #[cfg(test)]
    pub async fn load(path: PathBuf) -> Self {
        Self::load_with_gist(path, None).await
    }

    pub async fn load_with_gist(path: PathBuf, gist: Option<GistConfig>) -> Self {
        if let Some(ref cfg) = gist {
            match cfg.load().await {
                Ok(Some(remote)) => {
                    let _ = Self::persist_local(&path, &remote).await;
                    tracing::info!(file = %cfg.file, "loaded admin overrides from GitHub Gist");
                    return Self {
                        path,
                        gist,
                        data: RwLock::new(remote),
                    };
                }
                Ok(None) => {
                    tracing::warn!(
                        file = %cfg.file,
                        "file missing in GitHub Gist, falling back to local file"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "failed to load admin overrides from GitHub Gist, falling back to local file: {e:#}"
                    );
                }
            }
        }
        let parsed = match tokio::fs::read_to_string(&path).await {
            Ok(raw) => serde_json::from_str::<AdminData>(&raw).unwrap_or_default(),
            Err(_) => AdminData::default(),
        };
        Self {
            path,
            gist,
            data: RwLock::new(parsed),
        }
    }

    pub async fn snapshot(&self) -> AdminData {
        self.data.read().await.clone()
    }

    pub async fn city_overrides(&self, pref: &str) -> CityOverrides {
        let guard = self.data.read().await;
        guard.cities.get(pref).cloned().unwrap_or_default()
    }

    async fn persist_local(path: &PathBuf, data: &AdminData) -> anyhow::Result<String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        let body = serde_json::to_string_pretty(data)?;
        let tmp = PathBuf::from(format!("{}.tmp", path.display()));
        tokio::fs::write(&tmp, body.as_bytes()).await?;
        tokio::fs::rename(&tmp, path).await?;
        Ok(body)
    }

    async fn persist(&self, data: &AdminData) -> anyhow::Result<()> {
        let body = Self::persist_local(&self.path, data).await?;
        if let Some(gist) = &self.gist {
            gist.save(&body).await?;
        }
        Ok(())
    }

    pub async fn upsert_corridor(
        &self,
        pref: &str,
        mut corridor: CustomCorridor,
    ) -> anyhow::Result<CityOverrides> {
        let kor = corridor.kor.trim().to_uppercase();
        let route_name = corridor.route_name.trim().to_string();
        if kor.is_empty() || route_name.is_empty() {
            anyhow::bail!("kor and route_name are required");
        }
        let key = norm_code(&kor);
        let src_norm = corridor
            .source_kor
            .as_deref()
            .map(norm_code)
            .filter(|s| !s.is_empty());
        corridor.kor = kor.clone();
        corridor.source_kor = corridor
            .source_kor
            .as_deref()
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty());
        corridor.route_name = route_name;
        corridor.via = corridor
            .via
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
        corridor.origin = corridor.origin.trim().to_string();
        corridor.toward = corridor.toward.trim().to_string();
        corridor.jam_operasional = corridor
            .jam_operasional
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
        corridor.color = normalize_hex_color(&corridor.color, "#0284c7");
        corridor.pref = pref.to_string();
        if corridor.id.trim().is_empty() {
            corridor.id = format!("custom-{pref}-{}", key.to_lowercase());
        }
        corridor.points_a = corridor
            .points_a
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
        corridor.points_b = corridor
            .points_b
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);

        let mut guard = self.data.write().await;
        let city = guard.cities.entry(pref.to_string()).or_default();
        city.removed_corridors
            .retain(|c| norm_code(c) != key && src_norm.as_deref() != Some(norm_code(c).as_str()));
        let existing_pos = city.added_corridors.iter().position(|c| {
            let ck = norm_code(&c.kor);
            let cs = c.source_kor.as_deref().map(norm_code);
            ck == key
                || src_norm.as_deref() == Some(ck.as_str())
                || (src_norm.is_some() && cs == src_norm)
                || cs.as_deref() == Some(key.as_str())
        });
        if let Some(pos) = existing_pos {
            city.added_corridors[pos] = corridor;
        } else {
            city.added_corridors.push(corridor);
        }
        if let Some(src) = src_norm {
            if src != key {
                for item in &mut city.corridor_order {
                    if norm_code(item) == src {
                        *item = key.clone();
                    }
                }
            }
        }
        let updated = city.clone();
        self.persist(&guard).await?;
        Ok(updated)
    }

    pub async fn set_corridor_order(
        &self,
        pref: &str,
        order: Vec<String>,
    ) -> anyhow::Result<CityOverrides> {
        let mut seen = HashSet::new();
        let mut cleaned = Vec::new();
        for raw in order {
            let key = norm_code(&raw);
            if !key.is_empty() && seen.insert(key.clone()) {
                cleaned.push(key);
            }
        }
        let mut guard = self.data.write().await;
        let city = guard.cities.entry(pref.to_string()).or_default();
        city.corridor_order = cleaned;
        let updated = city.clone();
        self.persist(&guard).await?;
        Ok(updated)
    }

    pub async fn remove_corridor(
        &self,
        pref: &str,
        code: &str,
        hide_upstream: bool,
    ) -> anyhow::Result<CityOverrides> {
        let key = norm_code(code);
        if key.is_empty() {
            anyhow::bail!("code is required");
        }
        let mut guard = self.data.write().await;
        let city = guard.cities.entry(pref.to_string()).or_default();
        let mut orig_source: Option<String> = None;
        let before = city.added_corridors.len();
        city.added_corridors.retain(|c| {
            let matches = norm_code(&c.kor) == key
                || c.source_kor
                    .as_deref()
                    .map(norm_code)
                    .as_deref()
                    == Some(key.as_str());
            if matches {
                if let Some(src) = &c.source_kor {
                    orig_source = Some(norm_code(src));
                }
                false
            } else {
                true
            }
        });
        let removed_custom = city.added_corridors.len() < before;
        let target_hide = orig_source.unwrap_or_else(|| key.clone());
        if (hide_upstream || !removed_custom)
            && !city
                .removed_corridors
                .iter()
                .any(|c| norm_code(c) == target_hide)
        {
            city.removed_corridors.push(target_hide.clone());
        }
        if hide_upstream || !removed_custom {
            city.corridor_order
                .retain(|c| norm_code(c) != key && norm_code(c) != target_hide);
        }
        let updated = city.clone();
        self.persist(&guard).await?;
        Ok(updated)
    }

    pub async fn restore_corridor(
        &self,
        pref: &str,
        code: &str,
    ) -> anyhow::Result<CityOverrides> {
        let key = norm_code(code);
        let mut guard = self.data.write().await;
        let city = guard.cities.entry(pref.to_string()).or_default();
        city.removed_corridors.retain(|c| norm_code(c) != key);
        let updated = city.clone();
        self.persist(&guard).await?;
        Ok(updated)
    }

    pub async fn upsert_stop(
        &self,
        pref: &str,
        mut stop: CustomStop,
    ) -> anyhow::Result<CityOverrides> {
        let name = stop.sh_name.trim().to_string();
        if name.is_empty() || !stop.sh_lat.is_finite() || !stop.sh_lng.is_finite() {
            anyhow::bail!("valid sh_name, sh_lat, and sh_lng are required");
        }
        if stop.sh_lat.abs() > 90.0 || stop.sh_lng.abs() > 180.0 {
            anyhow::bail!("coordinates out of range");
        }
        stop.sh_name = name;
        let sh_id = stop.sh_id.trim().to_string();
        stop.sh_id = if sh_id.is_empty() {
            let ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            format!("custom-{ms}")
        } else {
            sh_id
        };

        let codes: Vec<String> = stop
            .in_koridor
            .split(['|', ','])
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect();
        let fallback_kor = stop.kor.trim().to_uppercase();
        let final_codes = if codes.is_empty() && !fallback_kor.is_empty() {
            vec![fallback_kor.clone()]
        } else {
            codes
        };
        if final_codes.is_empty() {
            anyhow::bail!("at least one serving corridor is required");
        }
        stop.kor = if fallback_kor.is_empty() {
            final_codes[0].clone()
        } else {
            fallback_kor
        };
        stop.in_koridor = final_codes.join("|");

        let raw_colors: Vec<String> = stop
            .color_koridor
            .split(['|', ','])
            .map(|s| normalize_hex_color(s, "#64748b"))
            .collect();
        let primary_color = normalize_hex_color(&stop.color, "#64748b");
        let final_colors: Vec<String> = (0..final_codes.len())
            .map(|i| {
                raw_colors
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| primary_color.clone())
            })
            .collect();
        stop.color = primary_color;
        stop.color_koridor = final_colors.join("|");
        stop.toward = stop.toward.trim().to_string();

        let mut guard = self.data.write().await;
        let city = guard.cities.entry(pref.to_string()).or_default();
        city.removed_stops.retain(|id| id != &stop.sh_id);
        if let Some(pos) = city
            .added_stops
            .iter()
            .position(|s| s.sh_id == stop.sh_id)
        {
            city.added_stops[pos] = stop;
        } else {
            city.added_stops.push(stop);
        }
        let updated = city.clone();
        self.persist(&guard).await?;
        Ok(updated)
    }

    pub async fn remove_stop(
        &self,
        pref: &str,
        id: &str,
        hide_upstream: bool,
    ) -> anyhow::Result<CityOverrides> {
        let target = id.trim().to_string();
        if target.is_empty() {
            anyhow::bail!("stop id is required");
        }
        let mut guard = self.data.write().await;
        let city = guard.cities.entry(pref.to_string()).or_default();
        let before = city.added_stops.len();
        city.added_stops.retain(|s| s.sh_id != target);
        let removed_custom = city.added_stops.len() < before;
        if (hide_upstream || !removed_custom) && !city.removed_stops.iter().any(|s| s == &target) {
            city.removed_stops.push(target);
        }
        let updated = city.clone();
        self.persist(&guard).await?;
        Ok(updated)
    }

    pub async fn restore_stop(&self, pref: &str, id: &str) -> anyhow::Result<CityOverrides> {
        let target = id.trim();
        let mut guard = self.data.write().await;
        let city = guard.cities.entry(pref.to_string()).or_default();
        city.removed_stops.retain(|s| s != target);
        let updated = city.clone();
        self.persist(&guard).await?;
        Ok(updated)
    }

    pub async fn upsert_bus(
        &self,
        pref: &str,
        mut bus: CustomBus,
    ) -> anyhow::Result<(CustomBus, CityOverrides)> {
        let name = bus.name.trim().to_string();
        let corridor = bus.corridor.trim().to_uppercase();
        if name.is_empty() || corridor.is_empty() || !bus.lat.is_finite() || !bus.lng.is_finite() {
            anyhow::bail!("valid name, corridor, lat, and lng are required");
        }
        if bus.lat.abs() > 90.0 || bus.lng.abs() > 180.0 {
            anyhow::bail!("coordinates out of range");
        }
        bus.name = name;
        bus.corridor = corridor;
        let id = bus.id.trim().to_string();
        bus.id = if id.is_empty() {
            let ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            format!("fake-{ms}")
        } else {
            id
        };
        bus.plate = bus
            .plate
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
        bus.toward = bus
            .toward
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
        bus.heading = bus.heading.filter(|h| h.is_finite()).map(|h| h.rem_euclid(360.0));
        bus.speed = bus.speed.filter(|s| s.is_finite()).map(|s| s.max(0.0));

        let mut guard = self.data.write().await;
        let city = guard.cities.entry(pref.to_string()).or_default();
        if let Some(pos) = city.added_buses.iter().position(|b| b.id == bus.id) {
            city.added_buses[pos] = bus.clone();
        } else {
            city.added_buses.push(bus.clone());
        }
        let updated = city.clone();
        self.persist(&guard).await?;
        Ok((bus, updated))
    }

    pub async fn remove_bus(&self, pref: &str, id: &str) -> anyhow::Result<CityOverrides> {
        let target = id.trim();
        if target.is_empty() {
            anyhow::bail!("bus id is required");
        }
        let mut guard = self.data.write().await;
        let city = guard.cities.entry(pref.to_string()).or_default();
        city.added_buses.retain(|b| b.id != target);
        let updated = city.clone();
        self.persist(&guard).await?;
        Ok(updated)
    }

    pub async fn reset_city_scope(
        &self,
        pref: &str,
        scope: &str,
    ) -> anyhow::Result<(CityOverrides, Vec<String>)> {
        let mut guard = self.data.write().await;
        let city = guard.cities.entry(pref.to_string()).or_default();
        let mut removed_bus_ids = Vec::new();
        match scope {
            "routes" => {
                city.added_corridors.clear();
                city.removed_corridors.clear();
                city.corridor_order.clear();
            }
            "stops" => {
                city.added_stops.clear();
                city.removed_stops.clear();
            }
            "buses" => {
                removed_bus_ids = city.added_buses.iter().map(|b| b.id.clone()).collect();
                city.added_buses.clear();
            }
            "all" => {
                city.added_corridors.clear();
                city.removed_corridors.clear();
                city.corridor_order.clear();
                city.added_stops.clear();
                city.removed_stops.clear();
                removed_bus_ids = city.added_buses.iter().map(|b| b.id.clone()).collect();
                city.added_buses.clear();
            }
            _ => anyhow::bail!("invalid reset scope"),
        }
        let updated = city.clone();
        self.persist(&guard).await?;
        Ok((updated, removed_bus_ids))
    }

    pub async fn menu_links(&self) -> Vec<MenuLink> {
        let guard = self.data.read().await;
        guard.menu_links.clone()
    }

    pub async fn set_menu_links(&self, links: Vec<MenuLink>) -> anyhow::Result<Vec<MenuLink>> {
        let mut out = Vec::with_capacity(links.len());
        for mut item in links {
            item.label = item.label.trim().to_string();
            item.url = item.url.trim().to_string();
            if item.enabled {
                if item.label.is_empty() || item.url.is_empty() {
                    anyhow::bail!("Label dan link wajib diisi untuk tombol yang aktif");
                }
                if !is_safe_redirect_url(&item.url) {
                    anyhow::bail!("Link harus diawali https://, http://, atau /");
                }
            } else if !item.url.is_empty() && !is_safe_redirect_url(&item.url) {
                anyhow::bail!("Link harus diawali https://, http://, atau /");
            }
            out.push(item);
        }
        let mut guard = self.data.write().await;
        guard.menu_links = out.clone();
        self.persist(&guard).await?;
        Ok(out)
    }

    pub async fn city_popup(&self, pref: &str) -> CityPopup {
        let guard = self.data.read().await;
        guard
            .cities
            .get(pref)
            .and_then(|c| c.welcome_popup.clone())
            .unwrap_or_default()
    }

    pub async fn set_city_popup(
        &self,
        pref: &str,
        mut popup: CityPopup,
    ) -> anyhow::Result<CityPopup> {
        popup.title = popup.title.trim().to_string();
        popup.text = popup.text.trim().to_string();
        popup.image_url = popup.image_url.trim().to_string();
        if !popup.image_url.is_empty() {
            if !is_safe_image_url(&popup.image_url) {
                anyhow::bail!("Gambar harus berupa URL http(s)://, path /, atau berkas gambar yang diunggah");
            }
            if popup.image_url.len() > 3_000_000 {
                anyhow::bail!("Ukuran gambar terlalu besar (maksimal 2 MB)");
            }
        }
        if popup.enabled
            && popup.title.is_empty()
            && popup.text.is_empty()
            && popup.image_url.is_empty()
        {
            anyhow::bail!("Isi minimal teks atau gambar untuk mengaktifkan popup");
        }
        let mut guard = self.data.write().await;
        guard
            .cities
            .entry(pref.to_string())
            .or_default()
            .welcome_popup = Some(popup.clone());
        self.persist(&guard).await?;
        Ok(popup)
    }
}

fn is_safe_redirect_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with('/')
}

fn is_safe_image_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with('/')
        || lower.starts_with("data:image/")
}

pub fn norm_code(s: &str) -> String {
    s.trim()
        .to_uppercase()
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect()
}

fn normalize_hex_color(raw: &str, fallback: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return fallback.to_string();
    }
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if (hex.len() == 3 || hex.len() == 6) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        format!("#{}", hex.to_lowercase())
    } else {
        fallback.to_string()
    }
}

pub fn verify_secret(expected: &Option<String>, provided: &str) -> bool {
    let Some(exp) = expected.as_deref() else {
        return false;
    };
    let a = exp.as_bytes();
    let b = provided.as_bytes();
    if a.is_empty() || a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn extract_username(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-admin-username")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

pub fn extract_secret(headers: &HeaderMap) -> Option<&str> {
    if let Some(val) = headers.get("x-admin-secret").and_then(|v| v.to_str().ok()) {
        return Some(val.trim());
    }
    if let Some(auth) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            return Some(token.trim());
        }
    }
    None
}

fn value_string(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn corridor_row_code(row: &Value) -> String {
    for key in ["kor", "kode", "corridor", "route_code"] {
        if let Some(v) = row.get(key) {
            let s = value_string(v);
            let norm = norm_code(&s);
            if !norm.is_empty() {
                return norm;
            }
        }
    }
    String::new()
}

fn patch_corridor_object(obj: &mut Map<String, Value>, custom: &CustomCorridor) {
    obj.insert("kor".into(), Value::String(custom.kor.clone()));
    for alt in ["kode", "corridor", "route_code"] {
        if obj.contains_key(alt) {
            obj.insert(alt.into(), Value::String(custom.kor.clone()));
        }
    }
    if let Some(src) = &custom.source_kor {
        if norm_code(src) != norm_code(&custom.kor) {
            obj.insert("source_kor".into(), Value::String(src.clone()));
        }
    }
    obj.insert("custom".into(), Value::Bool(true));
    obj.insert(
        "route_name".into(),
        Value::String(custom.route_name.clone()),
    );
    obj.insert(
        "via".into(),
        Value::String(custom.via.clone().unwrap_or_default()),
    );
    obj.insert("origin".into(), Value::String(custom.origin.clone()));
    obj.insert("toward".into(), Value::String(custom.toward.clone()));
    if let Some(hours) = &custom.jam_operasional {
        obj.insert("jam_operasional".into(), Value::String(hours.clone()));
    }
    obj.insert("color".into(), Value::String(custom.color.clone()));
    if let Some(pts_a) = &custom.points_a {
        obj.insert("points_a".into(), Value::String(pts_a.clone()));
    }
    if let Some(pts_b) = &custom.points_b {
        obj.insert("points_b".into(), Value::String(pts_b.clone()));
    }
}

pub fn upstream_kor_for(data: &AdminData, pref: &str, kor: &str) -> String {
    let target = norm_code(kor);
    if let Some(city) = data.cities.get(pref) {
        for c in &city.added_corridors {
            if norm_code(&c.kor) == target {
                if let Some(src) = &c.source_kor {
                    if !src.trim().is_empty() {
                        return src.clone();
                    }
                }
            }
        }
    }
    kor.to_string()
}

fn renamed_corridor_map(city: &CityOverrides) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for c in &city.added_corridors {
        if let Some(src) = &c.source_kor {
            let s = norm_code(src);
            if !s.is_empty() && s != norm_code(&c.kor) {
                map.insert(s, c.kor.clone());
            }
        }
    }
    map
}

fn rewrite_stop_corridors(row: &mut Value, renames: &HashMap<String, String>) {
    if renames.is_empty() {
        return;
    }
    let Some(obj) = row.as_object_mut() else {
        return;
    };
    if let Some(kor) = obj.get("kor").and_then(Value::as_str).map(norm_code) {
        if let Some(new_kor) = renames.get(&kor) {
            obj.insert("kor".into(), Value::String(new_kor.clone()));
        }
    }
    if let Some(in_kor) = obj.get("in_koridor").and_then(Value::as_str) {
        let updated: Vec<String> = in_kor
            .split('|')
            .map(|part| {
                let norm = norm_code(part);
                renames
                    .get(&norm)
                    .cloned()
                    .unwrap_or_else(|| part.trim().to_string())
            })
            .collect();
        obj.insert("in_koridor".into(), Value::String(updated.join("|")));
    }
}

pub fn apply_corridor_overrides(payload: &mut Value, data: &AdminData, pref: &str) {
    let Some(city) = data.cities.get(pref) else {
        return;
    };
    let Some(arr) = payload.get_mut("data").and_then(Value::as_array_mut) else {
        return;
    };
    let removed: HashSet<String> = city.removed_corridors.iter().map(|c| norm_code(c)).collect();
    if !removed.is_empty() {
        arr.retain(|row| !removed.contains(&corridor_row_code(row)));
    }
    for custom in &city.added_corridors {
        let match_target = custom
            .source_kor
            .as_deref()
            .map(norm_code)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| norm_code(&custom.kor));
        let fallback_target = norm_code(&custom.kor);
        if let Some(existing) = arr
            .iter_mut()
            .find(|row| {
                let rc = corridor_row_code(row);
                rc == match_target || rc == fallback_target
            })
            .and_then(Value::as_object_mut)
        {
            patch_corridor_object(existing, custom);
        } else if let Ok(val) = serde_json::to_value(custom) {
            arr.push(val);
        }
    }
    if !city.corridor_order.is_empty() {
        let ranks: HashMap<String, usize> = city
            .corridor_order
            .iter()
            .enumerate()
            .map(|(idx, c)| (norm_code(c), idx))
            .collect();
        for row in arr.iter_mut() {
            let rc = corridor_row_code(row);
            if let (Some(&rank), Some(obj)) = (ranks.get(&rc), row.as_object_mut()) {
                obj.insert("sort_order".into(), Value::Number(rank.into()));
            }
        }
        arr.sort_by_key(|row| {
            let rc = corridor_row_code(row);
            ranks.get(&rc).copied().unwrap_or(usize::MAX)
        });
    }
}

pub fn apply_stop_overrides(payload: &mut Value, data: &AdminData, pref: &str) {
    let Some(city) = data.cities.get(pref) else {
        return;
    };
    let Some(arr) = payload.get_mut("data").and_then(Value::as_array_mut) else {
        return;
    };
    let removed: HashSet<&str> = city.removed_stops.iter().map(String::as_str).collect();
    if !removed.is_empty() {
        arr.retain(|row| {
            let id = row.get("sh_id").map(value_string).unwrap_or_default();
            !removed.contains(id.as_str())
        });
    }
    let renames = renamed_corridor_map(city);
    if !renames.is_empty() {
        for row in arr.iter_mut() {
            rewrite_stop_corridors(row, &renames);
        }
    }
    for custom in &city.added_stops {
        if let Ok(val) = serde_json::to_value(custom) {
            if let Some(existing) = arr
                .iter_mut()
                .find(|row| row.get("sh_id").map(value_string).as_deref() == Some(&custom.sh_id))
            {
                *existing = val;
            } else {
                arr.push(val);
            }
        }
    }
}

pub fn apply_leg_stop_overrides(
    payload: &mut Value,
    data: &AdminData,
    pref: &str,
    kor: &str,
    want_toward: &str,
) {
    let Some(city) = data.cities.get(pref) else {
        return;
    };
    let Some(arr) = payload.get_mut("data").and_then(Value::as_array_mut) else {
        return;
    };
    let removed: HashSet<&str> = city.removed_stops.iter().map(String::as_str).collect();
    if !removed.is_empty() {
        arr.retain(|row| {
            let id = row.get("sh_id").map(value_string).unwrap_or_default();
            !removed.contains(id.as_str())
        });
    }
    let renames = renamed_corridor_map(city);
    if !renames.is_empty() {
        for row in arr.iter_mut() {
            rewrite_stop_corridors(row, &renames);
        }
    }
    let target_kor = norm_code(kor);
    let target_tw = want_toward.trim();
    for custom in &city.added_stops {
        let serves = norm_code(&custom.kor) == target_kor
            || custom
                .in_koridor
                .split(['|', ','])
                .any(|c| norm_code(c) == target_kor);
        if !serves {
            arr.retain(|row| {
                row.get("sh_id").map(value_string).as_deref() != Some(custom.sh_id.as_str())
            });
            continue;
        }
        let mut patched_any = false;
        for row in arr.iter_mut() {
            if row.get("sh_id").map(value_string).as_deref() == Some(custom.sh_id.as_str()) {
                if let Some(obj) = row.as_object_mut() {
                    obj.insert("sh_name".into(), Value::String(custom.sh_name.clone()));
                    obj.insert("sh_lat".into(), serde_json::json!(custom.sh_lat));
                    obj.insert("sh_lng".into(), serde_json::json!(custom.sh_lng));
                    obj.insert("in_koridor".into(), Value::String(custom.in_koridor.clone()));
                    obj.insert(
                        "color_koridor".into(),
                        Value::String(custom.color_koridor.clone()),
                    );
                }
                patched_any = true;
            }
        }
        if patched_any {
            continue;
        }
        let custom_tw = custom.toward.trim();
        let toward_matches =
            target_tw.is_empty() || custom_tw.is_empty() || custom_tw.eq_ignore_ascii_case(target_tw);
        if toward_matches {
            if let Ok(val) = serde_json::to_value(custom) {
                arr.push(val);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn verify_secret_constant_time_checks() {
        assert!(!verify_secret(&None, "abc"));
        assert!(!verify_secret(&Some(String::new()), ""));
        assert!(!verify_secret(&Some("secret123".into()), "secret12"));
        assert!(!verify_secret(&Some("secret123".into()), "secret124"));
        assert!(verify_secret(&Some("secret123".into()), "secret123"));
    }

    #[test]
    fn corridor_overrides_add_patch_and_hide() {
        let mut data = AdminData::default();
        data.cities.insert(
            "3501".into(),
            CityOverrides {
                added_corridors: vec![
                    CustomCorridor {
                        id: "override-r1".into(),
                        kor: "S1".into(),
                        source_kor: Some("R1".into()),
                        route_name: "Rute R1 Edited".into(),
                        via: Some("via Tunjungan".into()),
                        origin: "Origin A".into(),
                        toward: "Toward B".into(),
                        jam_operasional: Some("05:00-22:00".into()),
                        color: "#ff0000".into(),
                        pref: "3501".into(),
                        points_a: None,
                        points_b: Some("encoded_b".into()),
                    },
                    CustomCorridor {
                        id: "custom-k1".into(),
                        kor: "K1".into(),
                        source_kor: None,
                        route_name: "Koridor Baru".into(),
                        via: None,
                        origin: "Alpha".into(),
                        toward: "Beta".into(),
                        jam_operasional: None,
                        color: "#123456".into(),
                        pref: "3501".into(),
                        points_a: Some("abc".into()),
                        points_b: None,
                    },
                ],
                removed_corridors: vec!["FD2".into()],
                corridor_order: vec!["K1".into(), "S1".into()],
                added_stops: vec![],
                removed_stops: vec![],
                added_buses: vec![],
                welcome_popup: None,
            },
        );

        let mut payload = json!({
            "data": [
                {
                    "id": "1",
                    "kor": "R1",
                    "route_name": "Original",
                    "color": "#111111",
                    "points_a": "orig_a",
                    "points_b": "orig_b"
                },
                {
                    "id": "2",
                    "kor": "FD2",
                    "route_name": "Hidden Corridor",
                    "color": "#222222"
                }
            ]
        });

        apply_corridor_overrides(&mut payload, &data, "3501");
        let list = payload["data"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["kor"], "K1");
        assert_eq!(list[0]["sort_order"], 0);
        assert_eq!(list[1]["kor"], "S1");
        assert_eq!(list[1]["source_kor"], "R1");
        assert_eq!(list[1]["route_name"], "Rute R1 Edited");
        assert_eq!(list[1]["via"], "via Tunjungan");
        assert_eq!(list[1]["points_a"], "orig_a");
        assert_eq!(list[1]["points_b"], "encoded_b");
        assert_eq!(list[1]["sort_order"], 1);
    }

    #[test]
    fn stop_overrides_add_hide_and_rename() {
        let mut data = AdminData::default();
        data.cities.insert(
            "1".into(),
            CityOverrides {
                added_corridors: vec![CustomCorridor {
                    id: "edit-1a".into(),
                    kor: "1X".into(),
                    source_kor: Some("1A".into()),
                    route_name: "Renamed 1A".into(),
                    via: None,
                    origin: String::new(),
                    toward: String::new(),
                    jam_operasional: None,
                    color: "#ff0000".into(),
                    pref: "1".into(),
                    points_a: None,
                    points_b: None,
                }],
                removed_corridors: vec![],
                corridor_order: vec![],
                added_stops: vec![CustomStop {
                    sh_id: "custom-1".into(),
                    sh_name: "Halte Baru".into(),
                    sh_lat: -7.79,
                    sh_lng: 110.36,
                    kor: "1X".into(),
                    in_koridor: "1X|2A".into(),
                    color_koridor: "#ff0000|#00ff00".into(),
                    color: "#ff0000".into(),
                    toward: "Prambanan".into(),
                }],
                removed_stops: vec!["00100".into()],
                added_buses: vec![],
                welcome_popup: None,
            },
        );

        let mut payload = json!({
            "data": [
                { "sh_id": "00100", "sh_name": "Old Stop", "kor": "1A", "in_koridor": "1A" },
                { "sh_id": "00101", "sh_name": "Keep Stop", "kor": "1A", "in_koridor": "1A|3A" }
            ]
        });

        apply_stop_overrides(&mut payload, &data, "1");
        let list = payload["data"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["sh_id"], "00101");
        assert_eq!(list[0]["kor"], "1X");
        assert_eq!(list[0]["in_koridor"], "1X|3A");
        assert_eq!(list[1]["sh_id"], "custom-1");
    }

    #[tokio::test]
    async fn reset_city_scope_clears_selected_categories_only() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("suroboyo-reset-test-{nanos}.json"));
        let store = AdminStore::load(path.clone()).await;

        store
            .upsert_corridor(
                "3501",
                CustomCorridor {
                    id: "c1".into(),
                    kor: "AF1".into(),
                    source_kor: None,
                    route_name: "Test".into(),
                    via: None,
                    origin: "A".into(),
                    toward: "B".into(),
                    jam_operasional: None,
                    color: "#ff0000".into(),
                    pref: "3501".into(),
                    points_a: None,
                    points_b: None,
                },
            )
            .await
            .unwrap();
        store
            .upsert_stop(
                "3501",
                CustomStop {
                    sh_id: "s1".into(),
                    sh_name: "Stop 1".into(),
                    sh_lat: -7.25,
                    sh_lng: 112.75,
                    kor: "AF1".into(),
                    in_koridor: "AF1".into(),
                    color_koridor: "#ff0000".into(),
                    color: "#ff0000".into(),
                    toward: "B".into(),
                },
            )
            .await
            .unwrap();
        store
            .upsert_bus(
                "3501",
                CustomBus {
                    id: "b1".into(),
                    name: "Bus 1".into(),
                    plate: None,
                    corridor: "AF1".into(),
                    toward: None,
                    lat: -7.25,
                    lng: 112.75,
                    heading: None,
                    speed: None,
                    passengers: None,
                },
            )
            .await
            .unwrap();

        let (ov, removed) = store.reset_city_scope("3501", "buses").await.unwrap();
        assert_eq!(removed, vec!["b1"]);
        assert!(ov.added_buses.is_empty());
        assert_eq!(ov.added_corridors.len(), 1);
        assert_eq!(ov.added_stops.len(), 1);

        let (ov, removed) = store.reset_city_scope("3501", "stops").await.unwrap();
        assert!(removed.is_empty());
        assert!(ov.added_stops.is_empty());
        assert_eq!(ov.added_corridors.len(), 1);

        let (ov, _) = store.reset_city_scope("3501", "routes").await.unwrap();
        assert!(ov.added_corridors.is_empty());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn leg_stop_overrides_preserve_and_patch_multi_corridor_hub() {
        let mut data = AdminData::default();
        data.cities.insert(
            "1".into(),
            CityOverrides {
                added_stops: vec![
                    CustomStop {
                        sh_id: "0044688".into(),
                        sh_name: "Ngabean - Transit".into(),
                        sh_lat: -7.803712,
                        sh_lng: 110.356239,
                        kor: "10".into(),
                        in_koridor: "10|1B".into(),
                        color_koridor: "#e6ac00|#f4a2a6".into(),
                        color: "#e6ac00".into(),
                        toward: "Kusumanegara".into(),
                    },
                    CustomStop {
                        sh_id: "0009999".into(),
                        sh_name: "Detached Stop".into(),
                        sh_lat: -7.8,
                        sh_lng: 110.36,
                        kor: "2B".into(),
                        in_koridor: "2B".into(),
                        color_koridor: "#a0cfeb".into(),
                        color: "#a0cfeb".into(),
                        toward: "Halte Ngabean".into(),
                    },
                    CustomStop {
                        sh_id: "custom-new-1b".into(),
                        sh_name: "Custom New 1B".into(),
                        sh_lat: -7.79,
                        sh_lng: 110.38,
                        kor: "1B".into(),
                        in_koridor: "1B".into(),
                        color_koridor: "#f4a2a6".into(),
                        color: "#f4a2a6".into(),
                        toward: "Halte Ngabean".into(),
                    },
                ],
                ..Default::default()
            },
        );

        let mut leg_to_ngabean = json!({
            "data": [
                { "sh_id": "0009999", "sh_name": "Detached Stop", "kor": "1B", "toward": "Halte Ngabean" },
                { "sh_id": "0044688", "sh_name": "Ngabean", "sh_lat": "-7.803712", "sh_lng": "110.356239", "kor": "1B", "in_koridor": "10|1B", "toward": "Halte Ngabean" }
            ]
        });
        apply_leg_stop_overrides(&mut leg_to_ngabean, &data, "1", "1B", "Halte Ngabean");
        let list_a = leg_to_ngabean["data"].as_array().unwrap();
        assert_eq!(list_a.len(), 2);
        assert_eq!(list_a[0]["sh_id"], "0044688");
        assert_eq!(list_a[0]["sh_name"], "Ngabean - Transit");
        assert_eq!(list_a[0]["kor"], "1B");
        assert_eq!(list_a[0]["toward"], "Halte Ngabean");
        assert_eq!(list_a[1]["sh_id"], "custom-new-1b");

        let mut leg_from_ngabean = json!({
            "data": [
                { "sh_id": "0044688", "sh_name": "Ngabean", "sh_lat": "-7.803712", "sh_lng": "110.356239", "kor": "1B", "in_koridor": "10|1B", "toward": "Bandara Adi Sutjipto" }
            ]
        });
        apply_leg_stop_overrides(&mut leg_from_ngabean, &data, "1", "1B", "Bandara Adi Sutjipto");
        let list_b = leg_from_ngabean["data"].as_array().unwrap();
        assert_eq!(list_b.len(), 1);
        assert_eq!(list_b[0]["sh_id"], "0044688");
        assert_eq!(list_b[0]["sh_name"], "Ngabean - Transit");
    }

    #[tokio::test]
    async fn menu_links_save_and_reject_unsafe_urls() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("suroboyo-menu-test-{nanos}.json"));
        let store = AdminStore::load(path.clone()).await;

        assert!(store.menu_links().await.is_empty());
        assert!(store
            .set_menu_links(vec![MenuLink {
                enabled: true,
                label: "XSS".into(),
                url: "javascript:alert(1)".into(),
            }])
            .await
            .is_err());

        let saved = store
            .set_menu_links(vec![
                MenuLink {
                    enabled: true,
                    label: " Info Tarif ".into(),
                    url: " https://example.com/tarif ".into(),
                },
                MenuLink {
                    enabled: false,
                    label: "Cadangan".into(),
                    url: "/rute".into(),
                },
                MenuLink {
                    enabled: true,
                    label: "Bantuan".into(),
                    url: "https://example.com/bantuan".into(),
                },
            ])
            .await
            .unwrap();
        assert_eq!(saved.len(), 3);
        assert_eq!(saved[0].label, "Info Tarif");
        assert_eq!(saved[0].url, "https://example.com/tarif");
        assert!(saved[0].enabled);
        assert_eq!(saved[2].label, "Bantuan");

        let reloaded = AdminStore::load(path.clone()).await;
        assert_eq!(reloaded.menu_links().await, saved);
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn city_popup_is_isolated_per_city_and_survives_transit_reset() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("suroboyo-popup-test-{nanos}.json"));
        let store = AdminStore::load(path.clone()).await;

        let sby = store
            .set_city_popup(
                "3501",
                CityPopup {
                    enabled: true,
                    title: "Pengumuman Surabaya".into(),
                    text: "Layanan normal".into(),
                    image_url: "https://example.com/sby.png".into(),
                },
            )
            .await
            .unwrap();
        let yk = store
            .set_city_popup(
                "1",
                CityPopup {
                    enabled: false,
                    title: String::new(),
                    text: "Info Jogja".into(),
                    image_url: String::new(),
                },
            )
            .await
            .unwrap();

        assert_eq!(store.city_popup("3501").await, sby);
        assert_eq!(store.city_popup("1").await, yk);
        assert_ne!(store.city_popup("3501").await, store.city_popup("1").await);

        store.reset_city_scope("3501", "all").await.unwrap();
        assert_eq!(store.city_popup("3501").await, sby);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn extracts_gist_inline_and_truncated_raw_url() {
        let inline_payload = serde_json::json!({
            "files": {
                "admin_overrides.json": {
                    "truncated": false,
                    "content": "{\"menu_links\":[]}",
                    "raw_url": "https://gist.githubusercontent.com/example/raw"
                }
            }
        });
        assert_eq!(
            extract_gist_content(&inline_payload, "admin_overrides.json"),
            Some(GistContent::Inline("{\"menu_links\":[]}".into()))
        );

        let truncated_payload = serde_json::json!({
            "files": {
                "admin_overrides.json": {
                    "truncated": true,
                    "content": "partial",
                    "raw_url": "https://gist.githubusercontent.com/example/raw-full"
                }
            }
        });
        assert_eq!(
            extract_gist_content(&truncated_payload, "admin_overrides.json"),
            Some(GistContent::RawUrl(
                "https://gist.githubusercontent.com/example/raw-full".into()
            ))
        );

        assert_eq!(
            extract_gist_content(&truncated_payload, "missing.json"),
            None
        );
    }
}
