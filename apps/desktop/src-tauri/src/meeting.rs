//! Meeting recording: the microphone and the computer sound (WASAPI loopback) as two
//! separate 16 kHz WAV files, written every few seconds so a crash loses at most that much.
//! The tracks are labelled by source ("Microphone", "System audio"), never as people.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use serde::Serialize;

use app_asr::audio::{resample, rms};
use app_asr::SAMPLE_RATE;

const FLUSH_EVERY: Duration = Duration::from_secs(5);

#[derive(Default)]
struct Track {
    pending: Vec<f32>,
    rate: u32,
    level: f32,
    /// The device stopped delivering audio (unplugged, taken by another app).
    failed: bool,
    /// Writing the file failed (disk full, no permission). Recording of this track stopped.
    write_error: Option<String>,
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
    pub mic_on: bool,
    pub system_on: bool,
    pub mic_device: String,
    pub system_device: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveStatus {
    pub recording: bool,
    pub seconds: u32,
    pub mic_level: f32,
    pub system_level: f32,
    /// Plain-language problems, e.g. a source that stopped delivering audio.
    pub problems: Vec<String>,
    pub mic_device: String,
    pub system_device: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finished {
    pub id: String,
    pub mic_path: Option<String>,
    pub system_path: Option<String>,
    pub duration_ms: u64,
    /// Problems during recording (for example a file that could not be written).
    pub problems: Vec<String>,
}

/// Microphones and speakers/headphones Windows offers, for the device lists in the app.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Devices {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub default_input: Option<String>,
    pub default_output: Option<String>,
}

pub fn list_devices() -> Devices {
    let host = cpal::default_host();
    Devices {
        inputs: host
            .input_devices()
            .map(|d| d.filter_map(|x| x.name().ok()).collect())
            .unwrap_or_default(),
        outputs: host
            .output_devices()
            .map(|d| d.filter_map(|x| x.name().ok()).collect())
            .unwrap_or_default(),
        default_input: host.default_input_device().and_then(|d| d.name().ok()),
        default_output: host.default_output_device().and_then(|d| d.name().ok()),
    }
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

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    track: Arc<Mutex<Track>>,
    channels: usize,
    on_error: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    device.build_input_stream(
        config,
        move |d: &[T], _: &cpal::InputCallbackInfo| {
            let f: Vec<f32> = d.iter().map(|s| s.to_sample::<f32>()).collect();
            push_mono(&track, &f, channels)
        },
        on_error,
        None,
    )
}

/// Finds the chosen device by name; empty or missing names mean the Windows default.
fn pick_device(loopback: bool, name: &str) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    if !name.trim().is_empty() {
        let found = if loopback {
            host.output_devices()
                .ok()
                .and_then(|mut d| d.find(|x| x.name().map(|n| n == name).unwrap_or(false)))
        } else {
            host.input_devices()
                .ok()
                .and_then(|mut d| d.find(|x| x.name().map(|n| n == name).unwrap_or(false)))
        };
        if let Some(d) = found {
            return Ok(d);
        }
        return Err(format!(
            "The device \"{name}\" is not connected. Choose another one, or the Windows default."
        ));
    }
    if loopback {
        host.default_output_device()
            .ok_or_else(|| "No speakers or headphones found for system audio.".to_string())
    } else {
        host.default_input_device()
            .ok_or_else(|| "No microphone found.".to_string())
    }
}

/// Opens a capture stream. `loopback` = record what the speakers play.
fn open(
    track: Arc<Mutex<Track>>,
    loopback: bool,
    name: &str,
) -> Result<(cpal::Stream, String), String> {
    let device = pick_device(loopback, name)?;
    let device_name = device.name().unwrap_or_else(|_| "unknown device".into());
    let config = if loopback {
        device.default_output_config()
    } else {
        device.default_input_config()
    }
    .map_err(|e| format!("{device_name}: {e}"))?;
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
    let sc: cpal::StreamConfig = config.clone().into();
    // On Windows, an input stream on an output device records in loopback mode.
    use cpal::SampleFormat as F;
    let stream = match config.sample_format() {
        F::F32 => build::<f32>(&device, &sc, track, channels, on_error),
        F::F64 => build::<f64>(&device, &sc, track, channels, on_error),
        F::I8 => build::<i8>(&device, &sc, track, channels, on_error),
        F::I16 => build::<i16>(&device, &sc, track, channels, on_error),
        F::I32 => build::<i32>(&device, &sc, track, channels, on_error),
        F::I64 => build::<i64>(&device, &sc, track, channels, on_error),
        F::U8 => build::<u8>(&device, &sc, track, channels, on_error),
        F::U16 => build::<u16>(&device, &sc, track, channels, on_error),
        F::U32 => build::<u32>(&device, &sc, track, channels, on_error),
        F::U64 => build::<u64>(&device, &sc, track, channels, on_error),
        other => return Err(format!("{device_name}: unsupported audio format {other:?}")),
    }
    .map_err(|e| format!("{device_name}: {e}"))?;
    stream.play().map_err(|e| format!("{device_name}: {e}"))?;
    Ok((stream, device_name))
}

