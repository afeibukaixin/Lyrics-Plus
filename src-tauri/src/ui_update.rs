use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use futures::StreamExt;
use minisign_verify::{PublicKey, Signature};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::http::{Method, Request, Response};
use tauri::{Manager, WebviewUrl};
use zip::ZipArchive;

use crate::AppState;

/// Core API 版本只在 IPC 契约发生不兼容变化时递增。
pub(crate) const CORE_API_VERSION: u32 = 1;

const UI_PROTOCOL: &str = "lyrics-plus-ui";
const UI_UPDATE_INDEX_URL: &str =
    "https://raw.githubusercontent.com/afeibukaixin/Lyrics-Plus/ui-updates/latest.json";
const UI_UPDATES_DIRECTORY: &str = "ui-updates";
const UI_STATE_FILE: &str = "state.json";
const UI_PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEUwQUMxNjRENDc2NTU2RjQKUldUMFZtVkhUUmFzNEQ4NGFZN1N1TWEzWUw4aTRtazZtVHE2WWlwRDk2bTV2aGZtbyt5VlZnZkkK";
const MAX_INDEX_BYTES: u64 = 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_UNCOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 4096;
const ACTIVATION_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UiUpdateManifest {
    pub schema_version: u32,
    pub app_version: String,
    pub core_api_version: u32,
    pub revision: u64,
    pub ui_version: String,
    #[serde(default)]
    pub release_notes: String,
    #[serde(default)]
    pub published_at: Option<String>,
    pub url: String,
    pub size: u64,
    pub sha256: String,
    pub signature: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiUpdateIndex {
    releases: Vec<UiUpdateManifest>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredBundle {
    manifest: UiUpdateManifest,
    archive_file: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UiStateFile {
    app_version: String,
    active: Option<StoredBundle>,
    pending: Option<StoredBundle>,
    last_result: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UiUpdateStateView {
    pub app_version: String,
    pub core_api_version: u32,
    pub source: String,
    pub active_version: String,
    pub prepared_version: Option<String>,
    pub prepared_release_notes: Option<String>,
    pub pending_version: Option<String>,
    pub last_result: Option<String>,
}

#[derive(Clone)]
struct UiBundle {
    stored: StoredBundle,
    assets: Arc<HashMap<String, Vec<u8>>>,
}

struct Activation {
    candidate_version: String,
    expected_labels: HashSet<String>,
    ready_labels: HashSet<String>,
}

/// 负责界面资源的下载、校验、内存加载和原子切换。
pub(crate) struct UiUpdateManager {
    updates_dir: PathBuf,
    state_path: PathBuf,
    app_version: String,
    client: reqwest::Client,
    runtime: Mutex<RuntimeState>,
}

struct RuntimeState {
    state_file: UiStateFile,
    active_bundle: Option<Arc<UiBundle>>,
    prepared_bundle: Option<Arc<UiBundle>>,
    bundles: HashMap<String, Arc<UiBundle>>,
    activation: Option<Activation>,
}

impl UiUpdateManager {
    pub(crate) fn load(app_dir: &Path) -> Result<Self, String> {
        let updates_dir = app_dir.join(UI_UPDATES_DIRECTORY);
        fs::create_dir_all(&updates_dir)
            .map_err(|error| format!("无法创建界面更新目录：{error}"))?;
        let state_path = updates_dir.join(UI_STATE_FILE);
        let app_version = env!("CARGO_PKG_VERSION").to_owned();
        let mut state_file = read_state_file(&state_path).unwrap_or_default();
        let mut state_changed = false;

        if state_file.app_version != app_version {
            state_file = UiStateFile {
                app_version: app_version.clone(),
                ..UiStateFile::default()
            };
            state_changed = true;
        }

        // 进程在候选资源切换完成前退出时，active 仍然是上一份已确认资源，
        // 清理 pending 即可完成启动回滚。
        if state_file.pending.take().is_some() {
            state_file.last_result = Some("rolledBackAfterRestart".into());
            state_changed = true;
        }

        let mut active_bundle = None;
        let mut bundles = HashMap::new();
        if cfg!(target_os = "macos") {
            if let Some(stored) = state_file.active.clone() {
                match load_bundle_from_file(&updates_dir, stored, &app_version) {
                    Ok(bundle) => {
                        let key = bundle.stored.manifest.ui_version.clone();
                        let bundle = Arc::new(bundle);
                        bundles.insert(key, Arc::clone(&bundle));
                        active_bundle = Some(bundle);
                    }
                    Err(error) => {
                        log::warn!("忽略无效的界面更新缓存：{error}");
                        state_file.active = None;
                        state_file.last_result = Some("invalidCache".into());
                        state_changed = true;
                    }
                }
            }
        }

        if state_changed {
            if let Err(error) = write_state_file(&state_path, &state_file) {
                // 状态记录失败不应阻断应用启动；内存中的安全回退仍然有效，
                // 下次启动会再次检查并修复这份状态。
                log::warn!("保存界面更新状态失败，继续使用当前安全状态：{error}");
            }
        }

        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|error| format!("无法初始化界面更新网络客户端：{error}"))?;

        Ok(Self {
            updates_dir,
            state_path,
            app_version,
            client,
            runtime: Mutex::new(RuntimeState {
                state_file,
                active_bundle,
                prepared_bundle: None,
                bundles,
                activation: None,
            }),
        })
    }

    pub(crate) fn state_view(&self) -> UiUpdateStateView {
        let runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state_view_from_runtime(&self.app_version, &runtime)
    }

    pub(crate) async fn check_and_prepare(&self) -> Result<UiUpdateStateView, String> {
        // 首版只对 macOS 开放热更新；其他平台继续使用应用内置资源。
        if !cfg!(target_os = "macos") {
            return Ok(self.state_view());
        }
        let response = self
            .client
            .get(UI_UPDATE_INDEX_URL)
            .header("Cache-Control", "no-cache")
            .send()
            .await
            .map_err(|error| format!("读取界面更新清单失败：{error}"))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(self.state_view());
        }
        let response = response
            .error_for_status()
            .map_err(|error| format!("界面更新清单返回错误：{error}"))?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_INDEX_BYTES)
        {
            return Err("界面更新清单超出允许范围".into());
        }
        let mut index_stream = response.bytes_stream();
        let mut index_bytes = Vec::new();
        while let Some(chunk) = index_stream.next().await {
            let chunk = chunk.map_err(|error| format!("读取界面更新清单失败：{error}"))?;
            let next_len = index_bytes
                .len()
                .checked_add(chunk.len())
                .ok_or_else(|| "界面更新清单大小溢出".to_string())?;
            if next_len as u64 > MAX_INDEX_BYTES {
                return Err("界面更新清单超出允许范围".into());
            }
            index_bytes.extend_from_slice(&chunk);
        }
        let index = serde_json::from_slice::<UiUpdateIndex>(&index_bytes)
            .map_err(|error| format!("解析界面更新清单失败：{error}"))?;

        let manifest = index
            .releases
            .into_iter()
            .filter(|candidate| {
                candidate.schema_version == 1
                    && candidate.app_version == self.app_version
                    && candidate.core_api_version == CORE_API_VERSION
                    && candidate.revision > 0
            })
            .max_by_key(|candidate| candidate.revision);
        let Some(manifest) = manifest else {
            return Ok(self.state_view());
        };

        let current_revision = {
            let runtime = self
                .runtime
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if runtime.activation.is_some() {
                return Ok(state_view_from_runtime(&self.app_version, &runtime));
            }
            runtime
                .active_bundle
                .as_ref()
                .map(|bundle| bundle.stored.manifest.revision)
                .unwrap_or(0)
                .max(
                    runtime
                        .prepared_bundle
                        .as_ref()
                        .map(|bundle| bundle.stored.manifest.revision)
                        .unwrap_or(0),
                )
        };
        if manifest.revision <= current_revision {
            return Ok(self.state_view());
        }

        validate_manifest(&manifest, &self.app_version)?;
        let response = self
            .client
            .get(&manifest.url)
            .header("Cache-Control", "no-cache")
            .send()
            .await
            .map_err(|error| format!("下载界面更新失败：{error}"))?;
        let response = response
            .error_for_status()
            .map_err(|error| format!("下载界面更新返回错误：{error}"))?;
        if let Some(length) = response.content_length() {
            if length != manifest.size || length > MAX_ARCHIVE_BYTES {
                return Err("界面更新包大小与清单不一致".into());
            }
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::with_capacity(manifest.size as usize);
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| format!("读取界面更新包失败：{error}"))?;
            let next_len = bytes
                .len()
                .checked_add(chunk.len())
                .ok_or_else(|| "界面更新包大小溢出".to_string())?;
            if next_len as u64 > manifest.size || next_len as u64 > MAX_ARCHIVE_BYTES {
                return Err("界面更新包大小超出允许范围".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.len() as u64 != manifest.size {
            return Err("界面更新包大小超出允许范围".into());
        }

        let stored = StoredBundle {
            archive_file: archive_file_name(&manifest),
            manifest,
        };
        let bundle = load_bundle_from_bytes(stored, bytes.as_slice(), &self.app_version)?;
        let archive_path = self.updates_dir.join(&bundle.stored.archive_file);
        let temporary = archive_path.with_extension("zip.download");
        fs::write(&temporary, bytes.as_slice())
            .map_err(|error| format!("保存界面更新包失败：{error}"))?;
        restrict_file_permissions(&temporary)?;
        fs::rename(&temporary, &archive_path)
            .map_err(|error| format!("替换界面更新包失败：{error}"))?;

        let bundle = Arc::new(bundle);
        let key = bundle.stored.manifest.ui_version.clone();
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if runtime.activation.is_some() {
            return Ok(state_view_from_runtime(&self.app_version, &runtime));
        }
        runtime.bundles.insert(key, Arc::clone(&bundle));
        runtime.prepared_bundle = Some(bundle);
        Ok(state_view_from_runtime(&self.app_version, &runtime))
    }

    pub(crate) fn apply_prepared(&self, app: &tauri::AppHandle) -> Result<(), String> {
        let candidate = {
            let mut runtime = self
                .runtime
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if runtime.activation.is_some() {
                return Ok(());
            }
            let candidate = runtime
                .prepared_bundle
                .clone()
                .ok_or_else(|| "没有准备好的界面更新".to_string())?;
            let expected_labels = app
                .webview_windows()
                .into_iter()
                .filter(|(label, window)| {
                    is_managed_surface_label(label) && window.is_visible().unwrap_or(false)
                })
                .map(|(label, _)| label)
                .collect::<HashSet<_>>();
            let previous_state = runtime.state_file.clone();
            runtime.state_file.pending = Some(candidate.stored.clone());
            runtime.state_file.last_result = None;
            if let Err(error) = write_state_file(&self.state_path, &runtime.state_file) {
                runtime.state_file = previous_state;
                return Err(error);
            }
            runtime.activation = Some(Activation {
                candidate_version: candidate.stored.manifest.ui_version.clone(),
                expected_labels,
                ready_labels: HashSet::new(),
            });
            candidate
        };

        navigate_existing_windows(app, &candidate.stored.manifest.ui_version);
        if self.activation_ready() {
            self.commit_activation()?;
        } else {
            let manager = app.state::<AppState>().ui_update.clone();
            let handle = app.clone();
            let candidate_version = candidate.stored.manifest.ui_version.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(ACTIVATION_TIMEOUT).await;
                if let Err(error) =
                    manager.rollback_activation_if_candidate(&handle, &candidate_version)
                {
                    log::warn!("界面更新超时回滚失败：{error}");
                }
            });
        }
        Ok(())
    }

    pub(crate) fn report_ready(
        &self,
        label: &str,
        ui_version: &str,
    ) -> Result<UiUpdateStateView, String> {
        let should_commit = {
            let mut runtime = self
                .runtime
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let Some(activation) = runtime.activation.as_mut() else {
                return Ok(state_view_from_runtime(&self.app_version, &runtime));
            };
            if activation.candidate_version != ui_version
                || !activation.expected_labels.contains(label)
            {
                return Ok(state_view_from_runtime(&self.app_version, &runtime));
            }
            activation.ready_labels.insert(label.to_owned());
            activation.ready_labels.len() == activation.expected_labels.len()
        };
        if should_commit {
            self.commit_activation()?;
        }
        Ok(self.state_view())
    }

    /// 窗口在切换期间被销毁后不再阻塞候选版本提交。
    pub(crate) fn surface_destroyed(&self, label: &str) -> Result<(), String> {
        let should_commit = {
            let mut runtime = self
                .runtime
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let Some(activation) = runtime.activation.as_mut() else {
                return Ok(());
            };
            activation.expected_labels.remove(label);
            activation.ready_labels.remove(label);
            activation.ready_labels.len() == activation.expected_labels.len()
        };
        if should_commit {
            self.commit_activation()?;
        }
        Ok(())
    }

    pub(crate) fn webview_url(&self, path: &str) -> WebviewUrl {
        // 开发模式必须继续交给 Tauri 的 Vite Dev Server；热更新首版也不改变
        // Windows/Linux 的原有资源加载路径。
        if cfg!(debug_assertions) || !cfg!(target_os = "macos") {
            return WebviewUrl::App(path.into());
        }
        let source = self.source_version();
        let (asset_path, suffix) = split_asset_suffix(path);
        let url = format!("{UI_PROTOCOL}://localhost/{source}/{asset_path}{suffix}");
        WebviewUrl::CustomProtocol(
            tauri::Url::parse(&url).expect("generated Lyrics Plus UI URL must be valid"),
        )
    }

    pub(crate) fn serve_request(
        &self,
        app: &tauri::AppHandle,
        request: Request<Vec<u8>>,
    ) -> Response<Vec<u8>> {
        if !cfg!(target_os = "macos") {
            return response_error(404, "UI hot updates are unavailable on this platform");
        }
        // Wry 在 Windows/Android 上会先用 `<scheme>.localhost` 规避协议限制，
        // 再在交给 Tauri handler 前还原为 `lyrics-plus-ui://localhost`。
        if request.uri().host() != Some("localhost") {
            return response_error(404, "invalid UI protocol host");
        }
        if request.method() != Method::GET && request.method() != Method::HEAD {
            return Response::builder()
                .status(405)
                .header("Allow", "GET, HEAD")
                .header("Content-Type", "text/plain; charset=utf-8")
                .header("X-Content-Type-Options", "nosniff")
                .body(b"UI protocol only supports GET and HEAD".to_vec())
                .unwrap_or_else(|_| Response::new(Vec::new()));
        }
        let Some((source, asset_path)) = request_asset_path(request.uri().path()) else {
            return response_error(404, "invalid UI asset path");
        };
        let Some(bytes) = self.asset_bytes(app, source, &asset_path) else {
            return response_error(404, "UI asset not found");
        };
        let mime_type = mime_type_for_asset(&bytes, &asset_path);
        let is_html = asset_path.ends_with(".html");
        let mut builder = Response::builder()
            .header("Content-Type", mime_type)
            .header("Content-Length", bytes.len().to_string())
            .header("Access-Control-Allow-Origin", protocol_origin())
            .header("X-Content-Type-Options", "nosniff")
            .header(
                "Cache-Control",
                if is_html {
                    "no-cache"
                } else {
                    "public, max-age=31536000, immutable"
                },
            );
        if is_html {
            builder = builder.header("Content-Security-Policy", content_security_policy());
        }
        let body = if request.method() == Method::HEAD {
            Vec::new()
        } else {
            bytes
        };
        builder
            .body(body)
            .unwrap_or_else(|_| response_error(500, "failed to build UI response"))
    }

    fn asset_bytes(
        &self,
        app: &tauri::AppHandle,
        source: &str,
        asset_path: &str,
    ) -> Option<Vec<u8>> {
        if source == "embedded" {
            return app
                .asset_resolver()
                .get(format!("/{asset_path}"))
                .map(|asset| asset.bytes);
        }
        let runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        runtime
            .bundles
            .get(source)
            .and_then(|bundle| bundle.assets.get(asset_path).cloned())
    }

    fn source_version(&self) -> String {
        let runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        runtime
            .activation
            .as_ref()
            .map(|activation| activation.candidate_version.clone())
            .or_else(|| {
                runtime
                    .active_bundle
                    .as_ref()
                    .map(|bundle| bundle.stored.manifest.ui_version.clone())
            })
            .unwrap_or_else(|| "embedded".into())
    }

    fn activation_ready(&self) -> bool {
        let runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        runtime.activation.as_ref().is_some_and(|activation| {
            activation.ready_labels.len() == activation.expected_labels.len()
        })
    }

    fn commit_activation(&self) -> Result<(), String> {
        let mut runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(activation) = runtime.activation.take() else {
            return Ok(());
        };
        let Some(candidate) = runtime
            .prepared_bundle
            .clone()
            .filter(|bundle| bundle.stored.manifest.ui_version == activation.candidate_version)
        else {
            runtime.activation = Some(activation);
            return Err("界面更新候选包已丢失".into());
        };
        let previous_state = runtime.state_file.clone();
        runtime.state_file.active = Some(candidate.stored.clone());
        runtime.state_file.pending = None;
        runtime.state_file.last_result = Some("applied".into());
        if let Err(error) = write_state_file(&self.state_path, &runtime.state_file) {
            runtime.state_file = previous_state;
            runtime.activation = Some(activation);
            return Err(error);
        }
        runtime.active_bundle = Some(candidate);
        runtime.prepared_bundle = None;
        let active_version = runtime
            .active_bundle
            .as_ref()
            .map(|bundle| bundle.stored.manifest.ui_version.clone());
        runtime
            .bundles
            .retain(|version, _| Some(version) == active_version.as_ref());
        Ok(())
    }

    fn rollback_activation_if_candidate(
        &self,
        app: &tauri::AppHandle,
        candidate_version: &str,
    ) -> Result<(), String> {
        let should_navigate = {
            let mut runtime = self
                .runtime
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let Some(activation) = runtime.activation.take() else {
                return Ok(());
            };
            if activation.candidate_version != candidate_version {
                runtime.activation = Some(activation);
                return Ok(());
            }
            let previous_state = runtime.state_file.clone();
            let previous_prepared = runtime.prepared_bundle.clone();
            runtime.state_file.last_result = Some("rolledBack".into());
            runtime.state_file.pending = None;
            runtime.prepared_bundle = None;
            let removed_bundle = runtime.bundles.remove(candidate_version);
            if let Err(error) = write_state_file(&self.state_path, &runtime.state_file) {
                runtime.state_file = previous_state;
                runtime.prepared_bundle = previous_prepared;
                if let Some(bundle) = removed_bundle {
                    runtime.bundles.insert(candidate_version.to_owned(), bundle);
                }
                runtime.activation = Some(activation);
                return Err(error);
            }
            true
        };
        if should_navigate {
            let source = self
                .runtime
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .active_bundle
                .as_ref()
                .map(|bundle| bundle.stored.manifest.ui_version.clone())
                .unwrap_or_else(|| "embedded".into());
            navigate_existing_windows(app, &source);
        }
        Ok(())
    }
}

pub(crate) fn webview_url(app: &tauri::AppHandle, path: &str) -> WebviewUrl {
    app.state::<AppState>().ui_update.webview_url(path)
}

pub(crate) fn configure_protocol(
    builder: tauri::Builder<tauri::Wry>,
) -> tauri::Builder<tauri::Wry> {
    builder.register_uri_scheme_protocol(UI_PROTOCOL, |_context, request| {
        let app = _context.app_handle();
        let Some(state) = app.try_state::<AppState>() else {
            return response_error(503, "UI update runtime is unavailable");
        };
        state.ui_update.serve_request(app, request)
    })
}

fn state_view_from_runtime(app_version: &str, runtime: &RuntimeState) -> UiUpdateStateView {
    UiUpdateStateView {
        app_version: app_version.to_owned(),
        core_api_version: CORE_API_VERSION,
        source: if runtime.active_bundle.is_some() {
            "hot".into()
        } else {
            "embedded".into()
        },
        active_version: runtime
            .active_bundle
            .as_ref()
            .map(|bundle| bundle.stored.manifest.ui_version.clone())
            .unwrap_or_else(|| "embedded".into()),
        prepared_version: runtime
            .prepared_bundle
            .as_ref()
            .map(|bundle| bundle.stored.manifest.ui_version.clone()),
        prepared_release_notes: runtime
            .prepared_bundle
            .as_ref()
            .map(|bundle| bundle.stored.manifest.release_notes.clone()),
        pending_version: runtime
            .state_file
            .pending
            .as_ref()
            .map(|bundle| bundle.manifest.ui_version.clone()),
        last_result: runtime.state_file.last_result.clone(),
    }
}

fn read_state_file(path: &Path) -> Option<UiStateFile> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_state_file(path: &Path, state: &UiStateFile) -> Result<(), String> {
    let raw = serde_json::to_vec_pretty(state)
        .map_err(|error| format!("序列化界面更新状态失败：{error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, raw).map_err(|error| format!("保存界面更新状态失败：{error}"))?;
    restrict_file_permissions(&temporary)?;
    #[cfg(windows)]
    if path.exists() {
        // Windows 的标准 rename 不覆盖已有文件；首版虽不启用热更新，仍不能让
        // 其他平台在应用版本升级时因状态文件替换失败而无法启动。
        fs::remove_file(path).map_err(|error| format!("清理旧界面更新状态失败：{error}"))?;
    }
    fs::rename(&temporary, path).map_err(|error| format!("替换界面更新状态失败：{error}"))
}

fn load_bundle_from_file(
    updates_dir: &Path,
    stored: StoredBundle,
    app_version: &str,
) -> Result<UiBundle, String> {
    let archive_file = safe_archive_file(&stored.archive_file)?;
    if archive_file != archive_file_name(&stored.manifest) {
        return Err("界面更新缓存文件名与清单不匹配".into());
    }
    let archive_path = updates_dir.join(archive_file);
    let metadata = fs::symlink_metadata(&archive_path)
        .map_err(|error| format!("读取界面更新缓存信息失败：{error}"))?;
    if !metadata.file_type().is_file() {
        return Err("界面更新缓存不是普通文件".into());
    }
    let length = metadata.len();
    if length != stored.manifest.size || length > MAX_ARCHIVE_BYTES {
        return Err("界面更新缓存大小超出允许范围".into());
    }
    let mut file =
        fs::File::open(archive_path).map_err(|error| format!("读取界面更新缓存失败：{error}"))?;
    let mut bytes = Vec::with_capacity(length as usize);
    file.by_ref()
        .take(MAX_ARCHIVE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("读取界面更新缓存失败：{error}"))?;
    if bytes.len() as u64 != length {
        return Err("界面更新缓存在读取期间发生变化".into());
    }
    load_bundle_from_bytes(stored, &bytes, app_version)
}

fn load_bundle_from_bytes(
    stored: StoredBundle,
    bytes: &[u8],
    app_version: &str,
) -> Result<UiBundle, String> {
    validate_manifest(&stored.manifest, app_version)?;
    if bytes.len() as u64 != stored.manifest.size || bytes.len() as u64 > MAX_ARCHIVE_BYTES {
        return Err("界面更新缓存大小不符合清单".into());
    }
    let digest = format!("{:x}", Sha256::digest(bytes));
    if !digest.eq_ignore_ascii_case(stored.manifest.sha256.trim()) {
        return Err("界面更新缓存摘要校验失败".into());
    }
    verify_signature(bytes, &stored.manifest.signature)?;

    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| format!("打开界面更新 ZIP 失败：{error}"))?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err("界面更新 ZIP 文件数量超出限制".into());
    }
    let mut assets = HashMap::new();
    let mut total_size = 0u64;
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|error| format!("读取界面更新 ZIP 条目失败：{error}"))?;
        if file.is_dir() {
            continue;
        }
        let name = safe_asset_name(file.name())?.to_owned();
        let declared_size = file.size();
        let remaining = MAX_UNCOMPRESSED_BYTES.saturating_sub(total_size);
        if declared_size > remaining {
            return Err("界面更新 ZIP 解压大小超出限制".into());
        }
        let mut data = Vec::with_capacity(declared_size as usize);
        file.take(remaining.saturating_add(1))
            .read_to_end(&mut data)
            .map_err(|error| format!("读取界面更新资源失败：{error}"))?;
        if data.len() as u64 > remaining {
            return Err("界面更新 ZIP 解压大小超出限制".into());
        }
        total_size = total_size
            .checked_add(data.len() as u64)
            .ok_or_else(|| "界面更新 ZIP 解压大小溢出".to_string())?;
        if assets.insert(name, data).is_some() {
            return Err("界面更新 ZIP 包含重复资源路径".into());
        }
    }
    if !assets.contains_key("index.html") {
        return Err("界面更新 ZIP 缺少 index.html".into());
    }
    Ok(UiBundle {
        stored,
        assets: Arc::new(assets),
    })
}

