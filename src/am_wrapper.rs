use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, ORIGIN, USER_AGENT};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UnixStream};

use crate::error::{Result, StreamerError};

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
    let base_url = "https://music.apple.com";

    let homepage_html = reqwest::get(base_url).await?.text().await?;

    let js_re = Regex::new(r"/assets/index~[^/]+\.js")
        .map_err(|e| StreamerError::Message(format!("failed to compile index JS regex: {e}")))?;
    let index_js_uri = js_re
        .find(&homepage_html)
        .ok_or_else(|| StreamerError::Message("Could not find index JS URI".to_string()))?
        .as_str();

    let js_url = format!("{}{}", base_url, index_js_uri);
    let js_content = reqwest::get(&js_url).await?.text().await?;

    let token_re = Regex::new(r#"eyJh([^\"]*)"#)
        .map_err(|e| StreamerError::Message(format!("failed to compile token regex: {e}")))?;
    let token = token_re
        .find(&js_content)
        .ok_or_else(|| StreamerError::Message("Could not find token in JS".to_string()))?
        .as_str()
        .to_string();

    Ok(token)
}

pub async fn get_music_token() -> Result<String> {
    let mut buf = String::new();
    let mut file = File::open("wrapper/rootfs/data/data/com.apple.android.music/files/MUSIC_TOKEN").await?;

    file.read_to_end(unsafe { buf.as_mut_vec() }).await?;

    Ok(buf)

    // Ok("Aly9pn/PmymqF/e2kTZXuxhm9th/nZj4rIkygNL+yQDE7kcxxmlUJPTG0HA+7B94LYPRixtnssl817gqrCRccDuh0hmGP9R39ZQPu2mCW9waESN01cSi7ScDlwbULqqIbfFRhO61ORi/rKKZ2YajW3M06ZHhyufbrdqZpx0h7UAVsE+1tYe/nxFcM1mxA8tb4scVzu3hCoO1xab6fMTRNQPiZD9WG/8vSWwlfnvu+qpY4Tceeg==".to_string())
}

pub async fn get_lyrics(
    adam_id: &str,
    region: &str,
    language: &str,
) -> Result<String> {
    let url = format!(
        "https://amp-api.music.apple.com/v1/catalog/{}/songs/{}/syllable-lyrics?l[lyrics]={}&extend=ttmlLocalizations&l[script]=en-Latn",
        region, adam_id, language
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Music/5.7 Android/10 model/Pixel6GR1YH build/1234 (dt:66)"),
    );
    headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", get_token().await?))?);
    headers.insert("media-user-token", HeaderValue::from_str(get_music_token().await?.as_str())?);
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
