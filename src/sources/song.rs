use crate::audio::source::AudioChunk;
use crate::audio::source::AudioSource;
use crate::audio::source::SampleBuffer;
use crate::client::AppleMusicClient;
use crate::error::{Result, StreamerError};
use crate::sources::utils;
use crate::sources::utils::MemFile;
use crate::{am_wrapper, gpac};
use async_trait::async_trait;
use m3u8_rs::MediaPlaylist;
use reqwest::Url;
use tokio::net::UnixStream;

#[derive(Debug)]
pub struct Song {
    adam_id: String,
    media_playlist: MediaPlaylist,
    client: AppleMusicClient,
    raw_mp4: MemFile,
    gpac_iso_file: gpac::IsoFile,
    current_segment: usize,
    next_sample_number: u32,
    base_url: Url,
    keys: Vec<String>,
    decoder: crate::audio::decoder::AlacDecoder,
}

impl Song {
    pub async fn new(
        adam_id: &str,
        client: AppleMusicClient,
    ) -> Result<Self> {
        let m3u8_url = am_wrapper::get_m3u8(adam_id.to_string())
            .await
            .map_err(|e| StreamerError::Message(format!("failed to fetch playlist URL for {adam_id}: {e}")))?;
        let (media_playlist, codec_id) = utils::extract_media_playlist(&client, &m3u8_url, utils::Codec::Alac)
            .await
            .map_err(|e| StreamerError::Message(format!("failed to load media playlist for {adam_id}: {e}")))?;

        let (sample_rate, bit_depth) = utils::parse_alac_quality_from_codec_id(codec_id.as_str());
        let sample_rate = sample_rate.ok_or_else(|| {
            StreamerError::Unsupported(format!("could not parse ALAC sample rate from codec id {codec_id}"))
        })?;
        let bit_depth = bit_depth.ok_or_else(|| {
            StreamerError::Unsupported(format!("could not parse ALAC bit depth from codec id {codec_id}"))
        })?;

        let init_section = media_playlist
            .segments
            .first()
            .ok_or_else(|| StreamerError::Unsupported(format!("playlist for {adam_id} does not contain an init segment")))?
            .map
            .clone()
            .ok_or_else(|| StreamerError::Unsupported(format!("init segment for {adam_id} does not contain a map section")))?;

        let init_section_url = Url::parse(&m3u8_url)
            .map_err(|e| StreamerError::Message(format!("invalid playlist URL {m3u8_url}: {e}")))?
            .join(init_section.uri.as_str())
            .map_err(|e| StreamerError::Message(format!("invalid init segment URL for {adam_id}: {e}")))?;
        let init_section_data = client
            .download_byte_range(
                init_section_url.as_str(),
                init_section.byte_range.unwrap_or_default(),
            )
            .await?;

        let mut raw_mp4 = MemFile::new().map_err(|e| StreamerError::Message(format!("failed to create memfd for {adam_id}: {e}")))?;
        raw_mp4
            .write_all(init_section_data.as_slice())
            .map_err(|e| StreamerError::Message(format!("failed to write init segment for {adam_id}: {e}")))?;

        let gpac_iso_file = gpac::IsoFile::open_progressive(raw_mp4.path.as_str())?;

        let decoder = crate::audio::decoder::AlacDecoder::new(sample_rate, 2, bit_depth)?;

        Ok(Self {
            adam_id: adam_id.to_string(),
            keys: utils::collect_key_uris(&media_playlist),
            media_playlist,
            client,
            gpac_iso_file,
            raw_mp4,
            current_segment: 0,
            next_sample_number: 1,
            base_url: Url::parse(&m3u8_url)
                .map_err(|e| StreamerError::Message(format!("invalid playlist URL {m3u8_url}: {e}")))?
                .join(".")
                .map_err(|e| StreamerError::Message(format!("failed to derive base URL for {adam_id}: {e}")))?,
            decoder,
        })
    }

    pub async fn append_next_segment_and_collect_samples(
        &mut self,
        track: u32,
    ) -> Result<Vec<gpac::TrackSample>> {
        if self.current_segment >= self.media_playlist.segments.len() {
            return Ok(Vec::new());
        }

        let (segment_uri, byte_range) = {
            let segment = &self.media_playlist.segments[self.current_segment];
            (
                segment.uri.clone(),
                segment.byte_range.clone().unwrap_or_default(),
            )
        };

        let segment_url = self
            .base_url
            .join(segment_uri.as_str())
            .map_err(|e| StreamerError::Message(format!("invalid segment URL: {e}")))?;

        let segment_data = self
            .client
            .download_byte_range(segment_url.as_str(), byte_range)
            .await?;

        self.raw_mp4
            .write_all(segment_data.as_slice())
            .map_err(|e| StreamerError::Message(format!("failed to append segment bytes: {e}")))?;

        self.current_segment += 1;

        self.gpac_iso_file
            .read_new_samples(track, &mut self.next_sample_number)
    }
}

#[async_trait]
impl AudioSource for Song {
    async fn next_chunk(
        &mut self,
        _max_samples: usize,
    ) -> Result<Option<AudioChunk>> {
        let new_samples = self.append_next_segment_and_collect_samples(1).await?;
        if new_samples.is_empty() {
            return Ok(None);
        }

        let mut stream = UnixStream::connect(
            "../AppleMusicDecrypt/rust/wrapper/rootfs/data/data/com.apple.android.music/files/decrypt.sock",
        )
        .await
        .map_err(|e| StreamerError::Message(format!("failed to connect decrypt socket: {e}")))?;
        let mut current_state_key = String::new();
        let mut sample_buffer = Vec::new();
        let mut pcm_samples: Option<SampleBuffer> = None;

        for sample in new_samples.iter() {
            let key = self.keys.get(sample.desc_index).ok_or_else(|| {
                StreamerError::Unsupported(format!(
                    "missing decryption key for sample description index {}",
                    sample.desc_index
                ))
            })?;

            if *key != current_state_key {
                am_wrapper::setup_context(&mut stream, &mut current_state_key, &self.adam_id, key)
                    .await
                    .map_err(|e| StreamerError::Message(format!("failed to set decrypt context: {e}")))?;
            }

            sample_buffer.clear();
            am_wrapper::decrypt_sample(&mut stream, &sample.data, &mut sample_buffer).await?;

            let mut decoded = self.decoder.decode_sample(&sample_buffer)?;
            if let Some(samples) = pcm_samples.as_mut() {
                samples.append(&mut decoded);
            } else {
                pcm_samples = Some(decoded);
            }
        }

        Ok(Some(AudioChunk {
            samples: pcm_samples.unwrap_or(SampleBuffer::I16(Vec::new())),
            sample_rate: self.decoder.sample_rate(),
            channels: self.decoder.channels(),
        }))
    }

    fn description(&self) -> &str {
        "apple-music-song"
    }
}
