use crate::audio::sink::AudioSink;
use crate::audio::source::AudioSource;
use crate::audio::stream::pump_stream;
use crate::am_wrapper;
use crate::client::AppleMusicClient;
use crate::config::{AppConfig, PlaybackQuality};
use crate::error::Result;
use crate::sinks::playback::{PlaybackControls, PlaybackSink};
use crate::sources::song::Song;
use crate::cli::CliArgs;
use clap::Parser;

// pub async fn run_from_args() -> Result<()> {
//     let cli = CliArgs::parse();
//     run_from_cli(cli).await
// }
//
// pub async fn run_from_cli(cli: CliArgs) -> Result<()> {
//     let config = AppConfig::from_cli(cli)?;
//     run(config).await
// }

use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn execute_playback(
    adam_id: String,
    quality: PlaybackQuality,
    paused: Arc<AtomicBool>,
    samples_played: Arc<AtomicU64>,
    output_sample_rate: Arc<AtomicU64>,
    output_channels: Arc<AtomicU64>,
    active_sink: Arc<Mutex<Option<PlaybackControls>>>,
) -> Result<()> {
    execute_playback_at(
        adam_id,
        quality,
        paused,
        samples_played,
        output_sample_rate,
        output_channels,
        active_sink,
        0.0,
        None,
    )
    .await
}

pub async fn execute_playback_at(
    adam_id: String,
    quality: PlaybackQuality,
    paused: Arc<AtomicBool>,
    samples_played: Arc<AtomicU64>,
    output_sample_rate: Arc<AtomicU64>,
    output_channels: Arc<AtomicU64>,
    active_sink: Arc<Mutex<Option<PlaybackControls>>>,
    start_time: f64,
    existing_controls: Option<PlaybackControls>,
) -> Result<()> {
    let client = AppleMusicClient::new().await.map_err(|e| {
        crate::error::StreamerError::Message(format!("failed to build AppleMusicClient: {e}"))
    })?;

    let mut source = Song::new(&adam_id, client, quality).await?;
    if start_time > 0.0 {
        source.seek(start_time).await?;
    }

    let mut sink = if let Some(ref controls) = existing_controls {
        PlaybackSink::new_reusing(
            controls,
            samples_played.clone(),
            output_sample_rate,
            output_channels,
        )
    } else {
        PlaybackSink::new_with_metrics(
            samples_played.clone(),
            output_sample_rate,
            output_channels,
        )
    };
    let controls = sink.controls();
    
    // Register the sink for instant control
    {
        let mut active = active_sink.lock().await;
        *active = Some(controls);
    }

    log::info!(
        "starting stream at {}s source={} sink={} chunk_size={}",
        start_time,
        source.description(),
        sink.description(),
        2048
    );

    pump_stream(&mut source, &mut sink, 2048, paused).await?;
    
    // Unregister sink
    {
        let mut active = active_sink.lock().await;
        *active = None;
    }

    Ok(())
}

pub async fn run() -> Result<()> {
    let cli = CliArgs::parse();

    if cli.adam_id.is_some() {
        let config = AppConfig::from_cli(cli.clone())?;
        let adam_id = cli.adam_id.expect("checked is_some");
        am_wrapper::warm_up().await?;

        let paused = Arc::new(AtomicBool::new(false));
        let samples_played = Arc::new(AtomicU64::new(0));
        let output_sample_rate = Arc::new(AtomicU64::new(44_100));
        let output_channels = Arc::new(AtomicU64::new(2));
        let active_sink = Arc::new(Mutex::new(None));

        log::info!("Starting playback for Adam ID: {}", adam_id);
        
        execute_playback(
            adam_id,
            config.quality,
            paused,
            samples_played,
            output_sample_rate,
            output_channels,
            active_sink,
        ).await?;
    } else {
        println!("No Adam ID provided. Use --help for usage information.");
    }

    Ok(())
}

pub async fn preload_song(adam_id: String, quality: PlaybackQuality) -> Result<()> {
    let client = AppleMusicClient::new().await.map_err(|e| {
        crate::error::StreamerError::Message(format!("failed to build AppleMusicClient: {e}"))
    })?;

    // Constructing Song warms playlist/init caches and triggers segment prefetch.
    let song = Song::new(&adam_id, client, quality).await?;
    song.predownload_all_segments();
    Ok(())
}

// #[cfg(test)]
// mod tests {
//     use std::fs::File;
//
//     use crate::cli::CliOutputMode;
//
//     use super::*;
//
//     #[tokio::test]
//     async fn smoke_run_file_source_stub() {
//         let temp_dir = tempfile::tempdir().expect("temp dir");
//         let file_path = temp_dir.path().join("clip.wav");
//         File::create(&file_path).expect("create stub wav");
//
//         let cli = CliArgs {
//             config: None,
//             input: Some(file_path.to_string_lossy().to_string()),
//             output: Some(CliOutputMode::Noop),
//             chunk_size: Some(1024),
//         };
//
//         let stats = run_from_cli(cli).await.expect("run from cli");
//         assert_eq!(stats.chunks, 0);
//         assert_eq!(stats.samples, 0);
//     }
// }
