use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, ORIGIN, USER_AGENT};
use reqwest::Url;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UnixStream};

use crate::error::{Result, StreamerError};

pub const DEFAULT_LYRICS_REGION: &str = "vn";
pub const DEFAULT_LYRICS_LANGUAGE: &str = "en-gb";

pub async fn get_m3u8(adam_id: String) -> Result<String> {
    let mut m3u8_socket = TcpStream::connect("localhost:20020").await?;

    m3u8_socket.write_all(&[adam_id.len() as u8]).await?;
    m3u8_socket.write_all(adam_id.as_bytes()).await?;

    let mut buffer = vec![0u8; 1024];
    m3u8_socket.read(buffer.as_mut_slice()).await?;

    String::from_utf8(buffer)
        .map_err(|e| StreamerError::Message(format!("invalid UTF-8 in m3u8 response: {e}")))
}

pub async fn get_token() -> Result<String> {
    crate::client::AppleMusicClient::fetch_token().await
}

pub async fn get_music_token() -> Result<String> {
    static CACHE: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();

    CACHE.get_or_try_init(|| async {
        log::info!("Loading Music User Token from disk...");
        let mut buf = String::new();
        let mut file =
            File::open("wrapper/rootfs/data/data/com.apple.android.music/files/MUSIC_TOKEN").await?;

        file.read_to_string(&mut buf).await?;
        Ok(buf.trim().to_string())
    }).await.cloned()
}

pub async fn get_lyrics(adam_id: &str, region: &str, language: &str) -> Result<String> {
    let mut url = Url::parse(&format!(
        "https://amp-api.music.apple.com/v1/catalog/{}/songs/{}/syllable-lyrics",
        region, adam_id
    ))
    .map_err(|error| StreamerError::Message(format!("invalid lyrics URL for {adam_id}: {error}")))?;
    url.query_pairs_mut()
        .append_pair("l[lyrics]", language)
        .append_pair("l[script]", "en-Latn")
        .append_pair("extend", "ttmlLocalizations");

    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Music/5.7 Android/10 model/Pixel6GR1YH build/1234 (dt:66)"),
    );
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", get_token().await?))?,
    );
    headers.insert(
        "media-user-token",
        HeaderValue::from_str(get_music_token().await?.as_str())?,
    );
    headers.insert(ORIGIN, HeaderValue::from_static("https://music.apple.com"));

    let resp = reqwest::Client::new()
        .get(url)
        .headers(headers)
        .send()
        .await?;

    Ok(resp.text().await?)
}

pub async fn setup_context(
    stream: &mut UnixStream,
    current_state_key: &mut String,
    adam_id: &str,
    new_key: &str,
) -> Result<()> {
    // If we had a previous key, send the 4-byte reset signal
    if !current_state_key.is_empty() {
        stream.write_all(&[0, 0, 0, 0]).await?;
    }

    // Write AdamID: [u8 len][bytes]
    stream.write_u8(adam_id.len() as u8).await?;
    stream.write_all(adam_id.as_bytes()).await?;

    // Write Key: [u8 len][bytes]
    stream.write_u8(new_key.len() as u8).await?;
    stream.write_all(new_key.as_bytes()).await?;

    // Update state
    *current_state_key = new_key.to_string();
    Ok(())
}

pub async fn decrypt_sample(
    stream: &mut UnixStream,
    data: &[u8],
    output: &mut Vec<u8>,
) -> Result<()> {
    stream.write_u32_le(data.len() as u32).await?;
    stream.write_all(data).await?;
    output.resize(data.len(), 0);
    stream.read_exact(output.as_mut_slice()).await?;
    Ok(())
}