fn validate_manifest(manifest: &UiUpdateManifest, app_version: &str) -> Result<(), String> {
    if manifest.schema_version != 1 {
        return Err("不支持的界面更新清单版本".into());
    }
    if manifest.app_version != app_version {
        return Err("界面更新与当前应用版本不匹配".into());
    }
    if manifest.core_api_version != CORE_API_VERSION {
        return Err("界面更新与当前 Core API 不匹配".into());
    }
    if manifest.revision == 0 || manifest.ui_version.trim().is_empty() {
        return Err("界面更新版本无效".into());
    }
    let expected_ui_version = format!("{}-ui.{}", manifest.app_version, manifest.revision);
    if manifest.ui_version != expected_ui_version {
        return Err("界面更新版本与修订号不匹配".into());
    }
    if manifest.size == 0 || manifest.size > MAX_ARCHIVE_BYTES {
        return Err("界面更新包大小无效".into());
    }
    let url = tauri::Url::parse(&manifest.url).map_err(|_| "界面更新地址无效".to_string())?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("界面更新地址不是受信任的 GitHub HTTPS 地址".into());
    }
    if manifest.sha256.len() != 64 || !manifest.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("界面更新摘要格式无效".into());
    }
    if manifest.signature.trim().is_empty() {
        return Err("界面更新签名为空".into());
    }
    Ok(())
}

