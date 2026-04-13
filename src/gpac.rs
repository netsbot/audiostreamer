//! Safe Rust wrappers around the raw libgpac FFI.

use crate::error::{Result, StreamerError};
use crate::gpac_ffi::*;
use std::ffi::{CStr, CString};
use std::sync::Once;

// ── Global init ──────────────────────────────────────────────────────────────

static GPAC_INIT: Once = Once::new();

pub fn ensure_gpac_init() {
    GPAC_INIT.call_once(|| unsafe {
        gf_sys_init(GF_MemTrackerNone, std::ptr::null());
    });
}

// ── Error conversion ─────────────────────────────────────────────────────────

pub fn gpac_err(e: GF_Err) -> StreamerError {
    let msg = unsafe {
        let ptr = gf_error_to_string(e);
        if ptr.is_null() {
            format!("libgpac error {e}")
        } else {
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    };
    StreamerError::Message(msg)
}

fn check(e: GF_Err) -> Result<()> {
    if e == GF_OK {
        Ok(())
    } else {
        Err(gpac_err(e))
    }
}

#[derive(Debug, Clone)]
pub struct TrackSample {
    pub sample_number: u32,
    pub data: Vec<u8>,
    pub duration: u32,
    pub desc_index: usize,
}

// ── IsoFile ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct IsoFile {
    ptr: *mut GF_ISOFile,
    /// If true the file was opened for reading only and we call gf_isom_delete
    /// on drop (no-write). If false we call gf_isom_close (which also writes if
    /// the file was opened for writing/editing).
    read_only: bool,
}

unsafe impl Send for IsoFile {}

impl IsoFile {
    /// Open an existing file in READ_DUMP mode (keeps all fragment info intact,
    /// needed to read samples from fragmented MP4).
    pub fn open_read(path: &str) -> Result<Self> {
        ensure_gpac_init();
        let c_path = CString::new(path).map_err(|e| StreamerError::Message(e.to_string()))?;
        let ptr =
            unsafe { gf_isom_open(c_path.as_ptr(), GF_ISOM_OPEN_READ_DUMP, std::ptr::null()) };
        if ptr.is_null() {
            Err(StreamerError::Message(format!(
                "gf_isom_open failed for {path}"
            )))
        } else {
            let mut missing = 0u64;
            // Parse fragmented mp4 moofs if present
            unsafe { gf_isom_refresh_fragmented(ptr, &mut missing, std::ptr::null()) };
            Ok(Self {
                ptr,
                read_only: true,
            })
        }
    }

    /// Open a file for progressive/fragmented reading. Unlike `open_read`,
    /// this indexes fragment samples into the sample table so that
    /// `get_sample` works with 1-based sample numbers.
    pub fn open_progressive(path: &str) -> Result<Self> {
        ensure_gpac_init();
        let c_path = CString::new(path).map_err(|e| StreamerError::Message(e.to_string()))?;
        let mut ptr: *mut GF_ISOFile = std::ptr::null_mut();
        let mut missing = 0u64;
        let err =
            unsafe { gf_isom_open_progressive(c_path.as_ptr(), 0, 0, 0, &mut ptr, &mut missing) };
        if err != GF_OK && err != 1
        /* GF_ISOM_INCOMPLETE_FILE */
        {
            return Err(StreamerError::Message(format!(
                "gf_isom_open_progressive failed for {path}: {}",
                unsafe { CStr::from_ptr(gf_error_to_string(err)).to_string_lossy() }
            )));
        }
        if ptr.is_null() {
            return Err(StreamerError::Message(format!(
                "gf_isom_open_progressive returned null for {path}"
            )));
        }
        Ok(Self {
            ptr,
            read_only: true,
        })
    }

    pub fn refresh(&mut self) -> Result<()> {
        if self.ptr.is_null() {
            return Err(StreamerError::Message("IsoFile is closed".to_string()));
        }
        let mut missing = 0u64;
        check(unsafe { gf_isom_refresh_fragmented(self.ptr, &mut missing, std::ptr::null()) })
    }

