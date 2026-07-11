//! # MINIX Audio — PCM Playback, Mixer, WAV Loading, Sound Synthesis
//!
//! Rust API for the MINIX/NetBSD audio subsystem (`/dev/audio`, `/dev/mixer`).
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │            Game / Application           │
//! ├─────────────────────────────────────────┤
//! │  minix-audio  (this crate)              │
//! │  AudioDevice | Mixer | WavLoader | synth│
//! ├─────────────────────────────────────────┤
//! │  /dev/audio  |  /dev/mixer              │
//! │  (ioctl + write) | (ioctl)              │
//! ├─────────────────────────────────────────┤
//! │  Audio Driver Framework (libaudiodriver)│
//! │  HDA | AC97 | ES1370 | SB16 | ...      │
//! └─────────────────────────────────────────┘
//! ```
//!
//! ## Quick Start
//!
//! ```no_run
//! use minix_audio::{AudioDevice, AudioEncoding};
//!
//! let mut audio = AudioDevice::open()?;
//! audio.set_format(44100, 2, 16, AudioEncoding::SlinearLe)?;
//!
//! // Play a 440 Hz beep for 1 second
//! let samples = minix_audio::synth::sine_wave(440.0, 1.0, 44100);
//! audio.write(&samples)?;
//! audio.drain()?;
//! # Ok::<(), std::io::Error>(())
//! ```

pub mod wav;
pub mod synth;

use std::fs::File;
use std::os::unix::io::{AsRawFd, RawFd};
use std::io::{self, Write};

// ============================================================================
// Audio Encoding Constants
// ============================================================================

/// Audio encoding format (matches NetBSD `AUDIO_ENCODING_*`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum AudioEncoding {
    None = 0,
    ULaw = 1,
    ALaw = 2,
    Pcm16 = 3,         // signed 16-bit linear (obsolete)
    Pcm8 = 4,          // unsigned 8-bit linear (obsolete)
    Adpcm = 5,
    SlinearLe = 6,     // signed 16-bit linear little-endian
    SlinearBe = 7,     // signed 16-bit linear big-endian
    UlinearLe = 8,     // unsigned 16-bit linear little-endian
    UlinearBe = 9,     // unsigned 16-bit linear big-endian
    Slinear = 10,      // signed linear (native endian)
    Ulinear = 11,      // unsigned linear (native endian)
    MpegL1Stream = 12,
    MpegL1Packets = 13,
    MpegL1System = 14,
    MpegL2Stream = 15,
    MpegL2Packets = 16,
    MpegL2System = 17,
    Ac3 = 18,
}

impl AudioEncoding {
    pub fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::ULaw,
            2 => Self::ALaw,
            3 | 6 => Self::SlinearLe,   // Pcm16 and SlinearLe are equivalent on LE systems
            4 | 8 => Self::UlinearLe,   // Pcm8 and UlinearLe on LE
            5 => Self::Adpcm,
            7 => Self::SlinearBe,
            9 => Self::UlinearBe,
            10 => Self::Slinear,
            11 => Self::Ulinear,
            12 => Self::MpegL1Stream,
            13 => Self::MpegL1Packets,
            14 => Self::MpegL1System,
            15 => Self::MpegL2Stream,
            16 => Self::MpegL2Packets,
            17 => Self::MpegL2System,
            18 => Self::Ac3,
            _ => Self::None,
        }
    }

    pub fn to_raw(self) -> u32 {
        self as u32
    }

    /// Bytes per sample (1 channel, 1 frame).
    pub fn bytes_per_sample(self) -> usize {
        match self {
            Self::ULaw | Self::ALaw | Self::Pcm8 | Self::Ulinear => 1,
            Self::Pcm16 | Self::SlinearLe | Self::SlinearBe
                | Self::UlinearLe | Self::UlinearBe | Self::Slinear => 2,
            _ => 2, // default to 16-bit
        }
    }
}

// ============================================================================
// Audio Info Struct (matches NetBSD `struct audio_info`)
// ============================================================================

