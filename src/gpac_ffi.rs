//! Raw FFI bindings to libgpac.
//!
//! Only the subset of the libgpac API used by this project is declared here.
//! Types are kept minimal — opaque pointers where possible.

#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case, dead_code)]

use std::os::raw::{c_char, c_void};

// ── Primitive type aliases matching GPAC's typedefs ──────────────────────────

pub type u8_ = u8;
pub type u16_ = u16;
pub type u32_ = u32;
pub type u64_ = u64;
pub type s32 = i32;
pub type s64 = i64;
pub type Bool = u32;

// ── Opaque types ─────────────────────────────────────────────────────────────

/// Opaque ISO file handle.
pub enum GF_ISOFile {}

/// Opaque filter session handle.
pub enum GF_FilterSession {}

/// Opaque filter handle.
pub enum GF_Filter {}

/// Opaque descriptor base (used for gf_odf_desc_del).
pub enum GF_Descriptor {}

// ── GF_Err ───────────────────────────────────────────────────────────────────

pub type GF_Err = i32;
pub const GF_OK: GF_Err = 0;
pub const GF_EOS: GF_Err = 1; // end of session for gf_fs_run

// ── GF_ISOOpenMode ───────────────────────────────────────────────────────────

pub type GF_ISOOpenMode = u32;
pub const GF_ISOM_OPEN_READ_DUMP: GF_ISOOpenMode = 0;
pub const GF_ISOM_OPEN_READ: GF_ISOOpenMode = 1;
pub const GF_ISOM_OPEN_WRITE: GF_ISOOpenMode = 2;
pub const GF_ISOM_OPEN_EDIT: GF_ISOOpenMode = 3;
pub const GF_ISOM_WRITE_EDIT: GF_ISOOpenMode = 4;

// ── GF_MemTrackerType ────────────────────────────────────────────────────────

pub type GF_MemTrackerType = u32;
pub const GF_MemTrackerNone: GF_MemTrackerType = 0;

// ── GF_FilterSchedulerType / GF_FilterSessionFlags ───────────────────────────

pub type GF_FilterSchedulerType = u32;
pub const GF_FS_SCHEDULER_LOCK_FREE: GF_FilterSchedulerType = 0;

pub type GF_FilterSessionFlags = u32;

// ── GF_AudioSampleEntryImportMode ────────────────────────────────────────────

pub type GF_AudioSampleEntryImportMode = u32;
pub const GF_IMPORT_AUDIO_SAMPLE_ENTRY_NOT_SET: GF_AudioSampleEntryImportMode = 0;

// ── GF_ISOSample ─────────────────────────────────────────────────────────────

#[repr(C)]
pub struct GF_ISOSample {
    pub dataLength: u32_,
    pub data: *mut u8,
    pub DTS: u64_,
    pub CTS_Offset: i32,
    pub IsRAP: i32,
    pub alloc_size: u32_,
    pub nb_pack: u32_,
    pub duration: u32_,
}

// ── GF_GenericSampleDescription ──────────────────────────────────────────────

#[repr(C)]
pub struct GF_GenericSampleDescription {
    pub codec_tag: u32_,
    pub UUID: [u8; 16],
    pub version: u16_,
    pub revision: u16_,
    pub vendor_code: u32_,
    pub temporal_quality: u32_,
    pub spatial_quality: u32_,
    pub width: u16_,
    pub height: u16_,
    pub h_res: u32_,
    pub v_res: u32_,
    pub depth: u16_,
    pub color_table_index: u16_,
    pub compressor_name: [u8; 33],
    pub samplerate: u32_,
    pub nb_channels: u16_,
    pub bits_per_sample: u16_,
    pub is_qtff: Bool,
    pub lpcm_flags: u32_,
    pub extension_buf: *mut u8,
    pub extension_buf_size: u32_,
    pub ext_box_wrap: u32_,
}

// ── GF_DefaultDescriptor ─────────────────────────────────────────────────────

#[repr(C)]
pub struct GF_DefaultDescriptor {
    pub tag: u8,
    pub dataLength: u32_,
    pub data: *mut u8,
}

// ── GF_DecoderConfig ─────────────────────────────────────────────────────────

#[repr(C)]
pub struct GF_DecoderConfig {
    pub tag: u8,
    pub objectTypeIndication: u32_,
    pub streamType: u8,
    pub upstream: u8,
    pub bufferSizeDB: u32_,
    pub maxBitrate: u32_,
    pub avgBitrate: u32_,
    pub decoderSpecificInfo: *mut GF_DefaultDescriptor,
    pub predefined_rvc_config: u16_,
    pub rvc_config: *mut GF_DefaultDescriptor,
    pub profileLevelIndicationIndexDescriptor: *mut c_void, // GF_List*
    pub udta: *mut c_void,
}

// ── GF_ESD ───────────────────────────────────────────────────────────────────