type Writer = hound::WavWriter<std::io::BufWriter<std::fs::File>>;

fn writer(path: &Path) -> Result<Writer, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    hound::WavWriter::create(path, spec)
        .map_err(|e| format!("could not create {}: {e}", path.display()))
}

/// Moves the captured audio into the file. Errors are returned, never ignored.
fn drain(track: &Arc<Mutex<Track>>, w: &mut Writer) -> Result<(), String> {
    let (pending, rate) = match track.lock() {
        Ok(mut t) => (std::mem::take(&mut t.pending), t.rate),
        Err(_) => return Err("recorder state is unavailable".into()),
    };
    if pending.is_empty() || rate == 0 {
        return Ok(());
    }
    for s in resample(&pending, rate, SAMPLE_RATE) {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .map_err(|e| e.to_string())?;
    }
    // Rewrites the WAV header so the file is valid even if the app stops suddenly.
    w.flush().map_err(|e| e.to_string())
}

/// Drains one track; on a write failure, records the error, stops writing that track and
/// keeps what was already saved (the header was valid at the last flush).
fn drain_or_stop(track: &Arc<Mutex<Track>>, slot: &mut Option<Writer>, at_s: u32, label: &str) {
    if let Some(w) = slot.as_mut() {
        if let Err(e) = drain(track, w) {
            crate::diag::log(
                "meeting",
                &format!("{label} track write failed at {at_s} s: {e}"),
            );
            if let Ok(mut t) = track.lock() {
                t.write_error = Some(format!(
                    "Saving the {label} recording failed after {}:{:02} ({e}). The part before that is kept; nothing after it is recorded.",
                    at_s / 60,
                    at_s % 60
                ));
                t.pending.clear();
            }
            *slot = None;
        }
    }
}

impl Recording {
    pub fn start(
        meetings_dir: &Path,
        mic: bool,
        system: bool,
        mic_device: &str,
        system_device: &str,
    ) -> Result<Self, String> {
        if !mic && !system {
            return Err("Choose at least one sound source.".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        let dir = meetings_dir.join(&id);
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Could not create the recording folder: {e}"))?;
        let mic_track = Arc::new(Mutex::new(Track::default()));
        let sys_track = Arc::new(Mutex::new(Track::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let seconds = Arc::new(AtomicU32::new(0));
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(String, String), String>>();

        let worker = {
            let (mic_t, sys_t, stop_t, sec_t, dir_t) = (
                Arc::clone(&mic_track),
                Arc::clone(&sys_track),
                Arc::clone(&stop),
                Arc::clone(&seconds),
                dir.clone(),
            );
            let (mic_name, sys_name) = (mic_device.to_string(), system_device.to_string());
            std::thread::Builder::new()
                .name("meeting-recorder".into())
                .spawn(move || {
                    // Every start-up problem goes back through `ready_tx`, so the caller sees
                    // the real cause instead of a time-out.
                    let setup = (|| {
                        // Streams must live on this thread.
                        let mic_stream = if mic {
                            Some(open(Arc::clone(&mic_t), false, &mic_name)?)
                        } else {
                            None
                        };
                        let sys_stream = if system {
                            Some(open(Arc::clone(&sys_t), true, &sys_name)?)
                        } else {
                            None
                        };
                        let mic_w = if mic {
                            Some(writer(&dir_t.join("mic.wav"))?)
                        } else {
                            None
                        };
                        let sys_w = if system {
                            Some(writer(&dir_t.join("system.wav"))?)
                        } else {
                            None
                        };
                        Ok::<_, String>((mic_stream, sys_stream, mic_w, sys_w))
                    })();
                    let (mic_stream, sys_stream, mut mic_w, mut sys_w) = match setup {
                        Ok(s) => s,
                        Err(e) => {
                            let _ = ready_tx.send(Err(e.clone()));
                            return Err(e);
                        }
                    };
                    let names = (
                        mic_stream.as_ref().map(|s| s.1.clone()).unwrap_or_default(),
                        sys_stream.as_ref().map(|s| s.1.clone()).unwrap_or_default(),
                    );
                    let _ = ready_tx.send(Ok(names));
                    let started = Instant::now();
                    let mut last_flush = Instant::now();
                    while !stop_t.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(200));
                        let secs = started.elapsed().as_secs() as u32;
                        sec_t.store(secs, Ordering::Relaxed);
                        if last_flush.elapsed() >= FLUSH_EVERY {
                            drain_or_stop(&mic_t, &mut mic_w, secs, "microphone");
                            drain_or_stop(&sys_t, &mut sys_w, secs, "system audio");
                            last_flush = Instant::now();
                        }
                    }
                    drop(mic_stream);
                    drop(sys_stream);
                    let secs = started.elapsed().as_secs() as u32;
                    drain_or_stop(&mic_t, &mut mic_w, secs, "microphone");
                    drain_or_stop(&sys_t, &mut sys_w, secs, "system audio");
                    let mut errors = Vec::new();
                    for w in [mic_w, sys_w].into_iter().flatten() {
                        if let Err(e) = w.finalize() {
                            errors.push(e.to_string());
                        }
                    }
                    if errors.is_empty() {
                        Ok(())
                    } else {
                        Err(format!(
                            "Finishing the recording files failed: {}",
                            errors.join("; ")
                        ))
                    }
                })
                .map_err(|e| e.to_string())?
        };
        let (mic_device, system_device) = match ready_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(names)) => names,
            Ok(Err(e)) => {
                let _ = std::fs::remove_dir_all(&dir);
                return Err(e);
            }
            Err(_) => {
                stop.store(true, Ordering::SeqCst);
                return Err(
                    "The sound devices did not answer within 10 seconds. Another app may be using them."
                        .into(),
                );
            }
        };
        Ok(Self {
            id,
            dir,
            started: Instant::now(),
            stop,
            mic: mic_track,
            sys: sys_track,
            worker: Some(worker),
            seconds,
            mic_on: mic,
            system_on: system,
            mic_device,
            system_device,
        })
    }

