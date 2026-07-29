#!/usr/bin/env sh

chaft_desktop_path_prepend() {
  dir="$1"
  if [ -d "$dir" ]; then
    case ":$PATH:" in
      *":$dir:"*) ;;
      *)
        PATH="$dir:$PATH"
        export PATH
        ;;
    esac
  fi
}

chaft_desktop_add_tool_paths() {
  if [ -n "${QT_ROOT_DIR:-}" ]; then
    chaft_desktop_path_prepend "$QT_ROOT_DIR/bin"
  fi

  if [ -n "${Qt6_DIR:-}" ]; then
    qt6_prefix="$(CDPATH= cd "$(dirname "$Qt6_DIR")/../.." 2>/dev/null && pwd || true)"
    if [ -n "$qt6_prefix" ]; then
      chaft_desktop_path_prepend "$qt6_prefix/bin"
    fi
  fi

  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*)
      if [ -n "${VCToolsInstallDir:-}" ] && command -v cygpath >/dev/null 2>&1; then
        msvc_tools_dir="$(cygpath -u "$VCToolsInstallDir")"
        chaft_desktop_path_prepend "$msvc_tools_dir/bin/Hostx64/x64"
      fi
      ;;
  esac

  chaft_desktop_path_prepend /opt/homebrew/bin
  chaft_desktop_path_prepend /usr/local/bin

  if command -v brew >/dev/null 2>&1; then
    brew_prefix="$(brew --prefix 2>/dev/null || true)"
    if [ -n "$brew_prefix" ]; then
      chaft_desktop_path_prepend "$brew_prefix/bin"
    fi

    for formula in qtbase qt qt@6; do
      formula_prefix="$(brew --prefix "$formula" 2>/dev/null || true)"
      if [ -n "$formula_prefix" ]; then
        chaft_desktop_path_prepend "$formula_prefix/bin"
      fi
    done
  fi

  for prefix in \
    /opt/homebrew/opt/qtbase \
    /opt/homebrew/opt/qt \
    /opt/homebrew/opt/qt@6 \
    /usr/local/opt/qtbase \
    /usr/local/opt/qt \
    /usr/local/opt/qt@6
  do
    chaft_desktop_path_prepend "$prefix/bin"
  done
}

chaft_desktop_qt_prefix() {
  if command -v qmake6 >/dev/null 2>&1; then
    qmake6 -query QT_INSTALL_PREFIX 2>/dev/null && return 0
  fi

  if command -v qt-cmake >/dev/null 2>&1; then
    qt_cmake="$(command -v qt-cmake)"
    qt_bin_dir="$(dirname "$qt_cmake")"
    (CDPATH= cd "$qt_bin_dir/.." && pwd) && return 0
  fi

  for prefix in \
    /opt/homebrew/opt/qtbase \
    /opt/homebrew/opt/qt \
    /opt/homebrew/opt/qt@6 \
    /usr/local/opt/qtbase \
    /usr/local/opt/qt \
    /usr/local/opt/qt@6
  do
    if [ -x "$prefix/bin/qmake6" ] || [ -x "$prefix/bin/qt-cmake" ]; then
      printf '%s\n' "$prefix"
      return 0
    fi
  done

  return 1
}

chaft_desktop_qt_compatibility_cmake_arguments() {
  profile="$1"
  if [ "$profile" != "debug" ] \
      || [ "${CHAFT_QT_SDK_BUILD_TYPE:-}" != "Release" ]; then
    return
  fi

  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*)
      # Chaft's deterministic Windows SDK intentionally contains release Qt
      # libraries. Keep the unoptimized application/debug-symbol build, but
      # align its imported Qt configuration, MSVC CRT, and Qt header contract
      # with those libraries. Mixing /MDd application code with /MD Qt DLLs
      # corrupts process state during QApplication startup.
      printf '%s\n' \
        "-DCHAFT_DEBUG_USES_RELEASE_QT=ON" \
        "-DCMAKE_MAP_IMPORTED_CONFIG_DEBUG=Release" \
        "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDLL"
      ;;
  esac
}

