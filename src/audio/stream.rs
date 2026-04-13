use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use crate::audio::sink::AudioSink;
use crate::audio::source::AudioSource;
use crate::error::Result;

#[derive(Debug, Clone, Copy, Default)]
pub struct StreamStats {
    pub chunks: usize,
    pub samples: usize,
}

pub async fn pump_stream(
    source: &mut dyn AudioSource,
    sink: &mut dyn AudioSink,
    chunk_size: usize,
    paused: Arc<AtomicBool>,
) -> Result<StreamStats> {
    let mut stats = StreamStats::default();

    loop {
        if paused.load(Ordering::Relaxed) {
            sink.pause().await?;
            while paused.load(Ordering::Relaxed) {
                tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
            }
            sink.resume().await?;
        }

        match source.next_chunk(chunk_size).await? {
            Some(chunk) => {
                stats.chunks += 1;
                stats.samples += chunk.samples.len();
                sink.write_chunk(&chunk).await?;
            }
            None => break,
        }
    }

    sink.close().await?;
    Ok(stats)
}