    pub fn track_paths(&self) -> Vec<(PathBuf, String)> {
        let mut out = Vec::new();
        if self.mic_on {
            out.push((self.dir.join("mic.wav"), app_core::summary::MIC.to_string()));
        }
        if self.system_on {
            out.push((
                self.dir.join("system.wav"),
                app_core::summary::SYSTEM.to_string(),
            ));
        }
        out
    }

    fn problems(&self) -> Vec<String> {
        let read = |t: &Arc<Mutex<Track>>| {
            t.lock()
                .map(|t| (t.level, t.failed, t.write_error.clone()))
                .unwrap_or((0.0, true, None))
        };
        let mut problems = Vec::new();
        let (_, mic_failed, mic_write) = read(&self.mic);
        let (_, sys_failed, sys_write) = read(&self.sys);
        if self.mic_on && mic_failed {
            problems.push(
                "The microphone stopped delivering sound (unplugged or taken by another app)."
                    .to_string(),
            );
        }
        if self.system_on && sys_failed {
            problems.push(
                "System audio stopped (the output device changed or was unplugged).".to_string(),
            );
        }
        problems.extend(mic_write);
        problems.extend(sys_write);
        problems
    }

    pub fn status(&self) -> LiveStatus {
        let level = |t: &Arc<Mutex<Track>>| t.lock().map(|t| t.level).unwrap_or(0.0);
        LiveStatus {
            recording: true,
            seconds: self.seconds.load(Ordering::Relaxed),
            mic_level: level(&self.mic),
            system_level: level(&self.sys),
            problems: self.problems(),
            mic_device: self.mic_device.clone(),
            system_device: self.system_device.clone(),
        }
    }

    pub fn stop(mut self) -> Result<Finished, String> {
        self.stop.store(true, Ordering::SeqCst);
        let mut problems = Vec::new();
        if let Some(w) = self.worker.take() {
            match w.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => problems.push(e),
                Err(_) => problems.push("The recorder stopped unexpectedly.".into()),
            }
        }
        problems.extend(self.problems());
        let path = |name: &str| {
            let p = self.dir.join(name);
            p.is_file().then(|| p.to_string_lossy().into_owned())
        };
        for p in &problems {
            crate::diag::log("meeting", &format!("recording problem: {p}"));
        }
        Ok(Finished {
            id: self.id.clone(),
            mic_path: path("mic.wav"),
            system_path: path("system.wav"),
            duration_ms: self.started.elapsed().as_millis() as u64,
            problems,
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

/// Which pieces of the not-yet-transcribed audio to recognise now. While recording, the last
/// piece may still grow, so it waits (and nothing is done with less than 25 s of new audio);
/// at the end everything is taken. Returns (pieces, samples consumed).
pub fn pieces_to_take(new_audio: &[f32], final_pass: bool) -> (Vec<(usize, &[f32])>, usize) {
    let rate = SAMPLE_RATE as usize;
    if new_audio.is_empty() || (!final_pass && new_audio.len() < 25 * rate) {
        return (Vec::new(), 0);
    }
    let mut pieces = split_for_recognition(new_audio);
    if final_pass {
        return (pieces, new_audio.len());
    }
    let last = pieces.pop();
    let consumed = last.map_or(new_audio.len(), |(start, _)| start);
    (pieces, consumed)
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

    #[test]
    fn live_passes_wait_for_complete_pieces_and_the_end_takes_the_rest() {
        let rate = SAMPLE_RATE as usize;
        let s: Vec<f32> = (0..70 * rate)
            .map(|i| ((i as f32) * 0.05).sin() * 0.3)
            .collect();
        let (none, used) = pieces_to_take(&s[..20 * rate], false);
        assert!(
            none.is_empty() && used == 0,
            "too little new audio while recording"
        );
        let (live, used) = pieces_to_take(&s, false);
        assert_eq!(live.len(), 2, "the growing last piece waits");
        assert_eq!(used, live.iter().map(|p| p.1.len()).sum::<usize>());
        let (rest, used2) = pieces_to_take(&s[used..], true);
        assert_eq!(
            used + used2,
            s.len(),
            "nothing lost between live and final passes"
        );
        assert!(!rest.is_empty());
    }
}
