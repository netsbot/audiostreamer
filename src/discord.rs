use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::tauri_app::PlaySongMetadata;

pub const DISCORD_CLIENT_ID: &str = "773825528921849856";

pub struct DiscordState {
    client: Arc<Mutex<DiscordIpcClient>>,
}

impl DiscordState {
    pub fn new() -> Self {
        let mut client = DiscordIpcClient::new(DISCORD_CLIENT_ID);
        
        // Use a background task to handle connection attempts
        let _ = client.connect();

        Self {
            client: Arc::new(Mutex::new(client)),
        }
    }

    pub async fn update_playback(
        &self,
        metadata: &PlaySongMetadata,
        current_time_secs: f64,
        is_paused: bool,
    ) {
        let mut client = self.client.lock().await;

        let truncate = |s: &str| -> String {
            let s = s.trim();
            if s.len() < 2 {
                return format!("{: <2}", s);
            }
            if s.len() > 128 {
                let mut truncated: String = s.chars().take(125).collect();
                truncated.push_str("...");
                return truncated;
            }
            s.to_string()
        };

        let title = truncate(&metadata.title);
        let artist = truncate(&metadata.artist);
        let album = truncate(&metadata.album);

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let position_ms = (current_time_secs * 1000.0) as i64;
        let start_time = now_ms - position_ms;

        let mut assets = activity::Assets::new().large_text(&album);
        if let Some(url) = &metadata.artwork_url {
            assets = assets.large_image(url);
        }

        if is_paused {
            assets = assets
                .small_image("https://img.icons8.com/ios-filled/50/ffffff/pause--v1.png")
                .small_text("Paused");
        }

        let spotify_query = format!("artist:{} track:{}", metadata.artist, metadata.title);
        let spotify_url = format!(
            "https://open.spotify.com/search/{}",
            urlencoding::encode(&spotify_query)
        );

        let mut payload = activity::Activity::new()
            .details(&title)
            .state(&artist)
            .assets(assets)
            .buttons(vec![activity::Button::new("Search on Spotify", &spotify_url)])
            .activity_type(activity::ActivityType::Listening);

        if !is_paused {
            let mut timestamps = activity::Timestamps::new().start(start_time);
            if let Some(duration_ms) = metadata.duration_ms {
                timestamps = timestamps.end(start_time + duration_ms as i64);
            }
            payload = payload.timestamps(timestamps);
        }

        if let Err(e) = client.set_activity(payload) {
            log::warn!("Failed to set discord activity: {:?}. Attempting reconnect...", e);
            let _ = client.connect();
        }
    }

    pub async fn clear(&self) {
        let mut client = self.client.lock().await;
        let _ = client.clear_activity();
    }
}
