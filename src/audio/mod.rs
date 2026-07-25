//! audio - Audio capture, playback, and codec library
//!
//! Replaces the external C++ sound_app process with an integrated Rust library.
//! Uses ALSA for audio I/O, Opus for encoding/decoding, and SpeexDSP
//! for noise suppression, AGC, and resampling.

mod alsa_device;
mod audio_system;
mod backend;
mod frontend;
mod opus_codec;
mod play;
mod record;
mod speex;
pub mod stream_decoder;

pub use alsa_device::AlsaBackend;
pub use audio_system::{AudioConfig, AudioSystem};
pub use backend::{AudioBackend, AudioStreamParams, CaptureStream, MockBackend, PlaybackStream};
pub use frontend::{
    AfeResult, AudioFrameMut, AudioFrameRef, AudioFrontend, NullFrontend, SpeexFrontend,
};
pub use stream_decoder::StreamDecoder;
