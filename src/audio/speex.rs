//! Safe wrappers around SpeexDSP's preprocessor (denoise/AGC) and resampler.

use std::ffi::{c_int, c_void};

// ======================== FFI declarations ========================

/// Opaque type for SpeexPreprocessState
#[repr(C)]
pub struct SpeexPreprocessState {
    _private: [u8; 0],
}

/// Opaque type for SpeexResamplerState
#[repr(C)]
pub struct SpeexResamplerState {
    _private: [u8; 0],
}

// Preprocessor request constants
const SPEEX_PREPROCESS_SET_DENOISE: c_int = 0;
const SPEEX_PREPROCESS_SET_AGC: c_int = 2;
const SPEEX_PREPROCESS_SET_AGC_LEVEL: c_int = 6;
const SPEEX_PREPROCESS_SET_NOISE_SUPPRESS: c_int = 8;

// Resampler constants
const SPEEX_RESAMPLER_QUALITY_DEFAULT: c_int = 4;
const RESAMPLER_ERR_SUCCESS: c_int = 0;

unsafe extern "C" {
    fn speex_preprocess_state_init(frame_size: c_int, sampling_rate: c_int)
        -> *mut SpeexPreprocessState;
    fn speex_preprocess_state_destroy(st: *mut SpeexPreprocessState);
    fn speex_preprocess_run(st: *mut SpeexPreprocessState, x: *mut i16) -> c_int;
    fn speex_preprocess_ctl(
        st: *mut SpeexPreprocessState,
        request: c_int,
        ptr: *mut c_void,
    ) -> c_int;

    fn speex_resampler_init(
        nb_channels: u32,
        in_rate: u32,
        out_rate: u32,
        quality: c_int,
        err: *mut c_int,
    ) -> *mut SpeexResamplerState;
    fn speex_resampler_destroy(st: *mut SpeexResamplerState);
    fn speex_resampler_process_int(
        st: *mut SpeexResamplerState,
        channel_index: u32,
        in_: *const i16,
        in_len: *mut u32,
        out: *mut i16,
        out_len: *mut u32,
    ) -> c_int;
}

// ======================== Preprocessor (denoise + AGC) ========================

/// Safe wrapper around SpeexPreprocessState for noise suppression and AGC.
pub struct Preprocessor {
    state: *mut SpeexPreprocessState,
}

// SpeexPreprocessState is used from a single thread only
unsafe impl Send for Preprocessor {}

impl Preprocessor {
    /// Create a new preprocessor for a given frame size (in samples) and sample rate.
    pub fn new(frame_size: usize, sample_rate: u32) -> anyhow::Result<Self> {
        let state = unsafe {
            speex_preprocess_state_init(frame_size as c_int, sample_rate as c_int)
        };
        if state.is_null() {
            anyhow::bail!("Failed to initialize speex preprocessor");
        }
        Ok(Self { state })
    }

    /// Enable or disable denoising.
    pub fn set_denoise(&mut self, enable: bool) {
        let mut val: c_int = if enable { 1 } else { 0 };
        unsafe {
            speex_preprocess_ctl(
                self.state,
                SPEEX_PREPROCESS_SET_DENOISE,
                &mut val as *mut c_int as *mut c_void,
            );
        }
    }

    /// Set noise suppress level in dB (negative value, e.g. -25).
    pub fn set_noise_suppress(&mut self, level: i32) {
        let mut val: c_int = level;
        unsafe {
            speex_preprocess_ctl(
                self.state,
                SPEEX_PREPROCESS_SET_NOISE_SUPPRESS,
                &mut val as *mut c_int as *mut c_void,
            );
        }
    }

    /// Enable or disable automatic gain control.
    pub fn set_agc(&mut self, enable: bool) {
        let mut val: c_int = if enable { 1 } else { 0 };
        unsafe {
            speex_preprocess_ctl(
                self.state,
                SPEEX_PREPROCESS_SET_AGC,
                &mut val as *mut c_int as *mut c_void,
            );
        }
    }

    /// Set AGC level (target signal level).
    pub fn set_agc_level(&mut self, level: f32) {
        let mut val: f32 = level;
        unsafe {
            speex_preprocess_ctl(
                self.state,
                SPEEX_PREPROCESS_SET_AGC_LEVEL,
                &mut val as *mut f32 as *mut c_void,
            );
        }
    }

    /// Run the preprocessor on a frame of 16-bit PCM mono samples.
    /// The samples are modified in-place.
    pub fn process(&mut self, samples: &mut [i16]) {
        unsafe {
            speex_preprocess_run(self.state, samples.as_mut_ptr());
        }
    }
}

impl Drop for Preprocessor {
    fn drop(&mut self) {
        unsafe {
            speex_preprocess_state_destroy(self.state);
        }
    }
}

// ======================== Resampler ========================

/// Safe wrapper around SpeexResamplerState.
pub struct Resampler {
    state: *mut SpeexResamplerState,
}

unsafe impl Send for Resampler {}

impl Resampler {
    /// Create a new resampler.
    ///
    /// * `channels` - Number of channels
    /// * `in_rate`  - Input sample rate
    /// * `out_rate` - Output sample rate
    pub fn new(channels: u32, in_rate: u32, out_rate: u32) -> anyhow::Result<Self> {
        let mut err: c_int = 0;
        let state = unsafe {
            speex_resampler_init(
                channels,
                in_rate,
                out_rate,
                SPEEX_RESAMPLER_QUALITY_DEFAULT,
                &mut err,
            )
        };
        if err != RESAMPLER_ERR_SUCCESS || state.is_null() {
            anyhow::bail!("Failed to initialize speex resampler: err={}", err);
        }
        Ok(Self { state })
    }

    /// Resample a single channel of 16-bit PCM data.
    ///
    /// Returns `(input_samples_consumed, output_samples_produced)`.
    pub fn process_int(
        &mut self,
        channel: u32,
        input: &[i16],
        output: &mut [i16],
    ) -> anyhow::Result<(u32, u32)> {
        let mut in_len = input.len() as u32;
        let mut out_len = output.len() as u32;
        let err = unsafe {
            speex_resampler_process_int(
                self.state,
                channel,
                input.as_ptr(),
                &mut in_len,
                output.as_mut_ptr(),
                &mut out_len,
            )
        };
        if err != RESAMPLER_ERR_SUCCESS {
            anyhow::bail!("Speex resampler error: {}", err);
        }
        Ok((in_len, out_len))
    }
}

impl Drop for Resampler {
    fn drop(&mut self) {
        unsafe {
            speex_resampler_destroy(self.state);
        }
    }
}
