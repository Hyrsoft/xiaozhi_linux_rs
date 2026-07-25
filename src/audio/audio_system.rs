//! The main AudioSystem that manages recording and playback threads.
//!
//! Uses std::thread (NOT tokio tasks) for real-time audio I/O to avoid
//! contention with async network tasks.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use tokio::sync::mpsc;

use anyhow::Result;

use super::alsa_device::AlsaBackend;
use super::backend::{AudioBackend, AudioStreamParams};
use super::frontend::{AudioFrontend, SpeexFrontend};
use super::play::play_thread;
use super::record::record_thread;

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
        Self::start_with_backend(config, opus_tx, opus_rx, &AlsaBackend, |params| {
            Ok(Box::new(SpeexFrontend::new(
                params.period_size,
                params.sample_rate,
                params.channels,
            )?))
        })
    }

    pub fn start_with_backend<F>(
        config: AudioConfig,
        opus_tx: mpsc::Sender<Vec<u8>>,
        opus_rx: mpsc::Receiver<Vec<u8>>,
        backend: &dyn AudioBackend,
        frontend_factory: F,
    ) -> Result<Self>
    where
        F: FnOnce(AudioStreamParams) -> Result<Box<dyn AudioFrontend>>,
    {
        let running = Arc::new(AtomicBool::new(true));
        let capture = backend.open_capture(&config)?;
        let capture_params = capture.params();
        let playback = backend.open_playback(&config)?;
        let frontend = frontend_factory(capture_params)?;
        let (render_tx, render_rx) = mpsc::channel(8);

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
                    if let Err(e) =
                        record_thread(&config, capture, frontend, render_rx, opus_tx, &running)
                    {
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
                    if let Err(e) = play_thread(&config, playback, opus_rx, render_tx, &running) {
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