chaft_desktop_ffi_library_name() {
  case "$(uname -s)" in
    Darwin) printf 'libchaft_ffi.dylib\n' ;;
    MINGW*|MSYS*|CYGWIN*) printf 'chaft_ffi.dll\n' ;;
    *) printf 'libchaft_ffi.so\n' ;;
  esac
}

chaft_desktop_cli_binary_name() {
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) printf 'chaft-cli.exe\n' ;;
    *) printf 'chaft-cli\n' ;;
  esac
}

chaft_desktop_binary_candidates() {
  repo_root="$1"
  preset="$2"

  for base in \
    "$repo_root/build/$preset/apps/desktop-qt" \
    "$repo_root/build/$preset"
  do
    printf '%s\n' \
      "$base/Chaft.app/Contents/MacOS/Chaft" \
      "$base/ChaftDesktop.exe" \
      "$base/ChaftDesktop"
  done
}

chaft_desktop_installed_binary_candidates() {
  repo_root="$1"
  preset="$2"
  install_root="$repo_root/build/$preset/install"

  printf '%s\n' \
    "$install_root/Chaft.app/Contents/MacOS/Chaft" \
    "$install_root/bin/ChaftDesktop.exe" \
    "$install_root/bin/ChaftDesktop" \
    "$install_root/ChaftDesktop.exe" \
    "$install_root/ChaftDesktop"
}

chaft_desktop_find_binary() {
  repo_root="$1"
  preset="$2"

  for base in \
    "$repo_root/build/$preset/apps/desktop-qt" \
    "$repo_root/build/$preset"
  do
    for candidate in \
      "$base/Chaft.app/Contents/MacOS/Chaft" \
      "$base/ChaftDesktop.exe" \
      "$base/ChaftDesktop"
    do
      if [ -x "$candidate" ]; then
        printf '%s\n' "$candidate"
        return 0
      fi
    done
  done

  return 1
}

chaft_desktop_prepare_smoke_binary() {
  desktop_binary="$1"
  smoke_dir="$2"

  case "$(uname -s)" in
    Darwin)
      case "$desktop_binary" in
        *.app/Contents/MacOS/*)
          # Some macOS agent shells can leave direct launches from inside an
          # .app bundle in the kernel's launched-suspended state before main().
          # Smoke tests exercise the same executable from a temporary path that
          # preserves @loader_path-relative bundled frameworks/resources without
          # using a .app suffix. Normal launch/package paths keep the bundle.
          app_bundle="${desktop_binary%%.app/Contents/MacOS/*}.app"
          smoke_bundle="$smoke_dir/$(basename "$app_bundle" .app)-smoke"
          smoke_binary="$smoke_bundle/Contents/MacOS/$(basename "$desktop_binary")"
          rm -rf "$smoke_bundle"
          mkdir -p "$smoke_bundle/Contents/MacOS"
          rm -f "$smoke_binary"
          cp "$desktop_binary" "$smoke_binary"
          chmod +x "$smoke_binary"
          for bundle_item in Frameworks PlugIns Resources Info.plist; do
            if [ -e "$app_bundle/Contents/$bundle_item" ]; then
              ln -s "$app_bundle/Contents/$bundle_item" "$smoke_bundle/Contents/$bundle_item"
            fi
          done
          printf '%s\n' "$smoke_binary"
          return 0
          ;;
      esac
      ;;
  esac

  printf '%s\n' "$desktop_binary"
}

chaft_desktop_find_installed_binary() {
  repo_root="$1"
  preset="$2"

  for candidate in \
    "$repo_root/build/$preset/install/Chaft.app/Contents/MacOS/Chaft" \
    "$repo_root/build/$preset/install/bin/ChaftDesktop.exe" \
    "$repo_root/build/$preset/install/bin/ChaftDesktop" \
    "$repo_root/build/$preset/install/ChaftDesktop.exe" \
    "$repo_root/build/$preset/install/ChaftDesktop"
  do
    if [ -x "$candidate" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  return 1
}
