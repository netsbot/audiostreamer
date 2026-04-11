use crate::audio::source::SampleBuffer;
use crate::error::StreamerError;
use ffmpeg_next::format::sample::Type as SampleType;
use ffmpeg_next::software::resampling::Context as ResamplerContext;
use ffmpeg_next::{self as ffmpeg, Packet};
use ffmpeg_next::{frame, packet, ChannelLayout};

pub type Result<T> = std::result::Result<T, StreamerError>;

pub struct AlacDecoder {
    decoder: ffmpeg::codec::decoder::Audio,
    resampler: ResamplerContext,
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

        // Initialize resampler to convert from ALAC's output to interleaved f32
        let resampler = ResamplerContext::get(
            audio_decoder.format(),
            audio_decoder.channel_layout(),
            audio_decoder.rate(),
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            ChannelLayout::STEREO,
            audio_decoder.rate(),
        )
        .map_err(|e| StreamerError::Message(format!("Failed to create resampler: {}", e)))?;

        Ok(Self {
            decoder: audio_decoder,
            resampler,
        })
    }

    pub fn decode_sample(&mut self, data: &[u8]) -> Result<SampleBuffer> {
        let packet = Packet::copy(data);

        self.decoder
            .send_packet(&packet)
            .map_err(|e| StreamerError::Message(format!("Error sending packet: {}", e)))?;

        let mut decoded_frame = frame::Audio::empty();
        let mut pcm_data = Vec::new();

        while self.decoder.receive_frame(&mut decoded_frame).is_ok() {
            let mut resampled_frame = frame::Audio::empty();
            self.resampler
                .run(&decoded_frame, &mut resampled_frame)
                .map_err(|e| StreamerError::Message(format!("Resampling failed: {}", e)))?;

            // Interleaved f32 data is in plane 0
            if resampled_frame.is_packed() {
                let data = resampled_frame.data(0);
                let samples: &[f32] = unsafe {
                    std::slice::from_raw_parts(data.as_ptr() as *const f32, data.len() / 4)
                };
                pcm_data.extend_from_slice(samples);
            }
        }

        Ok(SampleBuffer::F32(pcm_data))
    }
}

impl std::fmt::Debug for AlacDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlacDecoder").finish()
    }
}
