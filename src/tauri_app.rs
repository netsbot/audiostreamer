use crate::discord::DiscordState;
use log::LevelFilter;
use serde::{Deserialize, Serialize};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Cef, Emitter, Manager, State};
use tokio::sync::Mutex;
use tokio::time::Duration;

use crate::sinks::playback::PlaybackControls;

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct PlaySongMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub artwork_url: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PlaySongRequest {
    #[serde(rename = "adamId")]
    adam_id: String,
    metadata: PlaySongMetadata,
}

struct PlaybackRuntime {
    adam_id: Option<String>,
    metadata: Option<PlaySongMetadata>,
    playback_generation: u64,
    paused: Arc<AtomicBool>,
    frames_played: Arc<AtomicU64>,
    resume_emitted: Arc<AtomicBool>,
    output_sample_rate: Arc<AtomicU64>,
    output_channels: Arc<AtomicU64>,
    active_sink: Arc<Mutex<Option<PlaybackControls>>>,
    playback_task: Option<tauri::async_runtime::JoinHandle<()>>,
    progress_task: Option<tauri::async_runtime::JoinHandle<()>>,
}

impl PlaybackRuntime {
    fn new() -> Self {
        Self {
            adam_id: None,
            metadata: None,
            playback_generation: 0,
            paused: Arc::new(AtomicBool::new(false)),
            frames_played: Arc::new(AtomicU64::new(0)),
            resume_emitted: Arc::new(AtomicBool::new(false)),
            output_sample_rate: Arc::new(AtomicU64::new(44_100)),
            output_channels: Arc::new(AtomicU64::new(2)),
            active_sink: Arc::new(Mutex::new(None)),
            playback_task: None,
            progress_task: None,
        }
    }
}

struct AppState {
    app_handle: AppHandle<Cef>,
    runtime: Arc<Mutex<PlaybackRuntime>>,
    controls: Arc<Mutex<MediaControls>>,
    discord: Arc<DiscordState>,
}

