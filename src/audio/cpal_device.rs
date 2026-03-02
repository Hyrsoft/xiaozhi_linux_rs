//! CPAL-based audio device wrappers for capture and playback.
//!
//! Replaces the previous ALSA-specific `alsa_device.rs` with a cross-platform
//! implementation using `cpal`. Audio data is exchanged via lock-free ring buffers
//! (`ringbuf`) between the CPAL callback threads and the application worker threads.

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, Device, SampleFormat, SampleRate, Stream, StreamConfig,
    SupportedStreamConfigRange,
};
use ringbuf::{
    traits::{Consumer, Producer, Split},
    HeapRb,
};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Ring buffer producer half (writing side) for i16 samples.
pub type RbProducer = ringbuf::HeapProd<i16>;
/// Ring buffer consumer half (reading side) for i16 samples.
pub type RbConsumer = ringbuf::HeapCons<i16>;

/// Parameters resolved from the CPAL device after stream creation.
#[derive(Debug, Clone)]
pub struct CpalParams {
    /// Actual sample rate used by the stream
    pub sample_rate: u32,
    /// Actual number of channels
    pub channels: u32,
}

// ---------------------------------------------------------------------------
//  Device lookup helpers
// ---------------------------------------------------------------------------

/// Find a CPAL input (capture) device by name. "default" returns the host default.
fn find_input_device(name: &str) -> Result<Device> {
    let host = cpal::default_host();
    if name == "default" {
        host.default_input_device()
            .context("No default input device available")
    } else {
        host.input_devices()?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .with_context(|| format!("Input device '{}' not found", name))
    }
}

/// Find a CPAL output (playback) device by name. "default" returns the host default.
fn find_output_device(name: &str) -> Result<Device> {
    let host = cpal::default_host();
    if name == "default" {
        host.default_output_device()
            .context("No default output device available")
    } else {
        host.output_devices()?
            .find(|d| d.name().map(|n| n == name).unwrap_or(false))
            .with_context(|| format!("Output device '{}' not found", name))
    }
}

// ---------------------------------------------------------------------------
//  Configuration negotiation helpers
// ---------------------------------------------------------------------------

/// Try to build a `StreamConfig` that matches the desired sample rate and channels,
/// falling back to the device's default/supported configuration if needed.
fn negotiate_input_config(
    device: &Device,
    desired_rate: u32,
    desired_channels: u32,
) -> Result<(StreamConfig, SampleFormat)> {
    negotiate_config(device, desired_rate, desired_channels, true)
}

fn negotiate_output_config(
    device: &Device,
    desired_rate: u32,
    desired_channels: u32,
    desired_period: usize,
) -> Result<(StreamConfig, SampleFormat)> {
    let (mut cfg, fmt) = negotiate_config(device, desired_rate, desired_channels, false)?;
    // If a period (buffer) size is requested, try to honour it.
    if desired_period > 0 {
        cfg.buffer_size = BufferSize::Fixed(desired_period as u32);
    }
    Ok((cfg, fmt))
}

fn negotiate_config(
    device: &Device,
    desired_rate: u32,
    desired_channels: u32,
    is_input: bool,
) -> Result<(StreamConfig, SampleFormat)> {
    let configs: Vec<SupportedStreamConfigRange> = if is_input {
        device
            .supported_input_configs()
            .context("Failed to query supported input configs")?
            .collect()
    } else {
        device
            .supported_output_configs()
            .context("Failed to query supported output configs")?
            .collect()
    };

    if configs.is_empty() {
        anyhow::bail!(
            "Device '{}' reports no supported {} configurations",
            device.name().unwrap_or_default(),
            if is_input { "input" } else { "output" }
        );
    }

    // Log available ranges for debugging
    for c in &configs {
        log::info!(
            "CPAL supported config: channels={}, rate=[{}-{}], format={:?}, buffer={:?}",
            c.channels(),
            c.min_sample_rate().0,
            c.max_sample_rate().0,
            c.sample_format(),
            c.buffer_size(),
        );
    }

    let target_rate = SampleRate(desired_rate);

    // Prefer I16, then F32
    let preferred_formats = [SampleFormat::I16, SampleFormat::F32];

    for &fmt in &preferred_formats {
        for c in &configs {
            if c.sample_format() == fmt
                && c.channels() as u32 == desired_channels
                && c.min_sample_rate() <= target_rate
                && c.max_sample_rate() >= target_rate
            {
                let cfg = StreamConfig {
                    channels: desired_channels as u16,
                    sample_rate: target_rate,
                    buffer_size: BufferSize::Default,
                };
                log::info!(
                    "CPAL negotiated: rate={}, ch={}, fmt={:?}",
                    desired_rate,
                    desired_channels,
                    fmt
                );
                return Ok((cfg, fmt));
            }
        }
    }

    // If exact match not found, try matching channels with any supported rate and format
    for &fmt in &preferred_formats {
        for c in &configs {
            if c.sample_format() == fmt && c.channels() as u32 == desired_channels {
                // Pick the closest supported rate
                let rate = if target_rate < c.min_sample_rate() {
                    c.min_sample_rate()
                } else if target_rate > c.max_sample_rate() {
                    c.max_sample_rate()
                } else {
                    target_rate
                };
                let cfg = StreamConfig {
                    channels: desired_channels as u16,
                    sample_rate: rate,
                    buffer_size: BufferSize::Default,
                };
                log::warn!(
                    "CPAL rate fallback: requested {}Hz, using {}Hz (fmt={:?})",
                    desired_rate,
                    rate.0,
                    fmt
                );
                return Ok((cfg, fmt));
            }
        }
    }

    // Last resort: use first available config
    let first = &configs[0];
    let rate = if target_rate >= first.min_sample_rate() && target_rate <= first.max_sample_rate() {
        target_rate
    } else {
        first.min_sample_rate()
    };
    let cfg = StreamConfig {
        channels: first.channels(),
        sample_rate: rate,
        buffer_size: BufferSize::Default,
    };
    let fmt = first.sample_format();
    log::warn!(
        "CPAL full fallback: using ch={}, rate={}, fmt={:?}",
        cfg.channels,
        rate.0,
        fmt
    );
    Ok((cfg, fmt))
}

