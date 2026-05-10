use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, ORIGIN, USER_AGENT};
use reqwest::Url;
use std::future::Future;
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UnixStream};
use tokio::sync::Mutex;

use crate::error::{Result, StreamerError};

pub const DEFAULT_LYRICS_REGION: &str = "vn";
pub const DEFAULT_LYRICS_LANGUAGE: &str = "en-gb";

fn wrapper_process() -> &'static Mutex<Option<Child>> {
    static WRAPPER_PROCESS: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
    WRAPPER_PROCESS.get_or_init(|| Mutex::new(None))
}

fn spawn_wrapper_process() -> std::io::Result<Child> {
    let wrapper_dir = env!("WRAPPER_DIR");
    Command::new(format!("{}/wrapper", wrapper_dir))
        .current_dir(wrapper_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
}

fn is_wrapper_retryable_error(error: &StreamerError) -> bool {
    match error {
        StreamerError::Io(io_error) => matches!(
            io_error.kind(),
            std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::ConnectionRefused
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::NotFound
                | std::io::ErrorKind::NotConnected
                | std::io::ErrorKind::TimedOut
                | std::io::ErrorKind::UnexpectedEof
        ),
        _ => false,
    }
}

pub async fn ensure_wrapper_running() -> Result<()> {
    let mut child_guard = wrapper_process().lock().await;

    let mut should_spawn = false;
    if let Some(child) = child_guard.as_mut() {
        match child.try_wait() {
            Ok(Some(status)) => {
                log::warn!("wrapper process exited with status {status}; restarting");
                *child_guard = None;
                should_spawn = true;
            }
            Ok(None) => {}
            Err(error) => {
                log::warn!("failed to check wrapper process state ({error}); restarting");
                *child_guard = None;
                should_spawn = true;
            }
        }
    } else {
        should_spawn = true;
    }

    if should_spawn {
        let child = spawn_wrapper_process()?;
        log::info!("wrapper process started");
        *child_guard = Some(child);
    }

    Ok(())
}

pub async fn restart_wrapper() -> Result<()> {
    let mut child_guard = wrapper_process().lock().await;

    if let Some(mut child) = child_guard.take() {
        if let Err(error) = child.kill() {
            log::warn!("failed to kill stale wrapper process: {error}");
        }
        let _ = child.wait();
    }

    let child = spawn_wrapper_process()?;
    log::info!("wrapper process restarted");
    *child_guard = Some(child);
    Ok(())
}

async fn wait_for_decrypt_socket_ready(sock_path: &str, wait_seconds: u64) -> std::result::Result<(), std::io::Error> {
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(wait_seconds);

    loop {
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(200),
            UnixStream::connect(sock_path),
        )
        .await
        {
            Ok(Ok(_stream)) => return Ok(()),
            Ok(Err(_)) | Err(_) => {}
        }

        if tokio::time::Instant::now() >= deadline {
            break;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "decrypt unix socket never became ready",
    ))
}

pub async fn warm_up() -> Result<()> {
    let sock_path = format!(
        "{}/rootfs/data/data/com.apple.android.music/files/decrypt.sock",
        env!("WRAPPER_DIR")
    );

    ensure_wrapper_running().await?;

    match wait_for_decrypt_socket_ready(&sock_path, 10).await {
        Ok(()) => Ok(()),
        Err(first_error) => {
            log::warn!(
                "wrapper warmup timed out ({first_error}); restarting wrapper once"
            );
            restart_wrapper().await?;
            wait_for_decrypt_socket_ready(&sock_path, 10)
                .await
                .map_err(StreamerError::from)
        }
    }
}

async fn run_with_wrapper_retry<T, F, Fut>(op_name: &str, mut op: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    ensure_wrapper_running().await?;

    match op().await {
        Ok(value) => Ok(value),
        Err(error) if is_wrapper_retryable_error(&error) => {
            log::warn!(
                "{op_name} failed due to wrapper IPC error ({error}); restarting wrapper and retrying once"
            );
            restart_wrapper().await?;
            op().await
        }
        Err(error) => Err(error),
    }
}

pub async fn get_m3u8(adam_id: String) -> Result<String> {
    run_with_wrapper_retry("get_m3u8", || {
        let adam_id = adam_id.clone();
        async move {
            let mut m3u8_socket = TcpStream::connect("localhost:20020").await?;

            m3u8_socket.write_all(&[adam_id.len() as u8]).await?;
            m3u8_socket.write_all(adam_id.as_bytes()).await?;

            let mut buffer = vec![0u8; 1024];
            let n = m3u8_socket.read(buffer.as_mut_slice()).await?;

            String::from_utf8(buffer[..n].to_vec()).map_err(|e| {
                StreamerError::Message(format!("invalid UTF-8 in m3u8 response: {e}"))
            })
        }
    })
    .await
}

pub async fn get_token() -> Result<String> {
    Ok(get_account_info().await?.dev_token)
}

#[derive(serde::Deserialize, Clone)]
pub struct AccountInfo {
    pub music_token: String,
    pub dev_token: String,
    pub storefront_id: String,
}

pub async fn get_account_info() -> Result<AccountInfo> {
    static CACHE: tokio::sync::OnceCell<AccountInfo> = tokio::sync::OnceCell::const_new();

    run_with_wrapper_retry("get_account_info", || async {
        CACHE
            .get_or_try_init(|| async {
                log::info!("Loading account info from wrapper socket...");
                let info = reqwest::get("http://localhost:30020")
                    .await?
                    .json::<AccountInfo>()
                    .await?;
                Ok(info)
            })
            .await
            .cloned()
    })
    .await
}

pub async fn get_music_token() -> Result<String> {
    Ok(get_account_info().await?.music_token)
}

pub async fn get_lyrics(adam_id: &str, region: &str, language: &str) -> Result<String> {
    let mut url = Url::parse(&format!(
        "https://amp-api.music.apple.com/v1/catalog/{}/songs/{}/syllable-lyrics",
        region, adam_id
    ))
    .map_err(|error| {
        StreamerError::Message(format!("invalid lyrics URL for {adam_id}: {error}"))
    })?;
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
    // If we had previous key, send 4-byte reset signal
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
