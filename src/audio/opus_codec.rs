//! Opus encoder/decoder with integrated Speex resampling and channel conversion.
//!
//! - Encoder: multi-channel input → channel mix → resample → Opus encode
//! - Decoder: Opus decode → resample → channel convert

use super::speex::Resampler;
use anyhow::Result;

// ======================== Opus Encoder ========================

pub struct OpusEncoder {
    encoder: opus::Encoder,
    resampler: Resampler,
    input_sample_rate: u32,
    input_channels: u32,
    output_sample_rate: u32,
    output_channels: u32,
    duration_ms: u32,
}

impl OpusEncoder {
    /// Create a new Opus encoder with integrated resampling.
    ///
    /// * `input_sample_rate`  - Sample rate from ALSA capture (e.g. 24000, 44100, 48000)
    /// * `input_channels`     - Number of ALSA capture channels (e.g. 2)
    /// * `duration_ms`        - Frame duration in ms (e.g. 60)
    /// * `output_sample_rate` - Opus codec sample rate (e.g. 24000)
    /// * `output_channels`    - Opus codec channels (e.g. 1 for mono)
    /// * `bitrate`            - Bitrate in bits/s (e.g. 64000)
    pub fn new(
        input_sample_rate: u32,
        input_channels: u32,
        duration_ms: u32,
        output_sample_rate: u32,
        output_channels: u32,
        bitrate: i32,
    ) -> Result<Self> {
        let channels = if output_channels == 1 {
            opus::Channels::Mono
        } else {
            opus::Channels::Stereo
        };

        let mut encoder =
            opus::Encoder::new(output_sample_rate, channels, opus::Application::Audio)?;
        encoder.set_bitrate(opus::Bitrate::Bits(bitrate))?;

        let resampler =
            Resampler::new(output_channels, input_sample_rate, output_sample_rate)?;

        Ok(Self {
            encoder,
            resampler,
            input_sample_rate,
            input_channels,
            output_sample_rate,
            output_channels,
            duration_ms,
        })
    }

    /// Number of samples per channel for one input frame.
    pub fn input_frame_size_per_channel(&self) -> usize {
        (self.input_sample_rate * self.duration_ms / 1000) as usize
    }

    /// Total number of interleaved i16 samples per input frame.
    pub fn input_frame_samples(&self) -> usize {
        self.input_frame_size_per_channel() * self.input_channels as usize
    }

    /// Encode one frame of interleaved PCM data to an Opus packet.
    ///
    /// Input length must equal `input_frame_samples()`.
    /// Returns the encoded Opus bytes.
    pub fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>> {
        let original_frame_size = self.input_frame_size_per_channel();
        let target_frame_size =
            (self.output_sample_rate * self.duration_ms / 1000) as usize;

        // Step 1: Channel mixing (multi-channel → output_channels)
        let mixed = self.mix_channels(pcm, original_frame_size);

        // Step 2: Resample (input_rate → output_rate)
        let mut resampled = vec![0i16; target_frame_size * self.output_channels as usize];
        let (_in_consumed, out_produced) =
            self.resampler.process_int(0, &mixed, &mut resampled)?;

        let actual_out_samples = out_produced as usize * self.output_channels as usize;

        // Step 3: Opus encode
        let mut opus_buf = vec![0u8; 4000];
        let encoded_len = self
            .encoder
            .encode(&resampled[..actual_out_samples], &mut opus_buf)?;

        opus_buf.truncate(encoded_len);
        Ok(opus_buf)
    }

    /// Mix input channels down to output_channels.
    fn mix_channels(&self, pcm: &[i16], frame_size: usize) -> Vec<i16> {
        if self.output_channels == 1 && self.input_channels > 1 {
            // Multi-channel → mono: average all channels
            let mut mono = vec![0i16; frame_size];
            for i in 0..frame_size {
                let mut sum: i32 = 0;
                for c in 0..self.input_channels as usize {
                    let idx = i * self.input_channels as usize + c;
                    if idx < pcm.len() {
                        sum += pcm[idx] as i32;
                    }
                }
                mono[i] = (sum / self.input_channels as i32) as i16;
            }
            mono
        } else if self.output_channels == self.input_channels {
            // Same channel count, passthrough
            pcm[..frame_size * self.input_channels as usize].to_vec()
        } else {
            // General channel mapping (wrap channels)
            let mut out = vec![0i16; frame_size * self.output_channels as usize];
            for i in 0..frame_size {
                for c in 0..self.output_channels as usize {
                    let src_idx =
                        i * self.input_channels as usize + (c % self.input_channels as usize);
                    let dst_idx = i * self.output_channels as usize + c;
                    if src_idx < pcm.len() {
                        out[dst_idx] = pcm[src_idx];
                    }
                }
            }
            out
        }
    }
}

