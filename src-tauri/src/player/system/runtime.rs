//! 系统事件、主动校准共用状态入口；接收线程不做图片解码或应用信息查询。
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Cursor};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Condvar, Mutex, RwLock,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::ImageReader;
use serde_json::Value;
use tokio::sync::Notify;

use super::super::PlaybackAction;
use super::adapter::{payload_to_timed, run_adapter, same_media_info};
use super::metadata::TimedInfo;
use super::presentation::{ControlContext, PlaybackPresentation};

struct Enrichment {
    generation: u64,
    bundle_id: Option<String>,
    artwork: Option<String>,
    artwork_revision: u64,
}

#[derive(Default)]
struct EnrichmentQueue {
    pending: Option<Enrichment>,
    artwork: Option<String>,
    revision: u64,
}

struct RefreshSchedule {
    requested: bool,
    interval: Duration,
}

pub(super) struct AdapterRuntime {
    pub(super) latest: RwLock<Option<TimedInfo>>,
    // 所有版本读取/比较/递增均受 latest 锁保护，包括跳转确认。
    pub(super) state_version: AtomicU64,
    generation: AtomicU64,
    pub(super) playback_changed: Arc<Notify>,
    pub(super) script_path: PathBuf,
    pub(super) framework_path: PathBuf,
    // 定时校准和跳转确认串行发起 get，但从不阻塞事件提交。
    pub(super) query: Mutex<()>,
    pub(super) presentation: Mutex<PlaybackPresentation>,
    presentation_wake: Condvar,
    stopped: AtomicBool,
    child: Mutex<Option<Child>>,
    refresh: Mutex<RefreshSchedule>,
    refresh_wake: Condvar,
    enrichment: Mutex<EnrichmentQueue>,
    enrichment_wake: Condvar,
    _resources: tempfile::TempDir,
}

impl AdapterRuntime {
    pub(super) fn new(resources: tempfile::TempDir) -> Arc<Self> {
        Arc::new(Self {
            script_path: resources.path().join("mediaremote-adapter.pl"),
            framework_path: resources.path().join("MediaRemoteAdapter.framework"),
            latest: RwLock::new(None),
            state_version: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            playback_changed: Arc::new(Notify::new()),
            query: Mutex::new(()),
            presentation: Mutex::new(PlaybackPresentation::default()),
            presentation_wake: Condvar::new(),
            stopped: AtomicBool::new(false),
            child: Mutex::new(None),
            refresh: Mutex::new(RefreshSchedule {
                requested: true,
                interval: Duration::from_millis(750),
            }),
            refresh_wake: Condvar::new(),
            enrichment: Mutex::new(EnrichmentQueue::default()),
            enrichment_wake: Condvar::new(),
            _resources: resources,
        })
    }

