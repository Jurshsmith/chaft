# Chaft

Chaft is experimental, open-source team chat for small teams. It keeps workspace
history on participating devices, encrypts message and attachment content before
replication, and syncs signed history directly between trusted peers.

Participating devices provide most storage, compute, and replication, reducing
recurring infrastructure costs while making the model's tradeoffs explicit.

> **Warning**
>
> Chaft is canary-stage software. Published canary packages are deliberately
> unsigned and must not be used for sensitive or production communication.
> See the [download page](https://chaft.ai/download/) and [Security Policy](SECURITY.md).

## Project status

Chaft is in canary development:

- The Rust runtime, CLI, replica node, Qt/QML desktop application, and static website are implemented in this repository.
- CI builds and exercises desktop packages on Windows x86-64, macOS Intel, macOS Apple Silicon, and Linux x86-64.
- Those CI packages are development artifacts, not public releases.
- A dedicated publisher can create one immutable, versioned GitHub prerelease with packages for all four targets. The website advertises it only after a separate reviewed manifest change.
- Canary receipts explicitly record that signing and notarization were not performed. Stable releases retain their stronger signing and notarization gates.
- The public website is deployed with Cloudflare Workers Static Assets. `https://chaft.ai` is the canonical hostname; deployment automation and provenance remain subject to the reviewed infrastructure gates.
- Peer connectivity uses explicit endpoints today. Production public relay, global discovery, guaranteed delivery, and an availability SLA are not provided.
- Security-sensitive behavior is tested, but production key custody, signing, notarization, updates, and operational review are not complete.

Useful starting points:

- [Chaft website](https://chaft.ai)
- [Public guides](guides/public/index.md)
- [Security Policy](SECURITY.md)
- [Contributing](CONTRIBUTING.md)
- [Website development and deployment foundation](apps/website/README.md)
- [GitHub Releases](https://github.com/Jurshsmith/chaft/releases)

## Free within deliberate limits

Chaft's goal is to keep essential collaboration free for small teams without making a required central application service the authority for their workspace. That goal is bounded by the current design:

- each device supplies its own local compute and storage;
- peers supply most synchronization bandwidth and must be reachable to exchange new history;
- optional encrypted replicas can improve availability but are not an uptime guarantee or a plaintext authority;
- public relay, automatic internet-wide discovery, managed retention, and production support are not included today; and
- canary builds are for evaluation with non-sensitive data, not production operations.

“Free” here describes the product goal and the AGPL-licensed software, not a claim that hosting, signing, relay capacity, or every future optional service has zero cost. Any hosted service added later must publish its limits without making the local-first core dependent on it.

## Test the canary

Use a package only when the [download page](https://chaft.ai/download/) marks
the exact canary version and platform as available. Otherwise, build the
desktop application from source on a supported Windows, macOS, or Linux host.

1. Follow [Build the desktop app](guides/public/getting-started/build-desktop.md).
2. Use a fresh profile and non-sensitive test data.
3. Exercise workspace creation, invitations, and synchronization between at least two isolated devices.
4. [Open an issue](https://github.com/Jurshsmith/chaft/issues/new) with the operating system, source revision, reproduction steps, and sanitized logs. Never include keys, passphrases, recovery material, or private messages.

Contributors can start with [CONTRIBUTING.md](CONTRIBUTING.md) and the [testing guide](guides/public/development/testing.md).

## What Chaft is building

Chaft treats each device as an independent participant:

- Events are signed by persistent device identities.
- Writes are stored locally before replication.
- Materialized views are rebuilt from verified append-only history.
- Missing parents or authorization context remain visible as history gaps.
- Public-channel content uses a workspace key; private-channel content uses a
  channel-scoped key.
- OpenMLS group state can provide epoch-derived content keys, with manual key
  rings retained as a compatibility and recovery bridge.
- Peers and optional replica nodes are untrusted. Replicas receive encrypted,
  bounded event and blob data rather than plaintext authority.
- SQLite, FTS5, and content-addressed blob storage keep the local app usable
  without a continuously reachable service.

Direct TCP and native Iroh transports support local and explicit peer connections
today. Public relay and discovery remain default-deny. No central application
server is authoritative for workspace history; optional replicas may improve
availability.

Read the [architecture](guides/public/concepts/architecture.md),
[security model](guides/public/concepts/security-model.md), and
[networking guide](guides/public/concepts/networking-and-replication.md) for the
current boundaries.

## Repository map

```text
application/view-model/  Rust view-model projection for desktop surfaces
apps/chaft-cli/          Developer and recovery CLI
apps/chaft-node/         Headless encrypted replica node
apps/desktop-qt/         Qt 6 and QML desktop application
apps/website/            Fully static Astro website
bindings/ffi/            Rust-to-desktop FFI boundary
domain/                  Event and domain contracts
network/                 Transport, wire, sync, direct TCP, and Iroh crates
runtime/                 Local application runtime and materialization
security/                Cryptography, identity, and OpenMLS integration
storage/                 Event store, search, and media/blob storage
guides/public/           Canonical public documentation
tools/                   CI, desktop, smoke, release, and validation tooling
```

The Rust workspace uses edition 2024 and requires Rust 1.97.1 or newer.

## Build the Rust workspace

Install Rust 1.97.1 or newer, then run:

```sh
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
```

The repository gate adds formatting and Clippy:

```sh
tools/ci/rust-gates.sh --locked
```

When dependencies are already cached, `--offline` is supported:

```sh
tools/ci/rust-gates.sh --offline
```

## Run the developer CLI

The CLI is useful for inspecting the runtime without the desktop shell:

```sh
cargo run -p chaft-cli -- --data-dir ./scratch/cli device-id
cargo run -p chaft-cli -- --data-dir ./scratch/cli \
  init-workspace --name "Chaft Local" --channel general
cargo run -p chaft-cli -- --data-dir ./scratch/cli list-workspaces
```

Recovery export and import use a hidden terminal prompt by default. Encrypted
identity files support `--identity-passphrase-prompt`. Controlled automation
can use the documented standard-input or owner-only file modes; never place
passphrases directly in command arguments or environment variables. See the
[CLI reference](guides/public/reference/cli.md).

## Build and run the desktop app

Official package and release-input development requires Chaft's verified, patched
Qt 6.8.4 SDK for the exact native target, CMake 3.28+, Ninja, and Rust 1.97.1+.

Confirm the local toolchain:

```sh
tools/desktop/preflight.sh
```

Technical macOS testers can instead run `tools/macos/build-local.sh` from a fixed
checkout on Intel or Apple Silicon. It uses supported Homebrew Qt, verifies the
native app, applies only a local ad-hoc signature, and installs `~/Applications/Chaft.app`.
It is not Developer ID signed or notarized. Read
[Build Chaft Desktop](guides/public/getting-started/build-desktop.md) first.

Build and launch a fresh development profile:

```sh
tools/desktop/build.sh debug
tools/desktop/launch.sh debug --fresh
```

Rerun without `--fresh` to preserve the local identity and workspace data:

```sh
tools/desktop/launch.sh debug
```

Run multiple isolated local devices:

```sh
make dev-users N=3 FRESH=1
```

Each named instance receives separate identity, key, event, search, blob,
settings, and log paths. Do not copy `device.json` when simulating independent
devices.

More detail is available in
[Build the desktop app](guides/public/getting-started/build-desktop.md) and
[Workspace lifecycle](guides/public/getting-started/workspace-lifecycle.md).

## Test desktop and replication changes

Run the local encrypted replication smoke test after changing runtime, sync,
network, replica, or CLI behavior:

```sh
tools/smoke/local-p2p.sh --locked
```

Desktop changes should run:

```sh
tools/desktop/preflight.sh
tools/desktop/qml-lint.sh
tools/desktop/build.sh debug
tools/desktop/smoke.sh debug
```

The full expectations and focused test commands are in the
[testing guide](guides/public/development/testing.md).

## Run the website

The website requires Node.js 22.13+ and the pnpm version pinned in `apps/website/package.json`.

```sh
corepack enable
make website-install
make website-dev
```

Run its complete validation gate with:

```sh
make website-validate
```

A production build requires a complete HTTPS `SITE_URL`. Root-domain and
path-prefixed deployments are both supported:

```sh
SITE_URL=https://example.com make website-build
SITE_URL=https://example.com/chaft make website-build
```

The checked-in Wrangler configuration is asset-only. Canonical production
builds use `SITE_URL=https://chaft.ai`; preview URLs remain disabled. Deploy and
rollback workflows remain protected by reviewed infrastructure gates so
validation and candidate creation cannot mutate Cloudflare unexpectedly.

## Packages and downloads

The desktop CI matrix builds:

- Windows x86-64 packages
- macOS Intel (x86-64) packages
- macOS Apple Silicon (arm64) packages
- Linux x86-64 AppImage packages

CI artifacts expire under GitHub Actions retention and are not a supported distribution channel.

Public downloads are versioned, immutable assets attached to
[GitHub Releases](https://github.com/Jurshsmith/chaft/releases). A canary is a
GitHub prerelease and never the repository's stable or latest release. It is
not advertised by the website until its packages, checksums, SBOMs, provenance,
native smoke receipts, release inventory, and corresponding-source evidence
pass the canary promotion workflow and the resulting manifest change is
reviewed.

See the [release process](guides/public/development/release-process.md) for the
canary and stable publication boundaries.

## Documentation

`guides/public/` is the canonical source for user-facing and contributor-facing
guides. Those Markdown files remain readable on GitHub and are compiled into
the static website's `/docs/` routes.

Public guide changes must:

- include the required front matter;
- contain exactly one level-one heading;
- use repository-relative Markdown links;
- distinguish implemented behavior from plans;
- avoid private operational references and secret-bearing examples.

Engineering notes outside `guides/public/` may describe lower-level design, but
they are not part of the public navigation contract.

## Security

Treat every peer, replica, imported artifact, and signed event as untrusted
until validation succeeds. Keep runtime identity files, recovery bundles,
private keys, databases, and passphrase files out of Git.

Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md). Do
not open a public issue for key exposure, plaintext leakage, signature or authorization bypass, unsafe parsing, or denial-of-service findings.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request. Keep
changes focused, add tests at the affected boundary, and report the exact
validation commands run.

Pull requests should call out security, storage, wire-format, migration, release, and UI-thread implications where relevant.

## License

Chaft is licensed under the [GNU Affero General Public License v3.0 or later](LICENSE).