// ======================== Opus Decoder ========================

pub struct OpusDecoder {
    decoder: opus::Decoder,
    resampler: Resampler,
    #[allow(dead_code)]
    input_sample_rate: u32,
    input_channels: u32,
    output_sample_rate: u32,
    output_channels: u32,
    duration_ms: u32,
}

impl OpusDecoder {
    /// Create a new Opus decoder with integrated resampling.
    ///
    /// * `input_sample_rate`  - Opus stream sample rate (e.g. 24000)
    /// * `input_channels`     - Opus stream channels (e.g. 1)
    /// * `duration_ms`        - Expected frame duration in ms (e.g. 20)
    /// * `output_sample_rate` - ALSA playback sample rate
    /// * `output_channels`    - ALSA playback channels
    pub fn new(
        input_sample_rate: u32,
        input_channels: u32,
        duration_ms: u32,
        output_sample_rate: u32,
        output_channels: u32,
    ) -> Result<Self> {
        let channels = if input_channels == 1 {
            opus::Channels::Mono
        } else {
            opus::Channels::Stereo
        };

        let decoder = opus::Decoder::new(input_sample_rate, channels)?;
        let resampler =
            Resampler::new(input_channels, input_sample_rate, output_sample_rate)?;

        Ok(Self {
            decoder,
            resampler,
            input_sample_rate,
            input_channels,
            output_sample_rate,
            output_channels,
            duration_ms,
        })
    }

    /// Decode an Opus packet to interleaved PCM at output_sample_rate/output_channels.
    pub fn decode(&mut self, opus_data: &[u8]) -> Result<Vec<i16>> {
        // Step 1: Opus decode (max 120ms @ 48kHz = 5760 samples/channel, use 6000 for safety)
        let max_frame_size = 6000;
        let mut pcm_buf = vec![0i16; max_frame_size * self.input_channels as usize];
        let decoded_samples_per_ch =
            self.decoder.decode(opus_data, &mut pcm_buf, false)?;

        // Step 2: Resample (input_rate → output_rate)
        // Dynamically size the output buffer based on actual decoded samples,
        // not fixed duration_ms, to handle multi-frame Opus packets (e.g. 60ms).
        let expected_out_samples = (decoded_samples_per_ch as f64
            * (self.output_sample_rate as f64 / self.input_sample_rate as f64))
            .ceil() as usize;
        // Allocate slightly larger to handle rounding
        let mut resampled =
            vec![0i16; (expected_out_samples + 64) * self.input_channels as usize];

        let (in_consumed, out_produced) = self.resampler.process_int(
            0,
            &pcm_buf[..decoded_samples_per_ch * self.input_channels as usize],
            &mut resampled,
        )?;

        if in_consumed != decoded_samples_per_ch as u32 {
            log::warn!(
                "Resampler did not consume all input: consumed={}, total={}",
                in_consumed,
                decoded_samples_per_ch
            );
        }

        let actual_out = out_produced as usize;

        // Step 3: Channel conversion
        self.convert_channels(&resampled, actual_out)
    }

    /// Convert channels from input_channels to output_channels.
    fn convert_channels(
        &self,
        resampled: &[i16],
        frame_size: usize,
    ) -> Result<Vec<i16>> {
        if self.output_channels == self.input_channels {
            // Same channel count
            Ok(resampled[..frame_size * self.output_channels as usize].to_vec())
        } else if self.output_channels == 1 && self.input_channels > 1 {
            // Multi-channel → mono
            let mut mono = vec![0i16; frame_size];
            for i in 0..frame_size {
                let mut sum: i32 = 0;
                for c in 0..self.input_channels as usize {
                    sum += resampled[i * self.input_channels as usize + c] as i32;
                }
                mono[i] = (sum / self.input_channels as i32) as i16;
            }
            Ok(mono)
        } else {
            // Upmix / general channel mapping (e.g., mono → stereo: duplicate)
            let mut out = vec![0i16; frame_size * self.output_channels as usize];
            for i in 0..frame_size {
                for c in 0..self.output_channels as usize {
                    let src_c = c % self.input_channels as usize;
                    out[i * self.output_channels as usize + c] =
                        resampled[i * self.input_channels as usize + src_c];
                }
            }
            Ok(out)
        }
    }
}

// ======================== StreamDecoder impl ========================

use super::stream_decoder::StreamDecoder;

impl StreamDecoder for OpusDecoder {
    fn decode(&mut self, data: &[u8]) -> Result<Vec<i16>> {
        OpusDecoder::decode(self, data)
    }
}
