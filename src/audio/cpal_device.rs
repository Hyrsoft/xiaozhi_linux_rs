//! 基于 cpal 的音频录制与播放设备封装。
//! 使用 cpal 替代之前的 ALSA 实现，提供跨平台支持。
//! 音频数据通过无锁环形缓冲区（ringbuf）在 CPAL 回调线程和应用程序工作线程之间交换。

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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Ring buffer producer half (writing side) for i16 samples.
pub type RbProducer = ringbuf::HeapProd<i16>;
/// Ring buffer consumer half (reading side) for i16 samples.
pub type RbConsumer = ringbuf::HeapCons<i16>;


/// 创建音频流后，从 capl 设备协商得到的实际参数（采样率、通道数等）。
/// 应用程序工作线程使用这些参数进行正确的编码/解码处理。
#[derive(Debug, Clone)]
pub struct CpalParams {
    /// Actual sample rate used by the stream
    /// 实际的采样率
    pub sample_rate: u32,
    /// Actual number of channels
    /// 实际的通道数
    pub channels: u32,
}

// ---------------------------------------------------------------------------
//  Audio device enumeration (for CLI help / diagnostics)
// ---------------------------------------------------------------------------

pub struct AudioDevice;

impl AudioDevice {
    /// 打印所有可用的音频输入/输出设备及其支持的配置。
    /// 用于 `--help` / `--list-devices` 命令行输出，帮助用户选择设备名称。
    pub fn print_audio_devices() {
    let host = cpal::default_host();

    println!("音频后端: {:?}", host.id());
    println!();

    // ---- 输入（录音）设备 ----
    println!("══════════════════════════════════════════");
    println!("  录音（输入）设备");
    println!("══════════════════════════════════════════");

    let default_in_name = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();

    match host.input_devices() {
        Ok(devices) => {
            let mut count = 0u32;
            for device in devices {
                count += 1;
                let name = device.name().unwrap_or_else(|_| "<unknown>".into());
                let is_default = name == default_in_name;
                println!(
                    "  [{}] {}{}",
                    count,
                    name,
                    if is_default { "  ← 默认" } else { "" }
                );
            }
            if count == 0 {
                println!("  (无可用录音设备)");
            }
        }
        Err(e) => println!("  查询录音设备失败: {}", e),
    }

    println!();

    // ---- 输出（播放）设备 ----
    println!("══════════════════════════════════════════");
    println!("  播放（输出）设备");
    println!("══════════════════════════════════════════");

    let default_out_name = host
        .default_output_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();

    match host.output_devices() {
        Ok(devices) => {
            let mut count = 0u32;
            for device in devices {
                count += 1;
                let name = device.name().unwrap_or_else(|_| "<unknown>".into());
                let is_default = name == default_out_name;
                println!(
                    "  [{}] {}{}",
                    count,
                    name,
                    if is_default { "  ← 默认" } else { "" }
                );
            }
            if count == 0 {
                println!("  (无可用播放设备)");
            }
        }
        Err(e) => println!("  查询播放设备失败: {}", e),
    }

    println!();
    println!("提示: 将上述设备名称填入 config.toml 的 capture_device / playback_device 字段。");
    println!("      使用 \"default\" 则自动选择标记为 \"← 默认\" 的设备。");
}

// ---------------------------------------------------------------------------
//  Device lookup helpers
//  设备查找辅助函数
// ---------------------------------------------------------------------------

    /// 基于传入的设备名称查找 CPAL 输入（录音）设备。传入 "default" 则返回主机默认输入设备。
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

    /// 基于传入的设备名称查找 CPAL 输出（播放）设备。传入 "default" 则返回主机默认输出设备。
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
        Self::negotiate_config(device, desired_rate, desired_channels, true)
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

    // Log available ranges at debug level (can be very verbose on some devices)
    for c in &configs {
        log::debug!(
            "CPAL supported config: channels={}, rate=[{}-{}], format={:?}",
            c.channels(),
            c.min_sample_rate().0,
            c.max_sample_rate().0,
            c.sample_format(),
        );
    }
    log::info!(
        "CPAL {} device: {} supported config(s) found",
        if is_input { "capture" } else { "playback" },
        configs.len(),
    );

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
        let device = Self::find_input_device(device_name)?;
        let (config, sample_format) = Self::negotiate_input_config(&device, sample_rate, channels)?;

    let actual_rate = config.sample_rate.0;
    let actual_channels = config.channels as u32;

    // Ring buffer: ~500 ms worth of samples as headroom
    let rb_size = (actual_rate as usize) * (actual_channels as usize) / 2;
    let rb = HeapRb::<i16>::new(rb_size.max(4096));
    let (mut producer, consumer) = rb.split();

    // Rate-limit error logging to avoid log spam (e.g. repeated POLLERR)
    static CAPTURE_ERR_TS: AtomicU64 = AtomicU64::new(0);
    let err_fn = move |err: cpal::StreamError| {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let prev = CAPTURE_ERR_TS.load(Ordering::Relaxed);
        if now != prev {
            CAPTURE_ERR_TS.store(now, Ordering::Relaxed);
            log::error!("CPAL capture stream error: {}", err);
        }
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

    /// Open a playback (output) stream using the device's default configuration.
    ///
    /// The device's preferred sample rate, channels, and buffer size are used directly.
    /// Speex resampling in the decoder handles any rate/channel conversion.
    pub fn open_playback(
        device_name: &str,
        _running: Arc<AtomicBool>,
    ) -> Result<(Stream, CpalParams, RbProducer)> {
        let device = Self::find_output_device(device_name)?;
    let supported = device
        .default_output_config()
        .context("Failed to get default output config")?;
    let sample_format = supported.sample_format();
    let config = supported.config();

    let actual_rate = config.sample_rate.0;
    let actual_channels = config.channels as u32;

    log::info!(
        "CPAL playback device default: rate={}, ch={}, fmt={:?}",
        actual_rate,
        actual_channels,
        sample_format,
    );

    // Ring buffer: ~500 ms worth of samples
    let rb_size = (actual_rate as usize) * (actual_channels as usize) / 2;
    let rb = HeapRb::<i16>::new(rb_size.max(4096));
    let (producer, mut consumer) = rb.split();

    // Rate-limit error logging to avoid log spam (e.g. repeated POLLERR)
    static PLAYBACK_ERR_TS: AtomicU64 = AtomicU64::new(0);
    let err_fn = move |err: cpal::StreamError| {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let prev = PLAYBACK_ERR_TS.load(Ordering::Relaxed);
        if now != prev {
            PLAYBACK_ERR_TS.store(now, Ordering::Relaxed);
            log::error!("CPAL playback stream error: {}", err);
        }
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
}
