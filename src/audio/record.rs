//! Recording worker thread: reads PCM from the CPAL ring buffer,
//! applies Speex preprocessing, encodes with Opus, and sends packets
//! via the `opus_tx` channel.

use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use anyhow::Result;

use super::cpal_device::RbConsumer;
use super::opus_codec::OpusEncoder;
use super::speex::Preprocessor;
use super::audio_system::AudioConfig;
use ringbuf::traits::{Consumer, Observer};

/// Recording worker – runs on a dedicated OS thread.
///
/// Instead of blocking on ALSA `readi`, it continuously drains samples
/// from the lock-free ring buffer filled by the CPAL input callback.
pub fn record_thread(
    config: &AudioConfig,
    actual_rate: u32,
    actual_channels: u32,
    mut input_consumer: RbConsumer,
    opus_tx: mpsc::Sender<Vec<u8>>,
    running: &AtomicBool,
) -> Result<()> {
    // We use the negotiated rate/channels from CPAL rather than the config hints,
    // since the device may have picked different values.
    let period_size: usize = (actual_rate as usize * 20) / 1000; // ~20 ms worth of frames

    // 1. Initialize Speex preprocessors (one per channel for independent denoise/AGC)
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

    // 2. Initialize Opus encoder (with resampling + channel conversion)
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

    // Read buffer: one period of interleaved i16
    let samples_per_period = period_size * actual_channels as usize;
    let mut read_buf = vec![0i16; samples_per_period];

    log::info!(
        "Recording started: rate={}, ch={}, period={}, opus_frame_samples={}",
        actual_rate,
        actual_channels,
        period_size,
        input_frame_samples,
    );

    while running.load(Ordering::Relaxed) {
        // Try to drain one period from the ring buffer
        let available = input_consumer.occupied_len();
        if available >= samples_per_period {
            let popped = input_consumer.pop_slice(&mut read_buf);
            let frames = popped / actual_channels as usize;

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
            accum_buf.extend_from_slice(&read_buf[..frames * actual_channels as usize]);

            // Encode complete frames
            while accum_buf.len() >= input_frame_samples {
                let frame = &accum_buf[..input_frame_samples];
                match encoder.encode(frame) {
                    Ok(opus_data) => {
                        if !opus_data.is_empty() {
                            if opus_tx.blocking_send(opus_data).is_err() {
                                log::warn!("Failed to send opus data, receiver dropped");
                                return Ok(());
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Opus encode error: {}", e);
                    }
                }
                accum_buf.drain(..input_frame_samples);
            }
        } else {
            // Not enough data yet – sleep briefly to avoid busy-spinning.
            // 5 ms is well under the 20 ms period, so we won't miss data.
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    log::info!("Recording stopped");
    Ok(())
}
