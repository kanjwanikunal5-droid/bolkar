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

/// How much the loudness rises and falls, in dB: the spread between loud (95th percentile)
/// and quiet (10th percentile) 25 ms frames, counting only frames above the signal floor.
/// Speech rises and falls with every syllable; steady noise (fan, hiss, hum) stays flat.
pub fn modulation_db(samples: &[f32]) -> f32 {
    let frame = (SAMPLE_RATE / 40) as usize;
    let mut levels: Vec<f32> = samples
        .chunks(frame)
        .filter(|f| f.len() == frame)
        .map(rms)
        .filter(|l| *l > SIGNAL_FLOOR / 4.0)
        .collect();
    if levels.len() < 8 {
        return 0.0;
    }
    levels.sort_by(f32::total_cmp);
    let at = |p: f32| levels[((levels.len() - 1) as f32 * p) as usize].max(1e-9);
    20.0 * (at(0.95) / at(0.10)).log10()
}

/// Below this rise-and-fall the audio is treated as steady noise, not speech. Measured
/// 14 Nov 2026: white noise and mains hum 0.1-0.7 dB; every real speech clip >= 16.8 dB
/// (owner's voice notes 46-49 dB); speech buried in heavy noise down to 3.3 dB. 2 dB never
/// drops speech; fluctuating noise above it is left to Whisper and the guard.
pub const SPEECH_MODULATION_DB: f32 = 2.0;

/// False for steady noise with no speech in it (Whisper invents words for such audio).
pub fn looks_like_speech(samples: &[f32]) -> bool {
    modulation_db(samples) >= SPEECH_MODULATION_DB
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

/// Reads WAV, Ogg/Opus (WhatsApp voice notes) and other Symphonia formats as 16 kHz mono.
pub fn read_audio_16k_mono(path: &Path) -> Result<Vec<f32>, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{CodecRegistry, DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::errors::Error as SymError;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext == "wav" {
        return read_wav_16k_mono(path);
    }
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(&ext);
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("{}: unsupported audio ({e})", path.display()))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("no audio track")?;
    let track_id = track.id;
    let mut registry = CodecRegistry::new();
    registry.register_all::<symphonia_adapter_libopus::OpusDecoder>();
    symphonia::default::register_enabled_codecs(&mut registry);
    let mut decoder = registry
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("cannot decode: {e}"))?;
    let mut rate = track.codec_params.sample_rate.unwrap_or(48_000);
    let mut mono = Vec::new();
    // An error from next_packet is the end of the stream.
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                let spec = *buf.spec();
                rate = spec.rate;
                let channels = spec.channels.count();
                let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
                sb.copy_interleaved_ref(buf);
                mono.extend(to_mono(sb.samples(), channels));
            }
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode failed: {e}")),
        }
    }
    Ok(resample(&mono, rate, SAMPLE_RATE))
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
    fn steady_noise_is_not_speech_but_syllables_are() {
        let mut seed = 1u32;
        let mut noise = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
        };
        let hiss: Vec<f32> = (0..64_000).map(|_| noise() * 0.05).collect();
        assert!(!looks_like_speech(&hiss), "{}", modulation_db(&hiss));
        let hum = tone(50.0, 16_000, 4.0, 0.1);
        assert!(!looks_like_speech(&hum));
        // "Syllables": a tone switched on and off four times a second, over quiet hiss.
        let syllables: Vec<f32> = tone(220.0, 16_000, 4.0, 0.2)
            .iter()
            .enumerate()
            .map(|(i, s)| if (i / 2_000) % 2 == 0 { *s } else { 0.0 } + noise() * 0.002)
            .collect();
        assert!(
            looks_like_speech(&syllables),
            "{}",
            modulation_db(&syllables)
        );
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
