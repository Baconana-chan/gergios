//! # WAV File Loader
//!
//! Minimal WAV parser — reads RIFF/WAV PCM files with no external dependencies.
//!
//! Supports:
//! - PCM (uncompressed) format
//! - 8-bit unsigned, 16-bit signed
//! - Mono and stereo
//! - Any sample rate
//!
//! ## Usage
//!
//! ```no_run
//! use minix_audio::wav::WavData;
//!
//! let wav = WavData::from_file("/path/to/sound.wav").unwrap();
//! println!("Loaded: {} Hz, {} ch, {} bits", wav.sample_rate, wav.channels, wav.bits_per_sample);
//!
//! // Play it back
//! // audio.set_format(wav.sample_rate, wav.channels, wav.bits_per_sample, ...)?;
//! // audio.write(&wav.data)?;
//! ```

use std::fs;
use std::path::Path;

/// Error type for WAV parsing.
#[derive(Clone, Debug)]
pub enum WavError {
    /// File too small to be a valid WAV.
    TooSmall,
    /// Missing RIFF header.
    NoRiff,
    /// Not a WAV file (missing WAVE identifier).
    NotWav,
    /// Missing or invalid fmt chunk.
    BadFmt,
    /// Unsupported format (only PCM = 1 is supported).
    UnsupportedFormat(u16),
    /// Missing or invalid data chunk.
    BadData,
    /// I/O error reading file.
    Io(String),
}

impl std::fmt::Display for WavError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooSmall => write!(f, "File too small to be a valid WAV"),
            Self::NoRiff => write!(f, "Missing RIFF header"),
            Self::NotWav => write!(f, "Not a WAV file (missing WAVE identifier)"),
            Self::BadFmt => write!(f, "Missing or invalid fmt chunk"),
            Self::UnsupportedFormat(fmt) => write!(f, "Unsupported WAV format: {} (only PCM=1)", fmt),
            Self::BadData => write!(f, "Missing or invalid data chunk"),
            Self::Io(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl std::error::Error for WavError {}

impl From<std::io::Error> for WavError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Parsed WAV audio data.
#[derive(Clone, Debug)]
pub struct WavData {
    /// Sample rate in Hz (e.g., 44100).
    pub sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo).
    pub channels: u16,
    /// Bits per sample (8 or 16).
    pub bits_per_sample: u16,
    /// Raw PCM sample data (interleaved if stereo).
    pub data: Vec<u8>,
    /// Duration in seconds (approximate).
    pub duration_secs: f64,
}

impl WavData {
    /// Load a WAV file from a path.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, WavError> {
        let bytes = fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Parse WAV data from a byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WavError> {
        if bytes.len() < 44 {
            return Err(WavError::TooSmall);
        }

        // RIFF header
        if &bytes[0..4] != b"RIFF" {
            return Err(WavError::NoRiff);
        }
        if &bytes[8..12] != b"WAVE" {
            return Err(WavError::NotWav);
        }

        // Parse chunks — fmt first, then data
        let mut sample_rate: u32 = 0;
        let mut channels: u16 = 0;
        let mut bits_per_sample: u16 = 0;
        let mut data_offset: usize = 0;
        let mut data_len: usize = 0;
        let mut fmt_found = false;
        let mut data_found = false;

        let mut offset: usize = 12; // start after RIFF header
        while offset + 8 <= bytes.len() {
            let chunk_id = &bytes[offset..offset + 4];
            let chunk_size = u32::from_le_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]) as usize;

