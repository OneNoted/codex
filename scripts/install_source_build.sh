#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cargo_root="$repo_root/codex-rs"
manifest_path="$cargo_root/Cargo.toml"
version_override="${CODEX_SOURCE_BUILD_VERSION:-${CODEX_CLI_BUILD_VERSION:-}}"
manifest_backup="$(mktemp "${TMPDIR:-/tmp}/codex-cargo-toml.XXXXXX")"

cleanup() {
  if [[ -f "$manifest_backup" ]]; then
    cp "$manifest_backup" "$manifest_path"
    rm -f "$manifest_backup"
  fi
}

trap cleanup EXIT

cp "$manifest_path" "$manifest_backup"

determine_release_version() {
  if [[ -n "$version_override" ]]; then
    printf '%s\n' "$version_override"
    return 0
  fi

  local latest_tag=""
  latest_tag="$(
    curl -fsSL https://api.github.com/repos/openai/codex/releases/latest | jq -r '.tag_name // empty'
  )" || true
  if [[ "$latest_tag" =~ ^rust-v([0-9]+\.[0-9]+\.[0-9]+)$ ]]; then
    printf '%s\n' "${BASH_REMATCH[1]}"
    return 0
  fi

  echo "failed to determine a release version automatically" >&2
  echo "set CODEX_SOURCE_BUILD_VERSION=x.y.z and retry" >&2
  return 1
}

build_version="$(determine_release_version)"
if [[ ! "$build_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "expected a plain semver x.y.z build version, got: $build_version" >&2
  echo "set CODEX_SOURCE_BUILD_VERSION=x.y.z if you need an explicit override" >&2
  exit 1
fi

install_args=("$@")
if [[ "${install_args[0]:-}" == "--" ]]; then
  install_args=("${install_args[@]:1}")
fi

python3 - "$manifest_path" "$build_version" <<'PY'
from pathlib import Path
import sys

manifest_path = Path(sys.argv[1])
build_version = sys.argv[2]
lines = manifest_path.read_text().splitlines()

in_workspace_package = False
updated = False
for index, line in enumerate(lines):
    stripped = line.strip()
    if stripped == "[workspace.package]":
        in_workspace_package = True
        continue
    if in_workspace_package and stripped.startswith("[") and stripped.endswith("]"):
        break
    if in_workspace_package and stripped.startswith("version = "):
        lines[index] = f'version = "{build_version}"'
        updated = True
        break

if not updated:
    raise SystemExit("failed to update [workspace.package].version in Cargo.toml")

manifest_path.write_text("\n".join(lines) + "\n")
PY

echo "Installing source build with stamped version $build_version" >&2
(
  cd "$cargo_root"
  cargo install --path cli --locked "${install_args[@]}"
)

echo "Installed codex source build version $build_version" >&2
