//! audio - Audio capture, playback, and codec library
//!
//! Uses CPAL for cross-platform audio I/O with lock-free ring buffers,
//! Opus for encoding/decoding, and SpeexDSP for noise suppression,
//! AGC, and resampling.

mod cpal_device;
mod audio_system;
mod opus_codec;
mod play;
mod record;
mod speex;
pub mod stream_decoder;

pub use audio_system::{AudioConfig, AudioSystem};