fn verify_signature(bytes: &[u8], encoded_signature: &str) -> Result<(), String> {
    let public_key_text = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(UI_PUBLIC_KEY)
            .map_err(|error| format!("解析界面更新公钥失败：{error}"))?,
    )
    .map_err(|error| format!("解析界面更新公钥文本失败：{error}"))?;
    let public_key = PublicKey::decode(&public_key_text)
        .map_err(|error| format!("解析界面更新公钥失败：{error}"))?;
    let signature_text = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(encoded_signature)
            .map_err(|error| format!("解析界面更新签名失败：{error}"))?,
    )
    .map_err(|error| format!("解析界面更新签名文本失败：{error}"))?;
    let signature = Signature::decode(&signature_text)
        .map_err(|error| format!("解析界面更新签名失败：{error}"))?;
    public_key
        .verify(bytes, &signature, true)
        .map_err(|error| format!("界面更新签名校验失败：{error}"))
}

fn archive_file_name(manifest: &UiUpdateManifest) -> String {
    format!("ui-{}-r{}.zip", manifest.app_version, manifest.revision)
}

fn safe_archive_file(file: &str) -> Result<&str, String> {
    let path = Path::new(file);
    if path.file_name().and_then(|name| name.to_str()) != Some(file)
        || file.chars().any(|character| character.is_control())
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir
                    | Component::RootDir
                    | Component::CurDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err("界面更新缓存路径无效".into());
    }
    Ok(file)
}