// ---------------------------------------------------------------------------
//  Public API: open capture / playback streams
// ---------------------------------------------------------------------------

/// Open a capture (input) stream.
///
/// Returns the running `Stream`, negotiated parameters, and a `RbConsumer`
/// from which the recording worker thread reads PCM i16 samples.
pub fn open_capture(
    device_name: &str,
    sample_rate: u32,
    channels: u32,
    _running: Arc<AtomicBool>,
) -> Result<(Stream, CpalParams, RbConsumer)> {
    let device = find_input_device(device_name)?;
    let (config, sample_format) = negotiate_input_config(&device, sample_rate, channels)?;

    let actual_rate = config.sample_rate.0;
    let actual_channels = config.channels as u32;

    // Ring buffer: ~500 ms worth of samples as headroom
    let rb_size = (actual_rate as usize) * (actual_channels as usize) / 2;
    let rb = HeapRb::<i16>::new(rb_size.max(4096));
    let (mut producer, consumer) = rb.split();

    let err_fn = |err: cpal::StreamError| {
        log::error!("CPAL capture stream error: {}", err);
    };

    let stream = match sample_format {
        SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                // Directly push i16 samples into the ring buffer.
                // If the consumer is too slow, oldest data is silently dropped.
                let _ = producer.push_slice(data);
            },
            err_fn,
            None,
        )?,
        SampleFormat::F32 => {
            device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // Convert f32 → i16 and push
                    for &sample in data {
                        let s = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                        let _ = producer.try_push(s);
                    }
                },
                err_fn,
                None,
            )?
        }
        other => anyhow::bail!("Unsupported capture sample format: {:?}", other),
    };

    stream.play().context("Failed to start capture stream")?;

    let params = CpalParams {
        sample_rate: actual_rate,
        channels: actual_channels,
    };

    log::info!(
        "CPAL Capture opened: device=\"{}\", rate={}, ch={}, fmt={:?}, rb_size={}",
        device_name,
        actual_rate,
        actual_channels,
        sample_format,
        rb_size,
    );

    Ok((stream, params, consumer))
}

/// Open a playback (output) stream.
///
/// Returns the running `Stream`, negotiated parameters, and a `RbProducer`
/// into which the playback worker thread writes decoded PCM i16 samples.
pub fn open_playback(
    device_name: &str,
    sample_rate: u32,
    channels: u32,
    period_size: usize,
    _running: Arc<AtomicBool>,
) -> Result<(Stream, CpalParams, RbProducer)> {
    let device = find_output_device(device_name)?;
    let (config, sample_format) =
        negotiate_output_config(&device, sample_rate, channels, period_size)?;

    let actual_rate = config.sample_rate.0;
    let actual_channels = config.channels as u32;

    // Ring buffer: ~500 ms worth of samples
    let rb_size = (actual_rate as usize) * (actual_channels as usize) / 2;
    let rb = HeapRb::<i16>::new(rb_size.max(4096));
    let (producer, mut consumer) = rb.split();

    let err_fn = |err: cpal::StreamError| {
        log::error!("CPAL playback stream error: {}", err);
    };

    let stream = match sample_format {
        SampleFormat::I16 => device.build_output_stream(
            &config,
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                let filled = consumer.pop_slice(data);
                // Fill remainder with silence if ring buffer doesn't have enough data
                if filled < data.len() {
                    data[filled..].fill(0);
                }
            },
            err_fn,
            None,
        )?,
        SampleFormat::F32 => {
            device.build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // Pop i16 from ring buffer and convert to f32
                    for sample in data.iter_mut() {
                        if let Some(s) = consumer.try_pop() {
                            *sample = s as f32 / 32768.0;
                        } else {
                            *sample = 0.0; // silence
                        }
                    }
                },
                err_fn,
                None,
            )?
        }
        other => anyhow::bail!("Unsupported playback sample format: {:?}", other),
    };

    stream.play().context("Failed to start playback stream")?;

    let params = CpalParams {
        sample_rate: actual_rate,
        channels: actual_channels,
    };

    log::info!(
        "CPAL Playback opened: device=\"{}\", rate={}, ch={}, fmt={:?}, rb_size={}",
        device_name,
        actual_rate,
        actual_channels,
        sample_format,
        rb_size,
    );

    Ok((stream, params, producer))
}
