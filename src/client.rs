use m3u8_rs::ByteRange;
use reqwest::header;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::Semaphore;
use tokio::sync::RwLock;

use crate::error::{Result, StreamerError};
use crate::disk_cache::DiskCache;

const BYTE_RANGE_CACHE_MAX_ENTRIES: usize = 2048;
const BYTE_RANGE_CACHE_MAX_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Default)]
struct ByteRangeCache {
    map: HashMap<String, Arc<Vec<u8>>>,
    lru: VecDeque<String>,
    total_bytes: usize,
}

impl ByteRangeCache {
    fn get(&mut self, key: &str) -> Option<Arc<Vec<u8>>> {
        let value = self.map.get(key)?.clone();
        self.touch(key);
        Some(value)
    }

    fn insert(&mut self, key: String, value: Arc<Vec<u8>>) {
        if let Some(prev) = self.map.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(prev.len());
            self.remove_key_from_lru(&key);
        }

        self.total_bytes = self.total_bytes.saturating_add(value.len());
        self.map.insert(key.clone(), value);
        self.lru.push_back(key);
        self.evict_if_needed();
    }

    fn touch(&mut self, key: &str) {
        self.remove_key_from_lru(key);
        self.lru.push_back(key.to_string());
    }

    fn remove_key_from_lru(&mut self, key: &str) {
        if let Some(pos) = self.lru.iter().position(|k| k == key) {
            self.lru.remove(pos);
        }
    }

    fn evict_if_needed(&mut self) {
        while self.map.len() > BYTE_RANGE_CACHE_MAX_ENTRIES
            || self.total_bytes > BYTE_RANGE_CACHE_MAX_BYTES
        {
            let Some(oldest_key) = self.lru.pop_front() else {
                break;
            };
            if let Some(removed) = self.map.remove(&oldest_key) {
                self.total_bytes = self.total_bytes.saturating_sub(removed.len());
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppleMusicClient {
    client: reqwest::Client,
    request_lock: Arc<Semaphore>,
}

fn byte_range_cache() -> &'static RwLock<ByteRangeCache> {
    static CACHE: OnceLock<RwLock<ByteRangeCache>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(ByteRangeCache::default()))
}

pub fn disk_cache() -> &'static RwLock<Option<DiskCache>> {
    static CACHE: OnceLock<RwLock<Option<DiskCache>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(None))
}

pub async fn init_disk_cache(cache: DiskCache) {
    let mut w = disk_cache().write().await;
    *w = Some(cache);
}

impl AppleMusicClient {
    pub async fn new() -> Result<Self> {
        let token = Self::fetch_token().await?;
        let builder = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .default_headers({
                let mut headers = header::HeaderMap::new();
                headers.insert(header::ORIGIN, header::HeaderValue::from_static("https://music.apple.com"));
                headers.insert(header::AUTHORIZATION, header::HeaderValue::from_str(&format!("Bearer {token}"))?);
                headers.insert(
                    "media-user-token",
                    header::HeaderValue::from_str(crate::am_wrapper::get_music_token().await?.as_str())?,
                );
                headers
            });

        let client = builder.build()?;
        Ok(Self {
            client,
            request_lock: Arc::new(Semaphore::new(16)),
        })
    }

    pub async fn download_m3u8(&self, m3u8_url: &str) -> crate::error::Result<String> {
        let _permit = self.request_lock.acquire().await?;
        let resp = self.client.get(m3u8_url).send().await?;
        Ok(resp.text().await?)
    }

    pub async fn download_byte_range(
        &self,
        url: &str,
        byte_range: ByteRange,
    ) -> crate::error::Result<Vec<u8>> {
        let range_start = byte_range.offset.unwrap_or_default();
        let range_end = range_start.saturating_add(byte_range.length.saturating_sub(1));
        let cache_key = format!("{}#{}-{}", url, range_start, range_end);

        {
            let mut cache = byte_range_cache().write().await;
            if let Some(cached) = cache.get(&cache_key) {
                log::debug!("byte-range cache hit: {}", cache_key);
                return Ok(cached.as_ref().clone());
            }
        }

        {
            let cache_lock = disk_cache().read().await;
            if let Some(disk) = cache_lock.as_ref() {
                if let Some(cached) = disk.get(url, range_start, range_end).await {
                    log::debug!("disk cache hit: {}", cache_key);
                    let mut mem_cache = byte_range_cache().write().await;
                    mem_cache.insert(cache_key.clone(), cached.clone());
                    return Ok(cached.as_ref().clone());
                }
            }
        }

        let _permit = self.request_lock.acquire().await?;
        let resp = self
            .client
            .get(url)
            .header(
                header::RANGE,
                format!(
                    "bytes={}-{}",
                    range_start,
                    range_end
                ),
            )
            .send()
            .await?;
        let bytes = resp.bytes().await?.to_vec();

        {
            let mut cache = byte_range_cache().write().await;
            cache.insert(cache_key.clone(), Arc::new(bytes.clone()));
        }

        {
            let cache_lock = disk_cache().read().await;
            if let Some(disk) = cache_lock.as_ref() {
                let arc_bytes = Arc::new(bytes.clone());
                disk.insert(url, range_start, range_end, arc_bytes).await;
            }
        }

        Ok(bytes)
    }

    pub fn prefetch_byte_range(&self, url: String, byte_range: ByteRange) {
        let client = self.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = client.download_byte_range(&url, byte_range).await {
                log::debug!("byte-range prefetch failed for {}: {}", url, error);
            }
        });
    }

    pub async fn songs(&self, query: &str) -> Result<String> {
        let resp = self.client.get(format!("https://api.music.apple.com/v1/catalog/us/search?term={query}&limit=25&types=songs,albums,artists")).send().await?;
        Ok(resp.text().await?)
    }

    pub async fn fetch_token() -> Result<String> {
        static CACHE: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();
        
        CACHE.get_or_try_init(|| async {
            log::info!("Fetching new Apple Music developer token...");
            let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
                .build()?;
            let resp = client
                .get("https://music.apple.com")
                .send()
                .await?
                .text()
                .await?;
            let index_js_uri = regex::Regex::new(r"/assets/index~[^/]+\.js")?
                .find(&resp)
                .ok_or_else(|| {
                    StreamerError::Message("unable to locate Apple Music index bundle".to_string())
                })?
                .as_str()
                .to_string();
            let js_resp = client
                .get(format!("https://music.apple.com{index_js_uri}"))
                .send()
                .await?
                .text()
                .await?;
            let token = regex::Regex::new(r#"eyJh([^\"]*)"#)?
                .find(&js_resp)
                .ok_or_else(|| {
                    StreamerError::Message("unable to extract Apple Music bearer token".to_string())
                })?
                .as_str()
                .to_string();
            Ok(token)
        }).await.cloned()
    }
}
