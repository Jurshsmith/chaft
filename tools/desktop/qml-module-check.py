#!/usr/bin/env python3
import re
import sys
from pathlib import Path


def fail(message: str) -> None:
    raise SystemExit(message)


def qml_module_files(root: Path) -> list[str]:
    qml_root = root / "apps" / "desktop-qt" / "qml" / "Chaft"
    if not qml_root.is_dir():
        fail(f"QML module directory not found: {qml_root}")
    return sorted(
        f"qml/Chaft/{path.relative_to(qml_root).as_posix()}"
        for path in qml_root.rglob("*.qml")
        if path.is_file()
    )


def cmake_qml_files(root: Path) -> list[str]:
    cmake_path = root / "apps" / "desktop-qt" / "CMakeLists.txt"
    if not cmake_path.is_file():
        fail(f"CMakeLists.txt not found: {cmake_path}")
    text = cmake_path.read_text(encoding="utf-8")
    return sorted(set(re.findall(r"\bqml/Chaft/[^\s)]+\.qml\b", text)))


def qmldir_exports(root: Path) -> list[str]:
    qmldir_path = root / "apps" / "desktop-qt" / "qml" / "Chaft" / "qmldir"
    if not qmldir_path.is_file():
        fail(f"qmldir not found: {qmldir_path}")

    exports: list[str] = []
    for line_no, raw_line in enumerate(qmldir_path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw_line.strip()
        if not line or line.startswith("#") or line.startswith("module ") or line.startswith("depends "):
            continue

        parts = line.split()
        qml_path = ""
        if parts[0] == "singleton" and len(parts) >= 4:
            qml_path = parts[3]
        elif len(parts) >= 3:
            qml_path = parts[2]

        if qml_path.endswith(".qml"):
            exports.append(f"qml/Chaft/{qml_path}")
        elif parts[0] != "import":
            fail(f"unsupported qmldir row at line {line_no}: {raw_line}")

    return sorted(set(exports))


def main() -> int:
    root = Path(__file__).resolve().parents[2]
    module_files = qml_module_files(root)
    cmake_files = cmake_qml_files(root)
    exported_files = qmldir_exports(root)

    missing_from_cmake = sorted(set(module_files) - set(cmake_files))
    stale_in_cmake = sorted(set(cmake_files) - set(module_files))
    missing_export_files = sorted(set(exported_files) - set(module_files))

    errors: list[str] = []
    if missing_from_cmake:
        errors.append(
            "QML file(s) missing from apps/desktop-qt/CMakeLists.txt:\n"
            + "\n".join(f"  - {path}" for path in missing_from_cmake)
        )
    if stale_in_cmake:
        errors.append(
            "stale QML file(s) listed in apps/desktop-qt/CMakeLists.txt:\n"
            + "\n".join(f"  - {path}" for path in stale_in_cmake)
        )
    if missing_export_files:
        errors.append(
            "qmldir export(s) reference missing QML file(s):\n"
            + "\n".join(f"  - {path}" for path in missing_export_files)
        )

    if errors:
        fail("\n\n".join(errors))

    print(f"QML module file list verified: {len(module_files)} file(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
