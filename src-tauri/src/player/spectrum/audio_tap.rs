#[cfg(target_os = "macos")]
use std::ffi::{c_char, c_void, CString};
use std::sync::Arc;

use super::input::SpectrumInput;

#[cfg(target_os = "macos")]
type NativeTapContext = *mut c_void;

#[cfg(target_os = "macos")]
extern "C" {
    fn lyrics_plus_audio_tap_start(
        bundle_id: *const c_char,
        callback: unsafe extern "C" fn(*const f32, u32, f64, *mut c_void),
        context: *mut c_void,
        out_tap: *mut *mut c_void,
    ) -> i32;
    fn lyrics_plus_audio_tap_stop(tap: *mut c_void);
    fn lyrics_plus_audio_tap_matches_bundle(tap: *mut c_void, bundle_id: *const c_char) -> i32;
}

#[cfg(target_os = "macos")]
pub(super) struct NativeTap {
    opaque: *mut c_void,
    context: NativeTapContext,
    _input: Arc<SpectrumInput>,
}

#[cfg(target_os = "macos")]
unsafe impl Send for NativeTap {}

#[cfg(target_os = "macos")]
impl NativeTap {
    pub(super) fn start(bundle_id: &str, input: Arc<SpectrumInput>) -> Result<Self, i32> {
        let bundle_id = CString::new(bundle_id).map_err(|_| -10001)?;
        let context = Arc::into_raw(input.clone()) as *mut c_void;
        let mut opaque = std::ptr::null_mut();
        let status = unsafe {
            lyrics_plus_audio_tap_start(bundle_id.as_ptr(), audio_callback, context, &mut opaque)
        };
        if status != 0 || opaque.is_null() {
            unsafe {
                drop(Arc::from_raw(context as *const SpectrumInput));
            }
            return Err(status);
        }
        Ok(Self {
            opaque,
            context,
            _input: input,
        })
    }

    pub(super) fn matches_bundle(&self, bundle_id: &str) -> bool {
        let Ok(bundle_id) = CString::new(bundle_id) else {
            return false;
        };
        unsafe { lyrics_plus_audio_tap_matches_bundle(self.opaque, bundle_id.as_ptr()) != 0 }
    }
}

#[cfg(target_os = "macos")]
impl Drop for NativeTap {
    fn drop(&mut self) {
        unsafe {
            lyrics_plus_audio_tap_stop(self.opaque);
            drop(Arc::from_raw(self.context as *const SpectrumInput));
        }
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn audio_callback(
    samples: *const f32,
    sample_count: u32,
    sample_rate: f64,
    context: *mut c_void,
) {
    if samples.is_null() || context.is_null() || sample_count == 0 {
        return;
    }
    let input = unsafe { &*(context as *const SpectrumInput) };
    let samples = unsafe { std::slice::from_raw_parts(samples, sample_count as usize) };
    input.push(samples, sample_rate);
}

#[cfg(not(target_os = "macos"))]
pub(super) struct NativeTap;

pub(super) const LYRICS_PLUS_SPECTRUM_UNSUPPORTED: i32 = -10000;
pub(super) const AUDIO_DEVICE_PERMISSIONS_ERROR: i32 = i32::from_be_bytes(*b"!hog");
