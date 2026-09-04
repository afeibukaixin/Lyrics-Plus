use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, ProtocolObject};
use objc2::MainThreadMarker;
use objc2::{define_class, msg_send, sel, AnyThread, DefinedClass};
use objc2_app_kit::{
    NSApplicationDidChangeScreenParametersNotification, NSScreen,
    NSWindowDidChangeScreenNotification,
};
use objc2_core_foundation::CFRetained;
use objc2_core_graphics::CGDirectDisplayID;
use objc2_core_video::{kCVReturnSuccess, CVDisplayLink, CVOptionFlags, CVReturn, CVTimeStamp};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSNumber, NSObjectProtocol, NSOperationQueue, NSRunLoop,
    NSRunLoopCommonModes, NSString,
};
use objc2_quartz_core::CADisplayLink;
use tauri::Manager;

use crate::{AppState, TrayMenuState};

type FrameCallback = fn(&tauri::AppHandle);

thread_local! {
    static DISPLAY_DRIVER: RefCell<Option<DisplayDriver>> = const { RefCell::new(None) };
    static DISPLAY_OBSERVERS: RefCell<Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>> = const { RefCell::new(Vec::new()) };
}

static FALLBACK_LOOP_STARTED: AtomicBool = AtomicBool::new(false);
static DISPLAY_DRIVER_READY: AtomicBool = AtomicBool::new(false);

struct DisplayLinkTargetIvars {
    app: tauri::AppHandle,
    on_frame: FrameCallback,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = DisplayLinkTargetIvars]
    struct DisplayLinkTarget;

    unsafe impl NSObjectProtocol for DisplayLinkTarget {}

    impl DisplayLinkTarget {
        #[unsafe(method(displayLinkTick:))]
        fn display_link_tick(&self, link: &CADisplayLink) {
            if should_tick(&self.ivars().app) {
                (self.ivars().on_frame)(&self.ivars().app);
            } else {
                link.setPaused(true);
            }
        }
    }
);

impl DisplayLinkTarget {
    fn new(app: tauri::AppHandle, on_frame: FrameCallback) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DisplayLinkTargetIvars { app, on_frame });
        unsafe { msg_send![super(this), init] }
    }
}

struct CadDisplayLinkState {
    _target: Retained<DisplayLinkTarget>,
    link: Retained<CADisplayLink>,
    display_id: Option<CGDirectDisplayID>,
}

struct CvDisplayLinkContext {
    app: tauri::AppHandle,
    pending: Arc<AtomicBool>,
    on_frame: FrameCallback,
}

struct CvDisplayLinkState {
    link: CFRetained<CVDisplayLink>,
    _context: Box<CvDisplayLinkContext>,
    display_id: Option<CGDirectDisplayID>,
    running: bool,
}

enum DisplayDriver {
    Cad(CadDisplayLinkState),
    Cv(CvDisplayLinkState),
}

fn menu_bar_display_id(app: &tauri::AppHandle) -> Option<CGDirectDisplayID> {
    let tray_state = app.try_state::<TrayMenuState>()?;
    let display_id = Arc::new(Mutex::new(None));
    let display_id_for_main = display_id.clone();
    let _ = tray_state.lyrics_icon.with_inner_tray_icon(move |inner| {
        let Some(status_item) = inner.ns_status_item() else {
            return;
        };
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let Some(button) = status_item.button(mtm) else {
            return;
        };
        let Some(screen) = button.window().and_then(|window| window.screen()) else {
            return;
        };
        if let Ok(mut target) = display_id_for_main.lock() {
            *target = screen_display_id(&screen);
        }
    });
    display_id.lock().ok().and_then(|target| *target)
}

fn screen_display_id(screen: &NSScreen) -> Option<CGDirectDisplayID> {
    let description = screen.deviceDescription();
    let screen_number_key = NSString::from_str("NSScreenNumber");
    description
        .objectForKey(&screen_number_key)
        .and_then(|value| {
            value
                .downcast_ref::<NSNumber>()
                .map(NSNumber::unsignedIntValue)
        })
}

fn screen_for_display_id(
    display_id: Option<CGDirectDisplayID>,
    mtm: MainThreadMarker,
) -> Option<Retained<NSScreen>> {
    let display_id = display_id?;
    let screens = NSScreen::screens(mtm);
    for index in 0..screens.count() {
        let screen = screens.objectAtIndex(index);
        if screen_display_id(&screen) == Some(display_id) {
            return Some(screen);
        }
    }
    None
}

fn should_tick(app: &tauri::AppHandle) -> bool {
    let Some(state) = app.try_state::<AppState>() else {
        return false;
    };
    let config = state.config.snapshot();
    if !config.lyrics.displays.status_bar.enabled {
        return false;
    }
    let is_playing = state
        .last_snapshot
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .is_playing;
    is_playing
}

