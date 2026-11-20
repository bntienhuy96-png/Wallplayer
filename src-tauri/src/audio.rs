//! System audio analysis for the ambient light.
//!
//! Captures the default output device through WASAPI loopback (cpal), runs a
//! 4096-point FFT every 512 samples and emits `audio-spectrum` ≈60×/s as
//! `[rms, band0 … band31]` (32 log-spaced bands, 40 Hz – 16 kHz, linear
//! magnitudes, max-held between emits so no transient is lost).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use tauri::{AppHandle, Emitter};

/// Set while every window is hidden behind another app: skip the analysis.
pub static PAUSED: AtomicBool = AtomicBool::new(false);

const N: usize = 4096;
const HOP: usize = 512;
const BANDS: usize = 32;

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || loop {
        let _ = run(&app);
        std::thread::sleep(Duration::from_secs(4));
    });
}

fn band_edges(sample_rate: f32) -> Vec<usize> {
    let (lo, hi) = (40.0f32, 16_000.0f32.min(sample_rate * 0.45));
    let mut e: Vec<usize> = (0..=BANDS)
        .map(|k| {
            let f = lo * (hi / lo).powf(k as f32 / BANDS as f32);
            ((f / sample_rate * N as f32).round() as usize).clamp(1, N / 2 - 1)
        })
        .collect();
    // every band gets at least one bin
    for k in 1..e.len() {
        if e[k] <= e[k - 1] {
            e[k] = (e[k - 1] + 1).min(N / 2);
        }
    }
    e
}

fn run(app: &AppHandle) -> Result<(), String> {
    let host = cpal::default_host();
    let dev = host.default_output_device().ok_or("no output device")?;
    let name = dev.name().unwrap_or_default();
    let cfg = dev.default_output_config().map_err(|e| e.to_string())?;
    let channels = cfg.channels().max(1) as usize;
    let sr = cfg.sample_rate().0 as f32;

    let failed = Arc::new(AtomicBool::new(false));
    let fail2 = failed.clone();
    let app2 = app.clone();

    let fft = FftPlanner::<f32>::new().plan_fft_forward(N);
    let window: Vec<f32> = (0..N)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (N - 1) as f32).cos())
        .collect();
    let edges = band_edges(sr);
    let mut ring = vec![0f32; N];
    let mut wpos = 0usize;
    let mut since = 0usize;
    let mut buf = vec![Complex { re: 0.0f32, im: 0.0 }; N];
    let mut scratch = vec![Complex { re: 0.0f32, im: 0.0 }; fft.get_inplace_scratch_len()];
    let mut out = vec![0f32; BANDS + 1];
    let (mut acc, mut nacc) = (0f32, 0usize);
    let mut last_emit = Instant::now();
    let mut peak = 0f32;
    let mut sent_silence = false;

    let stream = match cfg.sample_format() {
        cpal::SampleFormat::F32 => dev
            .build_input_stream(
                &cfg.config(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if PAUSED.load(Ordering::Relaxed) {
                        return;
                    }
                    for frame in data.chunks(channels) {
                        let s = frame.iter().sum::<f32>() / channels as f32;
                        ring[wpos] = s;
                        wpos = (wpos + 1) % N;
                        since += 1;
                        acc += s * s;
                        nacc += 1;
                        if s.abs() > peak {
                            peak = s.abs();
                        }
                        if since < HOP {
                            continue;
                        }
                        since = 0;
                        // digital silence: nothing to analyse, and one silent frame is enough
                        if peak < 1e-6 {
                            if !sent_silence && last_emit.elapsed() >= Duration::from_millis(16) {
                                out.iter_mut().for_each(|v| *v = 0.0);
                                let _ = app2.emit("audio-spectrum", &out);
                                sent_silence = true;
                                acc = 0.0;
                                nacc = 0;
                                last_emit = Instant::now();
                            }
                            continue;
                        }
                        peak = 0.0;
                        for i in 0..N {
                            buf[i] = Complex { re: ring[(wpos + i) % N] * window[i], im: 0.0 };
                        }
                        fft.process_with_scratch(&mut buf, &mut scratch);
                        for b in 0..BANDS {
                            let (lo, hi) = (edges[b], edges[b + 1].max(edges[b] + 1));
                            let mut m = 0f32;
                            for k in lo..hi.min(N / 2) {
                                let v = buf[k].norm();
                                if v > m {
                                    m = v;
                                }
                            }
                            let v = m * 2.0 / N as f32;
                            if v > out[b + 1] {
                                out[b + 1] = v;
                            }
                        }
                        // lighter mode: 30 updates a second are plenty
                        let every = if crate::power::LOW.load(Ordering::Relaxed) { 33 } else { 16 };
                        if last_emit.elapsed() >= Duration::from_millis(every) {
                            sent_silence = false;
                            out[0] = if nacc > 0 { (acc / nacc as f32).sqrt() } else { 0.0 };
                            let _ = app2.emit("audio-spectrum", &out);
                            out.iter_mut().for_each(|v| *v = 0.0);
                            acc = 0.0;
                            nacc = 0;
                            last_emit = Instant::now();
                        }
                    }
                },
                move |_e| fail2.store(true, Ordering::SeqCst),
                None,
            )
            .map_err(|e| e.to_string())?,
        other => return Err(format!("unsupported sample format {other:?}")),
    };
    stream.play().map_err(|e| e.to_string())?;

    loop {
        std::thread::sleep(Duration::from_secs(2));
        if failed.load(Ordering::SeqCst) {
            return Err("stream error".into());
        }
        // rebuild when the default output device changes (headphones etc.)
        let now = host.default_output_device().and_then(|d| d.name().ok());
        if now.as_deref() != Some(name.as_str()) {
            return Ok(());
        }
    }
}
