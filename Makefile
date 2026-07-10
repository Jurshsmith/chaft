SHELL := /bin/sh

PROFILE ?= debug
PACKAGE_PROFILE ?= release
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
PLATFORM ?= macOS
else ifeq ($(UNAME_S),Linux)
PLATFORM ?= Linux
else ifneq (,$(filter MINGW% MSYS% CYGWIN%,$(UNAME_S)))
PLATFORM ?= Windows
else
PLATFORM ?=
endif
ARGS ?=
LAUNCH_ARGS ?=

CARGO ?= cargo
PYTHON ?= python3

.DEFAULT_GOAL := help

.PHONY: help
help:
	@printf '%s\n' \
		'Chaft make aliases' \
		'' \
		'Usage:' \
		'  make <target> [PROFILE=debug|release] [PACKAGE_PROFILE=release] [PLATFORM=Linux|macOS|Windows] [ARGS=...]' \
		'' \
		'Rust:' \
		'  make fmt              Format Rust workspace' \
		'  make fmt-check        Check Rust formatting' \
		'  make check            cargo check --workspace --all-targets' \
		'  make clippy           cargo clippy --workspace --all-targets -- -D warnings' \
		'  make test             cargo test --workspace --all-targets' \
		'  make test-app         Test chaft-app and chaft-ffi packages' \
		'  make bench-check      Compile hot-path benchmarks without running them' \
		'  make rust-gates       Run tools/ci/rust-gates.sh' \
		'' \
		'Desktop/QML:' \
		'  make desktop-preflight  Run desktop preflight' \
		'  make qml-lint           Run QML lint' \
		'  make style-lint         Run desktop style lint' \
		'  make theme-contrast     Run theme contrast check' \
		'  make desktop-checks     Run QML lint, style lint, and theme contrast' \
		'' \
		'Desktop build/run:' \
		'  make desktop-build             Build desktop app with PROFILE' \
		'  make desktop-smoke             Run desktop smoke with PROFILE' \
		'  make desktop-empty-smoke       Run empty workspace desktop smoke with PROFILE' \
		'  make desktop-launch            Build and launch normal desktop runtime' \
		'  make desktop-launch-fresh      Recreate normal desktop runtime before launch' \
		'  make desktop-launch-detached   Launch normal desktop runtime in background' \
		'  make desktop-launch-smoke      Build and launch seeded visual smoke workspace' \
		'  make desktop-launch-smoke-fresh Recreate seeded visual smoke workspace before launch' \
		'  make desktop-package           Build package with PACKAGE_PROFILE' \
		'  make desktop-package-smoke     Smoke packaged app with PACKAGE_PROFILE' \
		'' \
		'CI:' \
		'  make ci-rust           Run Rust CI gates' \
		'  make ci-desktop        Run desktop CI gates' \
		'  make ci-desktop-fast   Run desktop CI gates without packaging' \
		'  make ci                Run Rust and desktop CI gates' \
		'' \
		'Smoke/release:' \
		'  make smoke-p2p                Run local P2P smoke' \
		'  make smoke-access             Run access request/response transport smoke' \
		'  make smoke-lifecycle          Run workspace lifecycle/admin smoke' \
		'  make smoke-visual             Generate visual workspace smoke data' \
		'  make screenshot-smoke         Run screenshot smoke with PROFILE' \
		'  make release-metadata         Generate release metadata with PACKAGE_PROFILE' \
		'  make release-metadata-check   Verify release metadata for PLATFORM' \
		'  make release-metadata-smoke   Run release metadata smoke'

.PHONY: fmt fmt-check check clippy test test-app bench-check rust-gates
fmt:
	$(CARGO) fmt --all

fmt-check:
	$(CARGO) fmt --all --check

check:
	$(CARGO) check --workspace --all-targets $(ARGS)

clippy:
	$(CARGO) clippy --workspace --all-targets $(ARGS) -- -D warnings

test:
	$(CARGO) test --workspace --all-targets $(ARGS)

test-app:
	$(CARGO) test -p chaft-app -p chaft-ffi $(ARGS)

bench-check:
	$(CARGO) bench -p chaft-benchmarks --bench hot_paths --no-run $(ARGS)

rust-gates:
	tools/ci/rust-gates.sh $(ARGS)

.PHONY: desktop-preflight qml-lint style-lint theme-contrast desktop-checks
desktop-preflight:
	tools/desktop/preflight.sh

qml-lint:
	tools/desktop/qml-lint.sh

style-lint:
	$(PYTHON) tools/desktop/style-lint.py

theme-contrast:
	$(PYTHON) tools/desktop/theme-contrast-check.py

desktop-checks:
	$(MAKE) qml-lint
	$(MAKE) style-lint
	$(MAKE) theme-contrast

.PHONY: desktop-build desktop-smoke desktop-empty-smoke desktop-launch desktop-launch-fresh desktop-launch-detached desktop-launch-smoke desktop-launch-smoke-fresh desktop-package desktop-package-smoke
desktop-build:
	tools/desktop/build.sh $(PROFILE)

desktop-smoke:
	tools/desktop/smoke.sh $(PROFILE)

desktop-empty-smoke:
	tools/desktop/empty-workspace-smoke.sh $(PROFILE)

desktop-launch:
	tools/desktop/launch.sh $(PROFILE) $(LAUNCH_ARGS)

desktop-launch-fresh:
	tools/desktop/launch.sh $(PROFILE) --fresh $(LAUNCH_ARGS)

desktop-launch-detached:
	tools/desktop/launch.sh $(PROFILE) --detached $(LAUNCH_ARGS)

desktop-launch-smoke:
	tools/desktop/launch.sh $(PROFILE) --smoke-workspace $(LAUNCH_ARGS)

desktop-launch-smoke-fresh:
	tools/desktop/launch.sh $(PROFILE) --smoke-workspace --fresh $(LAUNCH_ARGS)

desktop-package:
	tools/desktop/package.sh $(PACKAGE_PROFILE)

desktop-package-smoke:
	tools/desktop/package-smoke.sh $(PACKAGE_PROFILE)

.PHONY: ci-rust ci-desktop ci-desktop-fast ci
ci-rust:
	tools/ci/rust-gates.sh $(ARGS)

ci-desktop:
	tools/desktop/ci-gates.sh $(PLATFORM)

ci-desktop-fast:
	CHAFT_DESKTOP_SKIP_PACKAGE=1 tools/desktop/ci-gates.sh $(PLATFORM)

ci:
	$(MAKE) ci-rust
	$(MAKE) ci-desktop

.PHONY: smoke-p2p smoke-access smoke-lifecycle smoke-visual screenshot-smoke release-metadata release-metadata-check release-metadata-smoke
smoke-p2p:
	tools/smoke/local-p2p.sh $(ARGS)

smoke-access:
	tools/smoke/access-transport.sh $(ARGS)

smoke-lifecycle:
	tools/smoke/workspace-lifecycle.sh $(ARGS)

smoke-visual:
	tools/smoke/visual-workspace.sh $(ARGS)

screenshot-smoke:
	tools/desktop/screenshot-smoke.sh $(PROFILE)

release-metadata:
	$(PYTHON) tools/desktop/release-metadata.py $(PACKAGE_PROFILE)

release-metadata-check:
	$(PYTHON) tools/desktop/verify-release-metadata.py $(PACKAGE_PROFILE) --platform $(PLATFORM)

release-metadata-smoke:
	tools/desktop/release-metadata-smoke.sh