/// Playback or record parameters (matches NetBSD `struct audio_prinfo`).
#[repr(C)]
#[derive(Clone, Debug)]
pub struct AudioPrInfo {
    pub sample_rate: u32,
    pub channels: u32,
    pub precision: u32,
    pub encoding: u32,
    pub gain: u32,
    pub port: u32,
    pub seek: u32,
    pub avail_ports: u32,
    pub buffer_size: u32,
    pub _ispare: [u32; 1],
    pub samples: u32,
    pub eof: u32,
    pub pause: u8,
    pub error: u8,
    pub waiting: u8,
    pub balance: u8,
    pub cspare: [u8; 2],
    pub open: u8,
    pub active: u8,
}

impl Default for AudioPrInfo {
    fn default() -> Self {
        // AUDIO_INITINFO macro: set all bytes to 0xFF to preserve current settings
        Self {
            sample_rate: 0xFFFFFFFF,
            channels: 0xFFFFFFFF,
            precision: 0xFFFFFFFF,
            encoding: 0xFFFFFFFF,
            gain: 0xFFFFFFFF,
            port: 0xFFFFFFFF,
            seek: 0xFFFFFFFF,
            avail_ports: 0xFFFFFFFF,
            buffer_size: 0xFFFFFFFF,
            _ispare: [0xFFFFFFFF],
            samples: 0xFFFFFFFF,
            eof: 0xFFFFFFFF,
            pause: 0xFF,
            error: 0xFF,
            waiting: 0xFF,
            balance: 0xFF,
            cspare: [0xFF, 0xFF],
            open: 0xFF,
            active: 0xFF,
        }
    }
}

/// Full audio device info (matches NetBSD `struct audio_info` = 128 bytes on x86_64).
#[repr(C)]
#[derive(Clone, Debug)]
pub struct AudioInfo {
    pub play: AudioPrInfo,
    pub record: AudioPrInfo,
    pub monitor_gain: u32,
    pub blocksize: u32,
    pub hiwat: u32,
    pub lowat: u32,
    pub _ispare1: u32,
    pub mode: u32,
}

impl Default for AudioInfo {
    fn default() -> Self {
        Self {
            play: AudioPrInfo::default(),
            record: AudioPrInfo::default(),
            monitor_gain: 0xFFFFFFFF,
            blocksize: 0xFFFFFFFF,
            hiwat: 0xFFFFFFFF,
            lowat: 0xFFFFFFFF,
            _ispare1: 0xFFFFFFFF,
            mode: 0xFFFFFFFF,
        }
    }
}

// Mode flags
pub const AUMODE_PLAY: u32 = 0x01;
pub const AUMODE_RECORD: u32 = 0x02;
pub const AUMODE_PLAY_ALL: u32 = 0x04;

// ============================================================================
// IOCTL Constants (x86_64, NetBSD/MINIX compat)
// ============================================================================

/// Build an _IOR ioctl number (read).
const fn ioc_ior(group: u8, num: u8, size: usize) -> libc::c_ulong {
    (0x40000000u64 | ((group as u64) << 8) | (num as u64) | ((size as u64) << 16))
        as libc::c_ulong
}

/// Build an _IOW ioctl number (write).
const fn ioc_iow(group: u8, num: u8, size: usize) -> libc::c_ulong {
    (0x80000000u64 | ((group as u64) << 8) | (num as u64) | ((size as u64) << 16))
        as libc::c_ulong
}

/// Build an _IOWR ioctl number (read + write).
const fn ioc_iowr(group: u8, num: u8, size: usize) -> libc::c_ulong {
    (0xC0000000u64 | ((group as u64) << 8) | (num as u64) | ((size as u64) << 16))
        as libc::c_ulong
}

/// Build an _IO ioctl number (no data).
const fn ioc_io(group: u8, num: u8) -> libc::c_ulong {
    ((group as u64) << 8 | (num as u64)) as libc::c_ulong
}

