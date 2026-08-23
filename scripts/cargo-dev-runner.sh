#!/bin/zsh

set -euo pipefail

script_dir="${0:A:h}"
project_root="${script_dir:h}"

command_name="${1:-}"
if [[ -z "$command_name" ]]; then
  exec cargo
fi
shift

if [[ "$command_name" != "run" ]]; then
  exec cargo "$command_name" "$@"
fi

cargo_args=()
app_args=()
reading_app_args=false
release_mode=false
target_triple=""
expect_target=false

for arg in "$@"; do
  if [[ "$reading_app_args" == true ]]; then
    app_args+=("$arg")
    continue
  fi

  if [[ "$arg" == "--" ]]; then
    reading_app_args=true
    continue
  fi

  cargo_args+=("$arg")

  if [[ "$expect_target" == true ]]; then
    target_triple="$arg"
    expect_target=false
    continue
  fi

  case "$arg" in
    --release)
      release_mode=true
      ;;
    --target)
      expect_target=true
      ;;
    --target=*)
      target_triple="${arg#--target=}"
      ;;
  esac
done

cargo build "${cargo_args[@]}"

cargo_target_dir="${CARGO_TARGET_DIR:-$PWD/target}"
if [[ -n "$target_triple" ]]; then
  cargo_target_dir="$cargo_target_dir/$target_triple"
fi

profile_dir="debug"
if [[ "$release_mode" == true ]]; then
  profile_dir="release"
fi

source_binary="$cargo_target_dir/$profile_dir/lyrics-plus"
app_bundle="$cargo_target_dir/$profile_dir/Lyrics Plus Dev.app"
contents_dir="$app_bundle/Contents"
macos_dir="$contents_dir/MacOS"
resources_dir="$contents_dir/Resources"
display_binary="$macos_dir/Lyrics Plus"
entitlements="$project_root/src-tauri/Entitlements.plist"

mkdir -p "$macos_dir" "$resources_dir"
cp -f "$source_binary" "$display_binary"
cp -f "$project_root/src-tauri/icons/icon.icns" "$resources_dir/icon.icns"
cp -f "$script_dir/dev-Info.plist" "$contents_dir/Info.plist"

# Cargo 只为独立 Mach-O 写入链接器临时签名；放入 .app 后需要重新签署整个 bundle。
codesign --force --deep --sign - --entitlements "$entitlements" "$app_bundle"
codesign --verify --deep --strict "$app_bundle"

# Tauri 热重载可能直接终止旧 runner，导致由 LaunchServices 启动的应用成为残留进程。
# 启动新版本前只清理当前 Dev Bundle 的旧实例，避免菜单栏图标不断重复。
existing_app_pids=("${(@f)$(/usr/bin/pgrep -f "^${display_binary}( |$)" || true)}")
for existing_app_pid in "${existing_app_pids[@]}"; do
  if [[ -n "$existing_app_pid" ]]; then
    /bin/kill -TERM "$existing_app_pid" 2>/dev/null || true
  fi
done

for _ in {1..50}; do
  if ! /usr/bin/pgrep -f "^${display_binary}( |$)" >/dev/null 2>&1; then
    break
  fi
  /bin/sleep 0.1
done

# 通过 LaunchServices 启动，确保 TCC 将应用自身而不是终端识别为权限责任主体。
if (( ${#app_args[@]} > 0 )); then
  /usr/bin/open -n "$app_bundle" --args "${app_args[@]}"
else
  /usr/bin/open -n "$app_bundle"
fi

# open -W 会在开发进程继承较多文件描述符时触发 kqueue 限制，改为等待新启动的应用进程。
app_pid=""
for _ in {1..50}; do
  app_pid="$(/usr/bin/pgrep -n -f "^${display_binary}( |$)" || true)"
  if [[ -n "$app_pid" ]]; then
    break
  fi
  /bin/sleep 0.1
done

if [[ -z "$app_pid" ]]; then
  print -u2 "无法定位刚启动的 Lyrics Plus Dev 进程"
  exit 1
fi

stop_app() {
  /bin/kill -TERM "$app_pid" 2>/dev/null || true
}
trap stop_app INT TERM HUP

while /bin/kill -0 "$app_pid" 2>/dev/null; do
  /bin/sleep 0.25
done
