//! # minix-hda — Intel HDA Audio Driver for MINIX
//!
//! Native Rust implementation following the patterns from minix-ahci, virtio-blk,
//! and minix-nvme: `no_std`, PCI probe, BAR0 MMIO, CORB/RIRB codec communication,
//! BDL-based DMA streams, chardev interface with NetBSD audio ioctl API.
//!
//! ## Architecture
//!
//! ```ignore
//! PCI probe → BAR0 MMIO → controller reset → CORB/RIRB setup →
//! codec enumeration → stream allocation → chardev_task()
//! ```

#![cfg_attr(target_os = "minix", no_std)]

pub mod ffi;
pub mod registers;
pub mod controller;
pub mod codec;
pub mod stream;

use core::ffi::{c_char, c_int, c_uint, c_ulong, c_void};
use core::ptr;

use controller::HdaController;
use codec::HdaCodec;
use stream::{AudioStream, StreamManager, FRAGMENT_SIZE};

// ============================================================================
// Constants
// ============================================================================

/// Chardev major for audio.
const CDEV_MAJOR_AUDIO: c_int = 44;

/// Minor numbers.
const MINOR_AUDIO: c_int = 0;   // /dev/audio
const MINOR_AUDIOCTL: c_int = 1; // /dev/audioctl
const MINOR_MIXER: c_int = 2;   // /dev/mixer

/// AUDIO ioctl codes (from sys/sys/audioio.h).
const AUDIO_GETINFO: c_ulong = 0x4155_15;  // _IOR('A', 21, audio_info_t)
const AUDIO_SETINFO: c_ulong = 0x4155_16;  // _IOWR('A', 22, audio_info_t)
const AUDIO_DRAIN: c_ulong = 0x4155_17;    // _IO('A', 23)
const AUDIO_FLUSH: c_ulong = 0x4155_18;    // _IO('A', 24)
const AUDIO_WSEEK: c_ulong = 0x4155_19;    // _IOR('A', 25, u_long)
const AUDIO_RERROR: c_ulong = 0x4155_1A;   // _IOR('A', 26, int)
const AUDIO_GETDEV: c_ulong = 0x4155_1B;   // _IOR('A', 27, audio_device_t)
const AUDIO_GETENC: c_ulong = 0x4155_1C;   // _IOWR('A', 28, audio_encoding_t)
const AUDIO_GETFD: c_ulong = 0x4155_1D;    // _IOR('A', 29, int)
const AUDIO_SETFD: c_ulong = 0x4155_1E;    // _IOWR('A', 30, int)
const AUDIO_PERROR: c_ulong = 0x4155_1F;   // _IOR('A', 31, int)
const AUDIO_GETIOFFS: c_ulong = 0x4155_20; // _IOR('A', 32, audio_offset_t)
const AUDIO_GETOOFFS: c_ulong = 0x4155_21; // _IOR('A', 33, audio_offset_t)
const AUDIO_GETPROPS: c_ulong = 0x4155_22; // _IOR('A', 34, int)
const AUDIO_GETBUFINFO: c_ulong = 0x4155_23; // _IOR('A', 35, audio_info_t)
const AUDIO_MIXER_READ: c_ulong = 0x4D5F_00;  // _IOWR('M', 0, mixer_ctrl_t)
const AUDIO_MIXER_WRITE: c_ulong = 0x4D5F_01; // _IOWR('M', 1, mixer_ctrl_t)
const AUDIO_MIXER_DEVINFO: c_ulong = 0x4D5F_02; // _IOWR('M', 2, mixer_devinfo_t)

// Audio encodings (from sys/sys/audioio.h)
const AUDIO_ENCODING_SLINEAR_LE: u16 = 6;

// ============================================================================
// Audio info structures (matching NetBSD audioio.h)
// ============================================================================

#[repr(C)]
struct AudioPrinfo {
    sample_rate: u32,
    channels: u32,
    precision: u32,
    encoding: u32,
    gain: u32,
    port: u32,
    seek: u32,
    avail_ports: u32,
    buffer_size: u32,
    _ispare: [u32; 1],
    samples: u32,
    eof: u32,
    pause: u8,
    error: u8,
    waiting: u8,
    balance: u8,
    _cspare: [u8; 2],
    open: u8,
    active: u8,
}

