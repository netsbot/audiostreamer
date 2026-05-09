use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::audio::sink::AudioSink;
use crate::audio::source::{AudioChunk, SampleBuffer};
use crate::error::{Result, StreamerError};

struct SendStream(#[allow(dead_code)] cpal::Stream);
unsafe impl Send for SendStream {}

#[derive(Clone)]
pub struct PlaybackControls {
    stream: Arc<Mutex<Option<SendStream>>>,
    buffer: Arc<Mutex<Option<PlaybackBuffer>>>,
}

impl PlaybackControls {
    pub fn pause(&self) -> Result<()> {
        if let Some(stream) = self.stream.lock().unwrap().as_ref() {
            stream
                .0
                .pause()
                .map_err(|e| StreamerError::Message(format!("Failed to pause stream: {}", e)))?;
        }
        Ok(())
    }

    pub fn resume(&self) -> Result<()> {
        if let Some(stream) = self.stream.lock().unwrap().as_ref() {
            stream
                .0
                .play()
                .map_err(|e| StreamerError::Message(format!("Failed to resume stream: {}", e)))?;
        }
        Ok(())
    }

    pub fn clear_buffer(&self) {
        if let Ok(mut buffer) = self.buffer.lock() {
            *buffer = None;
        }
    }
}

#[derive(Debug)]
enum PlaybackBuffer {
    I16(VecDeque<i16>),
    I32(VecDeque<i32>),
    F32(VecDeque<f32>),
}

impl PlaybackBuffer {
    fn from_chunk(chunk: &AudioChunk) -> Self {
        match &chunk.samples {
            SampleBuffer::I16(samples) => Self::I16(samples.iter().copied().collect()),
            SampleBuffer::I32(samples) => Self::I32(samples.iter().copied().collect()),
            SampleBuffer::F32(samples) => Self::F32(samples.iter().copied().collect()),
        }
    }

    fn len(&self) -> usize {
        match self {
            PlaybackBuffer::I16(samples) => samples.len(),
            PlaybackBuffer::I32(samples) => samples.len(),
            PlaybackBuffer::F32(samples) => samples.len(),
        }
    }

    fn append_chunk(&mut self, chunk: &AudioChunk) -> Result<()> {
        match (self, &chunk.samples) {
            (PlaybackBuffer::I16(buffer), SampleBuffer::I16(samples)) => {
                buffer.extend(samples.iter().copied());
                Ok(())
            }
            (PlaybackBuffer::I32(buffer), SampleBuffer::I32(samples)) => {
                buffer.extend(samples.iter().copied());
                Ok(())
            }
            (PlaybackBuffer::F32(buffer), SampleBuffer::F32(samples)) => {
                buffer.extend(samples.iter().copied());
                Ok(())
            }
            _ => Err(StreamerError::Unsupported(
                "chunk sample format changed during playback".to_string(),
            )),
        }
    }

    fn drain_f32(
        buffer: &Arc<Mutex<Option<PlaybackBuffer>>>,
        data: &mut [f32],
        frames_played: &Arc<AtomicU64>,
        channels: u16,
    ) {
        let mut locked = buffer.lock().unwrap();
        match locked.as_mut() {
            Some(PlaybackBuffer::F32(samples)) => {
                for sample in data.iter_mut() {
                    *sample = samples.pop_front().unwrap_or(0.0);
                }
                let channels = u64::from(channels.max(1));
                frames_played.fetch_add((data.len() as u64) / channels, Ordering::SeqCst);
            }
            _ => {
                for sample in data.iter_mut() {
                    *sample = 0.0;
                }
            }
        }
    }

    fn drain_i16(
        buffer: &Arc<Mutex<Option<PlaybackBuffer>>>,
        data: &mut [i16],
        frames_played: &Arc<AtomicU64>,
        channels: u16,
    ) {
        let mut locked = buffer.lock().unwrap();
        match locked.as_mut() {
            Some(PlaybackBuffer::I16(samples)) => {
                for sample in data.iter_mut() {
                    *sample = samples.pop_front().unwrap_or(0);
                }
                let channels = u64::from(channels.max(1));
                frames_played.fetch_add((data.len() as u64) / channels, Ordering::SeqCst);
            }
            _ => {
                for sample in data.iter_mut() {
                    *sample = 0;
                }
            }
        }
    }

    fn drain_i32(
        buffer: &Arc<Mutex<Option<PlaybackBuffer>>>,
        data: &mut [i32],
        frames_played: &Arc<AtomicU64>,
        channels: u16,
    ) {
        let mut locked = buffer.lock().unwrap();
        match locked.as_mut() {
            Some(PlaybackBuffer::I32(samples)) => {
                for sample in data.iter_mut() {
                    *sample = samples.pop_front().unwrap_or(0);
                }
                let channels = u64::from(channels.max(1));
                frames_played.fetch_add((data.len() as u64) / channels, Ordering::SeqCst);
            }
            _ => {
                for sample in data.iter_mut() {
                    *sample = 0;
                }
            }
        }
    }
}

pub struct PlaybackSink {
    stream: Arc<Mutex<Option<SendStream>>>,
    buffer: Arc<Mutex<Option<PlaybackBuffer>>>,
    frames_played: Arc<AtomicU64>,
    output_sample_rate: Arc<AtomicU64>,
    output_channels: Arc<AtomicU64>,
    device_sample_rate: u32,
    device_channels: u16,
}

unsafe impl Send for PlaybackSink {}

impl PlaybackSink {
    pub fn new() -> Self {
        Self::new_with_metrics(
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicU64::new(44_100)),
            Arc::new(AtomicU64::new(2)),
        )
    }

    pub fn new_with_counter(samples_played: Arc<AtomicU64>) -> Self {
        Self::new_with_metrics(
            samples_played,
            Arc::new(AtomicU64::new(44_100)),
            Arc::new(AtomicU64::new(2)),
        )
    }

    pub fn new_with_metrics(
        frames_played: Arc<AtomicU64>,
        output_sample_rate: Arc<AtomicU64>,
        output_channels: Arc<AtomicU64>,
    ) -> Self {
        Self {
            stream: Arc::new(Mutex::new(None)),
            buffer: Arc::new(Mutex::new(None)),
            frames_played,
            output_sample_rate,
            output_channels,
            device_sample_rate: 44100,
            device_channels: 2,
        }
    }

    /// Build a sink that reuses an existing CPAL stream + buffer from a previous
    /// PlaybackControls. The CPAL device stream won't be re-initialized.
    pub fn new_reusing(
        controls: &PlaybackControls,
        frames_played: Arc<AtomicU64>,
        output_sample_rate: Arc<AtomicU64>,
        output_channels: Arc<AtomicU64>,
    ) -> Self {
        let sr = output_sample_rate.load(Ordering::SeqCst) as u32;
        let ch = output_channels.load(Ordering::SeqCst) as u16;
        Self {
            stream: controls.stream.clone(),
            buffer: controls.buffer.clone(),
            frames_played,
            output_sample_rate,
            output_channels,
            device_sample_rate: if sr > 0 { sr } else { 44100 },
            device_channels: if ch > 0 { ch } else { 2 },
        }
    }

    fn ensure_stream(&mut self, chunk: &AudioChunk) -> Result<()> {
        if self.stream.lock().unwrap().is_some() {
            return Ok(());
        }

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| StreamerError::Message("No output device found".to_string()))?;

        let desired_format = chunk.samples.sample_format();
        let config = device
            .supported_output_configs()
            .map_err(|e| StreamerError::Message(format!("Failed to get supported configs: {}", e)))?
            .find(|config| {
                config.channels() == chunk.channels
                    && config.min_sample_rate().0 <= chunk.sample_rate
                    && config.max_sample_rate().0 >= chunk.sample_rate
                    && config.sample_format() == desired_format
            })
            .map(|config| config.with_sample_rate(cpal::SampleRate(chunk.sample_rate)))
            .ok_or_else(|| {
                StreamerError::Unsupported(format!(
                    "device cannot play source natively (rate={} channels={} format={:?})",
                    chunk.sample_rate, chunk.channels, desired_format
                ))
            })?;

        self.device_sample_rate = config.sample_rate().0;
        self.device_channels = config.channels();
        self.output_sample_rate
            .store(self.device_sample_rate as u64, Ordering::SeqCst);
        self.output_channels
            .store(self.device_channels as u64, Ordering::SeqCst);

        log::info!(
            "initializing bit-perfect playback stream: rate={} channels={} format={:?}",
            self.device_sample_rate,
            self.device_channels,
            config.sample_format()
        );

        let stream_config = config.config();
        let sample_format = config.sample_format();
        let buffer = self.buffer.clone();
        let frames_played = self.frames_played.clone();
        let channels = self.device_channels;

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &stream_config,
                move |data: &mut [f32], _| {
                    PlaybackBuffer::drain_f32(&buffer, data, &frames_played, channels);
                },
                |err| log::error!("Playback error: {}", err),
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                &stream_config,
                move |data: &mut [i16], _| {
                    PlaybackBuffer::drain_i16(&buffer, data, &frames_played, channels);
                },
                |err| log::error!("Playback error: {}", err),
                None,
            ),
            cpal::SampleFormat::I32 => device.build_output_stream(
                &stream_config,
                move |data: &mut [i32], _| {
                    PlaybackBuffer::drain_i32(&buffer, data, &frames_played, channels);
                },
                |err| log::error!("Playback error: {}", err),
                None,
            ),
            other => {
                return Err(StreamerError::Message(format!(
                    "unsupported output sample format: {:?}",
                    other
                )));
            }
        }
        .map_err(|e| StreamerError::Message(format!("Failed to build output stream: {}", e)))?;

        stream
            .play()
            .map_err(|e| StreamerError::Message(format!("Failed to start stream: {}", e)))?;

        *self.stream.lock().unwrap() = Some(SendStream(stream));
        Ok(())
    }

    pub fn controls(&self) -> PlaybackControls {
        PlaybackControls {
            stream: self.stream.clone(),
            buffer: self.buffer.clone(),
        }
    }

    pub fn samples_played(&self) -> u64 {
        self.frames_played.load(Ordering::SeqCst)
    }

    pub fn device_sample_rate(&self) -> u32 {
        self.device_sample_rate
    }

    pub fn device_channels(&self) -> u16 {
        self.device_channels
    }
}

