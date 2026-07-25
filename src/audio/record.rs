use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;

use super::audio_system::AudioConfig;
use super::backend::CaptureStream;
use super::frontend::{AudioFrameMut, AudioFrameRef, AudioFrontend, RenderReference};
use super::opus_codec::OpusEncoder;

pub(crate) fn record_thread(
    config: &AudioConfig,
    mut capture: Box<dyn CaptureStream>,
    mut frontend: Box<dyn AudioFrontend>,
    mut render_rx: mpsc::Receiver<RenderReference>,
    opus_tx: mpsc::Sender<Vec<u8>>,
    running: &AtomicBool,
) -> Result<()> {
    let params = capture.params();
    let actual_rate = params.sample_rate;
    let actual_channels = params.channels;
    let period_size = params.period_size;

    let mut encoder = OpusEncoder::new(
        actual_rate,
        actual_channels,
        config.encode_frame_duration_ms,
        config.opus_sample_rate,
        config.opus_channels,
        config.opus_bitrate,
    )?;
    let input_frame_samples = encoder.input_frame_samples();
    let mut accum_buf = Vec::with_capacity(input_frame_samples * 2);
    let mut read_buf = vec![0_i16; period_size * actual_channels as usize];

    log::info!(
        "Recording started: rate={}, ch={}, period={}, opus_frame_samples={}",
        actual_rate,
        actual_channels,
        period_size,
        input_frame_samples,
    );

    while running.load(Ordering::Relaxed) {
        match capture.read(&mut read_buf) {
            Ok(frames) => {
                while let Ok(reference) = render_rx.try_recv() {
                    frontend.process_render_reference(AudioFrameRef {
                        samples: &reference.samples,
                        sample_rate: reference.sample_rate,
                        channels: reference.channels,
                    })?;
                }

                let sample_count = frames * actual_channels as usize;
                frontend.process_capture(AudioFrameMut {
                    samples: &mut read_buf[..sample_count],
                    sample_rate: actual_rate,
                    channels: actual_channels,
                })?;
                accum_buf.extend_from_slice(&read_buf[..sample_count]);

                while accum_buf.len() >= input_frame_samples {
                    let frame = &accum_buf[..input_frame_samples];
                    match encoder.encode(frame) {
                        Ok(opus_data) if !opus_data.is_empty() => {
                            if opus_tx.blocking_send(opus_data).is_err() {
                                log::warn!("Failed to send opus data, receiver dropped");
                                return Ok(());
                            }
                        }
                        Ok(_) => {}
                        Err(error) => log::error!("Opus encode error: {}", error),
                    }
                    accum_buf.drain(..input_frame_samples);
                }
            }
            Err(error) => {
                log::warn!("Audio capture error: {}, recovering...", error);
                if let Err(recovery_error) = capture.recover() {
                    log::error!("Failed to recover audio capture: {}", recovery_error);
                    break;
                }
            }
        }
    }

    log::info!("Recording stopped");
    Ok(())
}
