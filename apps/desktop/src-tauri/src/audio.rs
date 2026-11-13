//! Microphone capture.
//!
//! The microphone is opened when recording starts and closed when it stops, so Windows'
//! microphone indicator is on only while the app is listening. With "keep microphone ready"
//! switched on, the stream stays open and a 300 ms pre-roll buffer protects the first word.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use app_asr::audio::{resample, rms};
use app_asr::SAMPLE_RATE;

const PRE_ROLL_MS: usize = 300;

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

#[derive(Clone)]
pub struct Recorder {
    tx: Sender<Cmd>,
    level: Arc<AtomicU32>,
    failed: Arc<AtomicBool>,
}

struct Open {
    _stream: cpal::Stream,
    rate: u32,
}

fn open_stream(
    shared: &Arc<Mutex<Shared>>,
    level: &Arc<AtomicU32>,
    failed: &Arc<AtomicBool>,
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

    let push = {
        let shared = Arc::clone(shared);
        let level = Arc::clone(level);
        move |mono: &[f32]| {
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
    let mono = move |frame: &[f32]| frame.iter().sum::<f32>() / channels.max(1) as f32;

    let stream_config: cpal::StreamConfig = config.clone().into();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let m: Vec<f32> = data.chunks(channels.max(1)).map(mono).collect();
                push(&m);
            },
            on_error,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                let m: Vec<f32> = data
                    .chunks(channels.max(1))
                    .map(|f| {
                        f.iter().map(|s| f32::from(*s) / 32768.0).sum::<f32>()
                            / channels.max(1) as f32
                    })
                    .collect();
                push(&m);
            },
            on_error,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            &stream_config,
            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                let m: Vec<f32> = data
                    .chunks(channels.max(1))
                    .map(|f| {
                        f.iter()
                            .map(|s| (f32::from(*s) - 32768.0) / 32768.0)
                            .sum::<f32>()
                            / channels.max(1) as f32
                    })
                    .collect();
                push(&m);
            },
            on_error,
            None,
        ),
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

impl Recorder {
    /// Spawns the audio thread (cpal streams must stay on the thread that made them).
    pub fn spawn() -> Self {
        let (tx, rx) = channel::<Cmd>();
        let level = Arc::new(AtomicU32::new(0));
        let failed = Arc::new(AtomicBool::new(false));
        let (level_t, failed_t) = (Arc::clone(&level), Arc::clone(&failed));

        std::thread::Builder::new()
            .name("audio".into())
            .spawn(move || {
                let shared = Arc::new(Mutex::new(Shared::default()));
                let mut open: Option<Open> = None;
                let mut keep_open = false;
                for cmd in rx {
                    match cmd {
                        Cmd::KeepOpen(v) => {
                            keep_open = v;
                            if v && open.is_none() {
                                open = open_stream(&shared, &level_t, &failed_t).ok();
                            } else if !v && !shared.lock().map(|s| s.recording).unwrap_or(false) {
                                open = None;
                            }
                        }
                        Cmd::Start(reply) => {
                            if open.is_none() || failed_t.load(Ordering::SeqCst) {
                                open = None;
                                match open_stream(&shared, &level_t, &failed_t) {
                                    Ok(o) => open = Some(o),
                                    Err(e) => {
                                        let _ = reply.send(Err(e));
                                        continue;
                                    }
                                }
                            }
                            if let Ok(mut s) = shared.lock() {
                                s.buffer = s.ring.drain(..).collect();
                                s.recording = true;
                            }
                            let _ = reply.send(Ok(()));
                        }
                        Cmd::Stop(reply) => {
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
                                Some(raw) => Ok(resample(&raw, rate, SAMPLE_RATE)),
                                None => Err("audio state poisoned".into()),
                            });
                        }
                        Cmd::Cancel => {
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
                }
            })
            .expect("audio thread");

        Self { tx, level, failed }
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

    /// True when the microphone disappeared or errored during capture.
    pub fn device_failed(&self) -> bool {
        self.failed.load(Ordering::SeqCst)
    }
}
