//! Playback worker thread: receives encoded audio packets, decodes them,
//! and writes the resulting PCM into the CPAL ring buffer for output.

use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use anyhow::Result;

use super::cpal_device::RbProducer;
use super::opus_codec::OpusDecoder;
use super::stream_decoder::StreamDecoder;
use super::audio_system::AudioConfig;
use ringbuf::traits::Producer;

/// Factory function: create a decoder based on the configured playback format.
fn create_decoder(
    config: &AudioConfig,
    output_rate: u32,
    output_channels: u32,
) -> Result<Box<dyn StreamDecoder>> {
    match config.stream_format.as_str() {
        "opus" => {
            let decoder = OpusDecoder::new(
                config.opus_sample_rate,
                config.opus_channels,
                config.decode_frame_duration_ms,
                output_rate,
                output_channels,
            )?;
            Ok(Box::new(decoder))
        }
        other => anyhow::bail!("Unsupported stream format: {}", other),
    }
}

/// Playback worker – runs on a dedicated OS thread.
///
/// Receives encoded packets from `opus_rx`, decodes them, and pushes
/// interleaved i16 PCM into the ring buffer consumed by the CPAL output callback.
pub fn play_thread(
    config: &AudioConfig,
    actual_rate: u32,
    actual_channels: u32,
    mut output_producer: RbProducer,
    mut opus_rx: mpsc::Receiver<Vec<u8>>,
    running: &AtomicBool,
) -> Result<()> {
    // Initialize decoder with CPAL-negotiated output parameters
    let mut decoder = create_decoder(config, actual_rate, actual_channels)?;

    log::info!(
        "Playback started: stream_format={}, rate={}, ch={}",
        config.stream_format,
        actual_rate,
        actual_channels,
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
                        // Push decoded PCM into the ring buffer.
                        // If the buffer is full, we busy-wait briefly so the CPAL
                        // output callback has time to drain it.
                        let mut written = 0;
                        while written < pcm_data.len() && running.load(Ordering::Relaxed) {
                            let n = output_producer.push_slice(&pcm_data[written..]);
                            written += n;
                            if written < pcm_data.len() {
                                std::thread::sleep(std::time::Duration::from_millis(2));
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
