use crate::audio::source::SampleBuffer;
use crate::error::StreamerError;
use ffmpeg_next::{self as ffmpeg, Packet};
use ffmpeg_next::format::Sample;
use ffmpeg_next::format::sample::Type as SampleType;
use ffmpeg_next::frame;

pub type Result<T> = std::result::Result<T, StreamerError>;

pub struct AlacDecoder {
    decoder: ffmpeg::codec::decoder::Audio,
}

impl AlacDecoder {
    pub fn new(
        extra_data: Vec<u8>,
        sample_rate: u32,
        _channels: u32,
        _bit_depth: u8,
    ) -> Result<Self> {
        ffmpeg::init().map_err(|e| StreamerError::Message(format!("FFmpeg init failed: {}", e)))?;

        let codec = ffmpeg::decoder::find(ffmpeg::codec::Id::ALAC)
            .ok_or_else(|| StreamerError::Message("ALAC decoder not found".to_string()))?;

        let mut context = ffmpeg::codec::context::Context::new_with_codec(codec);

        // Set parameters on the context directly via FFmpeg FFI where safe wrappers are missing
        unsafe {
            let ptr = context.as_mut_ptr();
            (*ptr).sample_rate = sample_rate as i32;

            // Set extradata (magic cookie)
            let extradata = ffmpeg_sys_next::av_malloc(
                extra_data.len() + ffmpeg_sys_next::AV_INPUT_BUFFER_PADDING_SIZE as usize,
            ) as *mut u8;
            std::ptr::copy_nonoverlapping(extra_data.as_ptr(), extradata, extra_data.len());
            (*ptr).extradata = extradata;
            (*ptr).extradata_size = extra_data.len() as i32;
        }

        let opened = context
            .decoder()
            .open_as(codec)
            .map_err(|e| StreamerError::Message(format!("Failed to open decoder: {}", e)))?;

        // Wrap the opened decoder in the Audio wrapper to get audio-specific methods
        let audio_decoder = opened
            .audio()
            .map_err(|e| StreamerError::Message(format!("Failed to get audio decoder: {}", e)))?;

        Ok(Self { decoder: audio_decoder })
    }

    pub fn channels(&self) -> u16 {
        self.decoder.channels() as u16
    }

    pub fn sample_rate(&self) -> u32 {
        self.decoder.rate() as u32
    }

    pub fn decode_sample(&mut self, data: &[u8]) -> Result<SampleBuffer> {
        let packet = Packet::copy(data);

        self.decoder
            .send_packet(&packet)
            .map_err(|e| StreamerError::Message(format!("Error sending packet: {}", e)))?;

        let mut decoded_frame = frame::Audio::empty();
        let mut output: Option<SampleBuffer> = None;

        while self.decoder.receive_frame(&mut decoded_frame).is_ok() {
            let mut decoded = decode_frame_native(&decoded_frame)?;

            if let Some(samples) = output.as_mut() {
                samples.append(&mut decoded);
            } else {
                output = Some(decoded);
            }
        }

        Ok(output.unwrap_or_else(|| SampleBuffer::I16(Vec::new())))
    }
}

fn decode_frame_native(frame: &frame::Audio) -> Result<SampleBuffer> {
    match frame.format() {
        Sample::I16(SampleType::Packed) => {
            let data = frame.data(0);
            let samples: &[i16] = unsafe {
                std::slice::from_raw_parts(data.as_ptr() as *const i16, data.len() / 2)
            };
            Ok(SampleBuffer::I16(samples.to_vec()))
        }
        Sample::I32(SampleType::Packed) => {
            let data = frame.data(0);
            let samples: &[i32] = unsafe {
                std::slice::from_raw_parts(data.as_ptr() as *const i32, data.len() / 4)
            };
            Ok(SampleBuffer::I32(samples.to_vec()))
        }
        Sample::F32(SampleType::Packed) => {
            let data = frame.data(0);
            let samples: &[f32] = unsafe {
                std::slice::from_raw_parts(data.as_ptr() as *const f32, data.len() / 4)
            };
            Ok(SampleBuffer::F32(samples.to_vec()))
        }
        Sample::I16(SampleType::Planar) => {
            let interleaved = interleave_planar_i16(frame);
            Ok(SampleBuffer::I16(interleaved))
        }
        Sample::I32(SampleType::Planar) => {
            let interleaved = interleave_planar_i32(frame);
            Ok(SampleBuffer::I32(interleaved))
        }
        Sample::F32(SampleType::Planar) => {
            let interleaved = interleave_planar_f32(frame);
            Ok(SampleBuffer::F32(interleaved))
        }
        other => Err(StreamerError::Unsupported(format!(
            "unsupported decoded sample format: {other:?}"
        ))),
    }
}

fn interleave_planar_i16(frame: &frame::Audio) -> Vec<i16> {
    let channels = frame.channels() as usize;
    let per_channel_samples = frame.samples();
    let mut out = Vec::with_capacity(channels * per_channel_samples);

    for i in 0..per_channel_samples {
        for ch in 0..channels {
            let plane = frame.data(ch);
            let samples: &[i16] = unsafe {
                std::slice::from_raw_parts(plane.as_ptr() as *const i16, per_channel_samples)
            };
            out.push(samples[i]);
        }
    }

    out
}

fn interleave_planar_i32(frame: &frame::Audio) -> Vec<i32> {
    let channels = frame.channels() as usize;
    let per_channel_samples = frame.samples();
    let mut out = Vec::with_capacity(channels * per_channel_samples);

    for i in 0..per_channel_samples {
        for ch in 0..channels {
            let plane = frame.data(ch);
            let samples: &[i32] = unsafe {
                std::slice::from_raw_parts(plane.as_ptr() as *const i32, per_channel_samples)
            };
            out.push(samples[i]);
        }
    }

    out
}

fn interleave_planar_f32(frame: &frame::Audio) -> Vec<f32> {
    let channels = frame.channels() as usize;
    let per_channel_samples = frame.samples();
    let mut out = Vec::with_capacity(channels * per_channel_samples);

    for i in 0..per_channel_samples {
        for ch in 0..channels {
            let plane = frame.data(ch);
            let samples: &[f32] = unsafe {
                std::slice::from_raw_parts(plane.as_ptr() as *const f32, per_channel_samples)
            };
            out.push(samples[i]);
        }
    }

    out
}

impl std::fmt::Debug for AlacDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlacDecoder").finish()
    }
}
