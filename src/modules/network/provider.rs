//! Network provider — public IP fetching and GeoIP lookups.
//!
//! Uses `reqwest` with configurable timeouts and an LRU cache
//! to avoid hammering the free ip-api.com endpoint.

use lru::LruCache;
use serde::Deserialize;
use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::Duration;

use crate::core::config::AppConfig;
use crate::core::store::Store;

// ---------------------------------------------------------------------------
// GeoIP response
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct GeoIpResponse {
    pub status: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub city: String,
}

/// Resolved location info.
#[derive(Debug, Clone)]
pub struct GeoInfo {
    pub country: String,
    pub city: String,
}

impl GeoInfo {
    pub fn display(&self) -> String {
        if self.city.is_empty() && self.country.is_empty() {
            "Unknown".to_string()
        } else if self.city.is_empty() {
            self.country.clone()
        } else {
            format!("{}, {}", self.country, self.city)
        }
    }
}

// ---------------------------------------------------------------------------
// GeoIP cache
// ---------------------------------------------------------------------------

/// Thread-safe LRU cache for GeoIP results.
pub struct GeoIpCache {
    cache: Mutex<LruCache<String, GeoInfo>>,
    url_template: String,
    timeout: Duration,
}

impl GeoIpCache {
    pub fn new(config: &AppConfig) -> Self {
        let cap = NonZeroUsize::new(config.network.geoip_cache_size.max(1)).unwrap();
        Self {
            cache: Mutex::new(LruCache::new(cap)),
            url_template: config.network.geoip_url_template.clone(),
            timeout: Duration::from_secs(config.network.request_timeout_secs),
        }
    }

    /// Look up the geographic location of an IP address.
    ///
    /// Returns a cached result if available, otherwise makes an HTTP request.
    pub async fn lookup(&self, ip: &str) -> GeoInfo {
        // Check cache first
        {
            let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(info) = cache.get(ip) {
                return info.clone();
            }
        }

        // Not cached — do the HTTP lookup
        let url = self.url_template.replace("{ip}", ip);
        let info = match reqwest::Client::new()
            .get(&url)
            .timeout(self.timeout)
            .send()
            .await
        {
            Ok(resp) => match resp.json::<GeoIpResponse>().await {
                Ok(geo) if geo.status == "success" => GeoInfo {
                    country: geo.country,
                    city: geo.city,
                },
                _ => GeoInfo {
                    country: String::new(),
                    city: String::new(),
                },
            },
            Err(_) => GeoInfo {
                country: String::new(),
                city: String::new(),
            },
        };

        // Cache the result
        {
            let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            cache.put(ip.to_string(), info.clone());
        }

        info
    }
}

// ---------------------------------------------------------------------------
// Public IP fetch
// ---------------------------------------------------------------------------

/// Fetch the host's public IP address. Writes directly to the store.
pub async fn fetch_public_ip(store: &Store, config: &AppConfig) {
    let timeout = Duration::from_secs(config.network.request_timeout_secs);
    let url = &config.network.public_ip_url;

    let result = reqwest::Client::new()
        .get(url)
        .timeout(timeout)
        .send()
        .await;

    let ip = match result {
        Ok(resp) => resp.text().await.unwrap_or_else(|_| "Error".into()),
        Err(_) => "Unavailable".into(),
    };

    store.write().await.public_ip = ip.trim().to_string();
}
