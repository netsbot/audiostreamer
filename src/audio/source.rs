use async_trait::async_trait;
use cpal;

use crate::error::Result;

#[derive(Debug, Clone)]
pub enum SampleBuffer {
    I16(Vec<i16>),
    I32(Vec<i32>),
    F32(Vec<f32>),
}

impl SampleBuffer {
    pub fn len(&self) -> usize {
        match self {
            SampleBuffer::I16(v) => v.len(),
            SampleBuffer::I32(v) => v.len(),
            SampleBuffer::F32(v) => v.len(),
        }
    }

    pub fn sample_format(&self) -> cpal::SampleFormat {
        match self {
            SampleBuffer::I16(_) => cpal::SampleFormat::I16,
            SampleBuffer::I32(_) => cpal::SampleFormat::I32,
            SampleBuffer::F32(_) => cpal::SampleFormat::F32,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            SampleBuffer::I16(v) => v.is_empty(),
            SampleBuffer::I32(v) => v.is_empty(),
            SampleBuffer::F32(v) => v.is_empty(),
        }
    }

    pub fn append(&mut self, other: &mut Self) {
        match (self, other) {
            (SampleBuffer::I16(a), SampleBuffer::I16(b)) => a.append(b),
            (SampleBuffer::I32(a), SampleBuffer::I32(b)) => a.append(b),
            (SampleBuffer::F32(a), SampleBuffer::F32(b)) => a.append(b),
            _ => panic!("Mismatched sample buffer types"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub samples: SampleBuffer,
    pub sample_rate: u32,
    pub channels: u16,
}

#[async_trait]
pub trait AudioSource: Send {
    async fn next_chunk(&mut self, max_samples: usize) -> Result<Option<AudioChunk>>;
    async fn seek(&mut self, _seconds: f64) -> Result<()> {
        Err(crate::error::StreamerError::Unsupported("seeking not supported for this source".to_string()))
    }
    fn description(&self) -> &str;
}
