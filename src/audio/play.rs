use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;

use super::audio_system::AudioConfig;
use super::backend::PlaybackStream;
use super::frontend::RenderReference;
use super::opus_codec::OpusDecoder;
use super::stream_decoder::StreamDecoder;

fn create_decoder(
    config: &AudioConfig,
    output_rate: u32,
    output_channels: u32,
) -> Result<Box<dyn StreamDecoder>> {
    match config.stream_format.as_str() {
        "opus" => Ok(Box::new(OpusDecoder::new(
            config.opus_sample_rate,
            config.opus_channels,
            config.decode_frame_duration_ms,
            output_rate,
            output_channels,
        )?)),
        other => anyhow::bail!("Unsupported stream format: {}", other),
    }
}

pub(crate) fn play_thread(
    config: &AudioConfig,
    mut playback: Box<dyn PlaybackStream>,
    mut audio_rx: mpsc::Receiver<Vec<u8>>,
    render_tx: mpsc::Sender<RenderReference>,
    running: &AtomicBool,
) -> Result<()> {
    let params = playback.params();
    let actual_rate = params.sample_rate;
    let actual_channels = params.channels;
    let mut decoder = create_decoder(config, actual_rate, actual_channels)?;

    log::info!(
        "Playback started: stream_format={}, rate={}, ch={}, period={}",
        config.stream_format,
        actual_rate,
        actual_channels,
        params.period_size,
    );

    while running.load(Ordering::Relaxed) {
        let Some(audio_data) = audio_rx.blocking_recv() else {
            log::info!("Playback channel closed");
            break;
        };
        let pcm_data = match decoder.decode(&audio_data) {
            Ok(data) if !data.is_empty() => data,
            Ok(_) => continue,
            Err(error) => {
                log::error!("Audio decode error: {}", error);
                continue;
            }
        };

        let total_frames = pcm_data.len() / actual_channels as usize;
        let mut frames_written = 0;
        let mut retry_count = 0_u32;
        while frames_written < total_frames {
            let offset = frames_written * actual_channels as usize;
            match playback.write(&pcm_data[offset..]) {
                Ok(frames) => {
                    frames_written += frames;
                    retry_count = 0;
                }
                Err(error) => {
                    log::warn!("Audio playback error: {}, recovering...", error);
                    retry_count += 1;
                    if let Err(recovery_error) = playback.recover() {
                        log::error!("Failed to recover audio playback: {}", recovery_error);
                        break;
                    }
                    if retry_count >= 3 {
                        log::error!(
                            "Max recovery retries reached. Dropping {} unwritten frames.",
                            total_frames - frames_written
                        );
                        break;
                    }
                }
            }
        }

        if frames_written > 0 {
            let sample_count = frames_written * actual_channels as usize;
            let _ = render_tx.try_send(RenderReference {
                samples: pcm_data[..sample_count].to_vec(),
                sample_rate: actual_rate,
                channels: actual_channels,
            });
        }
    }

    log::info!("Playback stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_reference_channel_is_nonblocking_when_full() {
        let (tx, _rx) = mpsc::channel(8);
        for _ in 0..8 {
            tx.try_send(RenderReference {
                samples: vec![0; 4],
                sample_rate: 48_000,
                channels: 2,
            })
            .unwrap();
        }
        assert!(
            tx.try_send(RenderReference {
                samples: vec![0; 4],
                sample_rate: 48_000,
                channels: 2,
            })
            .is_err()
        );
    }
}