/// Simplified — we only read decoderConfig from this.
/// The actual struct has more fields but we access it through pointers.
#[repr(C)]
pub struct GF_ESD {
    pub tag: u8,
    pub ESID: u16_,
    pub dependsOnESID: u16_,
    pub OCRESID: u16_,
    pub streamPriority: u8,
    pub URLString: *mut c_char,
    pub decoderConfig: *mut GF_DecoderConfig,
    // ... remaining fields omitted, not accessed
}

// ── GF_AC3StreamInfo ─────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GF_AC3StreamInfo {
    pub fscod: u8,
    pub bsid: u8,
    pub bsmod: u8,
    pub acmod: u8,
    pub lfon: u8,
    pub asvc: u8,
    pub channels: u8,
    pub surround_channels: u8,
    pub nb_dep_sub: u8,
    pub chan_loc: u16_,
}

// ── GF_AC3Config ─────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone)]
pub struct GF_AC3Config {
    pub streams: [GF_AC3StreamInfo; 8],
    pub nb_streams: u8,
    pub is_ec3: u8,
    pub brcode: u16_,
    pub sample_rate: u32_,
    pub framesize: u32_,
    pub atmos_ec3_ext: u8,
    pub complexity_index_type: u8,
}

// ── Four character code helper ───────────────────────────────────────────────

pub const fn gf_4cc(a: u8, b: u8, c: u8, d: u8) -> u32 {
    ((a as u32) << 24) | ((b as u32) << 16) | ((c as u32) << 8) | (d as u32)
}

// Well-known media types
pub const GF_ISOM_MEDIA_AUDIO: u32 = gf_4cc(b's', b'o', b'u', b'n');
pub const GF_ISOM_SUBTYPE_ENCA: u32 = gf_4cc(b'e', b'n', b'c', b'a');

// Brand codes
pub const GF_ISOM_BRAND_M4A: u32 = gf_4cc(b'M', b'4', b'A', b' ');
pub const GF_ISOM_BRAND_MP42: u32 = gf_4cc(b'm', b'p', b'4', b'2');

// Codec subtypes
pub const GF_QT_SUBTYPE_ALAC: u32 = gf_4cc(b'a', b'l', b'a', b'c');
pub const GF_ISOM_SUBTYPE_MP4A: u32 = gf_4cc(b'm', b'p', b'4', b'a');
pub const GF_ISOM_SUBTYPE_AC3: u32 = gf_4cc(b'a', b'c', b'-', b'3');
pub const GF_ISOM_SUBTYPE_EC3: u32 = gf_4cc(b'e', b'c', b'-', b'3');

// ── Extern C functions ──────────────────────────────────────────────────────

