//! ALSA PCM device wrappers for audio capture and playback.

use alsa::pcm::{Access, Format, HwParams, PCM};
use alsa::{Direction, ValueOr};
use anyhow::{Context, Result};

/// Parameters negotiated with the ALSA hardware.
#[derive(Debug, Clone)]
pub struct AlsaParams {
    /// Actual sample rate after negotiation
    pub sample_rate: u32,
    /// Actual number of channels
    pub channels: u32,
    /// Period size in frames (one frame = channels × sample_width)
    pub period_size: usize,
}

/// Open a PCM device for capture (recording).
pub fn open_capture(device: &str, sample_rate: u32, channels: u32) -> Result<(PCM, AlsaParams)> {
    open_pcm(device, Direction::Capture, sample_rate, channels, None, "Capture")
}

/// Open a PCM device for playback.
pub fn open_playback(
    device: &str,
    sample_rate: u32,
    channels: u32,
    period_size: Option<usize>,
) -> Result<(PCM, AlsaParams)> {
    open_pcm(
        device,
        Direction::Playback,
        sample_rate,
        channels,
        period_size,
        "Playback",
    )
}

fn open_pcm(
    device: &str,
    direction: Direction,
    sample_rate: u32,
    channels: u32,
    period_size: Option<usize>,
    dir_name: &str,
) -> Result<(PCM, AlsaParams)> {
    let pcm = PCM::new(device, direction, false)
        .with_context(|| format!("Failed to open PCM device '{}' for {}", device, dir_name))?;

    // 1. 动态探测与配置硬件参数 (HwParams)
    let (actual_rate, actual_channels, actual_period_size, actual_buffer_size) = {
        let hwp =
            HwParams::any(&pcm).with_context(|| "Failed to initialize HwParams")?;

        // 动态探测设备支持的边界能力（便于日志排查 USB 声卡的限制）
        log::info!(
            "ALSA Probe [{} {}]: Rate=[{} - {}], Channels=[{} - {}]",
            device,
            dir_name,
            hwp.get_rate_min().unwrap_or(0),
            hwp.get_rate_max().unwrap_or(0),
            hwp.get_channels_min().unwrap_or(0),
            hwp.get_channels_max().unwrap_or(0),
        );

        hwp.set_access(Access::RWInterleaved)?;
        hwp.set_format(Format::S16LE)?;
        hwp.set_channels(channels)?;
        hwp.set_rate_near(sample_rate, ValueOr::Nearest)?;

        // 动态协商 Buffer Size（让 ALSA 分配尽量充裕的缓冲区，抗击 USB 传输抖动）
        let max_buffer = hwp.get_buffer_size_max().unwrap_or(8192) as usize;
        let target_buffer = std::cmp::min(max_buffer, 8192) as alsa::pcm::Frames;
        if let Err(e) = hwp.set_buffer_size_near(target_buffer) {
            log::warn!("Could not set optimal buffer size: {}", e);
        }

        if let Some(ps) = period_size {
            hwp.set_period_size_near(ps as alsa::pcm::Frames, ValueOr::Nearest)?;
        }
        pcm.hw_params(&hwp)?;

        // 读取底层最终协商确认的参数
        let hwp_current = pcm.hw_params_current()?;
        (
            hwp_current.get_rate()?,
            hwp_current.get_channels()?,
            hwp_current.get_period_size()? as usize,
            hwp_current.get_buffer_size()? as usize,
        )
    };

    // 2. 动态配置软件参数 (SwParams)
    {
        if let Ok(swp) = pcm.sw_params_current() {
            // 设置启动阈值：缓冲区数据达到 buffer_size 的一半（或至少一个 period）时才开始传输
            // 给应用程序足够的 "蓄水" 时间，避免启动瞬间被抽干
            let start_threshold =
                (actual_buffer_size / 2).max(actual_period_size) as alsa::pcm::Frames;
            let _ = swp.set_start_threshold(start_threshold);

            // 设置可用空间下限阈值
            let _ = swp.set_avail_min(actual_period_size as alsa::pcm::Frames);

            if let Err(e) = pcm.sw_params(&swp) {
                log::warn!("Failed to inject ALSA sw_params (non-fatal): {}", e);
            }
        }
    }

    let params = AlsaParams {
        sample_rate: actual_rate,
        channels: actual_channels,
        period_size: actual_period_size,
    };

    log::info!(
        "ALSA {} Negotiated: device={}, rate={}, ch={}, period_size={}, buffer_size={}",
        dir_name,
        device,
        actual_rate,
        actual_channels,
        actual_period_size,
        actual_buffer_size,
    );

    Ok((pcm, params))
}