impl Drop for CadDisplayLinkState {
    fn drop(&mut self) {
        self.link.invalidate();
    }
}

impl Drop for CvDisplayLinkState {
    #[allow(deprecated)]
    fn drop(&mut self) {
        if self.running {
            let _ = self.link.stop();
            self.running = false;
        }
    }
}

unsafe extern "C-unwind" fn cv_display_link_callback(
    _display_link: NonNull<CVDisplayLink>,
    _in_now: NonNull<CVTimeStamp>,
    _in_output_time: NonNull<CVTimeStamp>,
    _flags_in: CVOptionFlags,
    _flags_out: NonNull<CVOptionFlags>,
    user_info: *mut c_void,
) -> CVReturn {
    let Some(context) = user_info.cast::<CvDisplayLinkContext>().as_ref() else {
        return kCVReturnSuccess;
    };
    if !should_tick(&context.app) || context.pending.swap(true, Ordering::AcqRel) {
        return kCVReturnSuccess;
    }

    let app = context.app.clone();
    let app_for_main = app.clone();
    let pending = context.pending.clone();
    let on_frame = context.on_frame;
    if app
        .run_on_main_thread(move || {
            pending.store(false, Ordering::Release);
            if should_tick(&app_for_main) {
                (on_frame)(&app_for_main);
            }
        })
        .is_err()
    {
        context.pending.store(false, Ordering::Release);
    }
    kCVReturnSuccess
}

fn create_cad_display_link(
    app: &tauri::AppHandle,
    screen: Retained<NSScreen>,
    display_id: Option<CGDirectDisplayID>,
    on_frame: FrameCallback,
) -> DisplayDriver {
    let target = DisplayLinkTarget::new(app.clone(), on_frame);
    let link =
        unsafe { screen.displayLinkWithTarget_selector(target.as_ref(), sel!(displayLinkTick:)) };
    let run_loop = NSRunLoop::mainRunLoop();
    unsafe {
        link.addToRunLoop_forMode(&run_loop, NSRunLoopCommonModes);
    }
    link.setPaused(!should_tick(app));
    DisplayDriver::Cad(CadDisplayLinkState {
        _target: target,
        link,
        display_id,
    })
}

#[allow(deprecated)]
fn create_cv_display_link(
    app: &tauri::AppHandle,
    display_id: Option<CGDirectDisplayID>,
    on_frame: FrameCallback,
) -> Option<DisplayDriver> {
    let mut raw_link = std::ptr::null_mut::<CVDisplayLink>();
    let output = NonNull::from(&mut raw_link);
    let status = unsafe {
        match display_id {
            Some(display_id) => CVDisplayLink::create_with_cg_display(display_id, output),
            None => CVDisplayLink::create_with_active_cg_displays(output),
        }
    };
    if status != kCVReturnSuccess {
        return None;
    }
    let raw_link = NonNull::new(raw_link)?;
    let link = unsafe { CFRetained::from_raw(raw_link) };
    let pending = Arc::new(AtomicBool::new(false));
    let context = Box::new(CvDisplayLinkContext {
        app: app.clone(),
        pending,
        on_frame,
    });
    let user_info = (&*context as *const CvDisplayLinkContext).cast_mut().cast();
    let status = unsafe { link.set_output_callback(Some(cv_display_link_callback), user_info) };
    if status != kCVReturnSuccess {
        return None;
    }
    let mut state = CvDisplayLinkState {
        link,
        _context: context,
        display_id,
        running: false,
    };
    if should_tick(app) && state.link.start() == kCVReturnSuccess {
        state.running = true;
    }
    Some(DisplayDriver::Cv(state))
}

fn install_display_observers(app: &tauri::AppHandle, on_frame: FrameCallback) {
    DISPLAY_OBSERVERS.with(|slot| {
        if !slot.borrow().is_empty() {
            return;
        }
        let center = NSNotificationCenter::defaultCenter();
        let queue = NSOperationQueue::mainQueue();
        let app_for_parameters = app.clone();
        let parameters_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            ensure_display_driver(&app_for_parameters, on_frame);
            update_display_driver_activity(&app_for_parameters);
        });
        let app_for_window = app.clone();
        let window_block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
            ensure_display_driver(&app_for_window, on_frame);
            update_display_driver_activity(&app_for_window);
        });
        let parameters_observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSApplicationDidChangeScreenParametersNotification),
                None,
                Some(&queue),
                &parameters_block,
            )
        };
        let window_observer = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWindowDidChangeScreenNotification),
                None,
                Some(&queue),
                &window_block,
            )
        };
        let mut observers = slot.borrow_mut();
        observers.push(parameters_observer);
        observers.push(window_observer);
    });
}