impl AppState {
    fn new(app_handle: AppHandle<Cef>, controls: MediaControls) -> Self {
        Self {
            app_handle,
            runtime: Arc::new(Mutex::new(PlaybackRuntime::new())),
            controls: Arc::new(Mutex::new(controls)),
            discord: Arc::new(DiscordState::new()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct PlaybackProgressEvent {
    #[serde(rename = "currentTime")]
    current_time: f64,
    #[serde(rename = "totalTime")]
    total_time: f64,
    paused: bool,
    ended: bool,
}

async fn start_playback(
    runtime: Arc<Mutex<PlaybackRuntime>>,
    app_handle: AppHandle<Cef>,
    adam_id: String,
    metadata: PlaySongMetadata,
    start_time: f64,
    existing_controls: Option<PlaybackControls>,
) -> Result<(), String> {
    let (paused, frames_played, resume_emitted, output_sample_rate, output_channels, active_sink, generation) = {
        let mut runtime = runtime.lock().await;

        if let Some(handle) = runtime.playback_task.take() {
            handle.abort();
        }
        if let Some(handle) = runtime.progress_task.take() {
            handle.abort();
        }

        runtime.adam_id = Some(adam_id.clone());
        runtime.metadata = Some(metadata.clone());
        runtime.playback_generation = runtime.playback_generation.wrapping_add(1);

        if existing_controls.is_some() {
            // Seek: reuse same Arcs, just reset values
            runtime.paused.store(false, Ordering::SeqCst);
            runtime.frames_played.store(0, Ordering::SeqCst);
            runtime.resume_emitted.store(false, Ordering::SeqCst);
        } else {
            // Fresh play: new Arcs
            runtime.paused = Arc::new(AtomicBool::new(false));
            runtime.frames_played = Arc::new(AtomicU64::new(0));
            runtime.resume_emitted = Arc::new(AtomicBool::new(false));
            runtime.output_sample_rate = Arc::new(AtomicU64::new(44_100));
            runtime.output_channels = Arc::new(AtomicU64::new(2));
            runtime.active_sink = Arc::new(Mutex::new(None));
        }

        (
            runtime.paused.clone(),
            runtime.frames_played.clone(),
            runtime.resume_emitted.clone(),
            runtime.output_sample_rate.clone(),
            runtime.output_channels.clone(),
            runtime.active_sink.clone(),
            runtime.playback_generation,
        )
    };

    // Update MPRIS
    let state = app_handle.state::<AppState>();
    let mut controls = state.controls.lock().await;
    let _ = controls.set_metadata(MediaMetadata {
        title: Some(&metadata.title),
        artist: Some(&metadata.artist),
        album: Some(&metadata.album),
        cover_url: metadata.artwork_url.as_deref(),
        duration: metadata.duration_ms.map(|d| Duration::from_millis(d)),
    });
    let _ = controls.set_playback(MediaPlayback::Playing { progress: None });

    // Update Discord
    let discord = state.discord.clone();
    let metadata_for_discord = metadata.clone();
    tauri::async_runtime::spawn(async move {
        discord
            .update_playback(&metadata_for_discord, start_time, false)
            .await;
    });

    let runtime_for_task = runtime.clone();
    let adam_id_for_task = adam_id.clone();
    let app_handle_for_playback = app_handle.clone();
    let playback_task = tauri::async_runtime::spawn(async move {
        let total_time = metadata.duration_ms.unwrap_or(0) as f64 / 1000.0;
        let playback_result = crate::app::execute_playback_at(
            adam_id_for_task.clone(),
            paused,
            frames_played,
            output_sample_rate,
            output_channels,
            active_sink,
            start_time,
            existing_controls,
        )
        .await;

        if let Err(error) = &playback_result {
            log::error!("playback failed for {}: {}", adam_id_for_task, error);
        }

        let mut runtime = runtime_for_task.lock().await;
        if runtime.playback_generation == generation
            && runtime.adam_id.as_deref() == Some(adam_id_for_task.as_str())
        {
            if playback_result.is_ok() {
                let final_payload = PlaybackProgressEvent {
                    current_time: total_time,
                    total_time,
                    paused: true,
                    ended: true,
                };
                if let Err(error) = app_handle_for_playback.emit("playback-progress", final_payload)
                {
                    log::warn!("failed to emit final playback-progress: {}", error);
                }
            }
            runtime.playback_task = None;
            if let Some(handle) = runtime.progress_task.take() {
                handle.abort();
            }
        }
    });

    let runtime_for_progress = runtime.clone();
    let adam_id_for_progress = adam_id.clone();
    let app_handle_for_progress = app_handle.clone();
    let total_time = metadata.duration_ms.unwrap_or(0) as f64 / 1000.0;
    let progress_task = tauri::async_runtime::spawn(async move {
        loop {
            let (still_current, paused, frames, sample_rate) = {
                let runtime = runtime_for_progress.lock().await;
                (
                    runtime.playback_generation == generation
                        && runtime.adam_id.as_deref() == Some(adam_id_for_progress.as_str()),
                    runtime.paused.load(Ordering::SeqCst),
                    runtime.frames_played.load(Ordering::SeqCst),
                    runtime.output_sample_rate.load(Ordering::SeqCst).max(1),
                )
            };

            if !still_current {
                break;
            }

            // Emit playback-resumed once when first audio frames actually flow
            if frames > 0 && !resume_emitted.load(Ordering::SeqCst) {
                resume_emitted.store(true, Ordering::SeqCst);
                let current_time = start_time + (frames as f64 / sample_rate as f64);
                if let Err(error) = app_handle_for_progress.emit("playback-resumed", current_time) {
                    log::warn!("failed to emit playback-resumed: {}", error);
                }
            }

            let local_seconds = start_time + (frames as f64 / sample_rate as f64);
            let payload = PlaybackProgressEvent {
                current_time: local_seconds,
                total_time,
                paused,
                ended: false,
            };
            if let Err(error) = app_handle_for_progress.emit("playback-progress", payload) {
                log::warn!("failed to emit playback-progress: {}", error);
            }

            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    });

    let mut runtime = runtime.lock().await;
    runtime.playback_task = Some(playback_task);
    runtime.progress_task = Some(progress_task);
    Ok(())
}

#[tauri::command]
async fn run_stream() -> Result<(), String> {
    crate::app::run().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_apple_music_token() -> Result<String, String> {
    crate::am_wrapper::get_token()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_apple_music_user_token() -> Result<String, String> {
    crate::am_wrapper::get_music_token()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_lyrics_payload(adam_id: String) -> Result<String, String> {
    crate::am_wrapper::get_lyrics(
        &adam_id,
        crate::am_wrapper::DEFAULT_LYRICS_REGION,
        crate::am_wrapper::DEFAULT_LYRICS_LANGUAGE,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn preload_song(adam_id: String) -> Result<(), String> {
    crate::app::preload_song(adam_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn play_song(state: State<'_, AppState>, request: PlaySongRequest) -> Result<(), String> {
    start_playback(
        state.runtime.clone(),
        state.app_handle.clone(),
        request.adam_id,
        request.metadata,
        0.0,
        None,
    )
    .await
}

#[tauri::command]
async fn toggle_playback(state: State<'_, AppState>) -> Result<bool, String> {
    let runtime = state.runtime.clone();
    let (paused, active_sink) = {
        let runtime = runtime.lock().await;
        (runtime.paused.clone(), runtime.active_sink.clone())
    };

    let currently_playing = !paused.load(Ordering::SeqCst);
    let control = { active_sink.lock().await.clone() };

    if currently_playing {
        if let Some(control) = control {
            control.pause().map_err(|error| error.to_string())?;
        }
        let discord = state.discord.clone();
        let runtime = state.runtime.clone();
        tauri::async_runtime::spawn(async move {
            let runtime = runtime.lock().await;
            if let Some(metadata) = &runtime.metadata {
                let metadata = metadata.clone();
                let current_time = runtime.frames_played.load(Ordering::SeqCst) as f64
                    / runtime.output_sample_rate.load(Ordering::SeqCst).max(1) as f64;
                discord.update_playback(&metadata, current_time, true).await;
            }
        });

        paused.store(true, Ordering::SeqCst);
        let _ = state
            .controls
            .lock()
            .await
            .set_playback(MediaPlayback::Paused { progress: None });
        Ok(false)
    } else {
        if let Some(control) = control {
            control.resume().map_err(|error| error.to_string())?;
        }

        let discord = state.discord.clone();
        let runtime = state.runtime.clone();
        tauri::async_runtime::spawn(async move {
            let runtime = runtime.lock().await;
            if let Some(metadata) = &runtime.metadata {
                let metadata = metadata.clone();
                let current_time = runtime.frames_played.load(Ordering::SeqCst) as f64
                    / runtime.output_sample_rate.load(Ordering::SeqCst).max(1) as f64;
                discord
                    .update_playback(&metadata, current_time, false)
                    .await;
            }
        });

        paused.store(false, Ordering::SeqCst);
        let _ = state
            .controls
            .lock()
            .await
            .set_playback(MediaPlayback::Playing { progress: None });
        Ok(true)
    }
}

#[tauri::command]
async fn seek(state: State<'_, AppState>, seconds: f64) -> Result<(), String> {
    let runtime = state.runtime.clone();
    let (adam_id, metadata, existing_controls) = {
        let runtime = runtime.lock().await;

        let adam_id = runtime
            .adam_id
            .clone()
            .ok_or_else(|| "no active playback session".to_string())?;
        let metadata = runtime
            .metadata
            .clone()
            .ok_or_else(|| "no active playback metadata".to_string())?;

        // Grab existing CPAL controls before aborting task
        let controls = runtime.active_sink.lock().await.clone();

        // Clear stale audio from buffer
        if let Some(ref c) = controls {
            c.clear_buffer();
        }

        (adam_id, metadata, controls)
    };

    start_playback(
        runtime,
        state.app_handle.clone(),
        adam_id,
        metadata,
        seconds,
        existing_controls,
    )
    .await
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "linux")]
            let config = PlatformConfig {
                dbus_name: "com.netsbit.audiostreamer",
                display_name: "AudioStreamer",
                hwnd: None,
            };

            #[cfg(not(target_os = "linux"))]
            let config = PlatformConfig {
                dbus_name: "com.netsbit.audiostreamer",
                display_name: "AudioStreamer",
                hwnd: None, // Simplified for now
            };

            let mut controls = MediaControls::new(config).map_err(|e| e.to_string())?;
            let handle = app.handle().clone();
            controls
                .attach(move |event| {
                    let _ = match event {
                        MediaControlEvent::Play
                        | MediaControlEvent::Pause
                        | MediaControlEvent::Toggle => handle.emit("mpris-event", "toggle"),
                        MediaControlEvent::Next => handle.emit("mpris-event", "next"),
                        MediaControlEvent::Previous => handle.emit("mpris-event", "previous"),
                        MediaControlEvent::Stop => handle.emit("mpris-event", "stop"),
                        _ => Ok(()),
                    };
                })
                .map_err(|e| e.to_string())?;

            app.manage(AppState::new(app.handle().clone(), controls));

            let handle_clone = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                use tauri::Manager;
                if let Ok(data_dir) = handle_clone.path().app_data_dir() {
                    let cache_dir = data_dir.join("segment_cache");
                    if let Ok(disk_cache) = crate::disk_cache::DiskCache::new(cache_dir, 2_000_000_000).await {
                        crate::client::init_disk_cache(disk_cache).await;
                        log::info!("Initialized segment disk cache (max 2 GB)");
                    }
                }

                if let Err(error) = crate::am_wrapper::warm_up().await {
                    log::warn!("wrapper warmup failed: {}", error);
                } else {
                    log::info!("wrapper warmup complete");
                }
            });

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(LevelFilter::Info)
                        .build(),
                )?;
            }
            app.handle().plugin(tauri_plugin_http::init())?;
            app.handle().plugin(tauri_plugin_cache::init())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            run_stream,
            play_song,
            preload_song,
            toggle_playback,
            seek,
            get_apple_music_token,
            get_apple_music_user_token,
            get_lyrics_payload
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
