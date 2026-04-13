use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use std::sync::{Arc, Mutex};
use tauri::Emitter;

pub struct MediaControlsManager {
    controls: Arc<Mutex<MediaControls>>,
}

impl MediaControlsManager {
    pub fn new<R: tauri::Runtime>(app_handle: tauri::AppHandle<R>) -> anyhow::Result<Self> {
        let config = PlatformConfig {
            dbus_name: "audiostreamer",
            display_name: "AudioStreamer",
            hwnd: None, // Not needed on Linux
        };

        let mut controls = MediaControls::new(config)
            .map_err(|e| anyhow::anyhow!("Failed to create media controls: {:?}", e))?;

        let app_handle_clone = app_handle.clone();
        controls
            .attach(move |event| {
                match event {
                    MediaControlEvent::Toggle | MediaControlEvent::Play | MediaControlEvent::Pause => {
                        let _ = app_handle_clone.emit("playback-toggle", ());
                    }
                    MediaControlEvent::Next => {
                        let _ = app_handle_clone.emit("playback-next", ());
                    }
                    MediaControlEvent::Previous => {
                        let _ = app_handle_clone.emit("playback-prev", ());
                    }
                    MediaControlEvent::Stop => {
                        let _ = app_handle_clone.emit("playback-stop", ());
                    }
                    _ => {}
                }
            })
            .map_err(|e| anyhow::anyhow!("Failed to attach media controls: {:?}", e))?;

        Ok(Self {
            controls: Arc::new(Mutex::new(controls)),
        })
    }

    pub fn set_metadata(&self, title: &str, artist: &str, album: &str, artwork_url: Option<&str>) -> anyhow::Result<()> {
        let mut controls = self.controls.lock().unwrap();
        controls
            .set_metadata(MediaMetadata {
                title: Some(title),
                artist: Some(artist),
                album: Some(album),
                cover_url: artwork_url,
                duration: None, // We'll add this later if possible
            })
            .map_err(|e| anyhow::anyhow!("Failed to set metadata: {:?}", e))
    }

    pub fn set_playback_status(&self, playing: bool) -> anyhow::Result<()> {
        let mut controls = self.controls.lock().unwrap();
        let status = if playing {
            MediaPlayback::Playing { progress: None }
        } else {
            MediaPlayback::Paused { progress: None }
        };
        controls
            .set_playback(status)
            .map_err(|e| anyhow::anyhow!("Failed to set playback status: {:?}", e))
    }
}
