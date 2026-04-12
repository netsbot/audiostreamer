use crate::audio::source::AudioChunk;
use crate::audio::source::AudioSource;
use crate::audio::source::SampleBuffer;
use crate::client::AppleMusicClient;
use crate::error::StreamerError;
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
    #[allow(dead_code)]
    sample_rate: u32,
    #[allow(dead_code)]
    bit_depth: u8,
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
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let m3u8_url = am_wrapper::get_m3u8(adam_id.to_string()).await?;
        let (media_playlist, codec_id) =
            utils::extract_media_playlist(&client, &m3u8_url, utils::Codec::Alac).await?;

        let (sample_rate, bit_depth) = utils::parse_alac_quality_from_codec_id(codec_id.as_str());

        let init_section = media_playlist.segments.first().unwrap().map.clone().unwrap();

        let init_section_url = Url::parse(&m3u8_url)?.join(init_section.uri.as_str())?;
        let init_section_data = client
            .download_byte_range(
                init_section_url.as_str(),
                init_section.byte_range.unwrap_or_default(),
            )
            .await?;

        let mut raw_mp4 = MemFile::new()?;
        raw_mp4.write_all(init_section_data.as_slice())?;

        let gpac_iso_file = gpac::IsoFile::open_progressive(raw_mp4.path.as_str())?;

        let alac_cookie =
            utils::generate_alac_extradata(sample_rate.unwrap(), 2, bit_depth.unwrap(), 0);

        let decoder = crate::audio::decoder::AlacDecoder::new(
            alac_cookie,
            sample_rate.unwrap(),
            2,
            bit_depth.unwrap(),
        )?;

        Ok(Self {
            adam_id: adam_id.to_string(),
            keys: utils::collect_key_uris(&media_playlist),
            media_playlist,
            client,
            gpac_iso_file,
            raw_mp4,
            sample_rate: sample_rate.unwrap(),
            bit_depth: bit_depth.unwrap(),
            current_segment: 0,
            next_sample_number: 1,
            base_url: Url::parse(&m3u8_url)?.join(".")?,
            decoder,
        })
    }

    pub async fn append_next_segment_and_collect_samples(
        &mut self,
        track: u32,
    ) -> crate::error::Result<Vec<gpac::TrackSample>> {
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
    ) -> crate::error::Result<Option<AudioChunk>> {
        let new_samples = self.append_next_segment_and_collect_samples(1).await?;
        if new_samples.is_empty() {
            return Ok(None);
        }

        let mut stream = UnixStream::connect("../AppleMusicDecrypt/rust/wrapper/rootfs/data/data/com.apple.android.music/files/decrypt.sock").await.unwrap();
        let mut current_state_key = String::new();
        let mut sample_buffer = Vec::new();
        let mut pcm_samples: Option<SampleBuffer> = None;

        for sample in new_samples.iter() {
            let key = self.keys.get(sample.desc_index).unwrap();

            if *key != current_state_key {
                am_wrapper::setup_context(&mut stream, &mut current_state_key, &self.adam_id, key)
                    .await
                    .unwrap();
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