fn ensure_display_driver(app: &tauri::AppHandle, on_frame: FrameCallback) {
    let display_id = menu_bar_display_id(app);
    let use_cad = objc2::available!(macos = 14.0, ..);
    let needs_rebind = DISPLAY_DRIVER.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|driver| match driver {
                DisplayDriver::Cad(state) => !use_cad || state.display_id != display_id,
                DisplayDriver::Cv(state) => use_cad || state.display_id != display_id,
            })
            .unwrap_or(true)
    });
    if !needs_rebind {
        return;
    }

    DISPLAY_DRIVER.with(|slot| {
        slot.borrow_mut().take();
    });
    DISPLAY_DRIVER_READY.store(false, Ordering::Release);
    if let Some(state) = app.try_state::<AppState>() {
        state.status_bar_wake.notify_one();
    }
    let driver = if use_cad {
        MainThreadMarker::new()
            .and_then(|mtm| screen_for_display_id(display_id, mtm))
            .map(|screen| create_cad_display_link(app, screen, display_id, on_frame))
    } else {
        create_cv_display_link(app, display_id, on_frame)
    };
    if let Some(driver) = driver {
        DISPLAY_DRIVER.with(|slot| {
            *slot.borrow_mut() = Some(driver);
        });
        DISPLAY_DRIVER_READY.store(true, Ordering::Release);
    }
}

#[allow(deprecated)]
pub(super) fn update_display_driver_activity(app: &tauri::AppHandle) {
    let active = should_tick(app);
    DISPLAY_DRIVER.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(driver) = slot.as_mut() else {
            return;
        };
        match driver {
            DisplayDriver::Cad(state) => {
                if state.link.isPaused() == active {
                    state.link.setPaused(!active);
                }
            }
            DisplayDriver::Cv(state) => {
                if active && !state.running {
                    state.running = state.link.start() == kCVReturnSuccess;
                } else if !active && state.running {
                    let _ = state.link.stop();
                    state.running = false;
                }
            }
        }
    });
}

fn start_display_driver(app: &tauri::AppHandle, on_frame: FrameCallback) -> bool {
    install_display_observers(app, on_frame);
    ensure_display_driver(app, on_frame);
    update_display_driver_activity(app);
    DISPLAY_DRIVER.with(|slot| slot.borrow().is_some())
}

pub(super) fn start_driver(app: tauri::AppHandle, on_frame: FrameCallback) {
    if MainThreadMarker::new().is_some() {
        if start_display_driver(&app, on_frame) {
            return;
        }
        start_fallback_loop(app, on_frame);
        return;
    }

    let handle = app.clone();
    if app
        .run_on_main_thread(move || {
            if start_display_driver(&handle, on_frame) {
                return;
            }
            start_fallback_loop(handle, on_frame);
        })
        .is_err()
    {
        start_fallback_loop(app, on_frame);
    }
}

fn start_fallback_loop(app: tauri::AppHandle, on_frame: FrameCallback) {
    if FALLBACK_LOOP_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            if DISPLAY_DRIVER_READY.load(Ordering::Acquire) {
                if let Some(state) = app.try_state::<AppState>() {
                    let wake = state.status_bar_wake.clone();
                    wake.notified().await;
                } else {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                continue;
            }
            let Some(state) = app.try_state::<AppState>() else {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            };
            let enabled = state.config.snapshot().lyrics.displays.status_bar.enabled;
            let is_playing = state
                .last_snapshot
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .is_playing;
            let wake = state.status_bar_wake.clone();
            if !enabled {
                wake.notified().await;
                continue;
            }
            let handle = app.clone();
            if let Err(error) = app.run_on_main_thread(move || (on_frame)(&handle)) {
                log::debug!("Failed to schedule fallback menu bar frame: {error}");
            }
            if is_playing {
                tokio::time::sleep(Duration::from_millis(16)).await;
            } else {
                wake.notified().await;
            }
        }
    });
}

pub(super) fn wake_driver(app: &tauri::AppHandle, on_frame: FrameCallback) {
    if let Some(state) = app.try_state::<AppState>() {
        state.status_bar_wake.notify_one();
    }
    let handle = app.clone();
    if MainThreadMarker::new().is_some() {
        ensure_display_driver(&handle, on_frame);
        update_display_driver_activity(&handle);
    } else if let Err(error) = app.run_on_main_thread(move || {
        ensure_display_driver(&handle, on_frame);
        update_display_driver_activity(&handle);
    }) {
        log::debug!("Failed to wake menu bar display link: {error}");
    }
}