            match chunk_id {
                b"fmt " => {
                    if offset + 16 + chunk_size.min(16) > bytes.len() {
                        return Err(WavError::BadFmt);
                    }
                    let format = u16::from_le_bytes([bytes[offset + 8], bytes[offset + 9]]);
                    if format != 1 {
                        return Err(WavError::UnsupportedFormat(format));
                    }
                    channels = u16::from_le_bytes([bytes[offset + 10], bytes[offset + 11]]);
                    sample_rate = u32::from_le_bytes([
                        bytes[offset + 12],
                        bytes[offset + 13],
                        bytes[offset + 14],
                        bytes[offset + 15],
                    ]);
                    // Skip byte_rate (4) and block_align (2)
                    bits_per_sample = u16::from_le_bytes([bytes[offset + 22], bytes[offset + 23]]);
                    fmt_found = true;
                }
                b"data" => {
                    if offset + 8 + chunk_size > bytes.len() {
                        // Truncate to available data
                        data_len = bytes.len() - offset - 8;
                    } else {
                        data_len = chunk_size;
                    }
                    data_offset = offset + 8;
                    data_found = true;
                    break; // data chunk is usually last
                }
                _ => {
                    // Skip unknown chunks
                }
            }

            // Move to next chunk (chunks are 2-byte aligned)
            let chunk_total = 8 + chunk_size;
            if chunk_total == 0 {
                break;
            }
            offset += chunk_total;
            if chunk_total % 2 != 0 {
                offset += 1; // padding byte
            }
        }

        if !fmt_found {
            return Err(WavError::BadFmt);
        }
        if !data_found {
            return Err(WavError::BadData);
        }

        let data = bytes[data_offset..data_offset + data_len].to_vec();

        // Calculate duration
        let bytes_per_sec = sample_rate as f64 * channels as f64 * (bits_per_sample as f64 / 8.0);
        let duration_secs = if bytes_per_sec > 0.0 {
            data.len() as f64 / bytes_per_sec
        } else {
            0.0
        };

        Ok(Self {
            sample_rate,
            channels,
            bits_per_sample,
            data,
            duration_secs,
        })
    }

    /// Convert to mono by averaging stereo channels (if stereo).
    pub fn to_mono(&self) -> Vec<u8> {
        if self.channels == 1 {
            return self.data.clone();
        }

        let bytes_per_sample = (self.bits_per_sample / 8) as usize;
        let frame_size = bytes_per_sample * 2; // stereo frame
        let frames = self.data.len() / frame_size;
        let mut mono = Vec::with_capacity(frames * bytes_per_sample);

        for frame in 0..frames {
            let off = frame * frame_size;
            match bytes_per_sample {
                1 => {
                    // 8-bit unsigned: average left and right
                    let left = self.data[off] as u16;
                    let right = self.data[off + 1] as u16;
                    mono.push(((left + right) / 2) as u8);
                }
                2 => {
                    // 16-bit signed: average left and right
                    let left = i16::from_le_bytes([self.data[off], self.data[off + 1]]) as i32;
                    let right = i16::from_le_bytes([self.data[off + 2], self.data[off + 3]]) as i32;
                    let avg = ((left + right) / 2) as i16;
                    mono.extend_from_slice(&avg.to_le_bytes());
                }
                _ => {}
            }
        }

        mono
    }

    /// Resample to a different sample rate (simple linear interpolation).
    /// Only works for downsampling (target_rate <= sample_rate).
    pub fn resample(&self, target_rate: u32) -> Result<Self, WavError> {
        if target_rate >= self.sample_rate {
            return Ok(self.clone()); // can't upsample with simple linear
        }

        let bytes_per_sample = (self.bits_per_sample / 8) as usize;
        let frame_size = bytes_per_sample * self.channels as usize;
        let src_frames = self.data.len() / frame_size;
        let ratio = self.sample_rate as f64 / target_rate as f64;
        let dst_frames = (src_frames as f64 / ratio) as usize;
        let mut dst = Vec::with_capacity(dst_frames * frame_size);

        for dst_frame in 0..dst_frames {
            let src_pos = dst_frame as f64 * ratio;
            let src_idx = src_pos as usize;
            let frac = src_pos - src_idx as f64;

            let src_off = src_idx * frame_size;
            let next_off = ((src_idx + 1).min(src_frames - 1)) * frame_size;

            for ch in 0..frame_size {
                let a = self.data[src_off + ch] as f64;
                let b = self.data[next_off + ch] as f64;
                dst.push((a + (b - a) * frac) as u8);
            }
        }

        // Recalculate duration from new data
        let bytes_per_sec = target_rate as f64 * self.channels as f64 * (self.bits_per_sample as f64 / 8.0);
        let new_duration = if bytes_per_sec > 0.0 {
            dst.len() as f64 / bytes_per_sec
        } else {
            0.0
        };

        Ok(Self {
            sample_rate: target_rate,
            channels: self.channels,
            bits_per_sample: self.bits_per_sample,
            data: dst,
            duration_secs: new_duration,
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_wav() -> Vec<u8> {
        // Create a minimal WAV file in memory: 1 second, 44100 Hz, 16-bit mono
        let sample_rate = 44100u32;
        let channels: u16 = 1;
        let bits_per_sample: u16 = 16;
        let data_len = sample_rate as usize * 2; // 2 bytes per sample
        let data: Vec<u8> = (0..data_len).map(|i| (i % 256) as u8).collect();
        let fmt_chunk_size: u32 = 16;
        let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
        let block_align = channels * (bits_per_sample / 8);
        let data_chunk_size = data.len() as u32;
        let riff_size = 36 + data_chunk_size;

        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&riff_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&fmt_chunk_size.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_chunk_size.to_le_bytes());
        wav.extend_from_slice(&data);

        wav
    }

    #[test]
    fn test_wav_parse_valid() {
        let bytes = create_test_wav();
        let wav = WavData::from_bytes(&bytes).unwrap();
        assert_eq!(wav.sample_rate, 44100);
        assert_eq!(wav.channels, 1);
        assert_eq!(wav.bits_per_sample, 16);
        assert!((wav.duration_secs - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_wav_parse_empty() {
        let result = WavData::from_bytes(&[]);
        assert!(matches!(result, Err(WavError::TooSmall)));
    }

    #[test]
    fn test_wav_parse_no_riff() {
        let result = WavData::from_bytes(b"NOTRIFF............");
        assert!(matches!(result, Err(WavError::NoRiff)));
    }

    #[test]
    fn test_wav_to_mono_stays_mono() {
        let bytes = create_test_wav();
        let wav = WavData::from_bytes(&bytes).unwrap();
        let mono = wav.to_mono();
        assert_eq!(mono.len(), wav.data.len());
    }

    #[test]
    fn test_wav_stereo_to_mono() {
        let mut bytes = create_test_wav();
        // Change to stereo, 16-bit: 2 bytes * 2 channels = 4 bytes per frame
        bytes[22] = 2; // channels = 2
        bytes[34] = 4; // block_align = 4
        bytes[28] = 4; // byte_rate = sample_rate * 4

        let wav = WavData::from_bytes(&bytes).unwrap();
        assert_eq!(wav.channels, 2);
        let mono = wav.to_mono();
        // Should be half the size
        assert_eq!(mono.len(), wav.data.len() / 2);
    }

    #[test]
    fn test_wav_resample_down() {
        let bytes = create_test_wav();
        let wav = WavData::from_bytes(&bytes).unwrap();
        let resampled = wav.resample(22050).unwrap();
        assert_eq!(resampled.sample_rate, 22050);
        assert!(resampled.data.len() < wav.data.len());
    }

    #[test]
    fn test_wav_resample_same_rate() {
        let bytes = create_test_wav();
        let wav = WavData::from_bytes(&bytes).unwrap();
        let resampled = wav.resample(44100).unwrap();
        assert_eq!(resampled.data.len(), wav.data.len());
    }

    #[test]
    fn test_wav_unsupported_format() {
        let mut bytes = create_test_wav();
        bytes[20] = 2; // format = 2 (ADPCM, unsupported)
        let result = WavData::from_bytes(&bytes);
        assert!(matches!(result, Err(WavError::UnsupportedFormat(2))));
    }

    #[test]
    fn test_wav_data_truncated() {
        let bytes = create_test_wav();
        // Truncate to only the header
        let truncated = &bytes[..44];
        let wav = WavData::from_bytes(truncated).unwrap();
        assert_eq!(wav.data.len(), 0);
    }

    #[test]
    fn test_wav_displays() {
        let bytes = create_test_wav();
        let wav = WavData::from_bytes(&bytes).unwrap();
        let _ = format!("{:?}", wav);
    }
}
