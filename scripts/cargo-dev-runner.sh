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

mkdir -p "$macos_dir" "$resources_dir"
cp -f "$source_binary" "$display_binary"
cp -f "$project_root/src-tauri/icons/icon.icns" "$resources_dir/icon.icns"
cp -f "$script_dir/dev-Info.plist" "$contents_dir/Info.plist"
exec "$display_binary" "${app_args[@]}"