fn safe_asset_name(name: &str) -> Result<&str, String> {
    if name.is_empty()
        || name.contains('\\')
        || name.starts_with('/')
        || name.chars().any(|character| character.is_control())
        || name
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err("界面更新资源路径无效".into());
    }
    let path = Path::new(name);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::CurDir | Component::Prefix(_)
        )
    }) {
        return Err("界面更新资源路径包含目录穿越".into());
    }
    Ok(name)
}

fn request_asset_path(path: &str) -> Option<(&str, String)> {
    let path = path.strip_prefix('/')?;
    let mut segments = path.split('/');
    let source = segments.next()?;
    let asset_path = segments.collect::<Vec<_>>().join("/");
    if source.is_empty() || asset_path.is_empty() || asset_path.contains('%') {
        return None;
    }
    safe_asset_name(&asset_path).ok()?;
    Some((source, asset_path))
}

fn split_asset_suffix(path: &str) -> (&str, &str) {
    let query = path.find('?');
    let fragment = path.find('#');
    let split_at = query.into_iter().chain(fragment).min();
    split_at.map_or((path, ""), |index| (&path[..index], &path[index..]))
}

fn mime_type_for_asset(bytes: &[u8], path: &str) -> String {
    let extension = path
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    match extension.as_deref() {
        Some("html") => "text/html".into(),
        Some("css") => "text/css".into(),
        Some("js" | "mjs") => "text/javascript".into(),
        Some("json" | "map") => "application/json".into(),
        Some("svg") => "image/svg+xml".into(),
        Some("png") => "image/png".into(),
        Some("jpg" | "jpeg") => "image/jpeg".into(),
        Some("gif") => "image/gif".into(),
        Some("webp") => "image/webp".into(),
        Some("avif") => "image/avif".into(),
        Some("ico") => "image/vnd.microsoft.icon".into(),
        Some("woff") => "font/woff".into(),
        Some("woff2") => "font/woff2".into(),
        Some("ttf") => "font/ttf".into(),
        Some("otf") => "font/otf".into(),
        Some("wasm") => "application/wasm".into(),
        Some("txt") => "text/plain".into(),
        Some("xml") => "application/xml".into(),
        Some("mp4") => "video/mp4".into(),
        Some("webm") => "video/webm".into(),
        _ => tauri::utils::mime_type::MimeType::parse_with_fallback(
            bytes,
            path,
            tauri::utils::mime_type::MimeType::OctetStream,
        ),
    }
}

