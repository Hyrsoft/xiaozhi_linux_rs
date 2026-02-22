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

    // Configure hardware parameters
    {
        let hwp =
            HwParams::any(&pcm).with_context(|| "Failed to initialize HwParams")?;
        hwp.set_access(Access::RWInterleaved)?;
        hwp.set_format(Format::S16LE)?;
        hwp.set_channels(channels)?;
        hwp.set_rate_near(sample_rate, ValueOr::Nearest)?;
        if let Some(ps) = period_size {
            hwp.set_period_size_near(ps as alsa::pcm::Frames, ValueOr::Nearest)?;
        }
        pcm.hw_params(&hwp)?;
    }

    // Read back actual negotiated parameters
    let (actual_rate, actual_channels, period_size) = {
        let hwp = pcm.hw_params_current()?;
        let rate = hwp.get_rate()?;
        let ch = hwp.get_channels()?;
        let ps = hwp.get_period_size()? as usize;
        (rate, ch, ps)
    };

    let params = AlsaParams {
        sample_rate: actual_rate,
        channels: actual_channels,
        period_size,
    };

    log::info!(
        "ALSA {}: device={}, rate={}, channels={}, period_size={}",
        dir_name,
        device,
        actual_rate,
        actual_channels,
        period_size,
    );

    Ok((pcm, params))
}
