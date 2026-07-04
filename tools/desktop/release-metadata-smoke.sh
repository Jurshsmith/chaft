#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"

require_tool() {
  name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$name" >&2
    exit 1
  fi
}

cleanup() {
  if [ "${CHAFT_KEEP_SMOKE:-0}" != "1" ] && [ -n "${smoke_dir:-}" ]; then
    rm -rf "$smoke_dir"
  elif [ -n "${smoke_dir:-}" ]; then
    printf 'kept smoke directory: %s\n' "$smoke_dir"
  fi
}

write_artifact() {
  package_dir="$1"
  name="$2"

  mkdir -p "$package_dir"
  printf 'Chaft synthetic package artifact: %s\n' "$name" > "$package_dir/$name"
}

expect_failure() {
  description="$1"
  shift

  if "$@" > "$smoke_dir/unexpected-success.log" 2>&1; then
    printf 'expected failure did not occur: %s\n' "$description" >&2
    cat "$smoke_dir/unexpected-success.log" >&2
    exit 1
  fi
}

require_tool cargo
require_tool git
require_tool python3

smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/chaft-release-metadata-smoke.XXXXXX")"
trap cleanup EXIT INT TERM

linux_dir="$smoke_dir/linux-package"
macos_dir="$smoke_dir/macos-package"
windows_dir="$smoke_dir/windows-package"
ci_dir="$smoke_dir/ci-package"
materials_dir="$smoke_dir/materials-package"

write_artifact "$linux_dir" "Chaft-0.1.0-Linux.tar.gz"
write_artifact "$macos_dir" "Chaft-0.1.0-macOS.dmg"
write_artifact "$windows_dir" "Chaft-0.1.0-Windows.zip"
write_artifact "$ci_dir" "Chaft-0.1.0-CI.tar.gz"
write_artifact "$materials_dir" "Chaft-0.1.0-Materials.tar.gz"

for row in \
  "Linux:$linux_dir" \
  "macOS:$macos_dir" \
  "Windows:$windows_dir"
do
  platform="${row%%:*}"
  package_dir="${row#*:}"

  python3 "$repo_root/tools/desktop/release-metadata.py" release \
    --package-dir "$package_dir"
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$package_dir" \
    --platform "$platform"
done

expect_failure "platform package suffix mismatch" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$linux_dir" \
    --platform Windows

python3 - "$windows_dir/chaft-desktop-sbom.cdx.json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
sbom = json.loads(path.read_text(encoding="utf-8"))
for item in sbom.get("metadata", {}).get("properties", []):
    if item.get("name") == "chaft:sourceCommit":
        item["value"] = "stale-source-commit"
path.write_text(json.dumps(sbom, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
expect_failure "stale SBOM source commit" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$windows_dir" \
    --platform Windows

python3 "$repo_root/tools/desktop/release-metadata.py" release \
  --package-dir "$materials_dir"
python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
  --package-dir "$materials_dir" \
  --platform Linux

python3 - "$materials_dir/chaft-desktop-provenance.json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
provenance = json.loads(path.read_text(encoding="utf-8"))
for item in provenance.get("materials", []):
    if item.get("name") == "Cargo.toml":
        item["sha256"] = "0" * 64
        break
else:
    raise SystemExit("Cargo.toml provenance material row missing")
path.write_text(json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
expect_failure "stale provenance source material" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$materials_dir" \
    --platform Linux

ci_sha="$(git -C "$repo_root" rev-parse HEAD)"
GITHUB_ACTIONS=true \
GITHUB_REPOSITORY=Jurshsmith/chaft \
GITHUB_RUN_ID=1 \
GITHUB_SHA="$ci_sha" \
  python3 "$repo_root/tools/desktop/release-metadata.py" release \
    --package-dir "$ci_dir"

python3 - "$ci_dir/chaft-desktop-provenance.json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
provenance = json.loads(path.read_text(encoding="utf-8"))
provenance.setdefault("source", {})["dirty"] = False
provenance.setdefault("github", {})["GITHUB_SHA"] = "stale-ci-sha"
path.write_text(json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
expect_failure "CI provenance commit mismatch" \
  env \
    GITHUB_ACTIONS=true \
    GITHUB_REPOSITORY=Jurshsmith/chaft \
    GITHUB_RUN_ID=1 \
    GITHUB_SHA="$ci_sha" \
    python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
      --package-dir "$ci_dir" \
      --platform Linux

printf 'tampered package bytes\n' >> "$linux_dir/Chaft-0.1.0-Linux.tar.gz"
expect_failure "stale checksum/SBOM/provenance after package mutation" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$linux_dir" \
    --platform Linux

python3 - "$macos_dir/chaft-desktop-provenance.json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
provenance = json.loads(path.read_text(encoding="utf-8"))
provenance.setdefault("source", {})["dirty"] = True
path.write_text(json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
expect_failure "dirty provenance when clean release metadata is required" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$macos_dir" \
    --platform macOS \
    --require-clean

printf 'release metadata smoke passed\n'
