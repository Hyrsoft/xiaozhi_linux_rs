use crate::config::Config;
use tokio::sync::mpsc;
use crate::audio::{AudioConfig, AudioSystem};

pub enum AudioEvent {
    AudioData(Vec<u8>),
}

pub struct AudioBridge {
    _audio_system: AudioSystem,
    play_tx: mpsc::Sender<Vec<u8>>,
}

impl AudioBridge {
    /// Start the integrated audio system (replaces the external sound_app process).
    ///
    /// Recording data is forwarded as `AudioEvent::AudioData` via `tx`.
    /// Call `send_audio()` to send Opus packets for playback.
    pub fn start(config: &Config, tx: mpsc::Sender<AudioEvent>) -> anyhow::Result<Self> {
        let audio_config = AudioConfig {
            capture_device: config.capture_device.to_string(),
            playback_device: config.playback_device.to_string(),
            sample_rate: config.hello_sample_rate,
            channels: 2, // ALSA device typically 2 channels
            opus_sample_rate: config.hello_sample_rate,
            opus_channels: config.hello_channels as u32,
            opus_bitrate: 64000,
            encode_frame_duration_ms: 20,
            decode_frame_duration_ms: config.hello_frame_duration,
            stream_format: config.stream_format.as_str().to_string(),
            playback_sample_rate: config.playback_sample_rate,
            playback_channels: config.playback_channels,
            playback_period_size: config.playback_period_size,
        };

        let (opus_tx, mut opus_rx) = mpsc::channel::<Vec<u8>>(100);
        let (play_tx, play_rx) = mpsc::channel::<Vec<u8>>(100);

        println!(
            "AudioBridge: capture_device=\"{}\", playback_device=\"{}\"",
            audio_config.capture_device, audio_config.playback_device,
        );

        let audio_system = AudioSystem::start(audio_config, opus_tx, play_rx)?;

        // Forward recording Opus data as AudioEvent
        tokio::spawn(async move {
            while let Some(data) = opus_rx.recv().await {
                if tx.send(AudioEvent::AudioData(data)).await.is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            _audio_system: audio_system,
            play_tx,
        })
    }

    /// Send an Opus packet for playback.
    pub async fn send_audio(&self, data: &[u8]) -> anyhow::Result<()> {
        self.play_tx
            .send(data.to_vec())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send audio for playback: {}", e))
    }
}