extern "C" {
    // -- System init/close --
    pub fn gf_sys_init(mem_tracker_type: GF_MemTrackerType, profile: *const c_char) -> GF_Err;
    pub fn gf_sys_close();
    pub fn gf_error_to_string(e: GF_Err) -> *const c_char;

    // -- ISO file lifecycle --
    pub fn gf_isom_open(
        fileName: *const c_char,
        OpenMode: GF_ISOOpenMode,
        tmp_dir: *const c_char,
    ) -> *mut GF_ISOFile;
    pub fn gf_isom_close(isom_file: *mut GF_ISOFile) -> GF_Err;
    pub fn gf_isom_delete(isom_file: *mut GF_ISOFile);
    pub fn gf_isom_open_progressive(
        fileName: *const c_char,
        start_range: u64_,
        end_range: u64_,
        enable_frag_templates: Bool,
        isom_file: *mut *mut GF_ISOFile,
        BytesMissing: *mut u64_,
    ) -> GF_Err;
    pub fn gf_isom_last_error(isom_file: *mut GF_ISOFile) -> GF_Err;

    // -- Reading: track/media queries --
    pub fn gf_isom_get_track_count(isom_file: *mut GF_ISOFile) -> u32_;
    pub fn gf_isom_get_media_type(isom_file: *mut GF_ISOFile, trackNumber: u32_) -> u32_;
    pub fn gf_isom_get_media_subtype(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        sampleDescriptionIndex: u32_,
    ) -> u32_;
    pub fn gf_isom_get_media_timescale(isom_file: *mut GF_ISOFile, trackNumber: u32_) -> u32_;
    pub fn gf_isom_get_sample_count(isom_file: *mut GF_ISOFile, trackNumber: u32_) -> u32_;
    pub fn gf_isom_get_sample_description_count(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
    ) -> u32_;

    pub fn gf_isom_get_track_id(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
    ) -> u32_;

    pub fn gf_isom_get_fragmented_samples_info(
        isom_file: *mut GF_ISOFile,
        trackID: u32_,
        nb_samples: *mut u32_,
        duration: *mut u64_,
    ) -> GF_Err;

    // -- Reading: samples --
    pub fn gf_isom_get_sample(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        sampleNumber: u32_,
        sampleDescriptionIndex: *mut u32_,
    ) -> *mut GF_ISOSample;
    pub fn gf_isom_get_sample_duration(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        sampleNumber: u32_,
    ) -> u32_;
    pub fn gf_isom_sample_new() -> *mut GF_ISOSample;
    pub fn gf_isom_sample_del(samp: *mut *mut GF_ISOSample);

    // -- Reading: audio info --
    pub fn gf_isom_get_audio_info(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        sampleDescriptionIndex: u32_,
        SampleRate: *mut u32_,
        Channels: *mut u32_,
        bitsPerSample: *mut u32_,
    ) -> GF_Err;

    // -- Reading: creation time --
    pub fn gf_isom_refresh_fragmented(
        isom_file: *mut GF_ISOFile,
        MissingBytes: *mut u64_,
        new_location: *const c_char,
    ) -> GF_Err;

    pub fn gf_isom_get_creation_time(
        isom_file: *mut GF_ISOFile,
        creationTime: *mut u64_,
        modificationTime: *mut u64_,
    ) -> GF_Err;

    // -- Reading: sample descriptions --
    pub fn gf_isom_get_generic_sample_description(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        sampleDescriptionIndex: u32_,
    ) -> *mut GF_GenericSampleDescription;

    pub fn gf_isom_get_esd(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        sampleDescriptionIndex: u32_,
    ) -> *mut GF_ESD;

    pub fn gf_isom_ac3_config_get(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        sampleDescriptionIndex: u32_,
    ) -> *mut GF_AC3Config;

    // -- Writing: track creation --
    pub fn gf_isom_new_track(
        isom_file: *mut GF_ISOFile,
        trackID: u32_,
        MediaType: u32_,
        TimeScale: u32_,
    ) -> u32_;

    pub fn gf_isom_set_track_enabled(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        enableTrack: Bool,
    ) -> GF_Err;

    // -- Writing: samples --
    pub fn gf_isom_add_sample(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        sampleDescriptionIndex: u32_,
        sample: *const GF_ISOSample,
    ) -> GF_Err;

    pub fn gf_isom_set_last_sample_duration(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        duration: u32_,
    ) -> GF_Err;

    // -- Writing: audio info --
    pub fn gf_isom_set_audio_info(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        sampleDescriptionIndex: u32_,
        sampleRate: u32_,
        nbChannels: u32_,
        bitsPerSample: u8,
        asemode: GF_AudioSampleEntryImportMode,
    ) -> GF_Err;

    // -- Writing: sample descriptions --
    pub fn gf_isom_new_generic_sample_description(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        URLname: *const c_char,
        URNname: *const c_char,
        udesc: *mut GF_GenericSampleDescription,
        outDescriptionIndex: *mut u32_,
    ) -> GF_Err;

    pub fn gf_isom_new_mpeg4_description(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        esd: *const GF_ESD,
        URLname: *const c_char,
        URNname: *const c_char,
        outDescriptionIndex: *mut u32_,
    ) -> GF_Err;

    pub fn gf_isom_clone_sample_description(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        orig_file: *mut GF_ISOFile,
        orig_track: u32_,
        orig_desc_index: u32_,
        URLname: *const c_char,
        URNname: *const c_char,
        outDescriptionIndex: *mut u32_,
    ) -> GF_Err;

    pub fn gf_isom_ac3_config_new(
        isom_file: *mut GF_ISOFile,
        trackNumber: u32_,
        cfg: *mut GF_AC3Config,
        URLname: *const c_char,
        URNname: *const c_char,
        outDescriptionIndex: *mut u32_,
    ) -> GF_Err;

    // -- Brands --
    pub fn gf_isom_set_brand_info(
        isom_file: *mut GF_ISOFile,
        MajorBrand: u32_,
        MinorVersion: u32_,
    ) -> GF_Err;

    pub fn gf_isom_modify_alternate_brand(
        isom_file: *mut GF_ISOFile,
        Brand: u32_,
        AddIt: Bool,
    ) -> GF_Err;

    // -- ODF descriptors --
    pub fn gf_odf_desc_del(desc: *mut GF_Descriptor);
    pub fn gf_odf_desc_esd_new(sl_predefined: u32_) -> *mut GF_ESD;

    // -- Filter session --
    pub fn gf_fs_new(
        nb_threads: s32,
        sched_type: GF_FilterSchedulerType,
        flags: GF_FilterSessionFlags,
        blacklist: *const c_char,
    ) -> *mut GF_FilterSession;
    pub fn gf_fs_del(session: *mut GF_FilterSession);
    pub fn gf_fs_load_source(
        session: *mut GF_FilterSession,
        url: *const c_char,
        args: *const c_char,
        parent_url: *const c_char,
        err: *mut GF_Err,
    ) -> *mut GF_Filter;
    pub fn gf_fs_load_destination(
        session: *mut GF_FilterSession,
        url: *const c_char,
        args: *const c_char,
        parent_url: *const c_char,
        err: *mut GF_Err,
    ) -> *mut GF_Filter;
    pub fn gf_fs_run(session: *mut GF_FilterSession) -> GF_Err;
}
