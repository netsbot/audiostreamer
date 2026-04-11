use async_trait::async_trait;

use crate::audio::source::AudioChunk;
use crate::error::Result;

#[async_trait]
pub trait AudioSink: Send {
    async fn write_chunk(&mut self, chunk: &AudioChunk) -> Result<()>;
    async fn close(&mut self) -> Result<()>;
    fn description(&self) -> &str;
}