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

write_signature() {
  package_dir="$1"
  package_name="$2"
  signature_suffix="$3"

  printf 'Chaft synthetic detached signature for: %s\n' "$package_name" \
    > "$package_dir/$package_name$signature_suffix"
}

assert_file() {
  path="$1"
  if [ ! -f "$path" ]; then
    printf 'expected file was not generated: %s\n' "$path" >&2
    exit 1
  fi
}

assert_not_file() {
  path="$1"
  if [ -e "$path" ]; then
    printf 'unexpected legacy metadata file was generated: %s\n' "$path" >&2
    exit 1
  fi
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
signature_dir="$smoke_dir/signature-package"
orphan_signature_dir="$smoke_dir/orphan-signature-package"
platform_mismatch_dir="$smoke_dir/platform-mismatch-package"

write_artifact "$linux_dir" "Chaft-0.1.0-Linux.tar.gz"
write_artifact "$linux_dir" "Chaft-0.1.0-Linux.AppImage"
write_signature "$linux_dir" "Chaft-0.1.0-Linux.AppImage" ".asc"
write_artifact "$macos_dir" "Chaft-0.1.0-macOS.dmg"
write_signature "$macos_dir" "Chaft-0.1.0-macOS.dmg" ".sig"
write_artifact "$windows_dir" "Chaft-0.1.0-Windows.zip"
write_artifact "$windows_dir" "Chaft-0.1.0-Windows.msi"
write_artifact "$windows_dir" "Chaft-0.1.0-Windows.exe"
write_signature "$windows_dir" "Chaft-0.1.0-Windows.msi" ".sig"
write_artifact "$ci_dir" "Chaft-0.1.0-CI.tar.gz"
write_artifact "$materials_dir" "Chaft-0.1.0-Materials.tar.gz"
write_artifact "$signature_dir" "Chaft-0.1.0-Signed.AppImage"
write_signature "$signature_dir" "Chaft-0.1.0-Signed.AppImage" ".sig"
write_artifact "$orphan_signature_dir" "Chaft-0.1.0-Orphan.AppImage"
write_artifact "$platform_mismatch_dir" "Chaft-0.1.0-Wrong-Platform.tar.gz"

for row in \
  "Linux:$linux_dir" \
  "macOS:$macos_dir" \
  "Windows:$windows_dir"
do
  platform="${row%%:*}"
  package_dir="${row#*:}"

  case "$platform" in
    Linux) platform_slug="linux" ;;
    macOS) platform_slug="macos" ;;
    Windows) platform_slug="windows" ;;
    *) printf 'unsupported smoke platform: %s\n' "$platform" >&2; exit 1 ;;
  esac

  python3 "$repo_root/tools/desktop/release-metadata.py" release \
    --package-dir "$package_dir" \
    --platform "$platform"
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$package_dir" \
    --platform "$platform"

  assert_file "$package_dir/chaft-desktop-$platform_slug-SHA256SUMS"
  assert_file "$package_dir/chaft-desktop-$platform_slug-sbom.cdx.json"
  assert_file "$package_dir/chaft-desktop-$platform_slug-provenance.json"
  assert_not_file "$package_dir/SHA256SUMS"
  assert_not_file "$package_dir/chaft-desktop-sbom.cdx.json"
  assert_not_file "$package_dir/chaft-desktop-provenance.json"
done