    /// Create a new file for writing (WRITE mode — moov appended at end).
    pub fn create(path: &str) -> Result<Self> {
        ensure_gpac_init();
        let c_path = CString::new(path).map_err(|e| StreamerError::Message(e.to_string()))?;
        let ptr = unsafe { gf_isom_open(c_path.as_ptr(), GF_ISOM_OPEN_WRITE, std::ptr::null()) };
        if ptr.is_null() {
            Err(StreamerError::Message(format!(
                "gf_isom_open(WRITE) failed for {path}"
            )))
        } else {
            Ok(Self {
                ptr,
                read_only: false,
            })
        }
    }

    /// Open an existing file for editing (EDIT mode).
    pub fn open_edit(path: &str) -> Result<Self> {
        ensure_gpac_init();
        let c_path = CString::new(path).map_err(|e| StreamerError::Message(e.to_string()))?;
        let ptr = unsafe { gf_isom_open(c_path.as_ptr(), GF_ISOM_OPEN_EDIT, std::ptr::null()) };
        if ptr.is_null() {
            Err(StreamerError::Message(format!(
                "gf_isom_open(EDIT) failed for {path}"
            )))
        } else {
            Ok(Self {
                ptr,
                read_only: false,
            })
        }
    }

    /// Close and finalize the file. Consumes self.
    pub fn close(mut self) -> Result<()> {
        let e = unsafe { gf_isom_close(self.ptr) };
        self.ptr = std::ptr::null_mut(); // prevent double-close in Drop
        check(e)
    }

    // ── Track queries ────────────────────────────────────────────────────────

    pub fn track_count(&self) -> u32 {
        unsafe { gf_isom_get_track_count(self.ptr) }
    }

    pub fn media_type(&self, track: u32) -> u32 {
        unsafe { gf_isom_get_media_type(self.ptr, track) }
    }

    pub fn media_subtype(&self, track: u32, desc_idx: u32) -> u32 {
        unsafe { gf_isom_get_media_subtype(self.ptr, track, desc_idx) }
    }

    pub fn media_timescale(&self, track: u32) -> u32 {
        unsafe { gf_isom_get_media_timescale(self.ptr, track) }
    }

    pub fn sample_count(&self, track: u32) -> u32 {
        unsafe { gf_isom_get_sample_count(self.ptr, track) }
    }

    pub fn track_id(&self, track: u32) -> u32 {
        unsafe { gf_isom_get_track_id(self.ptr, track) }
    }

    pub fn fragmented_samples_count(&self, track: u32) -> u32 {
        let tid = self.track_id(track);
        let mut count = 0u32;
        let mut duration = 0u64;
        unsafe {
            gf_isom_get_fragmented_samples_info(self.ptr, tid, &mut count, &mut duration);
        }
        count
    }

    /// Best-effort count of currently available samples for a track in a
    /// growing fragmented file.
    pub fn available_sample_count(&self, track: u32) -> u32 {
        self.sample_count(track)
            .max(self.fragmented_samples_count(track))
    }

    pub fn sample_description_count(&self, track: u32) -> u32 {
        unsafe { gf_isom_get_sample_description_count(self.ptr, track) }
    }

    /// Returns `(creation_time_mac_epoch, modification_time_mac_epoch)`.
    pub fn creation_time(&self) -> (u64, u64) {
        let mut ct = 0u64;
        let mut mt = 0u64;
        unsafe { gf_isom_get_creation_time(self.ptr, &mut ct, &mut mt) };
        (ct, mt)
    }

    // ── Sample reading ───────────────────────────────────────────────────────

    /// Fetch sample `sample_number` (1-based) from `track` (1-based).
    /// Returns `(data, duration, desc_index)` where desc_index is 0-based.
    pub fn get_sample(&self, track: u32, sample_number: u32) -> Result<(Vec<u8>, u32, usize)> {
        let mut desc_idx: u32 = 0;
        let samp = unsafe { gf_isom_get_sample(self.ptr, track, sample_number, &mut desc_idx) };
        if samp.is_null() {
            let e = unsafe { gf_isom_last_error(self.ptr) };
            return Err(StreamerError::Message(format!(
                "gf_isom_get_sample({track},{sample_number}) failed: {}",
                unsafe { CStr::from_ptr(gf_error_to_string(e)).to_string_lossy() }
            )));
        }
        let s = unsafe { &*samp };
        let data = unsafe { std::slice::from_raw_parts(s.data, s.dataLength as usize) }.to_vec();
        let duration = s.duration;
        // free the sample struct
        let mut samp_ptr = samp;
        unsafe { gf_isom_sample_del(&mut samp_ptr) };
        // desc_idx is 1-based from GPAC, convert to 0-based
        Ok((data, duration, desc_idx.saturating_sub(1) as usize))
    }