fn navigate_existing_windows(app: &tauri::AppHandle, source: &str) {
    for window in app.webview_windows().into_values() {
        if !is_managed_surface_label(window.label()) {
            continue;
        }
        let Ok(current) = window.url() else {
            continue;
        };
        let query = current
            .query()
            .map(|value| format!("?{value}"))
            .unwrap_or_default();
        let fragment = current
            .fragment()
            .map(|value| format!("#{value}"))
            .unwrap_or_default();
        let url = format!("{UI_PROTOCOL}://localhost/{source}/index.html{query}{fragment}");
        let result = tauri::Url::parse(&url)
            .map_err(|error| error.to_string())
            .and_then(|url| window.navigate(url).map_err(|error| error.to_string()));
        match result {
            Ok(()) => {}
            Err(error) => log::warn!(
                "导航界面更新资源失败：label={}, error={error}",
                window.label()
            ),
        }
    }
}

fn response_error(status: u16, message: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/plain; charset=utf-8")
        .header("X-Content-Type-Options", "nosniff")
        .body(message.as_bytes().to_vec())
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn protocol_origin() -> &'static str {
    if cfg!(windows) {
        "http://lyrics-plus-ui.localhost"
    } else {
        "lyrics-plus-ui://localhost"
    }
}

fn content_security_policy() -> &'static str {
    "default-src 'self' lyrics-plus-ui: customprotocol: asset:; connect-src ipc: http://ipc.localhost https://raw.githubusercontent.com; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'"
}

fn is_managed_surface_label(label: &str) -> bool {
    matches!(
        label,
        "main"
            | "quick-lyrics"
            | "lyrics-overlay"
            | "lyrics-unlock-handle"
            | "lyrics-list"
            | "lyrics-list-unlock-handle"
            | "lyrics-notch"
            | "lyrics-status-bar"
    )
}

fn restrict_file_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("无法限制界面更新文件权限：{error}"))?;
    }
    Ok(())
}
