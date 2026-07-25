use anyhow::Result;

use super::speex::Preprocessor;

pub type AfeResult<T> = Result<T>;

#[derive(Clone, Copy)]
pub struct AudioFrameRef<'a> {
    pub samples: &'a [i16],
    pub sample_rate: u32,
    pub channels: u32,
}

pub struct AudioFrameMut<'a> {
    pub samples: &'a mut [i16],
    pub sample_rate: u32,
    pub channels: u32,
}

pub(crate) struct RenderReference {
    pub samples: Vec<i16>,
    pub sample_rate: u32,
    pub channels: u32,
}

pub trait AudioFrontend: Send {
    fn process_render_reference(&mut self, frame: AudioFrameRef<'_>) -> AfeResult<()>;
    fn process_capture(&mut self, frame: AudioFrameMut<'_>) -> AfeResult<()>;
}

#[derive(Default)]
pub struct NullFrontend;

impl AudioFrontend for NullFrontend {
    fn process_render_reference(&mut self, _frame: AudioFrameRef<'_>) -> AfeResult<()> {
        Ok(())
    }

    fn process_capture(&mut self, _frame: AudioFrameMut<'_>) -> AfeResult<()> {
        Ok(())
    }
}

pub struct SpeexFrontend {
    preprocessors: Vec<Preprocessor>,
    channel_buffers: Vec<Vec<i16>>,
    frame_size: usize,
    sample_rate: u32,
    channels: u32,
}

impl SpeexFrontend {
    pub fn new(frame_size: usize, sample_rate: u32, channels: u32) -> AfeResult<Self> {
        let mut preprocessors = Vec::with_capacity(channels as usize);
        for _ in 0..channels {
            let mut preprocessor = Preprocessor::new(frame_size, sample_rate)?;
            preprocessor.set_denoise(true);
            preprocessor.set_noise_suppress(-25);
            preprocessor.set_agc(true);
            preprocessor.set_agc_level(24_000.0);
            preprocessors.push(preprocessor);
        }
        Ok(Self {
            preprocessors,
            channel_buffers: (0..channels).map(|_| vec![0; frame_size]).collect(),
            frame_size,
            sample_rate,
            channels,
        })
    }
}

impl AudioFrontend for SpeexFrontend {
    fn process_render_reference(&mut self, _frame: AudioFrameRef<'_>) -> AfeResult<()> {
        // Speex preprocessing currently has no AEC stage. Keeping this method in the
        // interface lets an AEC-capable frontend consume the same render reference later.
        Ok(())
    }

    fn process_capture(&mut self, frame: AudioFrameMut<'_>) -> AfeResult<()> {
        anyhow::ensure!(
            frame.sample_rate == self.sample_rate,
            "capture sample rate changed"
        );
        anyhow::ensure!(
            frame.channels == self.channels,
            "capture channel count changed"
        );
        let channels = self.channels as usize;
        anyhow::ensure!(
            frame.samples.len().is_multiple_of(channels),
            "capture frame is not interleaved"
        );
        let frames = frame.samples.len() / channels;
        anyhow::ensure!(
            frames <= self.frame_size,
            "capture frame exceeds frontend period"
        );

        for index in 0..frames {
            for channel in 0..channels {
                self.channel_buffers[channel][index] = frame.samples[index * channels + channel];
            }
        }
        for channel in 0..channels {
            self.preprocessors[channel].process(&mut self.channel_buffers[channel][..frames]);
        }
        for index in 0..frames {
            for channel in 0..channels {
                frame.samples[index * channels + channel] = self.channel_buffers[channel][index];
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_frontend_preserves_capture_and_accepts_render_reference() {
        let mut frontend = NullFrontend;
        let render = [1, 2, 3, 4];
        frontend
            .process_render_reference(AudioFrameRef {
                samples: &render,
                sample_rate: 48_000,
                channels: 2,
            })
            .unwrap();

        let mut capture = [5, 6, 7, 8];
        frontend
            .process_capture(AudioFrameMut {
                samples: &mut capture,
                sample_rate: 24_000,
                channels: 1,
            })
            .unwrap();
        assert_eq!(capture, [5, 6, 7, 8]);
    }
}
