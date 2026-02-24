use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use anyhow::Result;

use super::alsa_device;
use super::opus_codec::OpusDecoder;
use super::stream_decoder::StreamDecoder;
use super::audio_system::AudioConfig;

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

pub fn play_thread(
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
                        let mut retry_count = 0u32;

                        while frames_written < total_frames {
                            let offset = frames_written * actual_channels as usize;
                            match io.writei(&pcm_data[offset..]) {
                                Ok(n) => {
                                    frames_written += n;
                                    retry_count = 0; // 成功写入，重置重试计数
                                }
                                Err(e) => {
                                    log::warn!("ALSA XRUN or error: {}, recovering...", e);
                                    retry_count += 1;

                                    // 触发 ALSA 硬件恢复状态机
                                    if let Err(e2) = pcm.prepare() {
                                        log::error!(
                                            "Failed to recover PCM playback: {}",
                                            e2
                                        );
                                        break;
                                    }

                                    // 熔断器：底层持续跟不上写入速度时，丢弃剩余帧防止死循环
                                    if retry_count >= 3 {
                                        log::error!(
                                            "Max recovery retries ({}) reached. Dropping {} unwritten frames to break dead-loop.",
                                            retry_count,
                                            total_frames - frames_written
                                        );
                                        break;
                                    }
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