python3 - "$macos_dir/chaft-desktop-macos-provenance.json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
provenance = json.loads(path.read_text(encoding="utf-8"))
provenance["createdAt"] = "2026-07-18"
path.write_text(json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
expect_failure "date-only provenance timestamp" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$macos_dir" \
    --platform macOS
python3 "$repo_root/tools/desktop/release-metadata.py" release \
  --package-dir "$macos_dir" \
  --platform macOS

expect_failure "platform package suffix mismatch during generation" \
  python3 "$repo_root/tools/desktop/release-metadata.py" release \
    --package-dir "$platform_mismatch_dir" \
    --platform Windows
python3 "$repo_root/tools/desktop/release-metadata.py" release \
  --package-dir "$platform_mismatch_dir" \
  --platform Linux
mv "$platform_mismatch_dir/chaft-desktop-linux-SHA256SUMS" \
  "$platform_mismatch_dir/chaft-desktop-windows-SHA256SUMS"
mv "$platform_mismatch_dir/chaft-desktop-linux-sbom.cdx.json" \
  "$platform_mismatch_dir/chaft-desktop-windows-sbom.cdx.json"
mv "$platform_mismatch_dir/chaft-desktop-linux-provenance.json" \
  "$platform_mismatch_dir/chaft-desktop-windows-provenance.json"
expect_failure "platform package suffix mismatch during verification" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$platform_mismatch_dir" \
    --platform Windows

python3 "$repo_root/tools/desktop/release-metadata.py" release \
  --package-dir "$orphan_signature_dir" \
  --platform Linux
printf 'not attached to a package\n' > "$orphan_signature_dir/arbitrary-file.sig"
expect_failure "orphan detached signature during generation" \
  python3 "$repo_root/tools/desktop/release-metadata.py" release \
    --package-dir "$orphan_signature_dir" \
    --platform Linux
expect_failure "orphan detached signature during verification" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$orphan_signature_dir" \
    --platform Linux

python3 - \
  "$windows_dir/chaft-desktop-windows-SHA256SUMS" \
  "$windows_dir/chaft-desktop-windows-sbom.cdx.json" \
  "$windows_dir/chaft-desktop-windows-provenance.json" <<'PY'
import json
import sys
from pathlib import Path

checksums_path = Path(sys.argv[1])
sbom_path = Path(sys.argv[2])
provenance_path = Path(sys.argv[3])
signature_name = "Chaft-0.1.0-Windows.msi.sig"

checksum_names = {
    line.split("  ", 1)[1]
    for line in checksums_path.read_text(encoding="utf-8").splitlines()
    if line
}
if signature_name not in checksum_names:
    raise SystemExit("detached signature missing from platform checksum file")

sbom = json.loads(sbom_path.read_text(encoding="utf-8"))
sbom_properties = {
    item.get("name"): item.get("value")
    for item in sbom.get("properties", [])
    if isinstance(item, dict)
}
if sbom_properties.get(f"chaft:artifact:{signature_name}:signedArtifact") != "Chaft-0.1.0-Windows.msi":
    raise SystemExit("SBOM detached signature relationship missing")

provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
signature_rows = [
    item
    for item in provenance.get("artifacts", [])
    if item.get("name") == signature_name
]
if len(signature_rows) != 1 or signature_rows[0].get("signedArtifact") != "Chaft-0.1.0-Windows.msi":
    raise SystemExit("provenance detached signature relationship missing")

for item in sbom.get("metadata", {}).get("properties", []):
    if item.get("name") == "chaft:sourceCommit":
        item["value"] = "stale-source-commit"
sbom_path.write_text(json.dumps(sbom, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
expect_failure "stale SBOM source commit" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$windows_dir" \
    --platform Windows

python3 "$repo_root/tools/desktop/release-metadata.py" release \
  --package-dir "$materials_dir" \
  --platform Linux
python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
  --package-dir "$materials_dir" \
  --platform Linux

python3 - "$materials_dir/chaft-desktop-linux-provenance.json" <<'PY'
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
wrong_ci_sha="ffffffffffffffffffffffffffffffffffffffff"
GITHUB_ACTIONS=true \
GITHUB_REPOSITORY=Jurshsmith/chaft \
GITHUB_RUN_ID=1 \
GITHUB_SHA="$ci_sha" \
  python3 "$repo_root/tools/desktop/release-metadata.py" release \
    --package-dir "$ci_dir" \
    --platform Linux

python3 - "$ci_dir/chaft-desktop-linux-provenance.json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
provenance = json.loads(path.read_text(encoding="utf-8"))
provenance.setdefault("source", {})["dirty"] = False
path.write_text(json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY

env \
  GITHUB_ACTIONS=true \
  GITHUB_REPOSITORY=Jurshsmith/chaft \
  GITHUB_RUN_ID=2 \
  GITHUB_SHA="$wrong_ci_sha" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$ci_dir" \
    --platform Linux \
    --source-root "$repo_root" \
    --expected-commit "$ci_sha" \
    --require-clean

python3 - "$ci_dir/chaft-desktop-linux-provenance.json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
provenance = json.loads(path.read_text(encoding="utf-8"))
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

python3 "$repo_root/tools/desktop/release-metadata.py" release \
  --package-dir "$signature_dir" \
  --platform Linux
python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
  --package-dir "$signature_dir" \
  --platform Linux
printf 'tampered signature bytes\n' >> "$signature_dir/Chaft-0.1.0-Signed.AppImage.sig"
expect_failure "stale metadata after detached signature mutation" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$signature_dir" \
    --platform Linux

python3 - "$macos_dir/chaft-desktop-macos-provenance.json" <<'PY'
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