    /// Refresh a fragmented MP4 and return newly appended samples since
    /// `next_sample_number`.
    pub fn read_new_samples(
        &mut self,
        track: u32,
        next_sample_number: &mut u32,
    ) -> Result<Vec<TrackSample>> {
        if *next_sample_number == 0 {
            *next_sample_number = 1;
        }

        self.refresh()?;
        let available = self.sample_count(track);

        let mut out = Vec::new();
        while *next_sample_number <= available {
            let sample_number = *next_sample_number;
            let (data, duration, desc_index) = self.get_sample(track, sample_number)?;
            out.push(TrackSample {
                sample_number,
                data,
                duration,
                desc_index,
            });
            *next_sample_number += 1;
        }
        Ok(out)
    }

    // ── Audio info ───────────────────────────────────────────────────────────

    /// Returns `(sample_rate, channels, bits_per_sample)`.
    pub fn audio_info(&self, track: u32, desc_idx: u32) -> Result<(u32, u16, u16)> {
        let mut sr = 0u32;
        let mut ch = 0u32;
        let mut bps = 0u32;
        check(unsafe {
            gf_isom_get_audio_info(self.ptr, track, desc_idx, &mut sr, &mut ch, &mut bps)
        })?;
        Ok((sr, ch as u16, bps as u16))
    }

    // ── Decoder config reading ────────────────────────────────────────────────

    /// Get the raw extension buffer from a generic sample description (e.g. ALAC box).
    pub fn generic_sample_extension(&self, track: u32, desc_idx: u32) -> Option<Vec<u8>> {
        let ptr = unsafe { gf_isom_get_generic_sample_description(self.ptr, track, desc_idx) };
        if ptr.is_null() {
            return None;
        }
        let desc = unsafe { &*ptr };
        if desc.extension_buf.is_null() || desc.extension_buf_size == 0 {
            None
        } else {
            let data = unsafe {
                std::slice::from_raw_parts(desc.extension_buf, desc.extension_buf_size as usize)
            }
            .to_vec();
            Some(data)
        }
    }

