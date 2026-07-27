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

qt_provenance_dir="$smoke_dir/qt-provenance"
mkdir "$qt_provenance_dir"
python3 - "$repo_root" "$qt_provenance_dir" <<'PY'
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
output = Path(sys.argv[2])
sys.path.insert(0, str(root / "tools" / "qt"))
import build_qt as qt

manifest = qt.load_manifest(root / "tools" / "qt" / "qt-6.8.4.json")
runner_os = {"linux": "Linux", "macos": "macOS", "windows": "Windows"}
for platform in qt.SUPPORTED_PLATFORMS:
    contract = {
        "schemaVersion": 1,
        "platform": platform,
        "runner": {
            "os": runner_os[platform],
            "architecture": "X64",
            "imageOS": f"synthetic-{platform}",
            "imageVersion": "20260726.1",
        },
        "tools": {
            "cmake": "cmake version 4.1.0",
            "ninja": "1.13.1",
            "compiler": f"synthetic {platform} compiler 1.0",
            "python": "3.13.3",
        },
    }
    fingerprint = qt.toolchain_fingerprint(contract, platform)
    provenance = {
        "schemaVersion": 1,
        "identity": qt.sdk_identity(manifest, platform, fingerprint),
        "manifestSha256": qt.manifest_digest(manifest),
        "contractSha256": qt.contract_digest(manifest),
        "qtVersion": manifest["qtVersion"],
        "sdkRevision": manifest["sdkRevision"],
        "platform": platform,
        "platformSpecification": manifest["platforms"][platform],
        "buildConfiguration": manifest["build"],
        "generatedAt": "2026-07-26T00:00:00Z",
        "host": {
            "system": runner_os[platform],
            "release": "synthetic",
            "machine": "x86_64",
        },
        "toolchainFingerprint": fingerprint,
        "toolchainContract": contract,
        "sourceMaterials": qt.expected_source_materials(manifest, platform),
        "recipeMaterials": qt.recipe_materials(),
        "commands": [],
        "verification": {
            "completed": True,
            "completedAt": "2026-07-26T00:01:00Z",
        },
    }
    path = output / f"chaft-qt-sdk-{platform}.json"
    path.write_text(
        json.dumps(provenance, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
PY
export CHAFT_QT_SDK_PROVENANCE_DIR="$qt_provenance_dir"

linux_dir="$smoke_dir/linux-package"
macos_dir="$smoke_dir/macos-package"
windows_dir="$smoke_dir/windows-package"
ci_dir="$smoke_dir/ci-package"
dispatch_dir="$smoke_dir/dispatch-package"
materials_dir="$smoke_dir/materials-package"
signature_dir="$smoke_dir/signature-package"
orphan_signature_dir="$smoke_dir/orphan-signature-package"
platform_mismatch_dir="$smoke_dir/platform-mismatch-package"
qt_binding_dir="$smoke_dir/qt-binding-package"
canary_dir="$smoke_dir/canary-package"
invalid_canary_dir="$smoke_dir/invalid-canary-package"

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
write_artifact "$dispatch_dir" "Chaft-0.1.0-Dispatch.tar.gz"
write_artifact "$materials_dir" "Chaft-0.1.0-Materials.tar.gz"
write_artifact "$signature_dir" "Chaft-0.1.0-Signed.AppImage"
write_signature "$signature_dir" "Chaft-0.1.0-Signed.AppImage" ".sig"
write_artifact "$orphan_signature_dir" "Chaft-0.1.0-Orphan.AppImage"
write_artifact "$platform_mismatch_dir" "Chaft-0.1.0-Wrong-Platform.tar.gz"
write_artifact "$qt_binding_dir" "Chaft-0.1.0-Qt-Binding.tar.gz"
write_artifact "$canary_dir" "Chaft-0.1.0-canary.1-Linux-x86_64.AppImage"
write_artifact "$invalid_canary_dir" "Chaft-0.1.0-canary.1-Linux.AppImage"

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

CHAFT_DISTRIBUTION_VERSION=0.1.0-canary.1 \
  python3 "$repo_root/tools/desktop/release-metadata.py" release \
    --package-dir "$canary_dir" \
    --platform Linux
python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
  --package-dir "$canary_dir" \
  --platform Linux \
  --expected-source-version 0.1.0 \
  --expected-distribution-version 0.1.0-canary.1
python3 - \
  "$canary_dir/chaft-desktop-linux-sbom.cdx.json" \
  "$canary_dir/chaft-desktop-linux-provenance.json" <<'PY'
import json
import sys
from pathlib import Path

sbom = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
provenance = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
component = sbom["metadata"]["component"]
properties = {
    item["name"]: item["value"]
    for item in sbom["metadata"]["properties"]
}
if component["version"] != "0.1.0-canary.1":
    raise SystemExit("canary SBOM component does not use distribution version")
if properties.get("chaft:sourceVersion") != "0.1.0":
    raise SystemExit("canary SBOM does not retain stable source version")
if properties.get("chaft:distributionVersion") != "0.1.0-canary.1":
    raise SystemExit("canary SBOM distribution version is missing")
if provenance.get("sourceVersion") != "0.1.0":
    raise SystemExit("canary provenance does not retain stable source version")
if provenance.get("distributionVersion") != "0.1.0-canary.1":
    raise SystemExit("canary provenance distribution version is missing")
if provenance.get("version") != "0.1.0-canary.1":
    raise SystemExit("canary provenance version alias is stale")
PY
expect_failure "incorrect canary native package filename" \
  env CHAFT_DISTRIBUTION_VERSION=0.1.0-canary.1 \
  python3 "$repo_root/tools/desktop/release-metadata.py" release \
    --package-dir "$invalid_canary_dir" \
    --platform Linux
expect_failure "canary filename with omitted distribution version input" \
  python3 "$repo_root/tools/desktop/release-metadata.py" release \
    --package-dir "$canary_dir" \
    --platform Linux
expect_failure "mismatched expected canary distribution version" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$canary_dir" \
    --platform Linux \
    --expected-distribution-version 0.1.0-canary.2

python3 "$repo_root/tools/desktop/release-metadata.py" release \
  --package-dir "$qt_binding_dir" \
  --platform Linux
python3 - "$qt_binding_dir/chaft-desktop-linux-provenance.json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
provenance = json.loads(path.read_text(encoding="utf-8"))
provenance["qt"]["sdk"]["identity"] = "stale-qt-sdk-identity"
path.write_text(
    json.dumps(provenance, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
PY
expect_failure "stale Qt SDK release identity" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$qt_binding_dir" \
    --platform Linux

python3 "$repo_root/tools/desktop/release-metadata.py" release \
  --package-dir "$qt_binding_dir" \
  --platform Linux
python3 - "$qt_binding_dir/chaft-desktop-linux-provenance.json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
provenance = json.loads(path.read_text(encoding="utf-8"))
provenance["qt"]["correspondingSource"]["contractSha256"] = "0" * 64
path.write_text(
    json.dumps(provenance, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
PY
expect_failure "stale Qt corresponding-source contract" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$qt_binding_dir" \
    --platform Linux

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
    if item.get("name") == "tools/desktop/macos-adhoc-verify.cmake":
        item["sha256"] = "0" * 64
        break
else:
    raise SystemExit("macOS ad-hoc verification provenance material row missing")
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

GITHUB_ACTIONS=true \
GITHUB_REPOSITORY=Jurshsmith/chaft \
GITHUB_RUN_ID=3 \
GITHUB_SHA="$wrong_ci_sha" \
CHAFT_RELEASE_COMMIT="$ci_sha" \
  python3 "$repo_root/tools/desktop/release-metadata.py" release \
    --package-dir "$dispatch_dir" \
    --platform Linux

python3 - "$dispatch_dir/chaft-desktop-linux-provenance.json" <<'PY'
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
  GITHUB_RUN_ID=3 \
  GITHUB_SHA="$wrong_ci_sha" \
  CHAFT_RELEASE_COMMIT="$ci_sha" \
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$dispatch_dir" \
    --platform Linux \
    --source-root "$repo_root" \
    --expected-commit "$ci_sha" \
    --require-clean

python3 - "$dispatch_dir/chaft-desktop-linux-provenance.json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
provenance = json.loads(path.read_text(encoding="utf-8"))
provenance.setdefault("github", {})["CHAFT_RELEASE_COMMIT"] = "0" * 40
path.write_text(json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
expect_failure "workflow-dispatch release commit mismatch" \
  env \
    GITHUB_ACTIONS=true \
    GITHUB_REPOSITORY=Jurshsmith/chaft \
    GITHUB_RUN_ID=3 \
    GITHUB_SHA="$wrong_ci_sha" \
    CHAFT_RELEASE_COMMIT="$ci_sha" \
    python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
      --package-dir "$dispatch_dir" \
      --platform Linux \
      --source-root "$repo_root" \
      --expected-commit "$ci_sha" \
      --require-clean

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
