//! Generic stream decoder trait for multi-format audio playback support.

use anyhow::Result;

/// A trait for audio stream decoders that convert compressed audio data
/// into interleaved i16 PCM samples ready for ALSA playback.
///
/// Implementations handle format-specific decoding, resampling, and
/// channel conversion internally.
pub trait StreamDecoder: Send {
    /// Decode compressed audio bytes into interleaved i16 PCM samples.
    fn decode(&mut self, data: &[u8]) -> Result<Vec<i16>>;
}
