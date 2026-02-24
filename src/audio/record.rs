use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use anyhow::Result;

use super::alsa_device;
use super::opus_codec::OpusEncoder;
use super::speex::Preprocessor;
use super::audio_system::AudioConfig;

pub fn record_thread(
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
