//! # Sound Synthesis
//!
//! Simple sound generators for games and alerts — no external dependencies.
//!
//! ## Usage
//!
//! ```no_run
//! use minix_audio::synth;
//!
//! // Generate a 440 Hz sine wave beep (1 second, 44100 Hz, stereo 16-bit)
//! let beep = synth::sine_wave(440.0, 1.0, 44100);
//!
//! // Generate a laser sound (descending pitch)
//! let laser = synth::laser(0.3, 44100);
//!
//! // Generate an explosion sound
//! let explosion = synth::explosion(0.5, 44100);
//!
//! // Generate a power-up sound (ascending arpeggio)
//! let powerup = synth::powerup(0.4, 44100);
//! ```
//!
//! All generators return stereo interleaved 16-bit signed PCM (`Vec<u8>`)
//! — the format most commonly used by `/dev/audio`.

/// Number of channels in generated output.
const CHANNELS: usize = 2;
/// Bytes per sample (16-bit).
const BYTES_PER_SAMPLE: usize = 2;
/// Bytes per frame (stereo 16-bit).
const FRAME_SIZE: usize = CHANNELS * BYTES_PER_SAMPLE;

// ============================================================================
// Waveform generators — return stereo 16-bit LE PCM
// ============================================================================

/// Generate a sine wave.
///
/// `freq`: frequency in Hz (e.g., 440.0 = A4)
/// `duration`: length in seconds
/// `sample_rate`: sample rate in Hz (e.g., 44100, 22050)
/// Returns stereo 16-bit signed LE PCM.
pub fn sine_wave(freq: f64, duration: f64, sample_rate: u32) -> Vec<u8> {
    generate_tone(freq, duration, sample_rate, 1.0, WaveShape::Sine)
}

/// Generate a square wave (8-bit retro sound).
pub fn square_wave(freq: f64, duration: f64, sample_rate: u32) -> Vec<u8> {
    generate_tone(freq, duration, sample_rate, 1.0, WaveShape::Square)
}

/// Generate a sawtooth wave.
pub fn saw_wave(freq: f64, duration: f64, sample_rate: u32) -> Vec<u8> {
    generate_tone(freq, duration, sample_rate, 1.0, WaveShape::Saw)
}

/// Generate a triangle wave.
pub fn triangle_wave(freq: f64, duration: f64, sample_rate: u32) -> Vec<u8> {
    generate_tone(freq, duration, sample_rate, 1.0, WaveShape::Triangle)
}

/// Generate white noise.
///
/// `duration`: length in seconds
/// `sample_rate`: sample rate in Hz
pub fn noise(duration: f64, sample_rate: u32) -> Vec<u8> {
    let total_samples = (sample_rate as f64 * duration) as usize;
    let total_bytes = total_samples * FRAME_SIZE;
    let mut data = Vec::with_capacity(total_bytes);

    // Simple LCG for randomness
    let mut rng: u32 = 12345;

    for _ in 0..total_samples {
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        let sample = (rng >> 16) as i16; // 0..65535 → -32768..32767 mentally, but let's be precise
        let val = (sample.wrapping_add(32768)) as i16; // center around 0

        // Stereo: same value for both channels
        data.extend_from_slice(&val.to_le_bytes());
        data.extend_from_slice(&val.to_le_bytes());
    }

    data
}

// ============================================================================
// Tone generator with envelope
// ============================================================================

enum WaveShape {
    Sine,
    Square,
    Saw,
    Triangle,
}

