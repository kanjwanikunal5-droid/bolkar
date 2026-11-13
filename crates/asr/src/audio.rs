//! Small audio helpers: resampling to 16 kHz mono, level checks, WAV reading.

use std::path::Path;

use crate::SAMPLE_RATE;

/// Root-mean-square level of the samples (0.0 = silence, 1.0 = full scale).
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Quietest level still treated as "somebody may be speaking" (about -60 dBFS). Kept very
/// low on purpose so whispered speech is not thrown away.
pub const SIGNAL_FLOOR: f32 = 0.001;

/// True when any 100 ms window rises above the signal floor.
pub fn has_signal(samples: &[f32]) -> bool {
    samples
        .chunks((SAMPLE_RATE / 10) as usize)
        .any(|w| rms(w) > SIGNAL_FLOOR)
}

/// Interleaved multi-channel audio -> mono by averaging channels.
pub fn to_mono(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|f| f.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Linear-interpolation resampler with a simple averaging low-pass when downsampling.
/// Speech recognition quality is not sensitive to the resampler at these rates.
pub fn resample(input: &[f32], from_hz: u32, to_hz: u32) -> Vec<f32> {
    if from_hz == to_hz || input.is_empty() {
        return input.to_vec();
    }
    let ratio = f64::from(from_hz) / f64::from(to_hz);
    let out_len = (input.len() as f64 / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    let half = (ratio / 2.0).max(0.5);
    for i in 0..out_len {
        let centre = i as f64 * ratio;
        if ratio > 1.0 {
            let lo = (centre - half).max(0.0).floor() as usize;
            let hi = ((centre + half).ceil() as usize).min(input.len() - 1);
            let slice = &input[lo..=hi];
            out.push(slice.iter().sum::<f32>() / slice.len() as f32);
        } else {
            let idx = centre.floor() as usize;
            let frac = (centre - idx as f64) as f32;
            let a = input[idx.min(input.len() - 1)];
            let b = input[(idx + 1).min(input.len() - 1)];
            out.push(a + (b - a) * frac);
        }
    }
    out
}

/// Gentle gain for quiet speech: raises the level towards `target_rms`, never more than
/// `max_gain` times, never clipping.
pub fn normalise_quiet(samples: &mut [f32], target_rms: f32, max_gain: f32) {
    let level = rms(samples);
    if level <= f32::EPSILON || level >= target_rms {
        return;
    }
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let gain =
        (target_rms / level)
            .min(max_gain)
            .min(if peak > 0.0 { 0.98 / peak } else { max_gain });
    if gain > 1.0 {
        for s in samples.iter_mut() {
            *s *= gain;
        }
    }
}

/// Reads a WAV file as 16 kHz mono f32.
pub fn read_wav_16k_mono(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .filter_map(Result::ok)
                .map(|s| s as f32 / scale)
                .collect()
        }
    };
    let mono = to_mono(&samples, spec.channels as usize);
    Ok(resample(&mono, spec.sample_rate, SAMPLE_RATE))
}

/// Writes 16 kHz mono f32 audio as a 16-bit WAV file.
pub fn write_wav_16k_mono(path: &Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for s in samples {
        writer
            .write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f32, rate: u32, secs: f32, amp: f32) -> Vec<f32> {
        (0..(rate as f32 * secs) as usize)
            .map(|i| (i as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * amp)
            .collect()
    }

    #[test]
    fn resample_keeps_length_ratio_and_level() {
        let input = tone(440.0, 48_000, 1.0, 0.5);
        let out = resample(&input, 48_000, 16_000);
        assert_eq!(out.len(), 16_000);
        assert!((rms(&out) - rms(&input)).abs() < 0.03);
        let up = resample(&tone(440.0, 8_000, 1.0, 0.5), 8_000, 16_000);
        assert_eq!(up.len(), 16_000);
    }

    #[test]
    fn silence_has_no_signal_but_a_whisper_does() {
        assert!(!has_signal(&vec![0.0; 16_000]));
        assert!(!has_signal(&vec![0.0002; 16_000]));
        assert!(has_signal(&tone(300.0, 16_000, 0.5, 0.004)));
    }

    #[test]
    fn stereo_to_mono_averages() {
        assert_eq!(to_mono(&[1.0, 0.0, 0.5, 0.5], 2), vec![0.5, 0.5]);
    }

    #[test]
    fn quiet_audio_is_raised_without_clipping() {
        let mut quiet = tone(300.0, 16_000, 0.5, 0.01);
        normalise_quiet(&mut quiet, 0.05, 8.0);
        assert!(rms(&quiet) > 0.03);
        assert!(quiet.iter().all(|s| s.abs() <= 1.0));
        let mut loud = tone(300.0, 16_000, 0.5, 0.5);
        let before = rms(&loud);
        normalise_quiet(&mut loud, 0.05, 8.0);
        assert_eq!(rms(&loud), before);
    }
}
