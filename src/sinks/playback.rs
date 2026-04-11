use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::audio::sink::AudioSink;
use crate::audio::source::{AudioChunk, SampleBuffer};
use crate::error::Result;
use crate::error::StreamerError;

struct SendStream(cpal::Stream);
unsafe impl Send for SendStream {}

pub struct PlaybackSink {
    stream: Option<SendStream>,
    buffer: Arc<Mutex<VecDeque<f32>>>,
    _config: Option<cpal::SupportedStreamConfig>,
}

unsafe impl Send for PlaybackSink {}

impl PlaybackSink {
    pub fn new() -> Self {
        Self {
            stream: None,
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            _config: None,
        }
    }

    fn ensure_stream(&mut self, chunk: &AudioChunk) -> Result<()> {
        if self.stream.is_some() {
            return Ok(());
        }

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| StreamerError::Message("No output device found".to_string()))?;

        let config = device
            .supported_output_configs()
            .map_err(|e| StreamerError::Message(format!("Failed to get supported configs: {}", e)))?
            .filter(|c| {
                c.channels() == chunk.channels
                    && c.min_sample_rate().0 <= chunk.sample_rate
                    && c.max_sample_rate().0 >= chunk.sample_rate
            })
            .next()
            .map(|c| c.with_sample_rate(cpal::SampleRate(chunk.sample_rate)))
            .unwrap_or_else(|| {
                log::warn!("Source rate {} not supported by device, falling back to default", chunk.sample_rate);
                device.default_output_config().unwrap()
            });

        log::info!(
            "Initializing playback stream: rate={}, channels={}, format={:?}",
            chunk.sample_rate,
            chunk.channels,
            config.sample_format()
        );

        let buffer = self.buffer.clone();

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                device.build_output_stream(
                    &config.into(),
                    move |data: &mut [f32], _| {
                        let mut buf = buffer.lock().unwrap();
                        for sample in data.iter_mut() {
                            *sample = buf.pop_front().unwrap_or(0.0);
                        }
                    },
                    |err| log::error!("Playback error: {}", err),
                    None,
                )
            }
            _ => {
                return Err(StreamerError::Message(format!(
                    "Unsupported system sample format: {:?}",
                    config.sample_format()
                )));
            }
        }.map_err(|e| StreamerError::Message(format!("Failed to build output stream: {}", e)))?;

        stream.play().map_err(|e| StreamerError::Message(format!("Failed to start stream: {}", e)))?;
        self.stream = Some(SendStream(stream));
        Ok(())
    }
}

#[async_trait]
impl AudioSink for PlaybackSink {
    async fn write_chunk(&mut self, chunk: &AudioChunk) -> Result<()> {
        self.ensure_stream(chunk)?;

        {
            let mut buf = self.buffer.lock().unwrap();
            match &chunk.samples {
                SampleBuffer::F32(v) => {
                    buf.extend(v);
                }
                SampleBuffer::I16(v) => {
                    buf.extend(v.iter().map(|&s| s as f32 / 32768.0));
                }
                SampleBuffer::I32(v) => {
                    buf.extend(v.iter().map(|&s| s as f32 / 2147483648.0));
                }
            }
        }

        // Wait a bit if the buffer is too large to avoid memory bloat
        loop {
            let len = {
                let buf = self.buffer.lock().unwrap();
                buf.len()
            };
            if len <= 48000 * 2 * 2 {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        if let Some(stream) = self.stream.take() {
            loop {
                let len = {
                    let buf = self.buffer.lock().unwrap();
                    buf.len()
                };
                if len == 0 {
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            drop(stream);
        }
        Ok(())
    }

    fn description(&self) -> &str {
        "cpal-playback-sink"
    }
}