/// Generate a tone with a simple linear envelope (fade in/out).
fn generate_tone(
    freq: f64,
    duration: f64,
    sample_rate: u32,
    amplitude: f64,
    shape: WaveShape,
) -> Vec<u8> {
    let total_samples = (sample_rate as f64 * duration) as usize;
    let total_bytes = total_samples * FRAME_SIZE;
    let mut data = Vec::with_capacity(total_bytes);
    let sr = sample_rate as f64;
    let amp = amplitude.min(1.0).max(0.0);
    let max_val = (amp * 32767.0) as i16;

    // Envelope: 10ms fade-in, 10ms fade-out
    let fade_samples = (sr * 0.01) as usize; // 10ms

    for i in 0..total_samples {
        let t = i as f64 / sr;
        let phase = (t * freq * 2.0 * std::f64::consts::PI) % (2.0 * std::f64::consts::PI);

        let raw = match shape {
            WaveShape::Sine => phase.sin(),
            WaveShape::Square => {
                if phase.sin() >= 0.0 { 1.0 } else { -1.0 }
            }
            WaveShape::Saw => {
                ((t * freq) % 1.0) * 2.0 - 1.0
            }
            WaveShape::Triangle => {
                let saw = ((t * freq) % 1.0) * 2.0 - 1.0;
                (saw.abs() * 2.0 - 1.0)
            }
        };

        // Apply envelope
        let mut envelope = 1.0;
        if i < fade_samples {
            envelope = i as f64 / fade_samples as f64;
        }
        if i >= total_samples - fade_samples {
            envelope = (total_samples - i) as f64 / fade_samples as f64;
        }

        let sample = (raw * envelope * max_val as f64) as i16;

        // Stereo interleaved
        data.extend_from_slice(&sample.to_le_bytes());
        data.extend_from_slice(&sample.to_le_bytes());
    }

    data
}

// ============================================================================
// Game Sound Effects
// ============================================================================

/// Laser sound — descending pitch chirp.
pub fn laser(duration: f64, sample_rate: u32) -> Vec<u8> {
    let total_samples = (sample_rate as f64 * duration) as usize;
    let sr = sample_rate as f64;
    let mut data = Vec::with_capacity(total_samples * FRAME_SIZE);
    let fade_samples = (sr * 0.005) as usize; // 5ms

    for i in 0..total_samples {
        let t = i as f64 / sr;
        let progress = i as f64 / total_samples as f64;

        // Descending frequency: 2000 Hz → 200 Hz
        let freq = 2000.0 - progress * 1800.0;
        let phase = (t * freq * 2.0 * std::f64::consts::PI) % (2.0 * std::f64::consts::PI);
        let raw = phase.sin();

        // Amplitude envelope: quick attack, decay
        let mut envelope = (1.0 - progress).max(0.0);
        if i < fade_samples {
            envelope *= i as f64 / fade_samples as f64;
        }

        let sample = (raw * envelope * 20000.0) as i16;
        data.extend_from_slice(&sample.to_le_bytes());
        data.extend_from_slice(&sample.to_le_bytes());
    }

    data
}

/// Explosion sound — filtered noise with low-frequency thump.
pub fn explosion(duration: f64, sample_rate: u32) -> Vec<u8> {
    let total_samples = (sample_rate as f64 * duration) as usize;
    let sr = sample_rate as f64;
    let mut data = Vec::with_capacity(total_samples * FRAME_SIZE);
    let mut rng: u32 = 54321;

    for i in 0..total_samples {
        let progress = i as f64 / total_samples as f64;

        // White noise
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        let noise = (rng >> 16) as f64 / 65536.0 * 2.0 - 1.0;

        // Low frequency thump (sine at 50 Hz)
        let t = i as f64 / sr;
        let thump = (t * 50.0 * 2.0 * std::f64::consts::PI).sin();

        // Mix noise (decaying) with thump (strong at start)
        let noise_amp = (1.0 - progress).max(0.0) * 0.7;
        let thump_amp = (1.0 - progress * 3.0).max(0.0).min(1.0) * 0.5;
        let raw = noise * noise_amp + thump * thump_amp;

        let sample = (raw * 30000.0) as i16;
        data.extend_from_slice(&sample.to_le_bytes());
        data.extend_from_slice(&sample.to_le_bytes());
    }

    data
}

