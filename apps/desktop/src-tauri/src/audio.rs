//! Microphone capture.
//!
//! With "keep microphone ready" on (the default), the stream stays open and the last second is
//! kept in memory, so words said while pressing the keys are not lost. Measured on the owner's
//! laptop (19 Nov 2026): 10 of 12 recordings started with 2.2-4.0 s of exact digital zeros,
//! that is a microphone that had to start from cold; the speech said in that time was lost.
//! Therefore:
//! - a watchdog re-opens the stream in the background when it fails, stops delivering, or
//!   delivers only exact zeros (a working microphone always has some noise), so it is warm
//!   before the next key press;
//! - the recorder reports when real sound arrives (`is_live`), so the pill can say
//!   "Mic starting..." until then, and the start sound plays only once the microphone hears;
//! - leading exact zeros are cut from a recording before recognition, and their length is
//!   written to the diagnostics log.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};

use app_asr::audio::{resample, rms};
use app_asr::SAMPLE_RATE;

/// Audio kept from just before the key press (only while the stream is kept open).
const PRE_ROLL_MS: usize = 1000;
/// How often the watchdog looks at the kept-open stream.
const WATCH_EVERY: Duration = Duration::from_millis(500);
/// A kept-open stream with no callback for this long is dead.
const NO_DATA_LIMIT_MS: u64 = 2000;
/// A kept-open stream that delivered only exact zeros for this long is not hearing.
const ZEROS_LIMIT_MS: u64 = 4000;
/// Re-opens after repeated failures wait at least this long (a muted microphone gives zeros
/// on purpose; the app must not keep restarting it).
const REOPEN_BACKOFF: [u64; 4] = [0, 2000, 10_000, 60_000];

#[derive(Default)]
struct Shared {
    recording: bool,
    /// Mono audio at the device's own sample rate.
    buffer: Vec<f32>,
    ring: VecDeque<f32>,
    ring_cap: usize,
}

enum Cmd {
    Start(Sender<Result<(), String>>),
    Stop(Sender<Result<Vec<f32>, String>>),
    Cancel,
    KeepOpen(bool),
}

/// Times (ms since the audio thread started) of the last callback and the last non-zero sample.
#[derive(Default)]
struct Pulse {
    last_data: AtomicU64,
    last_sound: AtomicU64,
    /// When the current stream was opened.
    opened: AtomicU64,
}

#[derive(Clone)]
pub struct Recorder {
    tx: Sender<Cmd>,
    level: Arc<AtomicU32>,
    failed: Arc<AtomicBool>,
    is_open: Arc<AtomicBool>,
    pulse: Arc<Pulse>,
    clock: Instant,
    /// Set at Start: ms timestamp from which real sound counts as "the microphone is live".
    start_mark: Arc<AtomicU64>,
}

struct Open {
    _stream: cpal::Stream,
    rate: u32,
}

/// True when a block carries a real signal rather than digital silence. This is NOT a voice
/// detector, on purpose: the failure it catches (10 of 12 of the owner's recordings, Nov 2026)
/// is a stream that delivers exact zeros. A working microphone in a quiet room still delivers
/// small noise, and a noise-floor test here would wrongly call that stream dead and reopen it.
/// Values below one 16-bit step (left over from format conversion) count as zero.
fn carries_signal(block: &[f32]) -> bool {
    const ONE_STEP: f32 = 1.0 / 32768.0;
    block.iter().any(|s| s.abs() >= ONE_STEP)
}

fn now_ms(clock: Instant) -> u64 {
    clock.elapsed().as_millis() as u64 + 1
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    push: impl Fn(&[f32]) + Send + 'static,
    on_error: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            let m: Vec<f32> = data
                .chunks(channels.max(1))
                .map(|f| {
                    f.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / channels.max(1) as f32
                })
                .collect();
            push(&m);
        },
        on_error,
        None,
    )
}

