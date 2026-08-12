use std::cell::RefCell;
use std::ptr::NonNull;

use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::{AnyObject, NSObject, ProtocolObject};
use objc2::{define_class, msg_send, AnyThread, DefinedClass};
use objc2_app_kit::NSRunningApplication;
use objc2_core_services::{kAEDontReconnect, kAEWaitReply, AEKeyword, AppleEvent};
use objc2_foundation::{NSAppleEventDescriptor, NSError, NSNumber, NSObjectProtocol, NSString};
use objc2_scripting_bridge::{SBApplication, SBApplicationDelegate, SBObject};

use super::{ensure_track_id, now_ms, PlaybackErrorCode, PlaybackSnapshot, PlayerKind};

mod apple_music;
mod spotify;

pub(crate) use apple_music::{
    export_artwork as export_apple_music_artwork, ArtworkExport as AppleMusicArtworkExport,
};
pub(crate) use spotify::artwork_url as spotify_artwork_url;

const PLAYER_STATE: AEKeyword = u32::from_be_bytes(*b"pPlS");
const CURRENT_TRACK: AEKeyword = u32::from_be_bytes(*b"pTrk");
const PLAYER_POSITION: AEKeyword = u32::from_be_bytes(*b"pPos");
const NAME: AEKeyword = u32::from_be_bytes(*b"pnam");
const ARTIST: AEKeyword = u32::from_be_bytes(*b"pArt");
const ALBUM: AEKeyword = u32::from_be_bytes(*b"pAlb");
const DURATION: AEKeyword = u32::from_be_bytes(*b"pDur");
const PLAYING: u32 = u32::from_be_bytes(*b"kPSP");
const STOPPED: u32 = u32::from_be_bytes(*b"kPSS");
const REQUEST_TIMEOUT_TICKS: i64 = 3 * 60;
pub(super) const ARTWORK_TIMEOUT_TICKS: i64 = 4 * 60;

#[derive(Debug, Clone)]
pub(super) struct AutomationError {
    code: isize,
    message: String,
}

impl AutomationError {
    fn from_nserror(error: &NSError) -> Self {
        Self {
            code: error.code(),
            message: error.localizedDescription().to_string(),
        }
    }

    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            code: 0,
            message: message.into(),
        }
    }

    fn playback_code(&self) -> PlaybackErrorCode {
        match self.code {
            -1743 => PlaybackErrorCode::AutomationDenied,
            -1712 => PlaybackErrorCode::ResponseTimeout,
            _ => PlaybackErrorCode::Unavailable,
        }
    }

    fn user_message(&self) -> String {
        match self.code {
            -1743 => "没有播放器自动化权限。请到“系统设置 → 隐私与安全性 → 自动化”允许 Lyrics Plus 控制播放器。".into(),
            -1712 => "播放器响应超时".into(),
            _ if self.message.is_empty() => "播放器暂不可用".into(),
            _ => self.message.clone(),
        }
    }
}

#[derive(Default)]
struct AutomationDelegateIvars {
    error: RefCell<Option<AutomationError>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = AutomationDelegateIvars]
    struct AutomationDelegate;

    unsafe impl NSObjectProtocol for AutomationDelegate {}

    unsafe impl SBApplicationDelegate for AutomationDelegate {
        #[unsafe(method_id(eventDidFail:withError:))]
        unsafe fn event_did_fail(
            &self,
            _event: NonNull<AppleEvent>,
            error: &NSError,
        ) -> Option<Retained<AnyObject>> {
            self.ivars()
                .error
                .replace(Some(AutomationError::from_nserror(error)));
            None
        }
    }
);

impl AutomationDelegate {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(AutomationDelegateIvars::default());
        unsafe { msg_send![super(this), init] }
    }

    fn reset(&self) {
        self.ivars().error.replace(None);
    }

    fn take_error(&self) -> Option<AutomationError> {
        self.ivars().error.borrow_mut().take()
    }
}

pub(super) struct AutomationSession<'a> {
    app: &'a SBApplication,
    delegate: &'a AutomationDelegate,
}

