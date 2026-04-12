use m3u8_rs::ByteRange;
use reqwest::header;
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::error::{Result, StreamerError};

#[derive(Debug)]
pub struct AppleMusicClient {
    client: reqwest::Client,
    request_lock: Arc<Semaphore>,
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
        let _permit = self.request_lock.acquire().await?;
        let resp = self
            .client
            .get(url)
            .header(
                header::RANGE,
                format!(
                    "bytes={}-{}",
                    byte_range.offset.unwrap_or_default(),
                    byte_range.offset.unwrap_or_default() + byte_range.length - 1
                ),
            )
            .send()
            .await?;
        Ok(resp.bytes().await?.to_vec())
    }

    async fn fetch_token() -> Result<String> {
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
            .ok_or_else(|| StreamerError::Message("unable to locate Apple Music index bundle".to_string()))?
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
            .ok_or_else(|| StreamerError::Message("unable to extract Apple Music bearer token".to_string()))?
            .as_str()
            .to_string();
        Ok(token)
    }
}
