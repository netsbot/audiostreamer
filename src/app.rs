use clap::Parser;

use crate::audio::sink::AudioSink;
use crate::audio::source::AudioSource;
use crate::audio::stream::{pump_stream, StreamStats};
use crate::cli::CliArgs;
use crate::client::AppleMusicClient;
use crate::config::{AppConfig, OutputSelection};
use crate::error::Result;
use crate::sinks::noop::NoopSink;
use crate::sinks::playback::PlaybackSink;
use crate::sources::song::Song;

// pub async fn run_from_args() -> Result<()> {
//     let cli = CliArgs::parse();
//     run_from_cli(cli).await
// }
//
// pub async fn run_from_cli(cli: CliArgs) -> Result<()> {
//     let config = AppConfig::from_cli(cli)?;
//     run(config).await
// }

pub async fn run() -> Result<()> {
    let client = AppleMusicClient::new().await.unwrap();
    let mut source = Song::new("635770202", client).await.unwrap();

    let mut sink = PlaybackSink::new();

    log::info!(
        "starting stream source={} sink={} chunk_size={}",
        source.description(),
        sink.description(),
        1024
    );

    pump_stream(&mut source, &mut sink, 1024).await?;

    Ok(())
}

fn create_sink(output: OutputSelection) -> Box<dyn AudioSink + Send> {
    match output {
        OutputSelection::Playback => Box::new(PlaybackSink::new()),
        OutputSelection::Noop => Box::new(NoopSink::new()),
    }
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