#[async_trait]
impl AudioSink for PlaybackSink {
    async fn write_chunk(&mut self, chunk: &AudioChunk) -> Result<()> {
        self.ensure_stream(chunk)?;

        {
            let mut locked = self.buffer.lock().unwrap();
            match locked.as_mut() {
                Some(buffer) => buffer.append_chunk(chunk)?,
                None => {
                    *locked = Some(PlaybackBuffer::from_chunk(chunk));
                }
            }
        }

        loop {
            let buffered = {
                let locked = self.buffer.lock().unwrap();
                locked.as_ref().map(|buffer| buffer.len()).unwrap_or(0)
            };

            if buffered <= self.device_sample_rate as usize * self.device_channels as usize * 2 {
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        if self.stream.lock().unwrap().is_some() {
            loop {
                let buffered = {
                    let locked = self.buffer.lock().unwrap();
                    locked.as_ref().map(|buffer| buffer.len()).unwrap_or(0)
                };

                if buffered == 0 {
                    break;
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }

        self.stream.lock().unwrap().take();

        Ok(())
    }

    async fn pause(&mut self) -> Result<()> {
        if let Some(stream) = self.stream.lock().unwrap().as_ref() {
            stream.0.pause().map_err(|e| StreamerError::Message(format!("Failed to pause stream: {}", e)))?;
        }
        Ok(())
    }

    async fn resume(&mut self) -> Result<()> {
        if let Some(stream) = self.stream.lock().unwrap().as_ref() {
            stream.0.play().map_err(|e| StreamerError::Message(format!("Failed to resume stream: {}", e)))?;
        }
        Ok(())
    }


    fn description(&self) -> &str {
        "cpal-playback-sink"
    }
}