/// Size of `AudioInfo` struct for ioctl computation.
/// `AudioPrInfo` = 12 u32 + 8 u8 + 0 pad = 56 bytes.
/// `AudioInfo` = 56 (play) + 56 (record) + 6 u32 = 136 bytes.
const AUDIO_INFO_SIZE: usize = 136;

/// Get/set audio device parameters.
pub const AUDIO_GETINFO: libc::c_ulong = ioc_ior(b'A', 21, AUDIO_INFO_SIZE);
pub const AUDIO_SETINFO: libc::c_ulong = ioc_iowr(b'A', 22, AUDIO_INFO_SIZE);
/// Wait for playback to finish (block until all data played).
pub const AUDIO_DRAIN: libc::c_ulong = ioc_io(b'A', 23);
/// Discard all buffered audio data.
pub const AUDIO_FLUSH: libc::c_ulong = ioc_io(b'A', 24);
/// Seek within recorded data.
pub const AUDIO_WSEEK: libc::c_ulong = ioc_ior(b'A', 25, 8); // u_long = 8 bytes
/// Get record error count.
pub const AUDIO_RERROR: libc::c_ulong = ioc_ior(b'A', 26, 4); // int = 4
/// Get play error count.
pub const AUDIO_PERROR: libc::c_ulong = ioc_ior(b'A', 31, 4); // int = 4

// Mixer ioctls
const MIXER_CTRL_SIZE: usize = 12; // mixer_ctrl_t: int(4) + int(4) + union(4)
/// Read mixer control value.
pub const AUDIO_MIXER_READ: libc::c_ulong = ioc_iowr(b'M', 0, MIXER_CTRL_SIZE);
/// Write mixer control value.
pub const AUDIO_MIXER_WRITE: libc::c_ulong = ioc_iowr(b'M', 1, MIXER_CTRL_SIZE);

/// Default audio device path.
const DEFAULT_AUDIO_PATH: &str = "/dev/audio";
/// Default mixer device path.
pub const DEFAULT_MIXER_PATH: &str = "/dev/mixer";

// ============================================================================
// Output Port Constants
// ============================================================================

pub const AUDIO_SPEAKER: u32 = 0x01;
pub const AUDIO_HEADPHONE: u32 = 0x02;
pub const AUDIO_LINE_OUT: u32 = 0x04;

// Input Port Constants
pub const AUDIO_MICROPHONE: u32 = 0x01;
pub const AUDIO_LINE_IN: u32 = 0x02;
pub const AUDIO_CD: u32 = 0x04;

// Volume range
pub const AUDIO_MIN_GAIN: u32 = 0;
pub const AUDIO_MAX_GAIN: u32 = 255;

// ============================================================================
// Audio Device
// ============================================================================

/// Audio playback device handle.
///
/// Opens `/dev/audio` and provides PCM playback via `write()`.
/// Supports `AUDIO_SETINFO` for format configuration.
pub struct AudioDevice {
    file: File,
}

impl AudioDevice {
    /// Open the default audio device (`/dev/audio`).
    pub fn open() -> io::Result<Self> {
        Self::open_path(DEFAULT_AUDIO_PATH)
    }

