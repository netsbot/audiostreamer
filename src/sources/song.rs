use crate::audio::source::AudioChunk;
use crate::audio::source::AudioSource;
use crate::audio::source::SampleBuffer;
use crate::client::AppleMusicClient;
use crate::error::{Result, StreamerError};
use crate::sources::utils;
use crate::sources::utils::MemFile;
use crate::{am_wrapper, gpac};
use async_trait::async_trait;
use m3u8_rs::ByteRange;
use m3u8_rs::MediaPlaylist;
use reqwest::Url;
use std::collections::{HashMap, VecDeque};
use std::io::ErrorKind;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::net::UnixStream;
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout, Duration, Instant};

#[derive(Debug, Clone)]
struct CachedSongBootstrap {
    m3u8_url: String,
    media_playlist: MediaPlaylist,
    codec_id: String,
    init_section_data: Vec<u8>,
}

const DECODED_SEGMENT_CACHE_MAX_ENTRIES: usize = 512;
const DECODED_SEGMENT_CACHE_MAX_BYTES: usize = 512 * 1024 * 1024;
const PLAYBACK_RETRY_ATTEMPTS: usize = 3;

#[derive(Debug, Default)]
struct DecodedSegmentCache {
    map: HashMap<String, Arc<AudioChunk>>,
    lru: VecDeque<String>,
    total_bytes: usize,
}

impl DecodedSegmentCache {
    fn get(&mut self, key: &str) -> Option<Arc<AudioChunk>> {
        let value = self.map.get(key)?.clone();
        self.touch(key);
        Some(value)
    }

    fn insert(&mut self, key: String, value: Arc<AudioChunk>) {
        let value_bytes = estimate_audio_chunk_bytes(&value);
        if let Some(prev) = self.map.remove(&key) {
            self.total_bytes = self
                .total_bytes
                .saturating_sub(estimate_audio_chunk_bytes(&prev));
            self.remove_key_from_lru(&key);
        }

        self.total_bytes = self.total_bytes.saturating_add(value_bytes);
        self.map.insert(key.clone(), value);
        self.lru.push_back(key);
        self.evict_if_needed();
    }

    fn touch(&mut self, key: &str) {
        self.remove_key_from_lru(key);
        self.lru.push_back(key.to_string());
    }

    fn remove_key_from_lru(&mut self, key: &str) {
        if let Some(pos) = self.lru.iter().position(|k| k == key) {
            self.lru.remove(pos);
        }
    }

    fn evict_if_needed(&mut self) {
        while self.map.len() > DECODED_SEGMENT_CACHE_MAX_ENTRIES
            || self.total_bytes > DECODED_SEGMENT_CACHE_MAX_BYTES
        {
            let Some(oldest_key) = self.lru.pop_front() else {
                break;
            };
            if let Some(oldest) = self.map.remove(&oldest_key) {
                self.total_bytes = self
                    .total_bytes
                    .saturating_sub(estimate_audio_chunk_bytes(&oldest));
            }
        }
    }
}

fn estimate_audio_chunk_bytes(chunk: &AudioChunk) -> usize {
    match &chunk.samples {
        SampleBuffer::I16(v) => v.len() * std::mem::size_of::<i16>(),
        SampleBuffer::I32(v) => v.len() * std::mem::size_of::<i32>(),
        SampleBuffer::F32(v) => v.len() * std::mem::size_of::<f32>(),
    }
}

fn decoded_segment_cache() -> &'static RwLock<DecodedSegmentCache> {
    static CACHE: OnceLock<RwLock<DecodedSegmentCache>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(DecodedSegmentCache::default()))
}

fn song_bootstrap_cache() -> &'static RwLock<HashMap<String, CachedSongBootstrap>> {
    static CACHE: OnceLock<RwLock<HashMap<String, CachedSongBootstrap>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn is_wrapper_ipc_retryable(error: &StreamerError) -> bool {
    matches!(
        error,
        StreamerError::Io(io_error)
            if matches!(
                io_error.kind(),
                ErrorKind::BrokenPipe
                    | ErrorKind::ConnectionRefused
                    | ErrorKind::ConnectionReset
                    | ErrorKind::NotConnected
                    | ErrorKind::TimedOut
                    | ErrorKind::UnexpectedEof
            )
    )
}