impl AutomationSession<'_> {
    fn finish(&self, object: &SBObject) -> Result<(), AutomationError> {
        if let Some(error) = self.delegate.take_error() {
            return Err(error);
        }
        if let Some(error) = unsafe { object.lastError() } {
            return Err(AutomationError::from_nserror(&error));
        }
        Ok(())
    }

    pub(super) fn value(
        &self,
        object: &SBObject,
        code: AEKeyword,
    ) -> Result<Option<Retained<AnyObject>>, AutomationError> {
        self.delegate.reset();
        let property = unsafe { object.propertyWithCode(code) };
        let value = unsafe { property.get() };
        self.finish(&property)?;
        Ok(value)
    }

    pub(super) fn object(
        &self,
        object: &SBObject,
        code: AEKeyword,
    ) -> Result<Option<Retained<SBObject>>, AutomationError> {
        self.value(object, code)?
            .map(|value| {
                value
                    .downcast::<SBObject>()
                    .map_err(|_| AutomationError::unavailable("播放器返回了无效对象"))
            })
            .transpose()
    }

    pub(super) fn string(
        &self,
        object: &SBObject,
        code: AEKeyword,
    ) -> Result<Option<String>, AutomationError> {
        self.value(object, code)
            .map(|value| value.as_deref().and_then(value_as_string))
    }

    fn number(&self, object: &SBObject, code: AEKeyword) -> Result<Option<f64>, AutomationError> {
        self.value(object, code)
            .map(|value| value.as_deref().and_then(value_as_number))
    }

    fn type_code(
        &self,
        object: &SBObject,
        code: AEKeyword,
    ) -> Result<Option<u32>, AutomationError> {
        self.value(object, code).map(|value| {
            value
                .as_deref()
                .and_then(|value| value.downcast_ref::<NSAppleEventDescriptor>())
                .map(|value| value.enumCodeValue())
        })
    }

    pub(super) fn current_track(&self) -> Result<Option<Retained<SBObject>>, AutomationError> {
        self.object(self.app, CURRENT_TRACK)
    }

    pub(super) fn first_element(
        &self,
        object: &SBObject,
        code: AEKeyword,
    ) -> Result<Option<Retained<SBObject>>, AutomationError> {
        self.delegate.reset();
        let elements = unsafe { object.elementArrayWithCode(code) };
        let value = elements.firstObject();
        self.finish(self.app)?;
        value
            .map(|value| {
                value
                    .downcast::<SBObject>()
                    .map_err(|_| AutomationError::unavailable("播放器返回了无效对象"))
            })
            .transpose()
    }

    pub(super) fn command(
        &self,
        action: &str,
        position_ms: Option<u64>,
    ) -> Result<(), AutomationError> {
        self.delegate.reset();
        match action {
            "play_pause" => unsafe {
                let _: () = msg_send![self.app, playpause];
            },
            "next" => unsafe {
                let _: () = msg_send![self.app, nextTrack];
            },
            "previous" => unsafe {
                let _: () = msg_send![self.app, previousTrack];
            },
            "seek" => {
                let seconds =
                    position_ms.ok_or_else(|| AutomationError::unavailable("缺少跳转位置"))? as f64
                        / 1000.0;
                let property = unsafe { self.app.propertyWithCode(PLAYER_POSITION) };
                let value = NSNumber::numberWithDouble(seconds);
                unsafe { property.setTo(Some(&value)) };
                return self.finish(&property);
            }
            _ => return Err(AutomationError::unavailable("未知播放器操作")),
        }
        self.finish(self.app)
    }
}

