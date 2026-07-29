#!/usr/bin/env sh
set -eu

script_dir="$(CDPATH= cd "$(dirname "$0")" && pwd)"
repo_root="$(CDPATH= cd "$script_dir/../.." && pwd)"

cleanup() {
  if [ "${CHAFT_KEEP_SMOKE:-0}" != "1" ] && [ -n "${smoke_dir:-}" ]; then
    rm -rf "$smoke_dir"
  elif [ -n "${smoke_dir:-}" ]; then
    printf 'kept smoke directory: %s\n' "$smoke_dir"
  fi
}

expect_failure() {
  description="$1"
  shift
  if "$@" >"$smoke_dir/unexpected-success.log" 2>&1; then
    printf 'expected failure did not occur: %s\n' "$description" >&2
    cat "$smoke_dir/unexpected-success.log" >&2
    exit 1
  fi
}

write_artifact() {
  package_dir="$1"
  filename="$2"
  mkdir -p "$package_dir"
  printf 'Chaft synthetic release artifact: %s\n' "$filename" \
    >"$package_dir/$filename"
}

generate_metadata() {
  target="$1"
  package_dir="$2"
  shift 2
  env \
    CHAFT_QT_POLICY=release \
    CHAFT_QT_SDK_TARGET="$target" \
    CHAFT_QT_SDK_PROVENANCE_DIR="$qt_provenance_dir" \
    python3 "$repo_root/tools/desktop/release-metadata.py" release \
      --package-dir "$package_dir" \
      --target "$target" \
      "$@" || return $?

  case "$target" in
    macos-arm64) architecture="arm64" ;;
    *) architecture="x86_64" ;;
  esac
  python3 - \
    "$package_dir/chaft-desktop-$target-provenance.json" \
    "$architecture" <<'PY'
import json
import sys
from pathlib import Path