async fn download_segment_with_retry(
    client: AppleMusicClient,
    segment_url: String,
    byte_range: ByteRange,
) -> Result<Vec<u8>> {
    let mut last_error: Option<StreamerError> = None;

    for attempt in 0..PLAYBACK_RETRY_ATTEMPTS {
        match client
            .download_byte_range(segment_url.as_str(), byte_range.clone())
            .await
        {
            Ok(bytes) => return Ok(bytes),
            Err(error) => {
                log::warn!(
                    "[Song] segment download failed for {} attempt {}/{}: {}",
                    segment_url,
                    attempt + 1,
                    PLAYBACK_RETRY_ATTEMPTS,
                    error
                );
                last_error = Some(error);
                if attempt + 1 < PLAYBACK_RETRY_ATTEMPTS {
                    sleep(Duration::from_millis(150 * (attempt as u64 + 1))).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        StreamerError::Message(format!("failed to download segment from {segment_url}"))
    }))
}

async fn read_new_samples_with_retry(
    gpac_iso_file: &mut gpac::IsoFile,
    next_sample_number: &mut u32,
    track: u32,
) -> Result<Vec<gpac::TrackSample>> {
    let mut last_error: Option<StreamerError> = None;

    for attempt in 0..PLAYBACK_RETRY_ATTEMPTS {
        match gpac_iso_file.read_new_samples(track, next_sample_number) {
            Ok(samples) => return Ok(samples),
            Err(error) => {
                log::warn!(
                    "[Song] sample refresh failed for track {} attempt {}/{}: {}",
                    track,
                    attempt + 1,
                    PLAYBACK_RETRY_ATTEMPTS,
                    error
                );
                last_error = Some(error);
                if attempt + 1 < PLAYBACK_RETRY_ATTEMPTS {
                    sleep(Duration::from_millis(100 * (attempt as u64 + 1))).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        StreamerError::Message(format!("failed to refresh samples for track {track}"))
    }))
}

async fn wait_for_decrypt_socket(sock_path: &str, wait_seconds: u64) -> std::result::Result<UnixStream, std::io::Error> {
    let deadline = Instant::now() + Duration::from_secs(wait_seconds);

    let mut last_error = match timeout(Duration::from_millis(200), UnixStream::connect(sock_path)).await {
        Ok(Ok(stream)) => return Ok(stream),
        Ok(Err(error)) => error,
        Err(_) => std::io::Error::new(
            ErrorKind::TimedOut,
            "timed out waiting for decrypt unix socket",
        ),
    };

    loop {
        if Instant::now() >= deadline {
            break;
        }

        sleep(Duration::from_millis(100)).await;

        match timeout(Duration::from_millis(200), UnixStream::connect(sock_path)).await {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(error)) => last_error = error,
            Err(_) => {
                last_error = std::io::Error::new(
                    ErrorKind::TimedOut,
                    "timed out waiting for decrypt unix socket",
                );
            }
        }
    }

    Err(last_error)
}

async fn connect_decrypt_socket(sock_path: &str) -> Result<UnixStream> {
    am_wrapper::ensure_wrapper_running().await?;

    match wait_for_decrypt_socket(sock_path, 10).await {
        Ok(stream) => Ok(stream),
        Err(first_error) => {
            log::warn!(
                "decrypt socket not ready in time ({first_error}); restarting wrapper once"
            );
            am_wrapper::restart_wrapper().await?;
            wait_for_decrypt_socket(sock_path, 10)
                .await
                .map_err(StreamerError::from)
        }
    }
}

async fn load_song_bootstrap(
    adam_id: &str,
    client: &AppleMusicClient,
) -> Result<CachedSongBootstrap> {
    {
        let cache = song_bootstrap_cache().read().await;
        if let Some(cached) = cache.get(adam_id) {
            log::info!("[Song] cache hit for {}", adam_id);
            return Ok(cached.clone());
        }
    }

    log::info!("[Song] cache miss for {}, loading playlists", adam_id);
    let m3u8_url = am_wrapper::get_m3u8(adam_id.to_string())
        .await
        .map_err(|e| {
            StreamerError::Message(format!("failed to fetch playlist URL for {adam_id}: {e}"))
        })?;
    let (media_playlist, codec_id) =
        utils::extract_media_playlist(client, &m3u8_url, utils::Codec::Alac)
            .await
            .map_err(|e| {
                StreamerError::Message(format!("failed to load media playlist for {adam_id}: {e}"))
            })?;

    let init_section = media_playlist
        .segments
        .first()
        .ok_or_else(|| {
            StreamerError::Unsupported(format!(
                "playlist for {adam_id} does not contain an init segment"
            ))
        })?
        .map
        .clone()
        .ok_or_else(|| {
            StreamerError::Unsupported(format!(
                "init segment for {adam_id} does not contain a map section"
            ))
        })?;

    let init_section_url = Url::parse(&m3u8_url)
        .map_err(|e| StreamerError::Message(format!("invalid playlist URL {m3u8_url}: {e}")))?
        .join(init_section.uri.as_str())
        .map_err(|e| {
            StreamerError::Message(format!("invalid init segment URL for {adam_id}: {e}"))
        })?;

    let init_section_data = client
        .download_byte_range(
            init_section_url.as_str(),
            init_section.byte_range.unwrap_or_default(),
        )
        .await?;

    let bootstrap = CachedSongBootstrap {
        m3u8_url,
        media_playlist,
        codec_id,
        init_section_data,
    };

    {
        let mut cache = song_bootstrap_cache().write().await;
        cache.insert(adam_id.to_string(), bootstrap.clone());
    }

    Ok(bootstrap)
}

#[derive(Debug)]
pub struct Song {
    adam_id: String,
    media_playlist: MediaPlaylist,
    client: AppleMusicClient,
    raw_mp4: MemFile,
    gpac_iso_file: gpac::IsoFile,
    current_segment: usize,
    next_sample_number: u32,
    track_timescale: u32,
    pending_seek_offset_seconds: Option<f64>,
    base_url: Url,
    keys: Vec<String>,
    decoder: crate::audio::decoder::AlacDecoder,
}

impl Song {
    const PREFETCH_WINDOW_SEGMENTS: usize = 8;

    pub async fn new(adam_id: &str, client: AppleMusicClient) -> Result<Self> {
        let bootstrap = load_song_bootstrap(adam_id, &client).await?;
        let m3u8_url = bootstrap.m3u8_url;
        let media_playlist = bootstrap.media_playlist;
        let codec_id = bootstrap.codec_id;

        let (sample_rate, bit_depth) = utils::parse_alac_quality_from_codec_id(codec_id.as_str());
        let sample_rate = sample_rate.ok_or_else(|| {
            StreamerError::Unsupported(format!(
                "could not parse ALAC sample rate from codec id {codec_id}"
            ))
        })?;
        let bit_depth = bit_depth.ok_or_else(|| {
            StreamerError::Unsupported(format!(
                "could not parse ALAC bit depth from codec id {codec_id}"
            ))
        })?;

        let mut raw_mp4 = MemFile::new().map_err(|e| {
            StreamerError::Message(format!("failed to create memfd for {adam_id}: {e}"))
        })?;
        raw_mp4
            .write_all(bootstrap.init_section_data.as_slice())
            .map_err(|e| {
                StreamerError::Message(format!("failed to write init segment for {adam_id}: {e}"))
            })?;

        let gpac_iso_file = gpac::IsoFile::open_progressive(raw_mp4.path.as_str())?;
        let track_timescale = gpac_iso_file.media_timescale(1).max(1);

        let decoder = crate::audio::decoder::AlacDecoder::new(sample_rate, 2, bit_depth)?;

        let song = Self {
            adam_id: adam_id.to_string(),
            keys: utils::collect_key_uris(&media_playlist),
            media_playlist,
            client,
            gpac_iso_file,
            raw_mp4,
            current_segment: 0,
            next_sample_number: 1,
            track_timescale,
            pending_seek_offset_seconds: None,
            base_url: Url::parse(&m3u8_url)
                .map_err(|e| {
                    StreamerError::Message(format!("invalid playlist URL {m3u8_url}: {e}"))
                })?
                .join(".")
                .map_err(|e| {
                    StreamerError::Message(format!("failed to derive base URL for {adam_id}: {e}"))
                })?,
            decoder,
        };

        // Warm initial segments for faster first playback/auto-next transitions.
        song.predownload_upcoming_segments(Self::PREFETCH_WINDOW_SEGMENTS);

        Ok(song)
    }

    pub async fn append_next_segment_and_collect_samples(
        &mut self,
        track: u32,
    ) -> Result<(Vec<gpac::TrackSample>, usize)> {
        if self.current_segment >= self.media_playlist.segments.len() {
            return Ok((Vec::new(), self.current_segment));
        }

        let segment_index = self.current_segment;

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

        println!(
            "[Song] downloading segment {}/{} ({} bytes)",
            self.current_segment + 1,
            self.media_playlist.segments.len(),
            byte_range.length
        );
        let segment_data = download_segment_with_retry(
            self.client.clone(),
            segment_url.to_string(),
            byte_range,
        )
        .await?;

        self.raw_mp4
            .write_all(segment_data.as_slice())
            .map_err(|e| StreamerError::Message(format!("failed to append segment bytes: {e}")))?;

        self.current_segment += 1;

        self.predownload_upcoming_segments(Self::PREFETCH_WINDOW_SEGMENTS);

        let samples = read_new_samples_with_retry(&mut self.gpac_iso_file, &mut self.next_sample_number, track).await?;
        Ok((samples, segment_index))
    }

    pub fn predownload_upcoming_segments(&self, window: usize) {
        if window == 0 {
            return;
        }

        let start = self.current_segment;
        let end = (start + window).min(self.media_playlist.segments.len());
        for idx in start..end {
            let segment = &self.media_playlist.segments[idx];
            let byte_range = segment.byte_range.clone().unwrap_or(ByteRange {
                length: 0,
                offset: Some(0),
            });

            if byte_range.length == 0 {
                continue;
            }

            let Ok(segment_url) = self.base_url.join(segment.uri.as_str()) else {
                continue;
            };

            self.client
                .prefetch_byte_range(segment_url.to_string(), byte_range);
        }
    }

    pub fn predownload_all_segments(&self) {
        self.predownload_upcoming_segments(self.media_playlist.segments.len());
    }


    fn apply_pending_seek_offset(&mut self, samples: &mut Vec<gpac::TrackSample>) {
        let Some(mut remaining_seconds) = self.pending_seek_offset_seconds else {
            return;
        };

        if remaining_seconds <= 0.0 {
            self.pending_seek_offset_seconds = None;
            return;
        }

        let timescale = self.track_timescale.max(1) as f64;
        let mut drop_count = 0usize;
        for sample in samples.iter() {
            if remaining_seconds <= 0.0 {
                break;
            }
            remaining_seconds -= sample.duration as f64 / timescale;
            drop_count += 1;
        }

        if drop_count > 0 {
            samples.drain(0..drop_count);
        }

        if remaining_seconds <= 0.0 {
            self.pending_seek_offset_seconds = None;
        } else {
            self.pending_seek_offset_seconds = Some(remaining_seconds);
        }
    }
}

#[async_trait]
impl AudioSource for Song {
    async fn next_chunk(&mut self, _max_samples: usize) -> Result<Option<AudioChunk>> {
        let (mut new_samples, mut segment_index) =
            self.append_next_segment_and_collect_samples(1).await?;
        let mut trimmed_by_seek = false;

        loop {
            let had_pending_seek = self.pending_seek_offset_seconds.is_some();
            self.apply_pending_seek_offset(&mut new_samples);
            if had_pending_seek {
                trimmed_by_seek = true;
            }
            if !new_samples.is_empty() {
                break;
            }

            if self.current_segment >= self.media_playlist.segments.len() {
                return Ok(None);
            }

            let (samples, idx) = self.append_next_segment_and_collect_samples(1).await?;
            new_samples = samples;
            segment_index = idx;
            if new_samples.is_empty() {
                return Ok(None);
            }
        }

        if !trimmed_by_seek {
            let cache_key = format!("{}:{}", self.adam_id, segment_index);
            {
                let mut cache = decoded_segment_cache().write().await;
                if let Some(cached_chunk) = cache.get(&cache_key) {
                    return Ok(Some((*cached_chunk).clone()));
                }
            }
        }

        let sock_path = format!(
            "{}/rootfs/data/data/com.apple.android.music/files/decrypt.sock",
            env!("WRAPPER_DIR")
        );
        let mut stream = connect_decrypt_socket(&sock_path).await.map_err(|e| {
            StreamerError::Message(format!("failed to connect decrypt socket: {e}"))
        })?;

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

            let mut decrypt_error: Option<StreamerError> = None;
            for attempt in 0..PLAYBACK_RETRY_ATTEMPTS {
                if *key != current_state_key {
                    if let Err(error) = am_wrapper::setup_context(
                        &mut stream,
                        &mut current_state_key,
                        &self.adam_id,
                        key,
                    )
                    .await
                    {
                        decrypt_error = Some(StreamerError::Message(format!(
                            "failed to set decrypt context: {error}"
                        )));
                    } else {
                        decrypt_error = None;
                    }
                }

                if decrypt_error.is_none() {
                    sample_buffer.clear();
                    match am_wrapper::decrypt_sample(&mut stream, &sample.data, &mut sample_buffer)
                        .await
                    {
                        Ok(()) => break,
                        Err(error) => {
                            decrypt_error = Some(error);
                        }
                    }
                }

                if let Some(error) = decrypt_error.take() {
                    log::warn!(
                        "[Song] decrypt attempt {}/{} failed for {}: {}",
                        attempt + 1,
                        PLAYBACK_RETRY_ATTEMPTS,
                        self.adam_id,
                        error
                    );
                    if attempt + 1 < PLAYBACK_RETRY_ATTEMPTS {
                        if is_wrapper_ipc_retryable(&error) {
                            am_wrapper::restart_wrapper().await?;
                        }
                        stream = connect_decrypt_socket(&sock_path).await.map_err(|e| {
                            StreamerError::Message(format!(
                                "failed to reconnect decrypt socket: {e}"
                            ))
                        })?;
                        current_state_key.clear();
                        sleep(Duration::from_millis(100 * (attempt as u64 + 1))).await;
                        continue;
                    }
                    return Err(error);
                }
            }

            let mut decoded = self.decoder.decode_sample(&sample_buffer)?;
            if let Some(samples) = pcm_samples.as_mut() {
                samples.append(&mut decoded);
            } else {
                pcm_samples = Some(decoded);
            }
        }

        let chunk = AudioChunk {
            samples: pcm_samples.unwrap_or(SampleBuffer::I16(Vec::new())),
            sample_rate: self.decoder.sample_rate(),
            channels: self.decoder.channels(),
        };

        if !trimmed_by_seek {
            let cache_key = format!("{}:{}", self.adam_id, segment_index);
            let mut cache = decoded_segment_cache().write().await;
            cache.insert(cache_key, Arc::new(chunk.clone()));
        }

        Ok(Some(chunk))
    }

    async fn seek(&mut self, seconds: f64) -> Result<()> {
        let target_seconds = seconds.max(0.0);
        let mut accumulated_time = 0.0f64;
        let mut target_segment = 0usize;
        let mut segment_start_time = 0.0f64;

        for (i, segment) in self.media_playlist.segments.iter().enumerate() {
            let segment_duration = segment.duration as f64;
            if accumulated_time + segment_duration > target_seconds {
                target_segment = i;
                segment_start_time = accumulated_time;
                break;
            }
            accumulated_time += segment_duration;
            target_segment = i;
            segment_start_time = accumulated_time;
        }

        let in_segment_offset = (target_seconds - segment_start_time).max(0.0);
        log::info!(
            "[Song] seeking to {}s -> segment {}/{} offset={}s",
            target_seconds,
            target_segment + 1,
            self.media_playlist.segments.len(),
            in_segment_offset
        );

        // Reset playback state
        self.current_segment = target_segment;
        self.pending_seek_offset_seconds = Some(in_segment_offset);
        self.predownload_upcoming_segments(Self::PREFETCH_WINDOW_SEGMENTS);

        // Note: next_sample_number is tricky because we're jumping segments.
        // In fMP4, sample numbers are often global or relative to the moof.
        // Since we are using GPAC's read_new_samples, resetting next_sample_number to 1
        // and letting it find "new" samples in the newly appended segments might work
        // if we are starting a "fresh" file, but we are appending.

        // Actually, the most robust way to seek in this architecture is to stay with the current file
        // but just jump the segment download.

        // Flush decoder to clear stale buffers
        self.decoder.flush();

        Ok(())
    }

    fn description(&self) -> &str {
        "apple-music-song"
    }
}
