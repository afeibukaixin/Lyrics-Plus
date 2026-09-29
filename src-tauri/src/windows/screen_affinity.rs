//! AppKit 保存窗口外框；这里仅补充物理屏幕归属和屏幕内的贴边距离。

use std::cell::RefCell;
use std::collections::HashSet;
use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplicationDidChangeScreenParametersNotification, NSEvent, NSScreen, NSWindow,
};
use objc2_core_foundation::{CFRetained, CFUUID};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSNumber, NSObjectProtocol, NSOperationQueue, NSRect,
    NSString,
};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::AppState;

const OVERLAY_KEY: &str = "lyrics-screen-affinity.overlay";
const LIST_KEY: &str = "lyrics-screen-affinity.list";

#[link(name = "ColorSync", kind = "framework")]
unsafe extern "C" {
    fn CGDisplayCreateUUIDFromDisplayID(display_id: u32) -> *mut CFUUID;
}

thread_local! {
    static SCREEN_OBSERVER: RefCell<Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>> = const { RefCell::new(None) };
    static TEMPORARY_SCREEN_WINDOWS: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Affinity {
    display_uuid: String,
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyPosition {
    relative_x: Option<f64>,
    relative_y: Option<f64>,
}

fn preference_key(label: &str) -> Option<&'static str> {
    match label {
        "lyrics-overlay" => Some(OVERLAY_KEY),
        "lyrics-list" => Some(LIST_KEY),
        _ => None,
    }
}

fn display_id(screen: &NSScreen) -> Option<u32> {
    let description = screen.deviceDescription();
    let number = description.objectForKey(&NSString::from_str("NSScreenNumber"))?;
    number
        .downcast_ref::<NSNumber>()
        .map(NSNumber::unsignedIntValue)
}

fn display_uuid(screen: &NSScreen) -> Option<String> {
    let raw = NonNull::new(unsafe { CGDisplayCreateUUIDFromDisplayID(display_id(screen)?) })?;
    let uuid = unsafe { CFRetained::from_raw(raw) };
    let bytes: [u8; 16] = uuid.uuid_bytes().into();
    Some(format!(
        "{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    ))
}

fn screens(mtm: MainThreadMarker) -> Vec<Retained<NSScreen>> {
    let all = NSScreen::screens(mtm);
    (0..all.count())
        .map(|index| all.objectAtIndex(index))
        .collect()
}

fn unique_screen_by_uuid(mtm: MainThreadMarker, uuid: &str) -> Option<Retained<NSScreen>> {
    let matches: Vec<_> = screens(mtm)
        .into_iter()
        .filter(|screen| display_uuid(screen).as_deref() == Some(uuid))
        .collect();
    (matches.len() == 1)
        .then(|| matches.into_iter().next())
        .flatten()
}

fn unique_legacy_screen(mtm: MainThreadMarker, name: &str) -> Option<Retained<NSScreen>> {
    let matches: Vec<_> = screens(mtm)
        .into_iter()
        .filter(|screen| {
            let numbered = display_id(screen).map(|id| format!("Monitor #{id}"));
            numbered.as_deref() == Some(name) || screen.localizedName().to_string() == name
        })
        .collect();
    (matches.len() == 1)
        .then(|| matches.into_iter().next())
        .flatten()
}

fn capture(frame: NSRect, screen: &NSScreen) -> Option<Affinity> {
    let bounds = screen.frame();
    Some(Affinity {
        display_uuid: display_uuid(screen)?,
        left: frame.origin.x - bounds.origin.x,
        right: bounds.origin.x + bounds.size.width - frame.origin.x - frame.size.width,
        top: bounds.origin.y + bounds.size.height - frame.origin.y - frame.size.height,
        bottom: frame.origin.y - bounds.origin.y,
    })
}

fn projected_frame(mut frame: NSRect, screen: &NSScreen, saved: &Affinity) -> NSRect {
    let bounds = screen.frame();
    let x = if saved.left <= saved.right {
        bounds.origin.x + saved.left
    } else {
        bounds.origin.x + bounds.size.width - frame.size.width - saved.right
    };
    let y = if saved.bottom <= saved.top {
        bounds.origin.y + saved.bottom
    } else {
        bounds.origin.y + bounds.size.height - frame.size.height - saved.top
    };
    frame.origin.x = x.clamp(
        bounds.origin.x,
        (bounds.origin.x + bounds.size.width - frame.size.width).max(bounds.origin.x),
    );
    frame.origin.y = y.clamp(
        bounds.origin.y,
        (bounds.origin.y + bounds.size.height - frame.size.height).max(bounds.origin.y),
    );
    frame
}

fn write_affinity(app: &tauri::AppHandle, label: &str, affinity: &Affinity) {
    let Some(key) = preference_key(label) else {
        return;
    };
    let Ok(raw) = serde_json::to_string(affinity) else {
        return;
    };
    if let Err(error) = app.state::<AppState>().storage.set_preference(key, &raw) {
        log::warn!("保存歌词窗口目标显示器失败：{error}");
    }
}

fn read_affinity(app: &tauri::AppHandle, label: &str) -> Option<Affinity> {
    app.state::<AppState>()
        .storage
        .get_preference(preference_key(label)?)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
}

fn legacy_affinity(
    app: &tauri::AppHandle,
    label: &str,
    frame: NSRect,
    mtm: MainThreadMarker,
) -> Option<Affinity> {
    let prefix = if label == "lyrics-overlay" {
        "overlay"
    } else {
        "lyrics-list"
    };
    let last_key = if label == "lyrics-overlay" {
        "overlay.last_monitor"
    } else {
        "lyrics-list.last-monitor"
    };
    let storage = &app.state::<AppState>().storage;
    let name = storage.get_preference(last_key).ok().flatten()?;
    let screen = unique_legacy_screen(mtm, &name)?;
    let raw = storage
        .get_preference(&format!("{prefix}.position.{name}"))
        .ok()
        .flatten()?;
    let old: LegacyPosition = serde_json::from_str(&raw).ok()?;
    let bounds = screen.visibleFrame();
    let relative_x = old.relative_x?.clamp(0.0, 1.0);
    let relative_y = old.relative_y?.clamp(0.0, 1.0);
    if !relative_x.is_finite() || !relative_y.is_finite() {
        return None;
    }
    let mut legacy_frame = frame;
    legacy_frame.origin.x =
        bounds.origin.x + relative_x * (bounds.size.width - frame.size.width).max(0.0);
    legacy_frame.origin.y =
        bounds.origin.y + (1.0 - relative_y) * (bounds.size.height - frame.size.height).max(0.0);
    capture(legacy_frame, &screen)
}

fn ns_window(window: &tauri::WebviewWindow) -> Option<&NSWindow> {
    let pointer = window.ns_window().ok()?;
    Some(unsafe { &*pointer.cast::<NSWindow>() })
}

fn frame_is_visible(frame: NSRect, screens: &[Retained<NSScreen>]) -> bool {
    screens.iter().any(|screen| {
        let bounds = screen.frame();
        frame.origin.x < bounds.origin.x + bounds.size.width
            && frame.origin.x + frame.size.width > bounds.origin.x
            && frame.origin.y < bounds.origin.y + bounds.size.height
            && frame.origin.y + frame.size.height > bounds.origin.y
    })
}

fn reconcile_on_main(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    initial: bool,
    native_restored: bool,
) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    if window.label() == "lyrics-overlay" && crate::overlay_drag_active(app) {
        return;
    }
    if !initial && NSEvent::pressedMouseButtons() & 1 != 0 {
        return;
    }
    let Some(native) = ns_window(window) else {
        return;
    };
    let frame = native.frame();
    let all_screens = screens(mtm);
    let saved = read_affinity(app, window.label()).or_else(|| {
        let migrated = legacy_affinity(app, window.label(), frame, mtm);
        if let Some(value) = &migrated {
            write_affinity(app, window.label(), value);
        }
        migrated
    });
    if let Some(saved) = saved {
        if let Some(target) = unique_screen_by_uuid(mtm, &saved.display_uuid) {
            let returned =
                TEMPORARY_SCREEN_WINDOWS.with(|labels| labels.borrow_mut().remove(window.label()));
            // 同屏时信任 AppKit 的原始外框；只纠正系统恢复到另一块屏的情况。
            if returned
                || native
                    .screen()
                    .and_then(|screen| display_uuid(&screen))
                    .as_deref()
                    != Some(&saved.display_uuid)
            {
                native.setFrame_display(projected_frame(frame, &target, &saved), false);
                if !initial && window.label() == "lyrics-overlay" {
                    crate::overlay_placement::restore_overlay_content_position(app, window);
                }
            }
        } else {
            TEMPORARY_SCREEN_WINDOWS.with(|labels| {
                labels.borrow_mut().insert(window.label().to_string());
            });
            if !frame_is_visible(frame, &all_screens) {
                if let Some(primary) = NSScreen::mainScreen(mtm) {
                    native.setFrame_display(projected_frame(frame, &primary, &saved), false);
                }
            }
        }
        return;
    }
    // 没有可靠的旧目标时保留 AppKit 结果；真正首次使用才落在当前主屏。
    let has_legacy = {
        let key = if window.label() == "lyrics-overlay" {
            "overlay.last_monitor"
        } else {
            "lyrics-list.last-monitor"
        };
        app.state::<AppState>()
            .storage
            .get_preference(key)
            .ok()
            .flatten()
            .is_some()
    };
    if initial && !native_restored && !has_legacy {
        if let Some(primary) = NSScreen::mainScreen(mtm) {
            let bounds = primary.frame();
            let mut centered = frame;
            centered.origin.x = bounds.origin.x + (bounds.size.width - frame.size.width) / 2.0;
            centered.origin.y = bounds.origin.y + (bounds.size.height - frame.size.height) / 2.0;
            native.setFrame_display(centered, false);
        }
    }
    if let Some(screen) = native.screen() {
        if let Some(value) = capture(native.frame(), &screen) {
            write_affinity(app, window.label(), &value);
        }
    }
}

fn on_main<R: Send + 'static>(
    window: &tauri::WebviewWindow,
    operation: impl FnOnce() -> R + Send + 'static,
) -> Option<R> {
    if MainThreadMarker::new().is_some() {
        return Some(operation());
    }
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    window
        .run_on_main_thread(move || {
            let _ = sender.send(operation());
        })
        .ok()?;
    receiver.recv().ok()
}

pub(crate) fn restore_screen_affinity(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    native_restored: bool,
) {
    let app = app.clone();
    let window_copy = window.clone();
    let _ = on_main(window, move || {
        reconcile_on_main(&app, &window_copy, true, native_restored)
    });
}

pub(crate) fn save_user_placement(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let app = app.clone();
    let window = window.clone();
    let _ = on_main(&window, {
        let app = app.clone();
        let window = window.clone();
        move || {
            let Some(native) = ns_window(&window) else {
                return;
            };
            // 列表歌词缩放可能同时产生 Moved 事件，不把这种尺寸操作记为跨屏拖动。
            if window.label() == "lyrics-list" && native.inLiveResize() {
                return;
            }
            let Some(screen) = native.screen() else {
                return;
            };
            if let Some(value) = capture(native.frame(), &screen) {
                write_affinity(&app, window.label(), &value);
                TEMPORARY_SCREEN_WINDOWS.with(|labels| {
                    labels.borrow_mut().remove(window.label());
                });
            }
        }
    });
}

pub(crate) fn save_geometry_on_target(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let app = app.clone();
    let window = window.clone();
    let _ = on_main(&window, {
        let app = app.clone();
        let window = window.clone();
        move || {
            let Some(saved) = read_affinity(&app, window.label()) else {
                return;
            };
            let Some(native) = ns_window(&window) else {
                return;
            };
            let Some(screen) = native.screen() else {
                return;
            };
            if display_uuid(&screen).as_deref() == Some(&saved.display_uuid) {
                if let Some(value) = capture(native.frame(), &screen) {
                    write_affinity(&app, window.label(), &value);
                }
            }
        }
    });
}

pub(crate) fn reset_screen_affinity(app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    let Some(key) = preference_key(window.label()) else {
        return;
    };
    let _ = app.state::<AppState>().storage.remove_preference(key);
    save_user_placement(app, window);
}

pub(crate) fn install_screen_observer(app: &tauri::AppHandle) {
    let app = app.clone();
    let _ = app.run_on_main_thread({
        let app = app.clone();
        move || {
            SCREEN_OBSERVER.with(|slot| {
                if slot.borrow().is_some() {
                    return;
                }
                let center = NSNotificationCenter::defaultCenter();
                let queue = NSOperationQueue::mainQueue();
                let block = RcBlock::new(move |_note: NonNull<NSNotification>| {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                        while crate::primary_mouse_button_pressed()
                            || crate::overlay_drag_active(&app)
                        {
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        }
                        let queued = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            for label in ["lyrics-overlay", "lyrics-list"] {
                                if let Some(window) = queued.get_webview_window(label) {
                                    reconcile_on_main(&queued, &window, false, true);
                                }
                            }
                        });
                    });
                });
                let observer = unsafe {
                    center.addObserverForName_object_queue_usingBlock(
                        Some(NSApplicationDidChangeScreenParametersNotification),
                        None,
                        Some(&queue),
                        &block,
                    )
                };
                *slot.borrow_mut() = Some(observer);
            });
        }
    });
}
