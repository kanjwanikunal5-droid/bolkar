//! Meeting recording: microphone ("Me") and computer sound ("Others", WASAPI loopback) as two
//! separate 16 kHz WAV files, written every few seconds so a crash loses at most that much.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;

use app_asr::audio::{resample, rms};
use app_asr::SAMPLE_RATE;

const FLUSH_EVERY: Duration = Duration::from_secs(5);

#[derive(Default)]
struct Track {
    pending: Vec<f32>,
    rate: u32,
    level: f32,
    failed: bool,
}

pub struct Recording {
    pub id: String,
    pub dir: PathBuf,
    started: Instant,
    stop: Arc<AtomicBool>,
    mic: Arc<Mutex<Track>>,
    sys: Arc<Mutex<Track>>,
    worker: Option<std::thread::JoinHandle<Result<(), String>>>,
    seconds: Arc<AtomicU32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveStatus {
    pub recording: bool,
    pub seconds: u32,
    pub mic_level: f32,
    pub system_level: f32,
    /// Plain-language problems, e.g. a source that stopped delivering audio.
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finished {
    pub id: String,
    pub mic_path: Option<String>,
    pub system_path: Option<String>,
    pub duration_ms: u64,
}

fn push_mono(track: &Arc<Mutex<Track>>, data: &[f32], channels: usize) {
    let mono: Vec<f32> = data
        .chunks(channels.max(1))
        .map(|f| f.iter().sum::<f32>() / channels.max(1) as f32)
        .collect();
    if let Ok(mut t) = track.lock() {
        t.level = rms(&mono);
        t.pending.extend_from_slice(&mono);
    }
}

/// Opens a capture stream. `loopback` = record what the speakers play.
fn open(track: Arc<Mutex<Track>>, loopback: bool) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = if loopback {
        host.default_output_device()
            .ok_or("No speakers or headphones found for computer sound.")?
    } else {
        host.default_input_device().ok_or("No microphone found.")?
    };
    let config = if loopback {
        device.default_output_config()
    } else {
        device.default_input_config()
    }
    .map_err(|e| e.to_string())?;
    let channels = config.channels() as usize;
    if let Ok(mut t) = track.lock() {
        t.rate = config.sample_rate().0;
    }
    let err_track = Arc::clone(&track);
    let on_error = move |_e: cpal::StreamError| {
        if let Ok(mut t) = err_track.lock() {
            t.failed = true;
        }
    };
    let stream_config: cpal::StreamConfig = config.clone().into();
    // On Windows, an input stream on an output device records in loopback mode.
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |d: &[f32], _: &cpal::InputCallbackInfo| push_mono(&track, d, channels),
            on_error,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |d: &[i16], _: &cpal::InputCallbackInfo| {
                let f: Vec<f32> = d.iter().map(|s| f32::from(*s) / 32768.0).collect();
                push_mono(&track, &f, channels)
            },
            on_error,
            None,
        ),
        other => return Err(format!("Unsupported audio format {other:?}")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

fn writer(path: &Path) -> Result<hound::WavWriter<std::io::BufWriter<std::fs::File>>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    hound::WavWriter::create(path, spec).map_err(|e| e.to_string())
}

fn drain(track: &Arc<Mutex<Track>>, w: &mut hound::WavWriter<std::io::BufWriter<std::fs::File>>) {
    let (pending, rate) = match track.lock() {
        Ok(mut t) => (std::mem::take(&mut t.pending), t.rate),
        Err(_) => return,
    };
    if pending.is_empty() || rate == 0 {
        return;
    }
    for s in resample(&pending, rate, SAMPLE_RATE) {
        let _ = w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16);
    }
    // Rewrites the WAV header so the file is valid even if the app stops suddenly.
    let _ = w.flush();
}