#[repr(C)]
struct AudioInfo {
    play: AudioPrinfo,
    record: AudioPrinfo,
    monitor_gain: u32,
    blocksize: u32,
    hiwat: u32,
    lowat: u32,
    _ispare1: u32,
    mode: u32,
}

const AUMODE_PLAY: u32 = 0x01;
const AUMODE_RECORD: u32 = 0x02;

/// Audio device info string.
#[repr(C)]
struct AudioDevice {
    name: [u8; 16],
    version: [u8; 16],
    config: [u8; 16],
}

/// Audio encoding descriptor.
#[repr(C)]
struct AudioEncoding {
    index: i32,
    name: [u8; 16],
    encoding: i32,
    precision: i32,
    flags: i32,
}

/// Audio offset (for AUDIO_GETIOFFS/GETOOFFS).
#[repr(C)]
struct AudioOffset {
    samples: u32,
    deltamlks: u32,
    offset: u32,
}

/// Mixer control structures.
#[repr(C)]
#[derive(Clone, Copy)]
struct MixerLevel {
    num_channels: i32,
    level: [u8; 8],
}

#[repr(C)]
struct MixerCtrl {
    dev: i32,
    type_: i32,
    un: MixerCtrlUnion,
}

#[repr(C)]
union MixerCtrlUnion {
    ord: i32,
    mask: i32,
    value: MixerLevel,
}

impl Copy for MixerCtrlUnion {}
impl Clone for MixerCtrlUnion { fn clone(&self) -> Self { *self } }

const AUDIO_MIXER_VALUE: i32 = 3;
const AUDIO_MIXER_CLASS: i32 = 0;
const AUDIO_MIXER_ENUM: i32 = 1;

#[repr(C)]
struct MixerDevInfo {
    index: i32,
    label: [u8; 16],
    type_: i32,
    mixer_class: i32,
    next: i32,
    prev: i32,
    un: MixerDevInfoUnion,
}

#[repr(C)]
union MixerDevInfoUnion {
    e: MixerDevInfoEnum,
    s: MixerDevInfoSet,
    v: MixerDevInfoValue,
}

impl Copy for MixerDevInfoUnion {}
impl Clone for MixerDevInfoUnion { fn clone(&self) -> Self { *self } }