    /// Get the AAC decoder specific info bytes from the ESD of a track.
    pub fn aac_decoder_specific_info(&self, track: u32, desc_idx: u32) -> Option<Vec<u8>> {
        let esd_ptr = unsafe { gf_isom_get_esd(self.ptr, track, desc_idx) };
        if esd_ptr.is_null() {
            return None;
        }
        let esd = unsafe { &*esd_ptr };
        let result = if !esd.decoderConfig.is_null() {
            let dc = unsafe { &*esd.decoderConfig };
            if !dc.decoderSpecificInfo.is_null() {
                let dsi = unsafe { &*dc.decoderSpecificInfo };
                if !dsi.data.is_null() && dsi.dataLength > 0 {
                    Some(
                        unsafe { std::slice::from_raw_parts(dsi.data, dsi.dataLength as usize) }
                            .to_vec(),
                    )
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        // libgpac requires caller to free ESD
        unsafe { gf_odf_desc_del(esd_ptr as *mut GF_Descriptor) };
        result
    }

    /// Get AC3/EC3 config from a track.
    pub fn ac3_config(&self, track: u32, desc_idx: u32) -> Option<GF_AC3Config> {
        let ptr = unsafe { gf_isom_ac3_config_get(self.ptr, track, desc_idx) };
        if ptr.is_null() {
            None
        } else {
            let cfg = unsafe { (*ptr).clone() };
            // libgpac allocates this with gf_malloc — free via libc free
            unsafe { libc::free(ptr as *mut std::os::raw::c_void) };
            Some(cfg)
        }
    }

    // ── Track/sample writing ─────────────────────────────────────────────────

    /// Create a new audio track. Returns 1-based track number.
    pub fn new_audio_track(&self, timescale: u32) -> Result<u32> {
        let track = unsafe { gf_isom_new_track(self.ptr, 0, GF_ISOM_MEDIA_AUDIO, timescale) };
        if track == 0 {
            Err(StreamerError::Message(
                "gf_isom_new_track failed".to_string(),
            ))
        } else {
            check(unsafe { gf_isom_set_track_enabled(self.ptr, track, 1) })?;
            Ok(track)
        }
    }

    /// Clone a sample description (e.g. `enca`) from another open IsoFile.
    /// Returns 1-based description index.
    pub fn clone_sample_description(
        &self,
        track: u32,
        src: &IsoFile,
        src_track: u32,
        src_desc_idx: u32,
    ) -> Result<u32> {
        let mut out_idx = 0u32;
        check(unsafe {
            gf_isom_clone_sample_description(
                self.ptr,
                track,
                src.ptr,
                src_track,
                src_desc_idx,
                std::ptr::null(),
                std::ptr::null(),
                &mut out_idx,
            )
        })?;
        Ok(out_idx)
    }

    /// Add a generic (e.g. ALAC) sample description. Returns 1-based index.
    pub fn new_alac_description(
        &self,
        track: u32,
        sr: u32,
        channels: u16,
        bps: u16,
        alac_extension: &[u8],
    ) -> Result<u32> {
        let mut desc = GF_GenericSampleDescription {
            codec_tag: GF_QT_SUBTYPE_ALAC,
            UUID: [0u8; 16],
            version: 0,
            revision: 0,
            vendor_code: 0,
            temporal_quality: 0,
            spatial_quality: 0,
            width: 0,
            height: 0,
            h_res: 0,
            v_res: 0,
            depth: 0,
            color_table_index: 0,
            compressor_name: [0u8; 33],
            samplerate: sr,
            nb_channels: channels,
            bits_per_sample: bps,
            is_qtff: 1,
            lpcm_flags: 0,
            extension_buf: alac_extension.as_ptr() as *mut u8,
            extension_buf_size: alac_extension.len() as u32,
            ext_box_wrap: GF_QT_SUBTYPE_ALAC,
        };
        let mut out_idx = 0u32;
        check(unsafe {
            gf_isom_new_generic_sample_description(
                self.ptr,
                track,
                std::ptr::null(),
                std::ptr::null(),
                &mut desc,
                &mut out_idx,
            )
        })?;
        // Set audio info after creating the description
        check(unsafe {
            gf_isom_set_audio_info(
                self.ptr,
                track,
                out_idx,
                sr,
                channels as u32,
                bps as u8,
                GF_IMPORT_AUDIO_SAMPLE_ENTRY_NOT_SET,
            )
        })?;
        Ok(out_idx)
    }

    /// Add a MPEG-4 (AAC) sample description using decoder specific info bytes.
    /// Returns 1-based index.
    pub fn new_aac_description(
        &self,
        track: u32,
        sr: u32,
        channels: u16,
        bps: u16,
        dsi: &[u8],
    ) -> Result<u32> {
        // Build a minimal ESD programmatically using the GPAC ODF API
        let esd = unsafe { gf_odf_desc_esd_new(2) }; // sl_predefined=2 (no SL)
        if esd.is_null() {
            return Err(StreamerError::Message(
                "gf_odf_desc_esd_new failed".to_string(),
            ));
        }
        unsafe {
            let esd_ref = &mut *esd;
            if !esd_ref.decoderConfig.is_null() {
                let dc = &mut *esd_ref.decoderConfig;
                // AAC-LC objectTypeIndication = 0x40, streamType = 0x05 (audio)
                dc.objectTypeIndication = 0x40;
                dc.streamType = 0x05;
                if !dc.decoderSpecificInfo.is_null() {
                    let dsi_desc = &mut *dc.decoderSpecificInfo;
                    // Allocate and copy DSI data using GPAC's allocator via raw ptr
                    let buf = libc::malloc(dsi.len()) as *mut u8;
                    std::ptr::copy_nonoverlapping(dsi.as_ptr(), buf, dsi.len());
                    dsi_desc.data = buf;
                    dsi_desc.dataLength = dsi.len() as u32;
                }
            }
        }
        let mut out_idx = 0u32;
        let err = unsafe {
            gf_isom_new_mpeg4_description(
                self.ptr,
                track,
                esd,
                std::ptr::null(),
                std::ptr::null(),
                &mut out_idx,
            )
        };
        unsafe { gf_odf_desc_del(esd as *mut GF_Descriptor) };
        check(err)?;
        check(unsafe {
            gf_isom_set_audio_info(
                self.ptr,
                track,
                out_idx,
                sr,
                channels as u32,
                bps as u8,
                GF_IMPORT_AUDIO_SAMPLE_ENTRY_NOT_SET,
            )
        })?;
        Ok(out_idx)
    }

    /// Add an AC3/EC3 sample description. Returns 1-based index.
    pub fn new_ac3_description(&self, track: u32, cfg: &mut GF_AC3Config) -> Result<u32> {
        let mut out_idx = 0u32;
        check(unsafe {
            gf_isom_ac3_config_new(
                self.ptr,
                track,
                cfg as *mut GF_AC3Config,
                std::ptr::null(),
                std::ptr::null(),
                &mut out_idx,
            )
        })?;
        Ok(out_idx)
    }

    /// Add a sample to the track.
    pub fn add_sample(
        &self,
        track: u32,
        desc_idx: u32,
        data: &[u8],
        dts: u64,
        duration: u32,
        is_rap: bool,
    ) -> Result<()> {
        let sample = GF_ISOSample {
            dataLength: data.len() as u32,
            data: data.as_ptr() as *mut u8,
            DTS: dts,
            CTS_Offset: 0,
            IsRAP: if is_rap { 1 } else { 0 },
            alloc_size: 0,
            nb_pack: 0,
            duration,
        };
        check(unsafe { gf_isom_add_sample(self.ptr, track, desc_idx, &sample) })
    }

    /// Set duration of last added sample explicitly.
    pub fn set_last_sample_duration(&self, track: u32, duration: u32) -> Result<()> {
        check(unsafe { gf_isom_set_last_sample_duration(self.ptr, track, duration) })
    }

    // ── Brand info ───────────────────────────────────────────────────────────

    /// Set major brand and add alternate brands.
    pub fn set_brands(&self, major: u32, minor_version: u32, alts: &[u32]) -> Result<()> {
        check(unsafe { gf_isom_set_brand_info(self.ptr, major, minor_version) })?;
        for &brand in alts {
            check(unsafe { gf_isom_modify_alternate_brand(self.ptr, brand, 1) })?;
        }
        Ok(())
    }

    /// Raw pointer access, needed for clone_sample_description across files.
    #[allow(dead_code)]
    pub(crate) fn as_ptr(&self) -> *mut GF_ISOFile {
        self.ptr
    }
}

impl Drop for IsoFile {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            if self.read_only {
                unsafe { gf_isom_delete(self.ptr) };
            } else {
                unsafe { gf_isom_close(self.ptr) };
            }
        }
    }
}

// ── Filter session helper ────────────────────────────────────────────────────

/// Run a simple `src -> dest` gpac filter pipeline.
///
/// Used for EC3/AC3 → M4A muxing (`gpac -i media -o out.m4a`).
pub fn run_filter_pipeline(src_url: &str, dest_url: &str) -> Result<()> {
    ensure_gpac_init();
    let c_src = CString::new(src_url).map_err(|e| StreamerError::Message(e.to_string()))?;
    let c_dst = CString::new(dest_url).map_err(|e| StreamerError::Message(e.to_string()))?;

    let sess = unsafe { gf_fs_new(0, GF_FS_SCHEDULER_LOCK_FREE, 0, std::ptr::null()) };
    if sess.is_null() {
        return Err(StreamerError::Message("gf_fs_new failed".to_string()));
    }

    let mut err: GF_Err = GF_OK;
    let src = unsafe {
        gf_fs_load_source(
            sess,
            c_src.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            &mut err,
        )
    };
    if src.is_null() || err != GF_OK {
        unsafe { gf_fs_del(sess) };
        return Err(StreamerError::Message(format!(
            "gf_fs_load_source failed: {err}"
        )));
    }

    let dst = unsafe {
        gf_fs_load_destination(
            sess,
            c_dst.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            &mut err,
        )
    };
    if dst.is_null() || err != GF_OK {
        unsafe { gf_fs_del(sess) };
        return Err(StreamerError::Message(format!(
            "gf_fs_load_destination failed: {err}"
        )));
    }

    let run_err = unsafe { gf_fs_run(sess) };
    unsafe { gf_fs_del(sess) };

    // gf_fs_run returns GF_EOS on normal completion
    if run_err != GF_EOS && run_err != GF_OK {
        return Err(gpac_err(run_err));
    }
    Ok(())
}