fn open_stream(
    shared: &Arc<Mutex<Shared>>,
    level: &Arc<AtomicU32>,
    failed: &Arc<AtomicBool>,
    pulse: &Arc<Pulse>,
    clock: Instant,
) -> Result<Open, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("No microphone found. Plug one in and try again.")?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("Microphone is not available: {e}"))?;
    let rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    {
        let mut s = shared.lock().map_err(|_| "audio state poisoned")?;
        s.ring_cap = rate as usize * PRE_ROLL_MS / 1000;
        s.ring.clear();
    }
    failed.store(false, Ordering::SeqCst);
    let opened = now_ms(clock);
    pulse.opened.store(opened, Ordering::SeqCst);
    pulse.last_data.store(opened, Ordering::SeqCst);
    pulse.last_sound.store(0, Ordering::SeqCst);

    let push = {
        let shared = Arc::clone(shared);
        let level = Arc::clone(level);
        let pulse = Arc::clone(pulse);
        move |mono: &[f32]| {
            let t = now_ms(clock);
            pulse.last_data.store(t, Ordering::Relaxed);
            if carries_signal(mono) {
                pulse.last_sound.store(t, Ordering::Relaxed);
            }
            level.store(rms(mono).to_bits(), Ordering::Relaxed);
            if let Ok(mut s) = shared.lock() {
                if s.recording {
                    s.buffer.extend_from_slice(mono);
                } else if s.ring_cap > 0 {
                    s.ring.extend(mono.iter().copied());
                    let extra = s.ring.len().saturating_sub(s.ring_cap);
                    s.ring.drain(..extra);
                }
            }
        }
    };
    let on_error = {
        let failed = Arc::clone(failed);
        move |_e: cpal::StreamError| failed.store(true, Ordering::SeqCst)
    };
    let sc: cpal::StreamConfig = config.clone().into();
    use cpal::SampleFormat as F;
    let stream = match config.sample_format() {
        F::F32 => build::<f32>(&device, &sc, channels, push, on_error),
        F::F64 => build::<f64>(&device, &sc, channels, push, on_error),
        F::I8 => build::<i8>(&device, &sc, channels, push, on_error),
        F::I16 => build::<i16>(&device, &sc, channels, push, on_error),
        F::I32 => build::<i32>(&device, &sc, channels, push, on_error),
        F::U8 => build::<u8>(&device, &sc, channels, push, on_error),
        F::U16 => build::<u16>(&device, &sc, channels, push, on_error),
        F::U32 => build::<u32>(&device, &sc, channels, push, on_error),
        other => return Err(format!("Unsupported microphone format: {other:?}")),
    }
    .map_err(|e| format!("Microphone in use by another app or blocked: {e}"))?;
    stream
        .play()
        .map_err(|e| format!("Could not start the microphone: {e}"))?;
    Ok(Open {
        _stream: stream,
        rate,
    })
}

/// Number of leading exact-zero samples (a microphone that had not started yet).
pub fn leading_zeros(samples: &[f32]) -> usize {
    samples
        .iter()
        .position(|s| *s != 0.0)
        .unwrap_or(samples.len())
}

/// Why the kept-open stream needs re-opening now, if it does.
fn stream_problem(
    failed: bool,
    now: u64,
    last_data: u64,
    last_sound: u64,
    opened: u64,
) -> Option<&'static str> {
    if failed {
        return Some("stream error");
    }
    if now.saturating_sub(last_data) > NO_DATA_LIMIT_MS {
        return Some("no audio delivered");
    }
    let sound_or_open = last_sound.max(opened);
    if now.saturating_sub(sound_or_open) > ZEROS_LIMIT_MS {
        return Some("only silence-zeros delivered");
    }
    None
}