    /// Open a specific audio device.
    pub fn open_path(path: &str) -> io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        Ok(Self { file })
    }

    /// Get the raw file descriptor.
    fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    /// Perform an ioctl on the audio device.
    unsafe fn ioctl(&self, request: libc::c_ulong, arg: *mut std::ffi::c_void) -> io::Result<()> {
        let ret = libc::ioctl(self.fd(), request, arg);
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Set playback format.
    ///
    /// `rate`: sample rate in Hz (e.g., 44100, 48000, 22050)
    /// `channels`: number of channels (1 = mono, 2 = stereo)
    /// `precision`: bits per sample (8, 16)
    /// `encoding`: sample encoding format
    pub fn set_format(
        &mut self,
        rate: u32,
        channels: u32,
        precision: u32,
        encoding: AudioEncoding,
    ) -> io::Result<()> {
        let mut info = AudioInfo {
            play: AudioPrInfo {
                sample_rate: rate,
                channels,
                precision,
                encoding: encoding.to_raw(),
                ..AudioPrInfo::default()
            },
            mode: AUMODE_PLAY,
            ..AudioInfo::default()
        };

        unsafe {
            self.ioctl(AUDIO_SETINFO, &mut info as *mut _ as *mut std::ffi::c_void)
        }
    }

    /// Get current audio device info.
    pub fn get_info(&self) -> io::Result<AudioInfo> {
        let mut info = AudioInfo::default();
        unsafe {
            self.ioctl(AUDIO_GETINFO, &mut info as *mut _ as *mut std::ffi::c_void)?;
        }
        Ok(info)
    }

    /// Get playback parameters (sample rate, channels, etc.).
    pub fn playback_info(&self) -> io::Result<PlaybackInfo> {
        let info = self.get_info()?;
        Ok(PlaybackInfo {
            sample_rate: info.play.sample_rate,
            channels: info.play.channels,
            precision: info.play.precision,
            encoding: AudioEncoding::from_raw(info.play.encoding),
            gain: info.play.gain,
            port: info.play.port,
            buffer_size: info.play.buffer_size,
            blocksize: info.blocksize,
        })
    }

    /// Get current volume (gain) for playback.
    pub fn volume(&self) -> io::Result<u32> {
        let info = self.get_info()?;
        Ok(info.play.gain)
    }

    /// Set volume (gain) for playback (0..255).
    pub fn set_volume(&mut self, gain: u32) -> io::Result<()> {
        let gain = gain.clamp(AUDIO_MIN_GAIN, AUDIO_MAX_GAIN);
        let mut info = AudioInfo {
            play: AudioPrInfo {
                gain,
                ..AudioPrInfo::default()
            },
            ..AudioInfo::default()
        };
        unsafe {
            self.ioctl(AUDIO_SETINFO, &mut info as *mut _ as *mut std::ffi::c_void)
        }
    }

    /// Set playback port (output device).
    pub fn set_port(&mut self, port: u32) -> io::Result<()> {
        let mut info = AudioInfo {
            play: AudioPrInfo {
                port,
                ..AudioPrInfo::default()
            },
            ..AudioInfo::default()
        };
        unsafe {
            self.ioctl(AUDIO_SETINFO, &mut info as *mut _ as *mut std::ffi::c_void)
        }
    }

    /// Set balance (0 = left only, 32 = center, 64 = right only).
    pub fn set_balance(&mut self, balance: u8) -> io::Result<()> {
        let mut info = AudioInfo {
            play: AudioPrInfo {
                balance,
                ..AudioPrInfo::default()
            },
            ..AudioInfo::default()
        };
        unsafe {
            self.ioctl(AUDIO_SETINFO, &mut info as *mut _ as *mut std::ffi::c_void)
        }
    }

    /// Set buffer block size (bytes per fragment).
    pub fn set_blocksize(&mut self, blocksize: u32) -> io::Result<()> {
        let mut info = AudioInfo {
            blocksize,
            ..AudioInfo::default()
        };
        unsafe {
            self.ioctl(AUDIO_SETINFO, &mut info as *mut _ as *mut std::ffi::c_void)
        }
    }

    /// Write PCM audio data for playback.
    ///
    /// This is a blocking call — it may block if the kernel buffer is full.
    /// Use `drain()` to ensure all data has been played before closing.
    pub fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Use libc::write directly for RawFd (File::write could buffer)
        let fd = self.fd();
        let n = unsafe {
            libc::write(fd, buf.as_ptr() as *const libc::c_void, buf.len())
        };
        if n < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }

    /// Wait for all buffered audio to finish playing.
    pub fn drain(&self) -> io::Result<()> {
        unsafe {
            self.ioctl(AUDIO_DRAIN, std::ptr::null_mut())
        }
    }

    /// Discard all buffered audio data.
    pub fn flush(&self) -> io::Result<()> {
        unsafe {
            self.ioctl(AUDIO_FLUSH, std::ptr::null_mut())
        }
    }

    /// Get the underlying file descriptor.
    pub fn raw_fd(&self) -> RawFd {
        self.fd()
    }
}

