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
) -> Result<StreamStats> {
    let mut stats = StreamStats::default();

    while let Some(chunk) = source.next_chunk(chunk_size).await? {
        stats.chunks += 1;
        stats.samples += chunk.samples.len();
        sink.write_chunk(&chunk).await?;
    }

    sink.close().await?;
    Ok(stats)
}