impl Recorder {
    /// Spawns the audio thread (cpal streams must stay on the thread that made them).
    pub fn spawn() -> Self {
        let (tx, rx) = channel::<Cmd>();
        let level = Arc::new(AtomicU32::new(0));
        let failed = Arc::new(AtomicBool::new(false));
        let is_open = Arc::new(AtomicBool::new(false));
        let pulse = Arc::new(Pulse::default());
        let clock = Instant::now();
        let start_mark = Arc::new(AtomicU64::new(0));
        let (level_t, failed_t, open_t, pulse_t, mark_t) = (
            Arc::clone(&level),
            Arc::clone(&failed),
            Arc::clone(&is_open),
            Arc::clone(&pulse),
            Arc::clone(&start_mark),
        );

        std::thread::Builder::new()
            .name("audio".into())
            .spawn(move || {
                let shared = Arc::new(Mutex::new(Shared::default()));
                let mut open: Option<Open> = None;
                let mut keep_open = false;
                let mut reopen_failures = 0usize;
                let mut next_reopen = 0u64;
                let reopen = |open: &mut Option<Open>, why: &str| {
                    *open = None;
                    let result = open_stream(&shared, &level_t, &failed_t, &pulse_t, clock);
                    crate::diag::log(
                        "mic",
                        &format!(
                            "re-opened ({why}): {}",
                            if result.is_ok() { "ok" } else { "failed" }
                        ),
                    );
                    *open = result.ok();
                };
                loop {
                    let cmd = match rx.recv_timeout(WATCH_EVERY) {
                        Ok(c) => Some(c),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => return,
                    };
                    let recording = shared.lock().map(|s| s.recording).unwrap_or(false);
                    match cmd {
                        // Watchdog: keep the ready stream really working between dictations.
                        None => {
                            if keep_open && !recording {
                                let now = now_ms(clock);
                                let problem = if open.is_none() {
                                    Some("not open")
                                } else {
                                    stream_problem(
                                        failed_t.load(Ordering::SeqCst),
                                        now,
                                        pulse_t.last_data.load(Ordering::Relaxed),
                                        pulse_t.last_sound.load(Ordering::Relaxed),
                                        pulse_t.opened.load(Ordering::Relaxed),
                                    )
                                };
                                if let Some(why) = problem {
                                    if now >= next_reopen {
                                        reopen(&mut open, why);
                                        let sounding = open.is_some();
                                        reopen_failures = if sounding && why != "only silence-zeros delivered" {
                                            0
                                        } else {
                                            (reopen_failures + 1).min(REOPEN_BACKOFF.len() - 1)
                                        };
                                        next_reopen = now + REOPEN_BACKOFF[reopen_failures];
                                    }
                                } else {
                                    reopen_failures = 0;
                                }
                            }
                        }
                        Some(Cmd::KeepOpen(v)) => {
                            keep_open = v;
                            if v && open.is_none() {
                                open = open_stream(&shared, &level_t, &failed_t, &pulse_t, clock).ok();
                            } else if !v && !recording {
                                open = None;
                            }
                        }
                        Some(Cmd::Start(reply)) => {
                            let now = now_ms(clock);
                            // A kept stream that has not heard anything for a while is re-opened
                            // now rather than trusted.
                            let stale = open.is_some()
                                && stream_problem(
                                    failed_t.load(Ordering::SeqCst),
                                    now,
                                    pulse_t.last_data.load(Ordering::Relaxed),
                                    pulse_t.last_sound.load(Ordering::Relaxed),
                                    pulse_t.opened.load(Ordering::Relaxed),
                                )
                                .is_some();
                            let was_warm = open.is_some() && !stale;
                            if open.is_none() || stale {
                                open = None;
                                match open_stream(&shared, &level_t, &failed_t, &pulse_t, clock) {
                                    Ok(o) => open = Some(o),
                                    Err(e) => {
                                        let _ = reply.send(Err(e));
                                        open_t.store(false, Ordering::SeqCst);
                                        continue;
                                    }
                                }
                            }
                            // With a warm stream, sound already counts; after a fresh open, only
                            // sound from now on does.
                            mark_t.store(if was_warm { 1 } else { now_ms(clock) }, Ordering::SeqCst);
                            if let Ok(mut s) = shared.lock() {
                                s.buffer = s.ring.drain(..).collect();
                                s.recording = true;
                            }
                            crate::diag::log(
                                "mic",
                                if was_warm { "start: microphone was ready" } else { "start: microphone opened now (was not ready)" },
                            );
                            let _ = reply.send(Ok(()));
                        }
                        Some(Cmd::Stop(reply)) => {
                            let captured = shared.lock().ok().map(|mut s| {
                                s.recording = false;
                                std::mem::take(&mut s.buffer)
                            });
                            let rate = open.as_ref().map_or(SAMPLE_RATE, |o| o.rate);
                            let lost = failed_t.load(Ordering::SeqCst);
                            if !keep_open || lost {
                                open = None;
                            }
                            level_t.store(0, Ordering::Relaxed);
                            let _ = reply.send(match captured {
                                Some(mut raw) => {
                                    // Cut a microphone start-up of exact zeros (no speech in it).
                                    let zeros = leading_zeros(&raw);
                                    if zeros > 0 {
                                        let ms = zeros as u64 * 1000 / u64::from(rate.max(1));
                                        if ms >= 200 {
                                            crate::diag::log("mic", &format!("recording began with {ms} ms of silence-zeros (cut)"));
                                        }
                                        raw.drain(..zeros);
                                    }
                                    Ok(resample(&raw, rate, SAMPLE_RATE))
                                }
                                None => Err("audio state poisoned".into()),
                            });
                        }
                        Some(Cmd::Cancel) => {
                            if let Ok(mut s) = shared.lock() {
                                s.recording = false;
                                s.buffer.clear();
                            }
                            if !keep_open {
                                open = None;
                            }
                            level_t.store(0, Ordering::Relaxed);
                        }
                    }
                    open_t.store(open.is_some(), Ordering::SeqCst);
                }
            })
            .expect("audio thread");

        Self {
            tx,
            level,
            failed,
            is_open,
            pulse,
            clock,
            start_mark,
        }
    }