/// Power-up sound — ascending arpeggio (C-E-G-C).
pub fn powerup(duration: f64, sample_rate: u32) -> Vec<u8> {
    // Notes: C4=261.63, E4=329.63, G4=392.00, C5=523.25
    let notes = [261.63, 329.63, 392.00, 523.25];
    let note_duration = duration / notes.len() as f64;
    let sr = sample_rate as f64;
    let total_samples = (sr * duration) as usize;
    let mut data = Vec::with_capacity(total_samples * FRAME_SIZE);
    let fade_samples = (sr * 0.005) as usize;

    for i in 0..total_samples {
        let t = i as f64 / sr;
        let note_idx = ((t / note_duration) as usize).min(notes.len() - 1);
        let note_progress = (t - note_idx as f64 * note_duration) / note_duration;
        let freq = notes[note_idx];

        let phase = (t * freq * 2.0 * std::f64::consts::PI) % (2.0 * std::f64::consts::PI);
        let raw = phase.sin();

        // Envelope per-note
        let mut envelope = 1.0;
        let local_i = (i % (sr * note_duration) as usize).max(1);
        if local_i < fade_samples {
            envelope = local_i as f64 / fade_samples as f64;
        }
        if note_progress > 0.7 {
            envelope *= (1.0 - note_progress) / 0.3;
        }

        let sample = (raw * envelope * 20000.0) as i16;
        data.extend_from_slice(&sample.to_le_bytes());
        data.extend_from_slice(&sample.to_le_bytes());
    }

    data
}

/// Retro-style jump sound — quick descending chirp (same as laser).
/// TODO: replace with a proper ascending chirp for "jump" semantics.
pub fn jump(sample_rate: u32) -> Vec<u8> {
    laser(0.15, sample_rate)
}

/// Generate a simple click sound (for UI feedback).
pub fn click(sample_rate: u32) -> Vec<u8> {
    let total_samples = (sample_rate as f64 * 0.02) as usize; // 20ms
    let sr = sample_rate as f64;
    let mut data = Vec::with_capacity(total_samples * FRAME_SIZE);

    for i in 0..total_samples {
        let t = i as f64 / sr;
        let raw = (t * 1000.0 * 2.0 * std::f64::consts::PI).sin();
        let envelope = 1.0 - i as f64 / total_samples as f64;
        let sample = (raw * envelope * 30000.0) as i16;
        data.extend_from_slice(&sample.to_le_bytes());
        data.extend_from_slice(&sample.to_le_bytes());
    }

    data
}

/// Generate an 8-bit style coin collect sound (two quick ascending beeps).
pub fn coin(sample_rate: u32) -> Vec<u8> {
    let beep_dur = 0.08;
    let gap_dur = 0.04;
    let beep1 = square_wave(988.0, beep_dur, sample_rate); // B5
    let gap = vec![0u8; (gap_dur * sample_rate as f64) as usize * FRAME_SIZE];
    let beep2 = square_wave(1319.0, beep_dur, sample_rate); // E6

    let mut data = Vec::with_capacity(beep1.len() + gap.len() + beep2.len());
    data.extend_from_slice(&beep1);
    data.extend_from_slice(&gap);
    data.extend_from_slice(&beep2);
    data
}

/// Generate a low-health warning beep (slow pulse).
pub fn warning_beep(duration: f64, sample_rate: u32) -> Vec<u8> {
    let total_samples = (sample_rate as f64 * duration) as usize;
    let sr = sample_rate as f64;
    let pulse_rate = 4.0; // 4 pulses per second
    let mut data = Vec::with_capacity(total_samples * FRAME_SIZE);

    for i in 0..total_samples {
        let t = i as f64 / sr;

        // Slow pulse
        let pulse = ((t * pulse_rate * 2.0 * std::f64::consts::PI).sin() * 0.5 + 0.5).max(0.3);
        let raw = (t * 220.0 * 2.0 * std::f64::consts::PI).sin(); // A3
        let sample = (raw * pulse * 20000.0) as i16;

        data.extend_from_slice(&sample.to_le_bytes());
        data.extend_from_slice(&sample.to_le_bytes());
    }

    data
}

