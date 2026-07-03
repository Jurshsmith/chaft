# Contributing

Chaft is a native, local-first, peer-to-peer desktop chat workspace. The public
repository should contain source, build, test, packaging, and public security
documentation only. Product strategy, design research, local runtime data, and
developer secrets belong outside this repository.

## Repository Boundaries

- Keep private planning docs outside the repo. The local parent directory may
  contain `docs/`, `design/`, `scratch/`, `secrets/`, and `context/`.
- Do not commit generated local runtime databases, identity files, key material,
  recovery bundles, build output, or local peer data.
- Public docs should describe protocol invariants, security properties, and
  developer workflows without exposing private product strategy.

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

The smoke script builds the CLI and headless node, creates an invited two-device
workspace, backs up encrypted partial content to a local replica node, verifies
missing-history gaps from that partial backup, publishes full history, pulls it
to the second runtime, imports the workspace key, and checks decrypted search.

Desktop work should also run:

```sh
tools/desktop/preflight.sh
tools/desktop/build.sh debug
tools/desktop/smoke.sh debug
```

The desktop scripts discover common Homebrew Qt locations and configure/build
`apps/desktop-qt` through the root CMake presets. The smoke script creates the
same visual smoke runtime used by `tools/smoke/visual-workspace.sh`, launches
the Qt shell with the built FFI library, and waits for runtime-backed message
hydration. Use `tools/desktop/screenshot-smoke.sh debug` when UI work needs a
local screenshot artifact; it runs the same smoke and verifies the captured PNG
is nonblank.

Use `tools/desktop/package.sh release` when you need a local distributable
desktop artifact. The script installs the app, bundles the Rust FFI dynamic
library, and writes platform packages under `build/desktop-release/package`.
Use `tools/desktop/package-smoke.sh release` to also launch the installed app
without `CHAFT_FFI_LIBRARY` and verify it can load the bundled Rust runtime.

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