    pub(super) fn start(self: &Arc<Self>) -> Result<Vec<JoinHandle<()>>, String> {
        let mut workers = Vec::new();
        for (name, work) in [
            ("system-media-stream", Self::stream_loop as fn(&Self)),
            ("system-media-refresh", Self::refresh_loop as fn(&Self)),
            ("system-media-artwork", Self::enrichment_loop as fn(&Self)),
            (
                "system-media-presentation",
                Self::presentation_loop as fn(&Self),
            ),
        ] {
            let runtime = self.clone();
            match thread::Builder::new()
                .name(name.into())
                .spawn(move || work(&runtime))
            {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    self.stop();
                    for worker in workers {
                        let _ = worker.join();
                    }
                    return Err(format!("无法启动系统媒体工作线程：{error}"));
                }
            }
        }
        Ok(workers)
    }

    pub(super) fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        {
            let mut presentation = self.presentation.lock().unwrap_or_else(|e| e.into_inner());
            presentation.clear();
            self.presentation_wake.notify_all();
        }
        // 与等待者持有同一把锁，避免退出信号落在检查条件与 wait 之间。
        {
            let _schedule = self.refresh.lock().unwrap_or_else(|e| e.into_inner());
            self.refresh_wake.notify_all();
        }
        {
            let _queue = self.enrichment.lock().unwrap_or_else(|e| e.into_inner());
            self.enrichment_wake.notify_all();
        }
        if let Some(child) = self
            .child
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
        {
            let _ = child.kill();
        }
    }

    pub(super) fn request_refresh(&self) {
        let mut schedule = self.refresh.lock().unwrap_or_else(|e| e.into_inner());
        schedule.requested = true;
        self.refresh_wake.notify_one();
    }

    pub(super) fn set_refresh_interval(&self, interval: Duration) {
        let mut schedule = self.refresh.lock().unwrap_or_else(|e| e.into_inner());
        if schedule.interval != interval {
            schedule.interval = interval;
            self.refresh_wake.notify_one();
        }
    }

    /// expected_version 只用于主动查询；事件直接提交，查询不可覆盖期间到达的事件。
    pub(super) fn commit(
        &self,
        payload: &Value,
        expected_version: Option<u64>,
    ) -> Result<bool, String> {
        let received_at = Instant::now();
        let mut next = payload_to_timed(payload)?;
        let mut latest = self.latest.write().unwrap_or_else(|e| e.into_inner());
        if self.stopped.load(Ordering::SeqCst) {
            return Ok(false);
        }
        if expected_version
            .is_some_and(|version| self.state_version.load(Ordering::SeqCst) != version)
        {
            drop(latest);
            self.request_refresh();
            log::debug!("系统媒体校准丢弃：查询期间已有更新事件");
            return Ok(false);
        }
        let same_track = latest
            .as_ref()
            .zip(next.as_ref())
            .is_some_and(|(previous, next)| same_media_info(&previous.info, &next.info));
        let mut queue = self.enrichment.lock().unwrap_or_else(|e| e.into_inner());
        if !same_track {
            self.generation.fetch_add(1, Ordering::SeqCst);
            queue.pending = None;
            queue.artwork = None;
            queue.revision = queue.revision.wrapping_add(1);
        }
        if let Some(next) = next.as_mut() {
            if same_track {
                if let Some(previous) = latest.as_ref() {
                    next.info.album_cover = previous.info.album_cover.clone();
                    next.info.bundle_name = previous.info.bundle_name.clone();
                }
            }
            let artwork = payload.get("artworkData").and_then(Value::as_str);
            let artwork_changed =
                artwork.is_some_and(|value| queue.artwork.as_deref() != Some(value));
            if artwork_changed {
                queue.artwork = artwork.map(str::to_owned);
                queue.revision = queue.revision.wrapping_add(1);
            }
            if artwork_changed || !same_track {
                // 只保留最新补全任务；无封面的校准结果不丢弃尚待解码的同曲目封面。
                queue.pending = Some(Enrichment {
                    generation: self.generation.load(Ordering::SeqCst),
                    bundle_id: next.info.bundle_id.clone(),
                    artwork: queue.artwork.clone(),
                    artwork_revision: queue.revision,
                });
                self.enrichment_wake.notify_one();
            }
        }
        let version = self.state_version.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut presentation = self.presentation.lock().unwrap_or_else(|e| e.into_inner());
            presentation.update(next.as_ref());
            self.presentation_wake.notify_one();
        }
        *latest = next;
        drop(queue);
        drop(latest);
        self.playback_changed.notify_one();
        log::debug!(
            "系统媒体提交 source={} version={} elapsed_us={}",
            if expected_version.is_some() {
                "get"
            } else {
                "stream"
            },
            version,
            received_at.elapsed().as_micros()
        );
        Ok(true)
    }

    pub(super) fn prepare_control(&self, action: PlaybackAction) -> Result<ControlContext, String> {
        let _latest = self.latest.read().unwrap_or_else(|e| e.into_inner());
        self.presentation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .prepare_control(action)
    }

    pub(super) fn acknowledge_control(&self, context: &ControlContext) {
        let mut presentation = self.presentation.lock().unwrap_or_else(|e| e.into_inner());
        let changed = presentation.acknowledge_control(context);
        self.presentation_wake.notify_one();
        drop(presentation);
        if changed {
            self.playback_changed.notify_one();
        }
    }

    fn presentation_loop(&self) {
        let mut presentation = self.presentation.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if self.stopped.load(Ordering::SeqCst) {
                break;
            }
            if let Some((revision, deadline)) = presentation.pending_pause() {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    if presentation.confirm_pause(revision, deadline) {
                        log::debug!("系统媒体暂停展示确认 revision={revision}");
                        self.playback_changed.notify_one();
                    }
                } else {
                    presentation = self
                        .presentation_wake
                        .wait_timeout(presentation, remaining)
                        .unwrap_or_else(|e| e.into_inner())
                        .0;
                }
            } else {
                presentation = self
                    .presentation_wake
                    .wait(presentation)
                    .unwrap_or_else(|e| e.into_inner());
            }
        }
    }

    fn stream_loop(&self) {
        let mut failures = 0_usize;
        while !self.stopped.load(Ordering::SeqCst) {
            let started_at = Instant::now();
            let stream = {
                let mut slot = self.child.lock().unwrap_or_else(|e| e.into_inner());
                if self.stopped.load(Ordering::SeqCst) {
                    break;
                }
                match Command::new("/usr/bin/perl")
                    .arg(&self.script_path)
                    .arg(&self.framework_path)
                    .args(["stream", "--no-diff", "--micros", "--debounce=0"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(mut child) => {
                        let stdout = child.stdout.take();
                        *slot = Some(child);
                        stdout
                    }
                    Err(error) => {
                        log::warn!("系统媒体监听启动失败：{error}");
                        None
                    }
                }
            };
            if let Some(stdout) = stream {
                for line in BufReader::new(stdout).lines() {
                    if self.stopped.load(Ordering::SeqCst) {
                        break;
                    }
                    let line = match line {
                        Ok(line) => line,
                        Err(error) => {
                            log::warn!("系统媒体监听读取失败：{error}");
                            break;
                        }
                    };
                    let received_at = Instant::now();
                    match serde_json::from_str::<Value>(&line) {
                        Ok(event)
                            if event.get("type").and_then(Value::as_str) == Some("data")
                                && event.get("diff").and_then(Value::as_bool) == Some(false) =>
                        {
                            if let Some(payload) = event.get("payload") {
                                if let Err(error) = self.commit(payload, None) {
                                    log::debug!("系统媒体事件无效：{error}");
                                }
                                log::debug!(
                                    "系统媒体事件接收至提交 elapsed_us={}",
                                    received_at.elapsed().as_micros()
                                );
                            }
                        }
                        _ => log::debug!("系统媒体监听忽略无效事件"),
                    }
                }
            }
            if let Some(mut child) = self.child.lock().unwrap_or_else(|e| e.into_inner()).take() {
                let _ = child.kill();
                let _ = child.wait();
            }
            if self.stopped.load(Ordering::SeqCst) {
                break;
            }
            if started_at.elapsed() >= Duration::from_secs(10) {
                failures = 0;
            }
            let delay = [1, 2, 5][failures.min(2)];
            failures = failures.saturating_add(1);
            log::warn!("系统媒体监听已退出，{delay} 秒后重启");
            self.request_refresh();
            // 分段等待使销毁无需等待完整的重试退避时间。
            for _ in 0..delay * 10 {
                if self.stopped.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    fn refresh_loop(&self) {
        let mut last_refresh = Instant::now();
        let mut error_logged = false;
        loop {
            let mut schedule = self.refresh.lock().unwrap_or_else(|e| e.into_inner());
            while !self.stopped.load(Ordering::SeqCst)
                && !schedule.requested
                && last_refresh.elapsed() < schedule.interval
            {
                let remaining = schedule.interval.saturating_sub(last_refresh.elapsed());
                schedule = self
                    .refresh_wake
                    .wait_timeout(schedule, remaining)
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
            if self.stopped.load(Ordering::SeqCst) {
                break;
            }
            schedule.requested = false;
            drop(schedule);
            let _query = self.query.lock().unwrap_or_else(|e| e.into_inner());
            if self.stopped.load(Ordering::SeqCst) {
                break;
            }
            let (version, needs_artwork) = {
                let latest = self.latest.read().unwrap_or_else(|e| e.into_inner());
                (
                    self.state_version.load(Ordering::SeqCst),
                    latest
                        .as_ref()
                        .is_none_or(|timed| timed.info.album_cover.is_none()),
                )
            };
            let started_at = Instant::now();
            let mut arguments = vec!["get", "--now", "--micros"];
            if !needs_artwork {
                arguments.push("--no-artwork");
            }
            let result = run_adapter(&self.script_path, &self.framework_path, arguments).and_then(
                |output| {
                    // 上游超时可能以成功退出码打印 null 和 stderr，不能当作媒体清空。
                    if !output.status.success() || !output.stderr.is_empty() {
                        return Err(format!(
                            "系统媒体查询失败：{}",
                            String::from_utf8_lossy(&output.stderr).trim()
                        ));
                    }
                    let mut payload: Value = serde_json::from_slice(&output.stdout)
                        .map_err(|error| error.to_string())?;
                    // 进程存活检查也放在后台，快照读取无需访问 NSWorkspace。
                    if payload
                        .get("bundleIdentifier")
                        .and_then(Value::as_str)
                        .is_some_and(|bundle| {
                            !super::super::automation::is_application_running(bundle)
                        })
                    {
                        payload = Value::Null;
                    }
                    self.commit(&payload, Some(version))
                },
            );
            if let Err(error) = result {
                if !error_logged {
                    log::warn!("{error}");
                }
                error_logged = true;
            } else {
                error_logged = false;
            }
            last_refresh = Instant::now();
            log::debug!(
                "系统媒体后台校准 elapsed_ms={}",
                started_at.elapsed().as_millis()
            );
        }
    }

    fn enrichment_loop(&self) {
        let mut names = HashMap::<String, Option<String>>::new();
        loop {
            let task = {
                let mut queue = self.enrichment.lock().unwrap_or_else(|e| e.into_inner());
                while queue.pending.is_none() && !self.stopped.load(Ordering::SeqCst) {
                    queue = self
                        .enrichment_wake
                        .wait(queue)
                        .unwrap_or_else(|e| e.into_inner());
                }
                if self.stopped.load(Ordering::SeqCst) {
                    break;
                }
                queue.pending.take().expect("pending enrichment checked")
            };
            let name = task.bundle_id.as_ref().and_then(|bundle| {
                names
                    .entry(bundle.clone())
                    .or_insert_with(|| media_remote::get_bundle_info(bundle).map(|info| info.name))
                    .clone()
            });
            let artwork = task.artwork.as_ref().and_then(|encoded| {
                let data = BASE64.decode(encoded.replace('\n', "")).ok()?;
                ImageReader::new(Cursor::new(data))
                    .with_guessed_format()
                    .ok()?
                    .decode()
                    .ok()
                    // 缓存显示所需尺寸，快照复制与封面指纹不会再扫描原始大图。
                    .map(|image| image.thumbnail(192, 192))
            });
            let mut latest = self.latest.write().unwrap_or_else(|e| e.into_inner());
            let queue = self.enrichment.lock().unwrap_or_else(|e| e.into_inner());
            if self.stopped.load(Ordering::SeqCst)
                || self.generation.load(Ordering::SeqCst) != task.generation
                || queue.revision != task.artwork_revision
            {
                continue;
            }
            let Some(timed) = latest.as_mut() else {
                continue;
            };
            let mut changed = false;
            if name.is_some() && timed.info.bundle_name != name {
                timed.info.bundle_name = name;
                changed = true;
            }
            if let Some(artwork) = artwork {
                timed.info.album_cover = Some(artwork);
                changed = true;
            }
            drop(queue);
            drop(latest);
            if changed {
                self.playback_changed.notify_one();
            }
        }
    }
}