impl Recording {
    pub fn start(meetings_dir: &Path, mic: bool, system: bool) -> Result<Self, String> {
        if !mic && !system {
            return Err("Choose at least one sound source.".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let dir = meetings_dir.join(&id);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mic_track = Arc::new(Mutex::new(Track::default()));
        let sys_track = Arc::new(Mutex::new(Track::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let seconds = Arc::new(AtomicU32::new(0));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();

        let worker = {
            let (mic_t, sys_t, stop_t, sec_t, dir_t) = (
                Arc::clone(&mic_track),
                Arc::clone(&sys_track),
                Arc::clone(&stop),
                Arc::clone(&seconds),
                dir.clone(),
            );
            std::thread::Builder::new()
                .name("meeting-recorder".into())
                .spawn(move || {
                    // Streams must live on this thread.
                    let mic_stream = if mic {
                        Some(open(Arc::clone(&mic_t), false))
                    } else {
                        None
                    };
                    let sys_stream = if system {
                        Some(open(Arc::clone(&sys_t), true))
                    } else {
                        None
                    };
                    let failure = [&mic_stream, &sys_stream]
                        .iter()
                        .filter_map(|s| s.as_ref().and_then(|r| r.as_ref().err().cloned()))
                        .next();
                    if let Some(e) = failure {
                        let _ = ready_tx.send(Err(e.clone()));
                        return Err(e);
                    }
                    let mut mic_w = if mic {
                        Some(writer(&dir_t.join("mic.wav"))?)
                    } else {
                        None
                    };
                    let mut sys_w = if system {
                        Some(writer(&dir_t.join("system.wav"))?)
                    } else {
                        None
                    };
                    let _ = ready_tx.send(Ok(()));
                    let started = Instant::now();
                    let mut last_flush = Instant::now();
                    while !stop_t.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(200));
                        sec_t.store(started.elapsed().as_secs() as u32, Ordering::Relaxed);
                        if last_flush.elapsed() >= FLUSH_EVERY {
                            if let Some(w) = mic_w.as_mut() {
                                drain(&mic_t, w);
                            }
                            if let Some(w) = sys_w.as_mut() {
                                drain(&sys_t, w);
                            }
                            last_flush = Instant::now();
                        }
                    }
                    drop(mic_stream);
                    drop(sys_stream);
                    if let Some(mut w) = mic_w {
                        drain(&mic_t, &mut w);
                        w.finalize().map_err(|e| e.to_string())?;
                    }
                    if let Some(mut w) = sys_w {
                        drain(&sys_t, &mut w);
                        w.finalize().map_err(|e| e.to_string())?;
                    }
                    Ok(())
                })
                .map_err(|e| e.to_string())?
        };
        match ready_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err("The sound devices did not start.".into()),
        }
        Ok(Self {
            id,
            dir,
            started: Instant::now(),
            stop,
            mic: mic_track,
            sys: sys_track,
            worker: Some(worker),
            seconds,
        })
    }

    pub fn status(&self) -> LiveStatus {
        let read =
            |t: &Arc<Mutex<Track>>| t.lock().map(|t| (t.level, t.failed)).unwrap_or((0.0, true));
        let (mic_level, mic_failed) = read(&self.mic);
        let (system_level, sys_failed) = read(&self.sys);
        let mut problems = Vec::new();
        if mic_failed {
            problems
                .push("The microphone stopped (unplugged or taken by another app).".to_string());
        }
        if sys_failed {
            problems.push("Computer sound stopped (the output device changed).".to_string());
        }
        LiveStatus {
            recording: true,
            seconds: self.seconds.load(Ordering::Relaxed),
            mic_level,
            system_level,
            problems,
        }
    }

    pub fn stop(mut self) -> Result<Finished, String> {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(w) = self.worker.take() {
            w.join()
                .map_err(|_| "recorder thread crashed".to_string())??;
        }
        let path = |name: &str| {
            let p = self.dir.join(name);
            p.is_file().then(|| p.to_string_lossy().into_owned())
        };
        Ok(Finished {
            id: self.id.clone(),
            mic_path: path("mic.wav"),
            system_path: path("system.wav"),
            duration_ms: self.started.elapsed().as_millis() as u64,
        })
    }
}

/// Splits a recording into pieces of at most ~30 s for Whisper, cutting at the quietest
/// moment between 20 and 30 s so words are not cut in half. Returns (start sample, samples).
pub fn split_for_recognition(samples: &[f32]) -> Vec<(usize, &[f32])> {
    let rate = SAMPLE_RATE as usize;
    let (min_len, max_len, window) = (20 * rate, 29 * rate, rate / 10);
    let mut pieces = Vec::new();
    let mut start = 0;
    while start < samples.len() {
        let remaining = samples.len() - start;
        if remaining <= max_len {
            pieces.push((start, &samples[start..]));
            break;
        }
        let mut best = start + max_len;
        let mut best_level = f32::MAX;
        let mut pos = start + min_len;
        while pos + window <= start + max_len {
            let level = rms(&samples[pos..pos + window]);
            if level < best_level {
                best_level = level;
                best = pos + window / 2;
            }
            pos += window;
        }
        pieces.push((start, &samples[start..best]));
        start = best;
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_long_audio_at_quiet_points_within_limits() {
        let rate = SAMPLE_RATE as usize;
        // 70 s of "speech" with a silent gap at 25 s and 52 s.
        let mut s: Vec<f32> = (0..70 * rate)
            .map(|i| ((i as f32) * 0.05).sin() * 0.3)
            .collect();
        for gap in [25 * rate, 52 * rate] {
            for x in &mut s[gap..gap + rate / 5] {
                *x = 0.0;
            }
        }
        let pieces = split_for_recognition(&s);
        assert_eq!(
            pieces.iter().map(|p| p.1.len()).sum::<usize>(),
            s.len(),
            "no audio lost"
        );
        for (_, p) in &pieces {
            assert!(p.len() <= 30 * rate);
        }
        // First cut lands inside the first silent gap.
        let first_cut = pieces[1].0;
        assert!(
            (25 * rate..25 * rate + rate / 5).contains(&first_cut),
            "cut at {first_cut}"
        );
        assert_eq!(split_for_recognition(&s[..10 * rate]).len(), 1);
    }
}
