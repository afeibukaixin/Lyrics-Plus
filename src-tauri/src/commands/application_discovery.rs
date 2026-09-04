use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::config::{
    normalize_player_follower_application, normalize_system_media_applications,
    RegisteredApplication,
};
use crate::player::run_with_timeout;

fn plist_string(path: &Path, key: &str) -> Option<String> {
    let mut command = Command::new("/usr/bin/plutil");
    command.args(["-extract", key, "raw", "-o", "-"]).arg(path);
    let output = run_with_timeout(command, Duration::from_secs(3)).ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(target_os = "macos")]
fn localized_application_name(path: &Path) -> Option<String> {
    use objc2_foundation::{NSBundle, NSString};

    let bundle_path = NSString::from_str(path.to_string_lossy().as_ref());
    let bundle = NSBundle::bundleWithPath(&bundle_path)?;
    ["CFBundleDisplayName", "CFBundleName"]
        .into_iter()
        .find_map(|key| {
            let value = bundle.objectForInfoDictionaryKey(&NSString::from_str(key))?;
            let value = value.downcast_ref::<NSString>()?.to_string();
            (!value.trim().is_empty()).then_some(value)
        })
}

#[cfg(not(target_os = "macos"))]
fn localized_application_name(_path: &Path) -> Option<String> {
    None
}

fn application_display_name(name: String) -> String {
    name.strip_suffix(".app").unwrap_or(&name).to_owned()
}

pub(super) fn resolve_registered_application(path: &Path) -> Result<RegisteredApplication, String> {
    if !path.is_dir() || path.extension().and_then(|value| value.to_str()) != Some("app") {
        return Err(format!("不是有效的 .app：{}", path.display()));
    }
    let plist = ["Contents/Info.plist", "WrappedBundle/Info.plist"]
        .into_iter()
        .map(|relative_path| path.join(relative_path))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("应用缺少 Info.plist：{}", path.display()))?;
    let bundle_id = plist_string(&plist, "CFBundleIdentifier")
        .ok_or_else(|| format!("应用缺少 Bundle ID：{}", path.display()))?;
    let name = localized_application_name(path)
        .or_else(|| plist_string(&plist, "CFBundleDisplayName"))
        .or_else(|| plist_string(&plist, "CFBundleName"))
        .or_else(|| {
            path.file_stem()
                .map(|value| value.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| bundle_id.clone());
    Ok(RegisteredApplication {
        name: application_display_name(name),
        bundle_id,
    })
}

pub(super) fn discover_system_media_applications(
    paths: Vec<PathBuf>,
) -> Result<Vec<RegisteredApplication>, String> {
    normalize_system_media_applications(
        paths
            .iter()
            .map(|path| resolve_registered_application(path))
            .collect::<Result<Vec<_>, _>>()?,
    )
}

pub(super) fn discover_player_follower_application(
    path: &Path,
) -> Result<RegisteredApplication, String> {
    normalize_player_follower_application(Some(resolve_registered_application(path)?))?
        .ok_or_else(|| "未选择播放器".into())
}

pub(super) fn collect_application_icons(bundle_ids: Vec<String>) -> HashMap<String, String> {
    bundle_ids
        .into_iter()
        .filter_map(|bundle_id| application_icon(&bundle_id).map(|icon| (bundle_id, icon)))
        .collect()
}

#[cfg(target_os = "macos")]
pub(super) fn application_icon_at_path(path: &str) -> Option<String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use objc2::{rc::autoreleasepool, AnyThread};
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

    autoreleasepool(|_| {
        let workspace = NSWorkspace::sharedWorkspace();
        let icon = workspace.iconForFile(&NSString::from_str(path));
        let mut bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(64.0, 64.0));
        let image = unsafe { icon.CGImageForProposedRect_context_hints(&mut bounds, None, None)? };
        let bitmap = NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &image);
        let properties = NSDictionary::new();
        let png = unsafe {
            bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties)?
        };
        Some(format!(
            "data:image/png;base64,{}",
            STANDARD.encode(png.to_vec())
        ))
    })
}

#[cfg(target_os = "macos")]
fn application_icon(bundle_id: &str) -> Option<String> {
    use objc2::rc::autoreleasepool;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;

    autoreleasepool(|_| {
        let workspace = NSWorkspace::sharedWorkspace();
        let url =
            workspace.URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))?;
        let path = url.path()?;
        application_icon_at_path(&path.to_string())
    })
}

#[cfg(not(target_os = "macos"))]
fn application_icon(_bundle_id: &str) -> Option<String> {
    None
}

pub(super) fn resolve_application_bundle_id(
    bundle_id: &str,
) -> Result<RegisteredApplication, String> {
    resolve_application_bundle_id_inner(bundle_id)
}

#[cfg(target_os = "macos")]
fn resolve_application_bundle_id_inner(bundle_id: &str) -> Result<RegisteredApplication, String> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::NSString;

    let workspace = NSWorkspace::sharedWorkspace();
    let url = workspace
        .URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))
        .ok_or_else(|| format!("找不到应用：{bundle_id}"))?;
    let path = url
        .path()
        .ok_or_else(|| format!("无法读取应用路径：{bundle_id}"))?;
    resolve_registered_application(Path::new(&path.to_string()))
}

#[cfg(not(target_os = "macos"))]
fn resolve_application_bundle_id_inner(_bundle_id: &str) -> Result<RegisteredApplication, String> {
    Err("应用解析仅支持 macOS".into())
}