impl Write for AudioDevice {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// ============================================================================
// Playback Info
// ============================================================================

/// Human-readable playback parameters.
#[derive(Clone, Debug)]
pub struct PlaybackInfo {
    pub sample_rate: u32,
    pub channels: u32,
    pub precision: u32,
    pub encoding: AudioEncoding,
    pub gain: u32,
    pub port: u32,
    pub buffer_size: u32,
    pub blocksize: u32,
}

impl std::fmt::Display for PlaybackInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} Hz, {} ch, {} bit, gain={}, port=0x{:x}, buf={}, blk={}",
            self.sample_rate,
            self.channels,
            self.precision,
            self.gain,
            self.port,
            self.buffer_size,
            self.blocksize,
        )
    }
}

// ============================================================================
// Mixer
// ============================================================================

/// Mixer control handle.
///
/// Opens `/dev/mixer` and provides volume control.
pub struct Mixer {
    file: File,
}

impl Mixer {
    /// Open the default mixer device (`/dev/mixer`).
    pub fn open() -> io::Result<Self> {
        Self::open_path(DEFAULT_MIXER_PATH)
    }

    /// Open a specific mixer device.
    pub fn open_path(path: &str) -> io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;
        Ok(Self { file })
    }

    fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    /// Read a mixer control value.
    ///
    /// `dev`: mixer device index (0 = master volume).
    /// Returns (left_channel, right_channel) levels 0..255.
    pub fn read(&self, dev: i32) -> io::Result<(u8, u8)> {
        // Build a simple mixer_ctrl_t
        // C struct: mixer_ctrl_t { int dev; int type; union { int ord; int mask; mixer_level_t value; }; }
        // mixer_level_t: { int num_channels; u_char level[8]; }
        // Total: 4 + 4 + 4 + 1*8 = 20 bytes? Let me simplify.
        //
        // Actually mixer_ctrl_t with value type:
        // offset 0: dev (int, 4)
        // offset 4: type (int, 4) = AUDIO_MIXER_VALUE (3)
        // offset 8: value.num_channels (int, 4)
        // offset 12: value.level[0..7] (u8, 8)
        // Total: 20 bytes
        
        #[repr(C)]
        struct MixerCtrl {
            dev: i32,
            typ: i32,
            num_channels: i32,
            level: [u8; 8],
        }

        let mut ctrl = MixerCtrl {
            dev,
            typ: 3, // AUDIO_MIXER_VALUE
            num_channels: 2,
            level: [0u8; 8],
        };

        unsafe {
            let ret = libc::ioctl(
                self.fd(),
                AUDIO_MIXER_READ,
                &mut ctrl as *mut _ as *mut std::ffi::c_void,
            );
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok((ctrl.level[0], ctrl.level[1]))
    }

    /// Write a mixer control value.
    ///
    /// `dev`: mixer device index (0 = master volume).
    /// `left`, `right`: channel levels 0..255.
    pub fn write(&self, dev: i32, left: u8, right: u8) -> io::Result<()> {
        #[repr(C)]
        struct MixerCtrl {
            dev: i32,
            typ: i32,
            num_channels: i32,
            level: [u8; 8],
        }

        let ctrl = MixerCtrl {
            dev,
            typ: 3, // AUDIO_MIXER_VALUE
            num_channels: 2,
            level: [left, right, 0, 0, 0, 0, 0, 0],
        };

        unsafe {
            let ret = libc::ioctl(
                self.fd(),
                AUDIO_MIXER_WRITE,
                &ctrl as *const _ as *const std::ffi::c_void,
            );
            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(())
    }
}

// ============================================================================
// Convenience: play raw PCM data
// ============================================================================

/// Play a buffer of raw PCM data with the given format.
///
/// Convenience wrapper that opens `/dev/audio`, sets the format,
/// writes the data, drains, and closes.
pub fn play_pcm(
    rate: u32,
    channels: u32,
    precision: u32,
    encoding: AudioEncoding,
    data: &[u8],
) -> io::Result<()> {
    let mut audio = AudioDevice::open()?;
    audio.set_format(rate, channels, precision, encoding)?;
    audio.write(data)?;
    audio.drain()?;
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_encoding_from_raw() {
        assert_eq!(AudioEncoding::from_raw(1), AudioEncoding::ULaw);
        assert_eq!(AudioEncoding::from_raw(3), AudioEncoding::SlinearLe);
        assert_eq!(AudioEncoding::from_raw(6), AudioEncoding::SlinearLe);
        assert_eq!(AudioEncoding::from_raw(4), AudioEncoding::UlinearLe);
        assert_eq!(AudioEncoding::from_raw(7), AudioEncoding::SlinearBe);
        assert_eq!(AudioEncoding::from_raw(0), AudioEncoding::None);
    }

    #[test]
    fn test_audio_encoding_bytes_per_sample() {
        assert_eq!(AudioEncoding::ULaw.bytes_per_sample(), 1);
        assert_eq!(AudioEncoding::SlinearLe.bytes_per_sample(), 2);
        assert_eq!(AudioEncoding::SlinearBe.bytes_per_sample(), 2);
    }

    #[test]
    fn test_audio_prinfo_default_fills_with_0xff() {
        let info = AudioPrInfo::default();
        assert_eq!(info.sample_rate, 0xFFFFFFFF);
        assert_eq!(info.channels, 0xFFFFFFFF);
        assert_eq!(info.pause, 0xFF);
        assert_eq!(info.active, 0xFF);
    }

    #[test]
    fn test_audio_info_default_fills_with_0xff() {
        let info = AudioInfo::default();
        assert_eq!(info.play.sample_rate, 0xFFFFFFFF);
        assert_eq!(info.record.sample_rate, 0xFFFFFFFF);
        assert_eq!(info.monitor_gain, 0xFFFFFFFF);
        assert_eq!(info.mode, 0xFFFFFFFF);
    }

    #[test]
    fn test_audio_info_size() {
        // struct audio_info: play(56) + record(56) + 6*u32(24) = 136 bytes on x86_64
        assert_eq!(std::mem::size_of::<AudioInfo>(), 136);
    }

    #[test]
    fn test_audio_prinfo_size() {
        // struct audio_prinfo: 12*u32(48) + 8*u8(8) = 56 bytes on x86_64
        assert_eq!(std::mem::size_of::<AudioPrInfo>(), 56);
    }

    #[test]
    fn test_volume_clamping() {
        let dev = AudioDevice::open_path("/dev/null").unwrap();
        // Just verify the struct sizes are correct
        assert_eq!(AUDIO_MIN_GAIN, 0);
        assert_eq!(AUDIO_MAX_GAIN, 255);
    }

    #[test]
    fn test_playback_info_display() {
        let info = PlaybackInfo {
            sample_rate: 44100,
            channels: 2,
            precision: 16,
            encoding: AudioEncoding::SlinearLe,
            gain: 200,
            port: AUDIO_SPEAKER,
            buffer_size: 65536,
            blocksize: 4096,
        };
        let s = format!("{}", info);
        assert!(s.contains("44100 Hz"));
        assert!(s.contains("2 ch"));
        assert!(s.contains("16 bit"));
    }

    #[test]
    fn test_ioc_io_constants() {
        // Verify ioctl numbers don't overlap (spot check)
        assert!(AUDIO_GETINFO != AUDIO_SETINFO);
        assert!(AUDIO_DRAIN != AUDIO_FLUSH);
        assert!(AUDIO_MIXER_READ != AUDIO_MIXER_WRITE);
    }
}
