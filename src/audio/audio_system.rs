//! The main AudioSystem that manages recording and playback threads.
//!
//! Uses std::thread (NOT tokio tasks) for real-time audio I/O to avoid
//! contention with async network tasks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use tokio::sync::mpsc;

use anyhow::Result;

use super::alsa_device;
use super::opus_codec::{OpusDecoder, OpusEncoder};
use super::speex::Preprocessor;
use super::stream_decoder::StreamDecoder;

/// Audio system configuration.
#[derive(Debug, Clone)]
pub struct AudioConfig {
    /// ALSA capture device name (e.g. "default", "plughw:0,0")
    pub capture_device: String,
    /// ALSA playback device name
    pub playback_device: String,
    /// Desired ALSA sample rate for capture (may be negotiated by hardware)
    pub sample_rate: u32,
    /// Desired ALSA channel count for capture
    pub channels: u32,
    /// Opus codec sample rate (typically 24000)
    pub opus_sample_rate: u32,
    /// Opus codec channel count (typically 1 for mono)
    pub opus_channels: u32,
    /// Opus bitrate in bits/s (e.g. 64000)
    pub opus_bitrate: i32,
    /// Frame duration for Opus encoding in ms (e.g. 60)
    pub encode_frame_duration_ms: u32,
    /// Frame duration for Opus decoding in ms (e.g. 20)
    pub decode_frame_duration_ms: u32,
    /// 网络下发流的编码格式: "opus", "mp3", "pcm"
    pub stream_format: String,
    /// Desired ALSA playback sample rate
    pub playback_sample_rate: u32,
    /// Desired ALSA playback channel count
    pub playback_channels: u32,
    /// Desired ALSA playback period size (0 = let ALSA decide)
    pub playback_period_size: usize,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            capture_device: "default".to_string(),
            playback_device: "default".to_string(),
            sample_rate: 24000,
            channels: 2,
            opus_sample_rate: 24000,
            opus_channels: 1,
            opus_bitrate: 64000,
            encode_frame_duration_ms: 60,
            decode_frame_duration_ms: 20,
            stream_format: "opus".to_string(),
            playback_sample_rate: 48000,
            playback_channels: 2,
            playback_period_size: 1024,
        }
    }
}

/// The audio system manages recording and playback in dedicated OS threads.
///
/// - Recording thread: ALSA capture → Speex preprocess → Opus encode → `opus_tx`
/// - Playback thread: `opus_rx` → Opus decode → ALSA playback
pub struct AudioSystem {
    running: Arc<AtomicBool>,
    record_handle: Option<JoinHandle<()>>,
    play_handle: Option<JoinHandle<()>>,
}

impl AudioSystem {
    /// Start the audio system.
    ///
    /// * `config`  - Audio configuration
    /// * `opus_tx` - Sender for encoded Opus packets from recording
    /// * `opus_rx` - Receiver for Opus packets to decode and play
    pub fn start(
        config: AudioConfig,
        opus_tx: mpsc::Sender<Vec<u8>>,
        opus_rx: mpsc::Receiver<Vec<u8>>,
    ) -> Result<Self> {
        let running = Arc::new(AtomicBool::new(true));

        log::info!(
            "AudioSystem starting — capture: \"{}\", playback: \"{}\", rate: {}Hz, ch: {}, opus: {}Hz/{}ch",
            config.capture_device,
            config.playback_device,
            config.sample_rate,
            config.channels,
            config.opus_sample_rate,
            config.opus_channels,
        );

        let record_handle = {
            let running = running.clone();
            let config = config.clone();
            thread::Builder::new()
                .name("audio-record".into())
                .spawn(move || {
                    if let Err(e) = record_thread(&config, opus_tx, &running) {
                        log::error!("Recording thread error: {}", e);
                    }
                })?
        };

        let play_handle = {
            let running = running.clone();
            let config = config.clone();
            thread::Builder::new()
                .name("audio-play".into())
                .spawn(move || {
                    // Small delay to let capture device initialize first
                    thread::sleep(std::time::Duration::from_secs(1));
                    if let Err(e) = play_thread(&config, opus_rx, &running) {
                        log::error!("Playback thread error: {}", e);
                    }
                })?
        };

        Ok(Self {
            running,
            record_handle: Some(record_handle),
            play_handle: Some(play_handle),
        })
    }

    /// Signal threads to stop and wait for them to finish.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.record_handle.take() {
            let _ = h.join();
        }
        // Playback thread will exit when the channel sender is dropped.
        // We detach it here to avoid blocking.
        self.play_handle.take();
    }
}

impl Drop for AudioSystem {
    fn drop(&mut self) {
        self.stop();
    }
}

// ======================== Recording thread ========================