    pub fn start(&self) -> Result<(), String> {
        let (reply, rx) = channel();
        self.tx.send(Cmd::Start(reply)).map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }

    /// Stops and returns everything captured as 16 kHz mono.
    pub fn stop(&self) -> Result<Vec<f32>, String> {
        let (reply, rx) = channel();
        self.tx.send(Cmd::Stop(reply)).map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }

    pub fn cancel(&self) {
        let _ = self.tx.send(Cmd::Cancel);
    }

    pub fn set_keep_open(&self, keep: bool) {
        let _ = self.tx.send(Cmd::KeepOpen(keep));
    }

    /// Current input level, 0.0 to 1.0 (RMS of the latest audio block).
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    /// True once the microphone delivers real sound for the recording that is running (always
    /// at once with a ready microphone; after the start-up time with one that had to open).
    pub fn is_live(&self) -> bool {
        let mark = self.start_mark.load(Ordering::SeqCst);
        self.pulse.last_sound.load(Ordering::Relaxed) >= mark && mark > 0
    }

    /// Measures how long the default microphone takes to start delivering audio.
    /// Run by hand: `cargo test -p app-desktop --lib mic_open_latency -- --ignored --nocapture`
    #[cfg(test)]
    pub fn measure_open_ms() -> Result<u128, String> {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let level = Arc::new(AtomicU32::new(u32::MAX));
        let failed = Arc::new(AtomicBool::new(false));
        let pulse = Arc::new(Pulse::default());
        let started = Instant::now();
        let _open = open_stream(&shared, &level, &failed, &pulse, started)?;
        // `level` is written by the first audio callback.
        while level.load(Ordering::Relaxed) == u32::MAX {
            if started.elapsed().as_secs() > 3 {
                return Err("no audio within 3 s".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        Ok(started.elapsed().as_millis())
    }

    /// True while the microphone stream is open (always, when "keep ready" is on).
    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::SeqCst)
    }

    /// True when the microphone disappeared or errored during capture.
    pub fn device_failed(&self) -> bool {
        self.failed.load(Ordering::SeqCst)
    }

    #[allow(dead_code)]
    fn now(&self) -> u64 {
        now_ms(self.clock)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "opens the real microphone; run by hand"]
    fn mic_open_latency() {
        let mut times = Vec::new();
        for _ in 0..3 {
            times.push(super::Recorder::measure_open_ms().expect("microphone"));
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        println!("microphone open-to-first-audio ms: {times:?}");
    }

    #[test]
    fn digital_silence_is_not_a_signal_but_a_quiet_room_is() {
        assert!(!carries_signal(&[0.0; 480]));
        assert!(!carries_signal(&[1.0e-7; 480]), "conversion leftovers");
        // Room noise around -70 dBFS is a working microphone.
        assert!(carries_signal(&[0.0, 3.2e-4, -2.9e-4, 0.0]));
    }

    #[test]
    fn leading_silence_zeros_are_counted() {
        assert_eq!(leading_zeros(&[0.0, 0.0, 0.0, 0.01, 0.0]), 3);
        assert_eq!(leading_zeros(&[0.2, 0.0]), 0);
        assert_eq!(leading_zeros(&[0.0; 5]), 5);
    }

    #[test]
    fn a_ready_stream_is_judged_by_data_and_sound() {
        // Healthy: data and sound just now.
        assert_eq!(stream_problem(false, 10_000, 9_900, 9_900, 1), None);
        assert_eq!(
            stream_problem(true, 10_000, 9_900, 9_900, 1),
            Some("stream error")
        );
        assert_eq!(
            stream_problem(false, 10_000, 7_000, 9_000, 1),
            Some("no audio delivered")
        );
        // Data keeps coming, but only exact zeros for more than 4 s: not hearing.
        assert_eq!(
            stream_problem(false, 10_000, 9_990, 5_000, 1),
            Some("only silence-zeros delivered")
        );
        // Just opened: zeros are allowed for the first seconds.
        assert_eq!(stream_problem(false, 10_000, 9_990, 0, 8_000), None);
    }
}
