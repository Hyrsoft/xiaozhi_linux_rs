//! The main AudioSystem that manages recording and playback.
//!
//! Uses CPAL for cross-platform audio I/O with lock-free ring buffers
//! bridging the CPAL callback threads and dedicated OS worker threads
//! (for Speex preprocessing, Opus encoding/decoding).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use tokio::sync::mpsc;

use anyhow::Result;
use cpal::Stream;

use super::cpal_device;
use super::record::record_thread;
use super::play::play_thread;

/// Audio system configuration.
#[derive(Debug, Clone)]
pub struct AudioConfig {
    /// Capture device name (e.g. "default"; platform-specific naming)
    pub capture_device: String,
    /// Playback device name
    pub playback_device: String,
    /// Desired sample rate for capture
    pub sample_rate: u32,
    /// Desired channel count for capture
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
    /// Desired playback sample rate
    pub playback_sample_rate: u32,
    /// Desired playback channel count
    pub playback_channels: u32,
    /// Desired playback period (buffer) size (0 = let backend decide)
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

/// The audio system manages recording and playback.
///
/// Architecture (per the CPAL migration design):
///
/// ```text
/// CPAL input callback ──► RingBuffer ──► record_thread (Speex/Opus) ──► opus_tx
/// opus_rx ──► play_thread (Opus decode) ──► RingBuffer ──► CPAL output callback
/// ```
///
/// The `Stream` objects are held alive here; dropping them stops audio I/O.
pub struct AudioSystem {
    running: Arc<AtomicBool>,
    record_handle: Option<JoinHandle<()>>,
    play_handle: Option<JoinHandle<()>>,
    // CPAL streams must be kept alive – dropping them stops audio.
    _capture_stream: Stream,
    _playback_stream: Stream,
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

        // --- Open CPAL capture stream + ring buffer ---
        let (capture_stream, capture_params, input_consumer) = cpal_device::open_capture(
            &config.capture_device,
            config.sample_rate,
            config.channels,
            running.clone(),
        )?;

        // --- Open CPAL playback stream + ring buffer ---
        let (playback_stream, playback_params, output_producer) = cpal_device::open_playback(
            &config.playback_device,
            config.playback_sample_rate,
            config.playback_channels,
            config.playback_period_size,
            running.clone(),
        )?;

        // --- Spawn recording worker thread ---
        let record_handle = {
            let running = running.clone();
            let config = config.clone();
            let cap_rate = capture_params.sample_rate;
            let cap_ch = capture_params.channels;
            thread::Builder::new()
                .name("audio-record".into())
                .spawn(move || {
                    if let Err(e) = record_thread(
                        &config,
                        cap_rate,
                        cap_ch,
                        input_consumer,
                        opus_tx,
                        &running,
                    ) {
                        log::error!("Recording thread error: {}", e);
                    }
                })?
        };

        // --- Spawn playback worker thread ---
        let play_handle = {
            let running = running.clone();
            let config = config.clone();
            let play_rate = playback_params.sample_rate;
            let play_ch = playback_params.channels;
            thread::Builder::new()
                .name("audio-play".into())
                .spawn(move || {
                    if let Err(e) = play_thread(
                        &config,
                        play_rate,
                        play_ch,
                        output_producer,
                        opus_rx,
                        &running,
                    ) {
                        log::error!("Playback thread error: {}", e);
                    }
                })?
        };

        Ok(Self {
            running,
            record_handle: Some(record_handle),
            play_handle: Some(play_handle),
            _capture_stream: capture_stream,
            _playback_stream: playback_stream,
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