fn record_thread(
    config: &AudioConfig,
    opus_tx: mpsc::Sender<Vec<u8>>,
    running: &AtomicBool,
) -> Result<()> {
    // 1. Open ALSA capture device
    let (pcm, params) =
        alsa_device::open_capture(&config.capture_device, config.sample_rate, config.channels)?;

    let actual_rate = params.sample_rate;
    let actual_channels = params.channels;
    let period_size = params.period_size;

    // 2. Initialize Speex preprocessors (one per channel for independent denoise/AGC)
    let mut preprocessors: Vec<Preprocessor> = Vec::new();
    for _ in 0..actual_channels {
        let mut pp = Preprocessor::new(period_size, actual_rate)?;
        pp.set_denoise(true);
        pp.set_noise_suppress(-25);
        pp.set_agc(true);
        pp.set_agc_level(24000.0);
        preprocessors.push(pp);
    }

    // Per-channel buffers for splitting interleaved data
    let mut channel_buffers: Vec<Vec<i16>> =
        (0..actual_channels).map(|_| vec![0i16; period_size]).collect();

    // 3. Initialize Opus encoder (with resampling + channel conversion)
    let mut encoder = OpusEncoder::new(
        actual_rate,
        actual_channels,
        config.encode_frame_duration_ms,
        config.opus_sample_rate,
        config.opus_channels,
        config.opus_bitrate,
    )?;

    let input_frame_samples = encoder.input_frame_samples();

    // Accumulation buffer for PCM samples (i16)
    let mut accum_buf: Vec<i16> = Vec::with_capacity(input_frame_samples * 2);

    // ALSA read buffer (interleaved i16, one period)
    let mut read_buf = vec![0i16; period_size * actual_channels as usize];

    let io = pcm.io_i16()?;

    log::info!(
        "Recording started: rate={}, ch={}, period={}, opus_frame_samples={}",
        actual_rate,
        actual_channels,
        period_size,
        input_frame_samples,
    );

    while running.load(Ordering::Relaxed) {
        // Read one period from ALSA
        match io.readi(&mut read_buf) {
            Ok(frames) => {
                // Split interleaved → per-channel
                for i in 0..frames {
                    for ch in 0..actual_channels as usize {
                        channel_buffers[ch][i] =
                            read_buf[i * actual_channels as usize + ch];
                    }
                }

                // Run Speex preprocess on each channel independently
                for ch in 0..actual_channels as usize {
                    preprocessors[ch].process(&mut channel_buffers[ch][..frames]);
                }

                // Merge per-channel → interleaved
                for i in 0..frames {
                    for ch in 0..actual_channels as usize {
                        read_buf[i * actual_channels as usize + ch] =
                            channel_buffers[ch][i];
                    }
                }

                // Accumulate processed PCM samples
                accum_buf
                    .extend_from_slice(&read_buf[..frames * actual_channels as usize]);

                // Encode complete frames
                while accum_buf.len() >= input_frame_samples {
                    let frame = &accum_buf[..input_frame_samples];
                    match encoder.encode(frame) {
                        Ok(opus_data) => {
                            if !opus_data.is_empty() {
                                if opus_tx.blocking_send(opus_data).is_err() {
                                    log::warn!(
                                        "Failed to send opus data, receiver dropped"
                                    );
                                    return Ok(());
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Opus encode error: {}", e);
                        }
                    }
                    // Remove the consumed frame from the accumulation buffer
                    accum_buf.drain(..input_frame_samples);
                }
            }
            Err(e) => {
                log::warn!("ALSA capture error: {}, recovering...", e);
                if let Err(e2) = pcm.prepare() {
                    log::error!("Failed to recover PCM capture: {}", e2);
                    break;
                }
            }
        }
    }

    log::info!("Recording stopped");
    Ok(())
}

// ======================== Playback thread ========================

/// Factory function: create a decoder based on the configured playback format.
fn create_decoder(
    config: &AudioConfig,
    alsa_rate: u32,
    alsa_channels: u32,
) -> Result<Box<dyn StreamDecoder>> {
    match config.stream_format.as_str() {
        "opus" => {
            let decoder = OpusDecoder::new(
                config.opus_sample_rate,
                config.opus_channels,
                config.decode_frame_duration_ms,
                alsa_rate,
                alsa_channels,
            )?;
            Ok(Box::new(decoder))
        }
        other => anyhow::bail!("Unsupported stream format: {}", other),
    }
}

fn play_thread(
    config: &AudioConfig,
    mut opus_rx: mpsc::Receiver<Vec<u8>>,
    running: &AtomicBool,
) -> Result<()> {
    // 1. Open ALSA playback device with configurable sample rate, channels, and period size
    let period_size_opt = if config.playback_period_size > 0 {
        Some(config.playback_period_size)
    } else {
        None
    };
    let (pcm, params) = alsa_device::open_playback(
        &config.playback_device,
        config.playback_sample_rate,
        config.playback_channels,
        period_size_opt,
    )?;

    let actual_rate = params.sample_rate;
    let actual_channels = params.channels;
    let _period_size = params.period_size;

    // 2. Initialize decoder via factory pattern
    let mut decoder = create_decoder(config, actual_rate, actual_channels)?;

    let io = pcm.io_i16()?;

    log::info!(
        "Playback started: stream_format={}, rate={}, ch={}, period={}",
        config.stream_format,
        actual_rate,
        actual_channels,
        _period_size,
    );

    while running.load(Ordering::Relaxed) {
        // Block until we receive an audio packet (or channel closes)
        match opus_rx.blocking_recv() {
            Some(audio_data) => {
                match decoder.decode(&audio_data) {
                    Ok(pcm_data) => {
                        if pcm_data.is_empty() {
                            continue;
                        }
                        // Write decoded PCM to ALSA with retry loop to handle
                        // short writes and XRUN recovery without losing frames.
                        let total_frames = pcm_data.len() / actual_channels as usize;
                        let mut frames_written = 0;
                        while frames_written < total_frames {
                            let offset = frames_written * actual_channels as usize;
                            match io.writei(&pcm_data[offset..]) {
                                Ok(n) => {
                                    frames_written += n;
                                }
                                Err(e) => {
                                    log::warn!("ALSA playback error: {}, recovering...", e);
                                    if let Err(e2) = pcm.prepare() {
                                        log::error!(
                                            "Failed to recover PCM playback: {}",
                                            e2
                                        );
                                        break;
                                    }
                                    // After recovery, the loop retries writing remaining frames
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Audio decode error: {}", e);
                    }
                }
            }
            None => {
                // Channel closed, exit playback
                log::info!("Playback channel closed");
                break;
            }
        }
    }

    log::info!("Playback stopped");
    Ok(())
}