#[repr(C)]
#[derive(Clone, Copy)]
struct MixerDevInfoEnum {
    num_mem: i32,
    member: [MixerEnumMember; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MixerEnumMember {
    label: [u8; 16],
    ord: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MixerDevInfoSet {
    num_mem: i32,
    member: [MixerSetMember; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MixerSetMember {
    label: [u8; 16],
    mask: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MixerDevInfoValue {
    label: [u8; 16],
    num_channels: i32,
    delta: i32,
}

// ============================================================================
// Audio format helpers
// ============================================================================

fn bytes_to_sample_rate(bytes: u32, bits: u8, channels: u8) -> u32 {
    let frame_size = (bits as u32 / 8) * channels as u32;
    if frame_size == 0 { return 0; }
    bytes / frame_size
}

// ============================================================================
// Global state
// ============================================================================

static mut HDA: Option<HdaController> = None;
static mut CODEC: Option<HdaCodec> = None;
static mut STREAMS: Option<StreamManager> = None;

/// Audio stream tag for playback (allocated from HDA controller).
static mut PLAYBACK_TAG: u8 = 0;
static mut CAPTURE_TAG: u8 = 0;

/// Current audio state.
static mut SAMPLE_RATE: u32 = 48000;
static mut BITS_PER_SAMPLE: u8 = 16;
static mut CHANNELS: u8 = 2;
static mut VOLUME: u8 = 200;
static mut MUTED: bool = false;
static mut PLAYING: bool = false;
static mut RECORDING: bool = false;
static mut OPEN_COUNT: c_int = 0;
static mut TERMINATING: bool = false;

/// SAFETY: single-threaded chardriver context.
fn global_hda() -> &'static mut HdaController {
    unsafe { &mut *core::ptr::addr_of_mut!(HDA) }
        .as_mut()
        .expect("HDA: not initialized")
}

fn global_codec() -> &'static mut HdaCodec {
    unsafe { &mut *core::ptr::addr_of_mut!(CODEC) }
        .as_mut()
        .expect("HDA: codec not initialized")
}

fn global_streams() -> &'static mut StreamManager {
    unsafe { &mut *core::ptr::addr_of_mut!(STREAMS) }
        .as_mut()
        .expect("HDA: streams not initialized")
}

// ============================================================================
// Chardriver callbacks
// ============================================================================

unsafe extern "C" fn hda_open(minor: c_int, _access: c_int, _user_endpt: ffi::endpoint_t) -> c_int {
    if !(minor == MINOR_AUDIO || minor == MINOR_AUDIOCTL || minor == MINOR_MIXER) {
        return ffi::ENXIO;
    }

    if minor == MINOR_AUDIO {
        if PLAYING || RECORDING {
            return ffi::EBUSY;
        }
        // Allocate stream
        if PLAYBACK_TAG == 0 {
            let ctrl = global_hda();
            if let Some(tag) = ctrl.alloc_stream(false) {
                PLAYBACK_TAG = tag;
                let sd = ctrl.streams.iter().find(|s| s.active && s.stream_tag == tag);
                if let Some(s) = sd {
                    let stream = AudioStream::new(tag, false, s.dma_buf.phys, s.dma_buf.virt)
                        .unwrap_or_else(|| ffi::driver_panic(b"HDA: stream alloc failed\0"));
                    let mgr = global_streams();
                    for slot in mgr.streams.iter_mut() {
                        if slot.is_none() {
                            *slot = Some(stream);
                            break;
                        }
                    }
                }
            }
        }
        if PLAYBACK_TAG == 0 {
            return ffi::ENOMEM;
        }
    }

    OPEN_COUNT += 1;
    ffi::OK
}

unsafe extern "C" fn hda_close(minor: c_int) -> c_int {
    if OPEN_COUNT == 0 { return ffi::OK; }
    OPEN_COUNT -= 1;

    if minor == MINOR_AUDIO {
        // Stop playback
        if PLAYING && PLAYBACK_TAG != 0 {
            let ctrl = global_hda();
            ctrl.stop_stream(PLAYBACK_TAG);
            PLAYING = false;
        }
        if RECORDING && CAPTURE_TAG != 0 {
            let ctrl = global_hda();
            ctrl.stop_stream(CAPTURE_TAG);
            RECORDING = false;
        }

        // Free stream
        if PLAYBACK_TAG != 0 {
            let ctrl = global_hda();
            ctrl.free_stream(PLAYBACK_TAG);
            global_streams().free_by_tag(PLAYBACK_TAG);
            PLAYBACK_TAG = 0;
        }
        if CAPTURE_TAG != 0 {
            let ctrl = global_hda();
            ctrl.free_stream(CAPTURE_TAG);
            global_streams().free_by_tag(CAPTURE_TAG);
            CAPTURE_TAG = 0;
        }
    }

    if OPEN_COUNT == 0 && TERMINATING {
        if let Some(ctrl) = (*core::ptr::addr_of_mut!(HDA)).as_mut() {
            ctrl.stop();
        }
        ffi::chardriver_terminate();
    }

    ffi::OK
}

unsafe extern "C" fn hda_read(
    _minor: c_int,
    _position: u64,
    endpt: ffi::endpoint_t,
    grant: ffi::cp_grant_id_t,
    size: ffi::size_t,
    _flags: c_int,
    _id: ffi::cdev_id_t,
) -> isize {
    if !RECORDING || CAPTURE_TAG == 0 {
        return ffi::ENXIO as isize;
    }

    let mgr = global_streams();
    let stream = match mgr.find_by_tag(CAPTURE_TAG) {
        Some(s) => s,
        None => return ffi::ENXIO as isize,
    };

    stream.read_user_data(grant, endpt, size)
}

unsafe extern "C" fn hda_write(
    _minor: c_int,
    _position: u64,
    endpt: ffi::endpoint_t,
    grant: ffi::cp_grant_id_t,
    size: ffi::size_t,
    _flags: c_int,
    _id: ffi::cdev_id_t,
) -> isize {
    if !PLAYING || PLAYBACK_TAG == 0 {
        return ffi::ENXIO as isize;
    }

    let mgr = global_streams();
    let stream = match mgr.find_by_tag(PLAYBACK_TAG) {
        Some(s) => s,
        None => return ffi::ENXIO as isize,
    };

    stream.write_user_data(grant, endpt, size)
}

unsafe extern "C" fn hda_ioctl(
    minor: c_int,
    request: c_ulong,
    endpt: ffi::endpoint_t,
    grant: ffi::cp_grant_id_t,
    _flags: c_int,
    _user_endpt: ffi::c_ulong,
    _id: ffi::cdev_id_t,
) -> c_int {
    match minor {
        MINOR_AUDIO | MINOR_AUDIOCTL => handle_audio_ioctl(request, endpt, grant),
        MINOR_MIXER => handle_mixer_ioctl(request, endpt, grant),
        _ => ffi::ENXIO,
    }
}

/// Handle audio device ioctls (AUDIO_GETINFO, SETINFO, etc.).
fn handle_audio_ioctl(request: c_ulong, endpt: ffi::endpoint_t, grant: ffi::cp_grant_id_t) -> c_int {
    match request {
        AUDIO_GETINFO => {
            let sample_rate = unsafe { SAMPLE_RATE };
            let bits = unsafe { BITS_PER_SAMPLE };
            let channels = unsafe { CHANNELS };
            let volume = unsafe { VOLUME };
            let muted = unsafe { MUTED };
            let playing = unsafe { PLAYING };
            let recording = unsafe { RECORDING };

            let info = AudioInfo {
                play: AudioPrinfo {
                    sample_rate,
                    channels: channels as u32,
                    precision: bits as u32,
                    encoding: AUDIO_ENCODING_SLINEAR_LE as u32,
                    gain: volume as u32,
                    port: 0,
                    seek: 0,
                    avail_ports: 0x01, // AUDIO_SPEAKER
                    buffer_size: FRAGMENT_SIZE * 4,
                    _ispare: [0],
                    samples: 0,
                    eof: 0,
                    pause: if playing { 0 } else { 1 },
                    error: 0,
                    waiting: 0,
                    balance: 32, // AUDIO_MID_BALANCE
                    _cspare: [0; 2],
                    open: if playing || recording { 1 } else { 0 },
                    active: if playing || recording { 1 } else { 0 },
                },
                record: AudioPrinfo {
                    sample_rate,
                    channels: channels as u32,
                    precision: bits as u32,
                    encoding: AUDIO_ENCODING_SLINEAR_LE as u32,
                    gain: 200,
                    port: 0,
                    seek: 0,
                    avail_ports: 0x01, // AUDIO_MICROPHONE
                    buffer_size: FRAGMENT_SIZE * 4,
                    _ispare: [0],
                    samples: 0,
                    eof: 0,
                    pause: if recording { 0 } else { 1 },
                    error: 0,
                    waiting: 0,
                    balance: 32,
                    _cspare: [0; 2],
                    open: if recording { 1 } else { 0 },
                    active: if recording { 1 } else { 0 },
                },
                monitor_gain: 0,
                blocksize: FRAGMENT_SIZE,
                hiwat: 4,
                lowat: 2,
                _ispare1: 0,
                mode: if playing { AUMODE_PLAY } else { 0 }
                     | if recording { AUMODE_RECORD } else { 0 },
            };

            ffi::sys_safecopyto_ffi(
                endpt, grant, 0,
                &info as *const AudioInfo as *const c_void,
                core::mem::size_of::<AudioInfo>() as c_ulong,
            )
        }

        AUDIO_SETINFO => {
            let mut info = AudioInfo {
                play: AudioPrinfo {
                    sample_rate: 0, channels: 0, precision: 0, encoding: 0,
                    gain: 0, port: 0, seek: 0, avail_ports: 0, buffer_size: 0,
                    _ispare: [0], samples: 0, eof: 0, pause: 0, error: 0,
                    waiting: 0, balance: 0, _cspare: [0; 2], open: 0, active: 0,
                },
                record: AudioPrinfo {
                    sample_rate: 0, channels: 0, precision: 0, encoding: 0,
                    gain: 0, port: 0, seek: 0, avail_ports: 0, buffer_size: 0,
                    _ispare: [0], samples: 0, eof: 0, pause: 0, error: 0,
                    waiting: 0, balance: 0, _cspare: [0; 2], open: 0, active: 0,
                },
                monitor_gain: 0,
                blocksize: 0, hiwat: 0, lowat: 0, _ispare1: 0, mode: 0,
            };

            let r = ffi::sys_safecopyfrom_ffi(
                endpt, grant, 0,
                &mut info as *mut AudioInfo as *mut c_void,
                core::mem::size_of::<AudioInfo>() as c_ulong,
            );
            if r != ffi::OK { return r; }

            unsafe {
                SAMPLE_RATE = info.play.sample_rate;
                BITS_PER_SAMPLE = info.play.precision as u8;
                CHANNELS = info.play.channels as u8;
                VOLUME = info.play.gain as u8;

                // Apply volume to codec
                if let Some(codec) = (*core::ptr::addr_of_mut!(CODEC)).as_mut() {
                    let ctrl = global_hda();
                    codec.set_output_volume(ctrl, info.play.gain as u8, MUTED);
                }

                // Handle pause/play
                if info.play.pause == 0 && !PLAYING && PLAYBACK_TAG != 0 {
                    // Start stream
                    let ctrl = global_hda();
                    ctrl.start_stream(PLAYBACK_TAG, SAMPLE_RATE, BITS_PER_SAMPLE, CHANNELS);
                    PLAYING = true;

                    // Set converter stream/channel on DAC
                    let codec = global_codec();
                    if let Some(dac) = codec.dac_nid {
                        codec.set_converter(ctrl, dac, PLAYBACK_TAG, 0);
                        codec.set_converter_format(ctrl, dac, ctrl.stream_format);
                    }
                    // Enable output on speaker pin
                    if codec.num_output_pins > 0 {
                        let pin = codec.output_pins[0];
                        codec.set_pin_control(ctrl, pin, true, false, false);
                    }
                } else if info.play.pause != 0 && PLAYING && PLAYBACK_TAG != 0 {
                    let ctrl = global_hda();
                    ctrl.stop_stream(PLAYBACK_TAG);
                    PLAYING = false;
                }

                // Handle record
                if info.record.pause == 0 && !RECORDING && CAPTURE_TAG != 0 {
                    let ctrl = global_hda();
                    ctrl.start_stream(CAPTURE_TAG, SAMPLE_RATE, BITS_PER_SAMPLE, CHANNELS);
                    RECORDING = true;
                } else if info.record.pause != 0 && RECORDING && CAPTURE_TAG != 0 {
                    let ctrl = global_hda();
                    ctrl.stop_stream(CAPTURE_TAG);
                    RECORDING = false;
                }
            }

            ffi::OK
        }

        AUDIO_GETDEV => {
            let dev = AudioDevice {
                name: {
                    let mut n = [0u8; 16];
                    let name = b"Intel HDA\0";
                    n[..name.len().min(15)].copy_from_slice(&name[..name.len().min(15)]);
                    n
                },
                version: {
                    let mut v = [0u8; 16];
                    let ver = b"0.1.0\0";
                    v[..ver.len().min(15)].copy_from_slice(&ver[..ver.len().min(15)]);
                    v
                },
                config: {
                    let mut c = [0u8; 16];
                    let cfg = b"HDA\0";
                    c[..cfg.len().min(15)].copy_from_slice(&cfg[..cfg.len().min(15)]);
                    c
                },
            };

            ffi::sys_safecopyto_ffi(
                endpt, grant, 0,
                &dev as *const AudioDevice as *const c_void,
                core::mem::size_of::<AudioDevice>() as c_ulong,
            )
        }

        AUDIO_GETPROPS => {
            // Properties: full-duplex, playback, capture
            let props: i32 = 0x01 | 0x10 | 0x20; // FULLDUPLEX | PLAYBACK | CAPTURE
            ffi::sys_safecopyto_ffi(
                endpt, grant, 0,
                &props as *const i32 as *const c_void,
                core::mem::size_of::<i32>() as c_ulong,
            )
        }

        AUDIO_GETENC => {
            let mut enc = AudioEncoding {
                index: 0,
                name: {
                    let mut n = [0u8; 16];
                    let name = b"slinear_le\0";
                    n[..name.len().min(15)].copy_from_slice(&name[..name.len().min(15)]);
                    n
                },
                encoding: AUDIO_ENCODING_SLINEAR_LE as i32,
                precision: unsafe { BITS_PER_SAMPLE as i32 },
                flags: 0,
            };

            ffi::sys_safecopyto_ffi(
                endpt, grant, 0,
                &enc as *const AudioEncoding as *const c_void,
                core::mem::size_of::<AudioEncoding>() as c_ulong,
            )
        }

        AUDIO_GETBUFINFO | AUDIO_GETIOFFS | AUDIO_GETOOFFS => {
            let offset = AudioOffset {
                samples: 0,
                deltamlks: 0,
                offset: 0,
            };
            ffi::sys_safecopyto_ffi(
                endpt, grant, 0,
                &offset as *const AudioOffset as *const c_void,
                core::mem::size_of::<AudioOffset>() as c_ulong,
            )
        }

        AUDIO_FLUSH => {
            // Flush buffers
            let mgr = global_streams();
            for s in mgr.streams.iter_mut() {
                if let Some(stream) = s.as_mut() {
                    stream.reset();
                }
            }
            ffi::OK
        }

        AUDIO_DRAIN => {
            // Drain — wait for playback to complete (poll-based for now)
            ffi::OK
        }

        _ => ffi::ENOTTY,
    }
}

/// Handle mixer ioctls (AUDIO_MIXER_READ, WRITE, DEVINFO).
fn handle_mixer_ioctl(request: c_ulong, endpt: ffi::endpoint_t, grant: ffi::cp_grant_id_t) -> c_int {
    match request {
        AUDIO_MIXER_DEVINFO => {
            let mut info = MixerDevInfo {
                index: 0,
                label: {
                    let mut l = [0u8; 16];
                    let name = b"volume\0";
                    l[..name.len().min(15)].copy_from_slice(&name[..name.len().min(15)]);
                    l
                },
                type_: AUDIO_MIXER_VALUE,
                mixer_class: 0,
                next: -1, // AUDIO_MIXER_LAST
                prev: -1,
                un: MixerDevInfoUnion {
                    v: MixerDevInfoValue {
                        label: {
                            let mut l = [0u8; 16];
                            let name = b"volume\0";
                            l[..name.len().min(15)].copy_from_slice(&name[..name.len().min(15)]);
                            l
                        },
                        num_channels: 2,
                        delta: 1,
                    },
                },
            };

            ffi::sys_safecopyto_ffi(
                endpt, grant, 0,
                &info as *const MixerDevInfo as *const c_void,
                core::mem::size_of::<MixerDevInfo>() as c_ulong,
            )
        }

        AUDIO_MIXER_READ => {
            let volume = unsafe { VOLUME };
            let ctrl = MixerCtrl {
                dev: 0,
                type_: AUDIO_MIXER_VALUE,
                un: MixerCtrlUnion {
                    value: MixerLevel {
                        num_channels: 2,
                        level: [volume, volume, 0, 0, 0, 0, 0, 0],
                    },
                },
            };

            ffi::sys_safecopyto_ffi(
                endpt, grant, 0,
                &ctrl as *const MixerCtrl as *const c_void,
                core::mem::size_of::<MixerCtrl>() as c_ulong,
            )
        }

        AUDIO_MIXER_WRITE => {
            let mut ctrl = MixerCtrl {
                dev: 0,
                type_: 0,
                un: MixerCtrlUnion { ord: 0 },
            };

            let r = ffi::sys_safecopyfrom_ffi(
                endpt, grant, 0,
                &mut ctrl as *mut MixerCtrl as *mut c_void,
                core::mem::size_of::<MixerCtrl>() as c_ulong,
            );
            if r != ffi::OK { return r; }

            if ctrl.type_ == AUDIO_MIXER_VALUE {
                let vol = unsafe { ctrl.un.value.level[0] };
                unsafe {
                    VOLUME = vol;
                }
                if let Some(codec) = unsafe { (*core::ptr::addr_of_mut!(CODEC)).as_mut() } {
                    let hda_ctrl = global_hda();
                    codec.set_output_volume(hda_ctrl, vol, unsafe { MUTED });
                }
            }

            ffi::OK
        }

        _ => ffi::ENOTTY,
    }
}

// ============================================================================
// Chardriver table
// ============================================================================

static mut CDR_TABLE: ffi::Chardriver = ffi::Chardriver {
    cdr_type: CDEV_MAJOR_AUDIO,
    cdr_open: Some(hda_open),
    cdr_close: Some(hda_close),
    cdr_read: Some(hda_read),
    cdr_write: Some(hda_write),
    cdr_ioctl: Some(hda_ioctl),
};

// ============================================================================
// SEF callbacks
// ============================================================================

unsafe extern "C" fn sef_init_fresh(_type: c_int, _info: *const c_void) -> c_int {
    let verbose = ffi::env_parse_long(b"hda_verbose\0", 1, 0, 4) as u8;

    // Probe HDA controller
    let devind = match HdaController::probe(0) {
        Some(d) => d,
        None => {
            ffi::print(b"HDA: no matching device found\0");
            return ffi::ENXIO;
        }
    };

    // Initialize controller
    let mut ctrl = match HdaController::init(devind, verbose) {
        Some(c) => c,
        None => {
            ffi::print(b"HDA: controller init failed\0");
            return ffi::EIO;
        }
    };

    // Enumerate codecs
    let codec_mask = ctrl.codec_mask;
    let mut codec_found = false;
    for cad in 0..15 {
        if (codec_mask & (1 << cad)) != 0 {
            if let Some(codec) = HdaCodec::enumerate(&mut ctrl, cad as u8) {
                if codec.id.is_audio && codec.dac_nid.is_some() {
                    CODEC = Some(codec);
                    codec_found = true;
                    break;
                }
            }
        }
    }

    if !codec_found {
        ffi::print(b"HDA: no audio codec found\0");
        // We can still proceed without a codec (for testing)
    }

    // Initialize stream manager
    STREAMS = Some(StreamManager::new());

    // Store controller
    HDA = Some(ctrl);

    if verbose >= 1 {
        if codec_found {
            ffi::print(b"HDA: audio driver initialized\0");
        } else {
            ffi::print(b"HDA: initialized (no codec)\0");
        }
    }

    ffi::chardriver_announce(_type);
    ffi::OK
}

unsafe extern "C" fn sef_signal_handler(signo: c_int) {
    if signo != 15 { return; } // SIGTERM
    TERMINATING = true;
    if OPEN_COUNT == 0 {
        if let Some(ctrl) = (*core::ptr::addr_of_mut!(HDA)).as_mut() {
            ctrl.stop();
        }
    }
}

// ============================================================================
// Interrupt handler — called from chardriver interrupt callback
// ============================================================================

unsafe extern "C" fn hda_intr(_mask: c_uint) {
    if let Some(ctrl) = (*core::ptr::addr_of_mut!(HDA)).as_mut() {
        let completed = ctrl.handle_interrupt();
        if completed != 0 {
            let mgr = global_streams();
            mgr.handle_buffer_completions(completed);
        }
    }
}

// ============================================================================
// Alarm handler (periodic tick)
// ============================================================================

unsafe extern "C" fn hda_alarm(_stamp: u64) {
    // Periodic maintenance — could check for underrun recovery
}

// ============================================================================
// C-compatible main entry
// ============================================================================

/// C-compatible main entry — called from a C shim or directly.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hda_rust_main(argc: c_int, argv: *mut *mut c_char) -> c_int {
    ffi::env_setargs_ffi(argc, argv);
    ffi::sef_set_init_fresh(sef_init_fresh);
    ffi::sef_set_signal_handler(sef_signal_handler);
    ffi::sef_startup_ffi();
    let cdp = unsafe { &*core::ptr::addr_of_mut!(CDR_TABLE) };
    ffi::chardriver_task(cdp);
    ffi::OK
}

// ============================================================================
// Panic handler
// ============================================================================

#[cfg(all(not(test), target_os = "minix"))]
#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants() {
        assert_eq!(CDEV_MAJOR_AUDIO, 44);
        assert_eq!(AUDIO_ENCODING_SLINEAR_LE, 6);
    }

    #[test]
    fn audio_info_size() {
        // play + record (2×56) + monitor_gain, blocksize, hiwat, lowat, _ispare1, mode (6×4)
        assert_eq!(core::mem::size_of::<AudioInfo>(), 136);
    }

    #[test]
    fn audio_prinfo_size() {
        // 12×u32 (48) + 8×u8 (8) = 56
        assert_eq!(core::mem::size_of::<AudioPrinfo>(), 56);
    }

    #[test]
    fn audio_encoding_size() {
        assert_eq!(core::mem::size_of::<AudioEncoding>(), 32);
    }

    #[test]
    fn audio_device_size() {
        assert_eq!(core::mem::size_of::<AudioDevice>(), 48);
    }

    #[test]
    fn audio_offset_size() {
        assert_eq!(core::mem::size_of::<AudioOffset>(), 12);
    }

    #[test]
    fn mixer_ctrl_size() {
        assert!(
            core::mem::size_of::<MixerCtrl>() >= 12
        );
    }

    #[test]
    fn format_constants() {
        assert_eq!(AUMODE_PLAY, 0x01);
        assert_eq!(AUMODE_RECORD, 0x02);
    }
}
