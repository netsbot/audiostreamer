use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiskCacheEntry {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DiskCacheIndex {
    pub entries: HashMap<String, DiskCacheEntry>,
    pub lru: VecDeque<String>,
    pub total_bytes: u64,
}

#[derive(Debug)]
pub struct DiskCache {
    base_dir: PathBuf,
    max_bytes: u64,
    index: RwLock<DiskCacheIndex>,
}

impl DiskCache {
    pub async fn new(base_dir: impl AsRef<Path>, max_bytes: u64) -> std::io::Result<Self> {
        let base_dir = base_dir.as_ref().to_path_buf();
        fs::create_dir_all(&base_dir).await?;

        let index_path = base_dir.join("index.json");
        let index = if index_path.exists() {
            match fs::read_to_string(&index_path).await {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => DiskCacheIndex::default(),
            }
        } else {
            DiskCacheIndex::default()
        };

        // Note: Could prune orphaned files here, but keeping it simple
        Ok(Self {
            base_dir,
            max_bytes,
            index: RwLock::new(index),
        })
    }

    fn hash_key(url: &str, start: u64, end: u64) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{}#{}-{}", url, start, end).as_bytes());
        let result = hasher.finalize();
        result.iter().map(|b| format!("{:02x}", b)).collect::<String>()
    }

    pub async fn get(&self, url: &str, start: u64, end: u64) -> Option<Arc<Vec<u8>>> {
        let key = Self::hash_key(url, start, end);
        let path = {
            let mut idx = self.index.write().await;
            if let Some(entry) = idx.entries.get(&key).cloned() {
                // Update LRU
                if let Some(pos) = idx.lru.iter().position(|k| k == &key) {
                    idx.lru.remove(pos);
                }
                idx.lru.push_back(key.clone());
                Some(entry.path)
            } else {
                None
            }
        };

        if let Some(p) = path {
            let full_path = self.base_dir.join(&p);
            match fs::read(&full_path).await {
                Ok(bytes) => Some(Arc::new(bytes)),
                Err(e) => {
                    log::warn!("Disk cache read failed for {}: {}", p, e);
                    // Remove from index
                    let mut idx = self.index.write().await;
                    if let Some(entry) = idx.entries.remove(&key) {
                        idx.total_bytes = idx.total_bytes.saturating_sub(entry.size);
                        if let Some(pos) = idx.lru.iter().position(|k| k == &key) {
                            idx.lru.remove(pos);
                        }
                    }
                    None
                }
            }
        } else {
            None
        }
    }

    pub async fn insert(&self, url: &str, start: u64, end: u64, data: Arc<Vec<u8>>) {
        let key = Self::hash_key(url, start, end);
        let filename = format!("{}.cache", key);
        let full_path = self.base_dir.join(&filename);
        let temp_path = self.base_dir.join(format!("{}.tmp", filename));
        let size = data.len() as u64;

        if let Err(e) = fs::write(&temp_path, data.as_ref()).await {
            log::warn!("Failed to write temp cache file {}: {}", temp_path.display(), e);
            return;
        }

        if let Err(e) = fs::rename(&temp_path, &full_path).await {
            log::warn!("Failed to rename cache file to {}: {}", full_path.display(), e);
            let _ = fs::remove_file(&temp_path).await;
            return;
        }

        let mut idx = self.index.write().await;
        if let Some(prev) = idx.entries.insert(key.clone(), DiskCacheEntry {
            path: filename.clone(),
            size,
        }) {
            idx.total_bytes = idx.total_bytes.saturating_sub(prev.size);
            if let Some(pos) = idx.lru.iter().position(|k| k == &key) {
                idx.lru.remove(pos);
            }
        }

        idx.total_bytes = idx.total_bytes.saturating_add(size);
        idx.lru.push_back(key.clone());

        // Evict if over max bytes
        while idx.total_bytes > self.max_bytes {
            if let Some(oldest_key) = idx.lru.pop_front() {
                if let Some(oldest_entry) = idx.entries.remove(&oldest_key) {
                    idx.total_bytes = idx.total_bytes.saturating_sub(oldest_entry.size);
                    let remove_path = self.base_dir.join(&oldest_entry.path);
                    tokio::spawn(async move {
                        let _ = fs::remove_file(remove_path).await;
                    });
                }
            } else {
                break;
            }
        }

        // Save index
        let index_path = self.base_dir.join("index.json");
        let index_temp = self.base_dir.join("index.json.tmp");
        if let Ok(json) = serde_json::to_string(&*idx) {
            tokio::spawn(async move {
                if fs::write(&index_temp, json).await.is_ok() {
                    let _ = fs::rename(&index_temp, &index_path).await;
                }
            });
        }
    }
}
