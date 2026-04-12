use crate::client::AppleMusicClient;
use anyhow::anyhow;
use m3u8_rs::Playlist;
use regex::Regex;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::os::fd::FromRawFd;

#[derive(Debug)]
pub struct MemFile {
    #[allow(dead_code)]
    pub fd: std::os::unix::prelude::RawFd,
    pub path: String,
    _file: Option<File>,
}

impl MemFile {
    pub fn new() -> anyhow::Result<Self> {
        let fd = unsafe {
            libc::syscall(
                libc::SYS_memfd_create,
                b"applemusicdecrypt\0".as_ptr(),
                libc::MFD_CLOEXEC,
            )
        } as std::os::unix::prelude::RawFd;
        if fd < 0 {
            return Err(anyhow!("memfd_create failed"));
        }
        let path = format!("/proc/self/fd/{}", fd);
        Ok(Self {
            fd,
            path,
            _file: Some(unsafe { File::from_raw_fd(fd) }),
        })
    }

    pub fn write_all(&mut self, data: &[u8]) -> anyhow::Result<()> {
        if let Some(f) = self._file.as_mut() {
            f.write_all(data)?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn unwrap_file(mut self) -> Option<File> {
        self._file.take()
    }
}

impl Drop for MemFile {
    fn drop(&mut self) {
        if let Some(f) = self._file.take() {
            use std::os::unix::prelude::IntoRawFd;
            let fd = f.into_raw_fd();
            unsafe {
                libc::close(fd);
            }
        }
    }
}

pub async fn extract_media_playlist(
    client: &AppleMusicClient,
    m3u8_url: &str,
    codec: Codec,
) -> crate::error::Result<(m3u8_rs::MediaPlaylist, String)> {
    let master = client.download_m3u8(m3u8_url).await?;
    let master = sanitize_master_playlist_text(&master);

    let master_url = Url::parse(m3u8_url).unwrap();
    let master_playlist = match m3u8_rs::parse_playlist_res(master.as_bytes()) {
        Ok(Playlist::MasterPlaylist(playlist)) => playlist,
        _ => todo!(),
    };


    let mut selected_codec_id = None;
    let mut selected_stream_url = None;

    for variant in &master_playlist.variants {
        let audio = variant.audio.clone().unwrap_or_default();
        if codec_matches(&audio, codec) {
            selected_codec_id = Some(audio);
            let stream_url = master_url.join(&variant.uri).unwrap().to_string();
            selected_stream_url = Some(stream_url);
            break;
        }
    }

    let stream_url = selected_stream_url.unwrap();
    let stream = client.download_m3u8(&stream_url).await?;

    let media_playlist = match m3u8_rs::parse_playlist_res(stream.as_bytes()) {
        Ok(Playlist::MediaPlaylist(playlist)) => playlist,
        _ => todo!(),
    };
    Ok((media_playlist, selected_codec_id.unwrap()))
}

fn sanitize_master_playlist_text(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut pending_stream_inf_idx: Option<usize> = None;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        // Some manifests contain a trailing malformed marker like:
        // "#P272979488 A697195462 audio songEnhance"
        // Keep this out of parsing since it can be mis-associated as a variant URI.
        if is_malformed_song_enhance_marker(line) {
            continue;
        }

        if line.starts_with("#EXT-X-STREAM-INF:") {
            if let Some(idx) = pending_stream_inf_idx.take() {
                out.remove(idx);
            }
            out.push(line.to_string());
            pending_stream_inf_idx = Some(out.len() - 1);
            continue;
        }

        if pending_stream_inf_idx.is_some() {
            // STREAM-INF must be followed by a URI line immediately.
            if is_valid_playlist_uri_line(line) {
                out.push(line.to_string());
                pending_stream_inf_idx = None;
                continue;
            }
            if let Some(idx) = pending_stream_inf_idx.take() {
                out.remove(idx);
            }
        }

        if line.starts_with('#') {
            if !line.starts_with("#EXT") {
                continue;
            }
            out.push(line.to_string());
            continue;
        }

        if is_valid_playlist_uri_line(line) {
            out.push(line.to_string());
        }
    }

    if let Some(idx) = pending_stream_inf_idx.take() {
        out.remove(idx);
    }

    out.join("\n")
}

fn is_valid_playlist_uri_line(line: &str) -> bool {
    !line.is_empty() && !line.starts_with('#') && !line.chars().any(char::is_whitespace)
}

fn is_malformed_song_enhance_marker(line: &str) -> bool {
    let mut parts = line.split_whitespace();
    match (parts.next(), parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(first), Some(second), Some(third), Some(fourth), None) => {
            first.starts_with("#P")
                && first[2..].chars().all(|c| c.is_ascii_digit())
                && second.starts_with('A')
                && second[1..].chars().all(|c| c.is_ascii_digit())
                && third.eq_ignore_ascii_case("audio")
                && fourth.eq_ignore_ascii_case("songEnhance")
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Codec {
    #[serde(rename = "alac")]
    Alac,
    #[serde(rename = "ec3")]
    Ec3,
    #[serde(rename = "ac3")]
    Ac3,
    #[serde(rename = "aac-binaural")]
    AacBinaural,
    #[serde(rename = "aac-downmix")]
    AacDownmix,
    #[serde(rename = "aac")]
    Aac,
    #[serde(rename = "aac-legacy")]
    AacLegacy,
}

impl Codec {
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Codec::Alac => "alac",
            Codec::Ec3 => "ec3",
            Codec::Ac3 => "ac3",
            Codec::AacBinaural => "aac-binaural",
            Codec::AacDownmix => "aac-downmix",
            Codec::Aac => "aac",
            Codec::AacLegacy => "aac-legacy",
        }
    }
}

fn codec_matches(codec_id: &str, codec: Codec) -> bool {
    let pattern = match codec {
        Codec::Alac => r"audio-alac-stereo-\d{5,6}-\d{2}$",
        Codec::Ec3 => r"audio-(atmos|ec3)-\d{4}$",
        Codec::Ac3 => r"audio-ac3-\d{3}$",
        Codec::AacBinaural => r"audio-stereo-\d{3}-binaural$",
        Codec::AacDownmix => r"audio-stereo-\d{3}-downmix$",
        Codec::Aac | Codec::AacLegacy => r"audio-stereo-\d{3}$",
    };
    Regex::new(pattern).unwrap().is_match(codec_id)
}

pub fn parse_alac_quality_from_codec_id(codec_id: &str) -> (Option<u32>, Option<u8>) {
    let pattern = Regex::new(r"^audio-alac-stereo-(\d{5,6})-(\d{2})$").unwrap();
    let captures = match pattern.captures(codec_id) {
        Some(captures) => captures,
        None => return (None, None),
    };
    let sample_rate = captures.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
    let bit_depth = captures.get(2).and_then(|m| m.as_str().parse::<u8>().ok());
    (sample_rate, bit_depth)
}

pub fn collect_key_uris(playlist: &m3u8_rs::MediaPlaylist) -> Vec<String> {
    let mut keys = vec!["skd://itunes.apple.com/P000000000/s1/e1".to_string()];
    for segment in &playlist.segments {
        if let Some(uri) = segment_key_uri(segment) {
            if uri.starts_with("skd://") || uri.starts_with("skd?://") {
                if !keys.iter().any(|existing| existing == uri) {
                    keys.push(uri.to_string());
                }
            }
        }
    }
    keys
}

pub fn segment_key_uri(segment: &m3u8_rs::MediaSegment) -> Option<&str> {
    segment.key.as_ref().and_then(|key| key.uri.as_deref())
}

pub(crate) fn generate_alac_extradata(sample_rate: u32, channels: u8, bit_depth: u8, bitrate: u32) -> Vec<u8> {
    let mut data = vec![0u8; 36];

    // ALAC atom header
    data[0..4].copy_from_slice(&36u32.to_be_bytes());
    data[4..8].copy_from_slice(b"alac");

    // Samples per frame defaults to 4096 for ALAC
    data[12..16].copy_from_slice(&4096u32.to_be_bytes());
    data[16] = 0;
    data[17] = bit_depth;
    data[18] = 40;
    data[19] = 10;
    data[20] = 14;
    data[21] = channels;
    data[26] = 0x60;
    data[27] = 0x04;

    // Per-file average bitrate
    data[28..32].copy_from_slice(&bitrate.to_be_bytes());
    data[32..36].copy_from_slice(&sample_rate.to_be_bytes());
    data
}
