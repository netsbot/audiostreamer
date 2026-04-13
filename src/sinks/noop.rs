use async_trait::async_trait;

use crate::audio::sink::AudioSink;
use crate::audio::source::AudioChunk;
use crate::error::Result;

#[derive(Default)]
pub struct NoopSink {
    drained_samples: usize,
}

impl NoopSink {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AudioSink for NoopSink {
    async fn write_chunk(&mut self, chunk: &AudioChunk) -> Result<()> {
        self.drained_samples += chunk.samples.len();
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        log::info!("noop sink drained {} samples", self.drained_samples);
        Ok(())
    }

    fn description(&self) -> &str {
        "noop-sink"
    }
}
