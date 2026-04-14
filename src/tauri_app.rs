use log::LevelFilter;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Cef, Emitter, Manager, State};
use tokio::sync::Mutex;
use tokio::time::Duration;

use crate::sinks::playback::PlaybackControls;

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct PlaySongMetadata {
    title: String,
    artist: String,
    album: String,
    artwork_url: Option<String>,
    duration_ms: Option<u64>,
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
}

impl AppState {
    fn new(app_handle: AppHandle<Cef>) -> Self {
        Self {
            app_handle,
            runtime: Arc::new(Mutex::new(PlaybackRuntime::new())),
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
) -> Result<(), String> {
    let (paused, frames_played, output_sample_rate, output_channels, active_sink, generation) = {
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
        runtime.paused = Arc::new(AtomicBool::new(false));
        runtime.frames_played = Arc::new(AtomicU64::new(0));
        runtime.output_sample_rate = Arc::new(AtomicU64::new(44_100));
        runtime.output_channels = Arc::new(AtomicU64::new(2));
        runtime.active_sink = Arc::new(Mutex::new(None));

        (
            runtime.paused.clone(),
            runtime.frames_played.clone(),
            runtime.output_sample_rate.clone(),
            runtime.output_channels.clone(),
            runtime.active_sink.clone(),
            runtime.playback_generation,
        )
    };

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
                if let Err(error) = app_handle_for_playback.emit("playback-progress", final_payload) {
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
    crate::client::AppleMusicClient::fetch_token()
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
async fn play_song(
    state: State<'_, AppState>,
    request: PlaySongRequest,
) -> Result<(), String> {
    start_playback(
        state.runtime.clone(),
        state.app_handle.clone(),
        request.adam_id,
        request.metadata,
        0.0,
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
        paused.store(true, Ordering::SeqCst);
        Ok(false)
    } else {
        if let Some(control) = control {
            control.resume().map_err(|error| error.to_string())?;
        }
        paused.store(false, Ordering::SeqCst);
        Ok(true)
    }
}

#[tauri::command]
async fn seek(state: State<'_, AppState>, seconds: f64) -> Result<(), String> {
    let runtime = state.runtime.clone();
    let (adam_id, metadata) = {
        let mut runtime = runtime.lock().await;

        let adam_id = runtime
            .adam_id
            .clone()
            .ok_or_else(|| "no active playback session".to_string())?;
        let metadata = runtime
            .metadata
            .clone()
            .ok_or_else(|| "no active playback metadata".to_string())?;

        if let Some(handle) = runtime.playback_task.take() {
            handle.abort();
        }
        if let Some(handle) = runtime.progress_task.take() {
            handle.abort();
        }

        runtime.paused.store(false, Ordering::SeqCst);
        runtime.frames_played.store(0, Ordering::SeqCst);
        runtime.output_sample_rate.store(44_100, Ordering::SeqCst);
        runtime.output_channels.store(2, Ordering::SeqCst);
        runtime.active_sink = Arc::new(Mutex::new(None));

        (adam_id, metadata)
    };

    start_playback(runtime, state.app_handle.clone(), adam_id, metadata, seconds).await
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(AppState::new(app.handle().clone()));

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(LevelFilter::Info)
                        .build(),
                )?;
            }
            app.handle().plugin(tauri_plugin_http::init())?;
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