pub(super) fn with_application<T>(
    bundle_id: &str,
    timeout_ticks: i64,
    operation: impl FnOnce(&AutomationSession<'_>) -> Result<T, AutomationError>,
) -> Result<T, AutomationError> {
    autoreleasepool(|_| {
        let applications = NSRunningApplication::runningApplicationsWithBundleIdentifier(
            &NSString::from_str(bundle_id),
        );
        let running = applications
            .firstObject()
            .filter(|application| !application.isTerminated())
            .ok_or_else(|| AutomationError::unavailable("播放器未运行"))?;
        let pid = running.processIdentifier();
        if pid <= 0 {
            return Err(AutomationError::unavailable("无法获取播放器进程"));
        }
        let app = unsafe { SBApplication::applicationWithProcessIdentifier(pid) }
            .ok_or_else(|| AutomationError::unavailable("播放器不支持自动化"))?;
        let delegate = AutomationDelegate::new();
        unsafe {
            app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            app.setSendMode((kAEWaitReply | kAEDontReconnect) as i32);
            app.setTimeout(timeout_ticks);
        }
        operation(&AutomationSession {
            app: &app,
            delegate: &delegate,
        })
    })
}

pub(crate) fn snapshot(kind: PlayerKind) -> PlaybackSnapshot {
    match kind {
        PlayerKind::AppleMusic => apple_music::snapshot(),
        PlayerKind::Spotify => spotify::snapshot(),
        PlayerKind::System => unreachable!("system playback uses SystemMediaService"),
    }
}

pub(crate) fn perform_action(
    kind: PlayerKind,
    action: &str,
    position_ms: Option<u64>,
) -> Result<(), String> {
    match kind {
        PlayerKind::AppleMusic => apple_music::perform_action(action, position_ms),
        PlayerKind::Spotify => spotify::perform_action(action, position_ms),
        PlayerKind::System => Err("系统播放器操作必须通过系统媒体服务执行".into()),
    }
}

fn query(
    kind: PlayerKind,
    bundle_id: &str,
    duration_scale: u64,
    track_id_property: AEKeyword,
) -> PlaybackSnapshot {
    let result = with_application(bundle_id, REQUEST_TIMEOUT_TICKS, |session| {
        let state = session.type_code(session.app, PLAYER_STATE)?;
        if is_stopped(state) {
            return Ok(PlaybackSnapshot {
                player: Some(kind),
                is_running: true,
                observed_at_ms: now_ms(),
                ..Default::default()
            });
        }
        let Some(track) = session.current_track()? else {
            return Ok(PlaybackSnapshot {
                player: Some(kind),
                is_running: true,
                is_playing: is_playing(state),
                observed_at_ms: now_ms(),
                ..Default::default()
            });
        };
        let duration_ms = scaled_duration_ms(session.number(&track, DURATION)?, duration_scale);
        let position_ms = session
            .number(session.app, PLAYER_POSITION)?
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(|value| (value * 1000.0).round() as u64);
        let mut snapshot = PlaybackSnapshot {
            player: Some(kind),
            is_running: true,
            is_playing: is_playing(state),
            track_id: session.string(&track, track_id_property)?,
            title: session.string(&track, NAME)?,
            artist: session.string(&track, ARTIST)?,
            album: session.string(&track, ALBUM)?,
            duration_ms,
            position_ms,
            can_seek: true,
            observed_at_ms: now_ms(),
            ..Default::default()
        };
        ensure_track_id(&mut snapshot);
        Ok(snapshot)
    });
    result.unwrap_or_else(|error| {
        PlaybackSnapshot::unavailable_with_code(
            Some(kind),
            error.playback_code(),
            error.user_message(),
        )
    })
}

fn perform_action_for_app(
    bundle_id: &str,
    action: &str,
    position_ms: Option<u64>,
) -> Result<(), String> {
    with_application(bundle_id, REQUEST_TIMEOUT_TICKS, |session| {
        session.command(action, position_ms)
    })
    .map_err(|error| error.user_message())
}

fn value_as_string(value: &AnyObject) -> Option<String> {
    if let Some(value) = value.downcast_ref::<NSString>() {
        return Some(value.to_string());
    }
    if let Some(value) = value.downcast_ref::<NSNumber>() {
        return Some(value.stringValue().to_string());
    }
    value
        .downcast_ref::<NSAppleEventDescriptor>()
        .and_then(|value| value.stringValue())
        .map(|value| value.to_string())
}

fn value_as_number(value: &AnyObject) -> Option<f64> {
    if let Some(value) = value.downcast_ref::<NSNumber>() {
        return Some(value.doubleValue());
    }
    value
        .downcast_ref::<NSAppleEventDescriptor>()
        .map(|value| value.doubleValue())
}

fn is_playing(state: Option<u32>) -> bool {
    state == Some(PLAYING)
}

fn is_stopped(state: Option<u32>) -> bool {
    state == Some(STOPPED)
}

fn scaled_duration_ms(seconds: Option<f64>, scale: u64) -> Option<u64> {
    seconds
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| (value * scale as f64).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_apple_event_error_codes() {
        assert_eq!(
            AutomationError {
                code: -1743,
                message: String::new()
            }
            .playback_code(),
            PlaybackErrorCode::AutomationDenied
        );
        assert_eq!(
            AutomationError {
                code: -1712,
                message: String::new()
            }
            .playback_code(),
            PlaybackErrorCode::ResponseTimeout
        );
        assert_eq!(
            AutomationError::unavailable("closed").playback_code(),
            PlaybackErrorCode::Unavailable
        );
    }

    #[test]
    fn maps_player_state_and_duration() {
        assert!(is_playing(Some(PLAYING)));
        assert!(!is_playing(Some(STOPPED)));
        assert!(is_stopped(Some(STOPPED)));
        assert!(!is_stopped(None));
        assert_eq!(scaled_duration_ms(Some(123.456), 1000), Some(123_456));
        assert_eq!(scaled_duration_ms(Some(123_456.0), 1), Some(123_456));
        assert_eq!(scaled_duration_ms(Some(0.0), 1000), None);
        assert_eq!(scaled_duration_ms(Some(f64::NAN), 1000), None);
    }
}
