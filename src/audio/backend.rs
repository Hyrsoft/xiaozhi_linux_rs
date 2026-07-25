use anyhow::Result;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::audio_system::AudioConfig;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioStreamParams {
    pub sample_rate: u32,
    pub channels: u32,
    pub period_size: usize,
}

pub trait CaptureStream: Send {
    fn params(&self) -> AudioStreamParams;
    fn read(&mut self, samples: &mut [i16]) -> Result<usize>;
    fn recover(&mut self) -> Result<()>;
}

pub trait PlaybackStream: Send {
    fn params(&self) -> AudioStreamParams;
    fn write(&mut self, samples: &[i16]) -> Result<usize>;
    fn recover(&mut self) -> Result<()>;
}

pub trait AudioBackend: Send + Sync {
    fn open_capture(&self, config: &AudioConfig) -> Result<Box<dyn CaptureStream>>;
    fn open_playback(&self, config: &AudioConfig) -> Result<Box<dyn PlaybackStream>>;
}

#[derive(Clone)]
pub struct MockBackend {
    capture_params: AudioStreamParams,
    playback_params: AudioStreamParams,
    capture_frames: Arc<Mutex<VecDeque<Vec<i16>>>>,
    playback_frames: Arc<Mutex<Vec<Vec<i16>>>>,
}

impl MockBackend {
    pub fn new(capture_params: AudioStreamParams, playback_params: AudioStreamParams) -> Self {
        Self {
            capture_params,
            playback_params,
            capture_frames: Arc::new(Mutex::new(VecDeque::new())),
            playback_frames: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn push_capture_frame(&self, samples: Vec<i16>) {
        self.capture_frames
            .lock()
            .expect("mock capture mutex poisoned")
            .push_back(samples);
    }

    pub fn playback_frames(&self) -> Vec<Vec<i16>> {
        self.playback_frames
            .lock()
            .expect("mock playback mutex poisoned")
            .clone()
    }
}

impl AudioBackend for MockBackend {
    fn open_capture(&self, _config: &AudioConfig) -> Result<Box<dyn CaptureStream>> {
        Ok(Box::new(MockCaptureStream {
            params: self.capture_params,
            frames: self.capture_frames.clone(),
        }))
    }

    fn open_playback(&self, _config: &AudioConfig) -> Result<Box<dyn PlaybackStream>> {
        Ok(Box::new(MockPlaybackStream {
            params: self.playback_params,
            frames: self.playback_frames.clone(),
        }))
    }
}

struct MockCaptureStream {
    params: AudioStreamParams,
    frames: Arc<Mutex<VecDeque<Vec<i16>>>>,
}

impl CaptureStream for MockCaptureStream {
    fn params(&self) -> AudioStreamParams {
        self.params
    }

    fn read(&mut self, samples: &mut [i16]) -> Result<usize> {
        let channels = self.params.channels as usize;
        let frame = self
            .frames
            .lock()
            .expect("mock capture mutex poisoned")
            .pop_front()
            .unwrap_or_else(|| vec![0; self.params.period_size * channels]);
        let retained = frame.len().min(samples.len());
        samples[..retained].copy_from_slice(&frame[..retained]);
        Ok(retained / channels)
    }

    fn recover(&mut self) -> Result<()> {
        Ok(())
    }
}

struct MockPlaybackStream {
    params: AudioStreamParams,
    frames: Arc<Mutex<Vec<Vec<i16>>>>,
}

impl PlaybackStream for MockPlaybackStream {
    fn params(&self) -> AudioStreamParams {
        self.params
    }

    fn write(&mut self, samples: &[i16]) -> Result<usize> {
        self.frames
            .lock()
            .expect("mock playback mutex poisoned")
            .push(samples.to_vec());
        Ok(samples.len() / self.params.channels as usize)
    }

    fn recover(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_backend_captures_and_plays_without_hardware() {
        let params = AudioStreamParams {
            sample_rate: 24_000,
            channels: 1,
            period_size: 4,
        };
        let backend = MockBackend::new(params, params);
        backend.push_capture_frame(vec![1, 2, 3, 4]);

        let mut capture = backend.open_capture(&AudioConfig::default()).unwrap();
        let mut input = [0; 4];
        assert_eq!(capture.read(&mut input).unwrap(), 4);
        assert_eq!(input, [1, 2, 3, 4]);

        let mut playback = backend.open_playback(&AudioConfig::default()).unwrap();
        assert_eq!(playback.write(&input).unwrap(), 4);
        assert_eq!(backend.playback_frames(), vec![vec![1, 2, 3, 4]]);
    }
}