# This smoke creates all native-target fixtures on one host. Production metadata
# is never rewritten; the matrix jobs verify their naturally native host values.
path = Path(sys.argv[1])
value = json.loads(path.read_text(encoding="utf-8"))
value["platform"]["machine"] = sys.argv[2]
path.write_text(
    json.dumps(value, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
PY
}

verify_metadata() {
  target="$1"
  package_dir="$2"
  shift 2
  python3 "$repo_root/tools/desktop/verify-release-metadata.py" release \
    --package-dir "$package_dir" \
    --target "$target" \
    "$@"
}

for tool in cargo git python3; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$tool" >&2
    exit 1
  fi
done

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

manifest = qt.load_manifest(
    root / "tools" / "qt" / "qt-6.8.4.json",
    recipe_root=root,
)
runner_os = {"linux": "Linux", "macos": "macOS", "windows": "Windows"}
for target_name in qt.SUPPORTED_TARGETS:
    specification = manifest["targets"][target_name]
    platform_name = specification["platform"]
    contract = {
        "schemaVersion": 2,
        "target": target_name,
        "platform": platform_name,
        "runner": {
            "os": runner_os[platform_name],
            "architecture": specification["architecture"],
            "imageOS": f"synthetic-{target_name}",
            "imageVersion": "20260729.1",
        },
        "tools": {
            "cmake": "cmake version 4.1.0",
            "ninja": "1.13.1",
            "compiler": f"synthetic {target_name} compiler 1.0",
            "python": "3.13.3",
        },
    }
    fingerprint = qt.toolchain_fingerprint(
        contract,
        manifest,
        target_name,
    )
    provenance = {
        "schemaVersion": 2,
        "identity": qt.sdk_identity(
            manifest,
            target_name,
            fingerprint,
            recipe_root=root,
        ),
        "manifestSha256": qt.manifest_digest(
            manifest,
            recipe_root=root,
        ),
        "contractSha256": qt.contract_digest(
            manifest,
            recipe_root=root,
        ),
        "qtVersion": manifest["qtVersion"],
        "sdkRevision": manifest["sdkRevision"],
        "target": target_name,
        "platform": platform_name,
        "architecture": specification["architecture"],
        "targetSpecification": specification,
        "buildConfiguration": manifest["build"],
        "generatedAt": "2026-07-29T00:00:00Z",
        "host": {
            "system": runner_os[platform_name],
            "release": "synthetic",
            "machine": specification["architecture"],
        },
        "toolchainFingerprint": fingerprint,
        "toolchainContract": contract,
        "sourceMaterials": qt.expected_source_materials(
            manifest,
            target_name,
        ),
        "recipeMaterials": qt.recipe_materials(root),
        "commands": [],
        "verification": {
            "completed": True,
            "completedAt": "2026-07-29T00:01:00Z",
        },
    }
    qt.validate_provenance_object(
        provenance,
        manifest,
        target_name,
        recipe_root=root,
    )
    path = output / f"chaft-qt-sdk-{target_name}.json"
    path.write_text(
        json.dumps(provenance, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
PY

targets="
windows-x86_64|Chaft-0.1.0-Windows-x86_64.zip
macos-x86_64|Chaft-0.1.0-macOS-x86_64.dmg
macos-arm64|Chaft-0.1.0-macOS-arm64.dmg
linux-x86_64|Chaft-0.1.0-Linux-x86_64.AppImage
"

printf '%s\n' "$targets" | while IFS='|' read -r target filename; do
  [ -n "$target" ] || continue
  package_dir="$smoke_dir/$target-package"
  write_artifact "$package_dir" "$filename"
  generate_metadata "$target" "$package_dir"
  verify_metadata "$target" "$package_dir"

  for suffix in SHA256SUMS sbom.cdx.json provenance.json; do
    path="$package_dir/chaft-desktop-$target-$suffix"
    if [ ! -f "$path" ]; then
      printf 'expected target-qualified metadata was not generated: %s\n' "$path" >&2
      exit 1
    fi
  done
done

python3 - \
  "$smoke_dir/macos-x86_64-package/chaft-desktop-macos-x86_64-provenance.json" \
  "$smoke_dir/macos-arm64-package/chaft-desktop-macos-arm64-provenance.json" <<'PY'
import json
import sys
from pathlib import Path

x86 = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
arm = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
if (x86["packageTarget"], x86["packageArchitecture"]) != (
    "macos-x86_64",
    "x86_64",
):
    raise SystemExit("Intel macOS metadata is not target-bound")
if (arm["packageTarget"], arm["packageArchitecture"]) != (
    "macos-arm64",
    "arm64",
):
    raise SystemExit("Apple Silicon metadata is not target-bound")
if x86["qt"]["sdk"]["identity"] == arm["qt"]["sdk"]["identity"]:
    raise SystemExit("macOS Qt SDK identities must be architecture-specific")
PY

linux_dir="$smoke_dir/linux-x86_64-package"
macos_arm_dir="$smoke_dir/macos-arm64-package"

expect_failure "developer Qt cannot generate official release metadata" \
  env \
    CHAFT_QT_POLICY=developer \
    CHAFT_QT_SDK_TARGET=linux-x86_64 \
    CHAFT_QT_SDK_PROVENANCE_DIR="$qt_provenance_dir" \
    python3 "$repo_root/tools/desktop/release-metadata.py" release \
      --package-dir "$linux_dir" \
      --target linux-x86_64

expect_failure "wrong pinned Qt architecture cannot generate official metadata" \
  env \
    CHAFT_QT_POLICY=release \
    CHAFT_QT_SDK_TARGET=macos-x86_64 \
    CHAFT_QT_SDK_PROVENANCE_DIR="$qt_provenance_dir" \
    python3 "$repo_root/tools/desktop/release-metadata.py" release \
      --package-dir "$macos_arm_dir" \
      --target macos-arm64

wrong_name_dir="$smoke_dir/wrong-name-package"
write_artifact "$wrong_name_dir" "Chaft-0.1.0-macOS.dmg"
expect_failure "architecture-free macOS package name" \
  generate_metadata macos-arm64 "$wrong_name_dir"

canary_dir="$smoke_dir/canary-package"
write_artifact "$canary_dir" \
  "Chaft-0.1.0-canary.1-Linux-x86_64.AppImage"
generate_metadata linux-x86_64 "$canary_dir" \
  --distribution-version 0.1.0-canary.1
verify_metadata linux-x86_64 "$canary_dir" \
  --expected-source-version 0.1.0 \
  --expected-distribution-version 0.1.0-canary.1

printf 'tampered package bytes\n' \
  >>"$linux_dir/Chaft-0.1.0-Linux-x86_64.AppImage"
expect_failure "stale metadata after package mutation" \
  verify_metadata linux-x86_64 "$linux_dir"

signature_dir="$smoke_dir/signature-package"
write_artifact "$signature_dir" "Chaft-0.1.0-Linux-x86_64.AppImage"
printf 'synthetic detached signature\n' \
  >"$signature_dir/Chaft-0.1.0-Linux-x86_64.AppImage.sig"
generate_metadata linux-x86_64 "$signature_dir"
verify_metadata linux-x86_64 "$signature_dir"
printf 'tampered signature bytes\n' \
  >>"$signature_dir/Chaft-0.1.0-Linux-x86_64.AppImage.sig"
expect_failure "stale metadata after signature mutation" \
  verify_metadata linux-x86_64 "$signature_dir"

qt_binding_dir="$smoke_dir/qt-binding-package"
write_artifact "$qt_binding_dir" "Chaft-0.1.0-Linux-x86_64.AppImage"
generate_metadata linux-x86_64 "$qt_binding_dir"
python3 - \
  "$qt_binding_dir/chaft-desktop-linux-x86_64-provenance.json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
value = json.loads(path.read_text(encoding="utf-8"))
value["qt"]["sdk"]["identity"] = "stale-qt-sdk-identity"
path.write_text(
    json.dumps(value, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
PY
expect_failure "stale Qt SDK identity" \
  verify_metadata linux-x86_64 "$qt_binding_dir"

target_binding_dir="$smoke_dir/target-binding-package"
write_artifact "$target_binding_dir" \
  "Chaft-0.1.0-macOS-arm64.dmg"
generate_metadata macos-arm64 "$target_binding_dir"
python3 - \
  "$target_binding_dir/chaft-desktop-macos-arm64-provenance.json" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
value = json.loads(path.read_text(encoding="utf-8"))
value["packageArchitecture"] = "x86_64"
path.write_text(
    json.dumps(value, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
PY
expect_failure "desktop provenance architecture substitution" \
  verify_metadata macos-arm64 "$target_binding_dir"

printf 'release metadata smoke passed\n'
