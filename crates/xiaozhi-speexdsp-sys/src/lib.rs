#![allow(non_camel_case_types)]

use std::ffi::{c_int, c_void};

#[repr(C)]
pub struct SpeexPreprocessState {
    _private: [u8; 0],
}

#[repr(C)]
pub struct SpeexResamplerState {
    _private: [u8; 0],
}

unsafe extern "C" {
    pub fn speex_preprocess_state_init(
        frame_size: c_int,
        sampling_rate: c_int,
    ) -> *mut SpeexPreprocessState;
    pub fn speex_preprocess_state_destroy(st: *mut SpeexPreprocessState);
    pub fn speex_preprocess_run(st: *mut SpeexPreprocessState, samples: *mut i16) -> c_int;
    pub fn speex_preprocess_ctl(
        st: *mut SpeexPreprocessState,
        request: c_int,
        ptr: *mut c_void,
    ) -> c_int;

    pub fn speex_resampler_init(
        channels: u32,
        input_rate: u32,
        output_rate: u32,
        quality: c_int,
        error: *mut c_int,
    ) -> *mut SpeexResamplerState;
    pub fn speex_resampler_destroy(st: *mut SpeexResamplerState);
    pub fn speex_resampler_process_int(
        st: *mut SpeexResamplerState,
        channel_index: u32,
        input: *const i16,
        input_len: *mut u32,
        output: *mut i16,
        output_len: *mut u32,
    ) -> c_int;
}
