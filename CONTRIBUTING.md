# Contributing

Chaft is a native, local-first, peer-to-peer desktop chat workspace. The public
repository should contain source, build, test, packaging, and public security
documentation only. Product strategy, design research, local runtime data, and
developer secrets belong outside this repository.

## Repository Boundaries

- Keep private planning, credentials, operational state, and unpublished
  security material outside this public repository.
- Do not commit generated local runtime databases, identity files, key material,
  recovery bundles, build output, or local peer data.
- `guides/public/` is the canonical source for public user and contributor
  documentation. Public docs must not depend on or link to private operational
  repositories.

## Required Gates

Run the Rust workspace gates before opening a pull request:

```sh
tools/ci/rust-gates.sh --locked
```

For a local offline check after dependencies are already cached:

```sh
tools/ci/rust-gates.sh --offline
```

Run the local P2P smoke when changing runtime, sync, networking, replica-node,
or CLI behavior:

```sh
tools/smoke/local-p2p.sh --offline
```

Run the visual workspace smoke when changing snapshot materialization, desktop
hydration, timeline rendering assumptions, search, attachments, reactions, or
channel navigation:

```sh
tools/smoke/visual-workspace.sh --offline
```

The script runs:

- `cargo fmt --all --check`
- `cargo check --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`

When changing `guides/public/`, `apps/website/`, website workflows, release
metadata, or public navigation, also run:

```sh
make website-validate
```

The website gate validates guide metadata, routes, repository-relative links,
heading fragments, navigation order, both root and path-prefixed builds, the
sitemap, deployment safety invariants, and the static Wrangler dry run.

The smoke script builds the CLI and headless node, creates an invited two-device
workspace, backs up encrypted partial content to a local replica node, verifies
missing-history gaps from that partial backup, publishes full history, pulls it
to the second runtime, imports the workspace key, and checks decrypted search.

Desktop work should also run:

```sh
tools/desktop/preflight.sh
tools/desktop/qml-lint.sh
tools/desktop/build.sh debug
tools/desktop/smoke.sh debug
```

The desktop scripts discover common Homebrew Qt locations and configure/build
`apps/desktop-qt` through the root CMake presets. The smoke script creates the
same visual smoke runtime used by `tools/smoke/visual-workspace.sh`, launches
the Qt shell with the built FFI library, and waits for runtime-backed message
hydration. The QML lint script also verifies that every QML source is listed in
the Qt module so release packages do not depend on source-tree imports. Use
`tools/desktop/screenshot-smoke.sh debug` when UI work needs a local screenshot
artifact; it runs the same smoke and verifies the captured PNG is nonblank.

Use `tools/desktop/package.sh release` when you need a local distributable
desktop artifact. The script installs the app, bundles the Rust FFI dynamic
library, and writes platform packages under `build/desktop-release/package`.
Use `tools/desktop/package-smoke.sh release` to also launch the installed app
without `CHAFT_FFI_LIBRARY` and verify it can load the bundled Rust runtime and
packaged QML components for reply rows.
Generate and verify release metadata before sharing a package:

```sh
python3 tools/desktop/release-metadata.py release
python3 tools/desktop/verify-release-metadata.py release --platform Linux
```

Use `--platform macOS` or `--platform Windows` when checking those package
directories from a non-CI machine; the verifier rejects package suffixes that do
not match the target OS.

When changing hot runtime, sync, blob, search, snapshot, or FFI paths, also
compile the public benchmark target:

```sh
cargo bench -p chaft-benchmarks --bench hot_paths --no-run
```

For local performance investigation, run Criterion samples explicitly:

```sh
cargo bench -p chaft-benchmarks --bench hot_paths -- --sample-size 10
```

FFI JSON response shape is guarded by
`bindings/ffi/ffi-json-contract.snapshot.json`. Update that snapshot only
with intentional desktop API changes and keep the focused `chaft-ffi` tests
green.

## Public Documentation Rules

Every Markdown file under `guides/public/` must include:

```yaml
---
title: A concise page title
description: A one-sentence summary
section: getting-started
order: 1
audience: users
status: preview
draft: false
---
```

Use the section and audience values already established by neighboring guides.
Each guide must contain exactly one level-one heading. Keep page IDs stable by
moving files deliberately, use repository-relative `.md` links so GitHub
navigation works, and let the website transform those links for the deployed
base path.

Documentation must distinguish current behavior from planned work. Do not
publish passphrases in arguments, secret-bearing environment-variable examples,
private infrastructure names, internal URLs, or claims that development CI
artifacts are supported public downloads.

## Engineering Rules

- Keep the Rust core deterministic, bounded, and testable without a central
  server.
- Treat every peer, replica node, and imported event as untrusted until
  verified.
- Keep network, file IO, encryption, search, and snapshot hydration off the UI
  thread.
- Preserve local-first behavior: writes should update local state first, then
  sync.
- Add tests near the crate boundary affected by the change. Broaden tests when
  changing shared event, crypto, sync, runtime, FFI, or desktop contracts.

## Pull Request Shape

Pull requests should include:

- A short summary of behavior changed.
- The validation commands run.
- Any security, storage, wire-format, migration, or UI-thread implications.
- Screenshots or short notes for desktop UI changes when relevant.