/// Generate a classic NES-style death sound (descending arpeggio).
pub fn death(sample_rate: u32) -> Vec<u8> {
    let duration = 0.6;
    let sr = sample_rate as f64;
    let total_samples = (sr * duration) as usize;
    let notes = [523.25, 440.0, 349.23, 261.63]; // C5, A4, F4, C4
    let mut data = Vec::with_capacity(total_samples * FRAME_SIZE);

    for i in 0..total_samples {
        let t = i as f64 / sr;
        let progress = i as f64 / total_samples as f64;

        // Frequency descends through notes
        let note_idx = ((progress * notes.len() as f64) as usize).min(notes.len() - 1);
        let freq = notes[note_idx];

        let raw = (t * freq * 2.0 * std::f64::consts::PI).sin();
        let envelope = (1.0 - progress).max(0.0);
        let sample = (raw * envelope * 25000.0) as i16;

        data.extend_from_slice(&sample.to_le_bytes());
        data.extend_from_slice(&sample.to_le_bytes());
    }

    data
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Mix two audio buffers together (stereo 16-bit LE PCM).
///
/// Buffers must be the same length. Uses additive mixing with
/// soft clipping to prevent distortion.
pub fn mix(a: &[u8], b: &[u8]) -> Vec<u8> {
    let len = a.len().min(b.len());
    let frames = len / FRAME_SIZE;
    let mut result = Vec::with_capacity(frames * FRAME_SIZE);

    for frame in 0..frames {
        let off = frame * FRAME_SIZE;

        // Left channel
        let a_l = i16::from_le_bytes([a[off], a[off + 1]]) as i32;
        let b_l = i16::from_le_bytes([b[off], b[off + 1]]) as i32;
        let mixed_l = soft_clip(a_l + b_l);

        // Right channel
        let a_r = i16::from_le_bytes([a[off + 2], a[off + 3]]) as i32;
        let b_r = i16::from_le_bytes([b[off + 2], b[off + 3]]) as i32;
        let mixed_r = soft_clip(a_r + b_r);

        result.extend_from_slice(&(mixed_l as i16).to_le_bytes());
        result.extend_from_slice(&(mixed_r as i16).to_le_bytes());
    }

    result
}

/// Apply gain to an audio buffer.
pub fn apply_gain(data: &[u8], gain: f64) -> Vec<u8> {
    let frames = data.len() / FRAME_SIZE;
    let mut result = Vec::with_capacity(frames * FRAME_SIZE);

    for frame in 0..frames {
        let off = frame * FRAME_SIZE;
        let l = (i16::from_le_bytes([data[off], data[off + 1]]) as f64 * gain) as i16;
        let r = (i16::from_le_bytes([data[off + 2], data[off + 3]]) as f64 * gain) as i16;
        result.extend_from_slice(&l.to_le_bytes());
        result.extend_from_slice(&r.to_le_bytes());
    }

    result
}

/// Soft clip a sample to prevent harsh distortion.
fn soft_clip(sample: i32) -> i32 {
    const LIMIT: i32 = 32767;
    if sample > LIMIT {
        LIMIT - (sample - LIMIT) / 4
    } else if sample < -LIMIT {
        -LIMIT - (sample + LIMIT) / 4
    } else {
        sample
    }
}



// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn check_valid_pcm(data: &[u8]) {
        // Must be non-empty and aligned to frame size
        assert!(!data.is_empty());
        assert_eq!(data.len() % FRAME_SIZE, 0, "PCM data not aligned to frame size");
    }

    fn check_silent(data: &[u8]) -> bool {
        data.iter().all(|&b| b == 0)
    }

    #[test]
    fn test_sine_wave_creates_valid_pcm() {
        let data = sine_wave(440.0, 1.0, 44100);
        check_valid_pcm(&data);
        assert!(!check_silent(&data));
    }

    #[test]
    fn test_sine_wave_frequency() {
        // 1 second at 44100 Hz = 44100 frames = 44100*4 bytes
        let sr = 44100;
        let data = sine_wave(440.0, 1.0, sr);
        let frames = data.len() / FRAME_SIZE;
        assert_eq!(frames, sr as usize);
    }

    #[test]
    fn test_sine_wave_zero_crossings() {
        // 440 Hz sine for 1 second should have 880 zero crossings (440 cycles * 2)
        let sr = 8000; // lower rate for faster test
        let data = sine_wave(440.0, 0.5, sr);
        let frames = data.len() / FRAME_SIZE;
        let mut zero_crossings = 0;
        let mut last_sample: i32 = 0;

        for frame in 0..frames {
            let off = frame * FRAME_SIZE;
            let sample = i16::from_le_bytes([data[off], data[off + 1]]) as i32;
            if last_sample != 0 && (last_sample > 0) != (sample > 0) {
                zero_crossings += 1;
            }
            last_sample = sample;
        }

        // ~440 cycles should have ~880 zero crossings
        assert!(zero_crossings > 700 && zero_crossings < 1100,
            "Expected ~880 zero crossings, got {}", zero_crossings);
    }

    #[test]
    fn test_square_wave() {
        let data = square_wave(440.0, 0.1, 44100);
        check_valid_pcm(&data);
    }

    #[test]
    fn test_saw_wave() {
        let data = saw_wave(440.0, 0.1, 44100);
        check_valid_pcm(&data);
    }

    #[test]
    fn test_triangle_wave() {
        let data = triangle_wave(440.0, 0.1, 44100);
        check_valid_pcm(&data);
    }

    #[test]
    fn test_noise() {
        let data = noise(0.1, 44100);
        check_valid_pcm(&data);
        assert!(!check_silent(&data));
    }

    #[test]
    fn test_noise_is_different() {
        // Two noise samples should be different (random)
        let a = noise(0.05, 44100);
        let b = noise(0.05, 44100);
        assert_ne!(a, b);
    }

    #[test]
    fn test_laser() {
        let data = laser(0.3, 44100);
        check_valid_pcm(&data);
    }

    #[test]
    fn test_explosion() {
        let data = explosion(0.5, 44100);
        check_valid_pcm(&data);
    }

    #[test]
    fn test_powerup() {
        let data = powerup(0.4, 44100);
        check_valid_pcm(&data);
    }

    #[test]
    fn test_click() {
        let data = click(44100);
        check_valid_pcm(&data);
    }

    #[test]
    fn test_coin() {
        let data = coin(44100);
        check_valid_pcm(&data);
    }

    #[test]
    fn test_warning_beep() {
        let data = warning_beep(0.5, 44100);
        check_valid_pcm(&data);
    }

    #[test]
    fn test_death() {
        let data = death(44100);
        check_valid_pcm(&data);
    }

    #[test]
    fn test_silent_empty() {
        // Edge case: very short duration
        let data = sine_wave(440.0, 0.001, 44100);
        check_valid_pcm(&data);
    }

    #[test]
    fn test_mix() {
        let a = sine_wave(440.0, 0.1, 44100);
        let b = sine_wave(880.0, 0.1, 44100);
        let mixed = mix(&a, &b);
        check_valid_pcm(&mixed);
        assert_eq!(mixed.len(), a.len());
    }

    #[test]
    fn test_apply_gain() {
        let data = sine_wave(440.0, 0.1, 44100);
        let quieter = apply_gain(&data, 0.5);
        assert_eq!(quieter.len(), data.len());
        // Should be different (quieter)
        assert_ne!(quieter, data);
    }

    #[test]
    fn test_soft_clip() {
        assert_eq!(soft_clip(10000), 10000);
        assert_eq!(soft_clip(32767), 32767);
        assert!(soft_clip(40000) < 40000); // clipped
        assert!(soft_clip(-40000) > -40000); // clipped
    }

    #[test]
    fn test_duration_accuracy() {
        let sr = 22050;
        let data = sine_wave(440.0, 2.0, sr);
        let expected_frames = (2.0 * sr as f64) as usize;
        let actual_frames = data.len() / FRAME_SIZE;
        assert!((actual_frames as isize - expected_frames as isize).abs() <= 1);
    }

    #[test]
    fn test_sine_wave_amplitude() {
        let data = sine_wave(440.0, 0.05, 44100);
        let frames = data.len() / FRAME_SIZE;
        let mut max_amplitude: i16 = 0;

        for frame in 0..frames {
            let off = frame * FRAME_SIZE;
            let sample = i16::from_le_bytes([data[off], data[off + 1]]);
            max_amplitude = max_amplitude.max(sample.abs());
        }

        // Should be close to max (32767) at the peak of the sine
        assert!(max_amplitude > 30000, "Max amplitude too low: {}", max_amplitude);
    }

    #[test]
    fn test_coin_has_two_beeps() {
        let data = coin(44100);
        check_valid_pcm(&data);

        // Coin should have at least as many frames as one beep
        let beep_len = sine_wave(440.0, 0.08, 44100).len();
        assert!(data.len() > beep_len);
    }
}
