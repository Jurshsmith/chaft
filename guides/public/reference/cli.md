---
title: Chaft CLI reference
description: Use the Chaft developer CLI, including safe identity and recovery passphrase input.
section: reference
order: 10
audience: contributors
status: canary
draft: false
---

# Chaft CLI reference

`chaft-cli` is a developer and recovery interface to the local-first runtime.
It can create and inspect local workspaces, exercise replication, manage
attachments and keys, produce recovery material, and create portable exports.
Its command surface is canary-stage and may change before a stable release.

Run commands from the repository root:

```sh
cargo run -p chaft-cli -- --data-dir ./scratch/cli device-id
```

The first `--` ends Cargo's options. Arguments after it belong to `chaft-cli`.
Use built-in help as the authoritative option list:

```sh
cargo run -p chaft-cli -- --help
cargo run -p chaft-cli -- export-portable-workspace --help
```

## Global paths

`--data-dir` selects the runtime directory. If omitted, it defaults to
`./data/chaft-cli`. Treat this directory as sensitive: it may contain the
device identity, local event database, search index, blobs, key material, and
replication state.

Use a separate data directory for every simulated device:

```sh
cargo run -p chaft-cli -- --data-dir ./scratch/alice device-id
cargo run -p chaft-cli -- --data-dir ./scratch/bob device-id
```

`--identity-file` overrides the identity path. Do not point two independent
devices at the same identity file.

## Create and inspect a workspace

Create a local workspace and retain the returned workspace ID:

```sh
cargo run -p chaft-cli -- --data-dir ./scratch/alice \
  init-workspace --name "Chaft Local" --channel general
```

Inspect local state:

```sh
cargo run -p chaft-cli -- --data-dir ./scratch/alice list-workspaces
cargo run -p chaft-cli -- --data-dir ./scratch/alice \
  snapshot --workspace-id <workspace-id> --decrypt
cargo run -p chaft-cli -- --data-dir ./scratch/alice \
  search-workspace --workspace-id <workspace-id> --query "meeting"
```

Send a message after obtaining a channel ID:

```sh
cargo run -p chaft-cli -- --data-dir ./scratch/alice \
  send-message --workspace-id <workspace-id> \
  --channel-id <channel-id> --text "hello"
```

For a complete multi-device flow, use the tested smoke script instead of
manually reproducing every transport step:

```sh
tools/smoke/local-p2p.sh --locked
```

## Supply passphrases safely

Never place an identity or recovery passphrase directly in a command argument.
Command arguments can appear in process listings, shell history, CI logs, or
diagnostic output. Environment variables are also not the recommended CLI
secret-delivery mechanism.

### Hidden terminal prompt

Recovery export and import prompt securely by default. Export asks for the
passphrase twice; import asks once:

```sh
cargo run -p chaft-cli -- --data-dir ./scratch/alice \
  export-recovery-bundle --workspace-id <workspace-id> \
  > ./scratch/recovery-bundle.json

cargo run -p chaft-cli -- --data-dir ./scratch/restored \
  import-recovery-bundle --bundle-file ./scratch/recovery-bundle.json
```

Unlock an encrypted identity through the terminal with:

```sh
cargo run -p chaft-cli -- \
  --identity-file ./scratch/device.json \
  --identity-passphrase-prompt \
  device-id
```

### Standard input for controlled automation

Connect an approved secret-manager command directly to the CLI and isolate both
processes and their logs:

```sh
approved-secret-command | cargo run -p chaft-cli -- \
  --data-dir ./scratch/alice \
  export-recovery-bundle --workspace-id <workspace-id> \
  --passphrase-stdin > ./scratch/recovery-bundle.json
```

`approved-secret-command` is a placeholder, not a Chaft executable. Replace it
with a trusted local secret-manager client that writes exactly one secret to
standard output without logging it.

Use `--identity-passphrase-stdin` for an identity passphrase. One invocation
cannot read both the identity and recovery passphrases from standard input; use
a prompt or an owner-only file for one of them.

Standard-input mode reads a bounded UTF-8 value until EOF and removes one final
line ending (`LF` or `CRLF`). Other leading, trailing, and embedded whitespace
remains significant.

### Owner-only files on Unix

File input is available on Unix when the file is regular, owned by the current
user, not a symlink, and grants no permissions to group or other users:

```sh
umask 077
approved-secret-command > ./scratch/recovery-passphrase
chmod 600 ./scratch/recovery-passphrase

cargo run -p chaft-cli -- --data-dir ./scratch/alice \
  export-recovery-bundle --workspace-id <workspace-id> \
  --passphrase-file ./scratch/recovery-passphrase \
  > ./scratch/recovery-bundle.json
```

Use `--identity-passphrase-file` for an encrypted identity. Secure file
validation is not implemented on Windows; use the hidden prompt or controlled
standard input there. Delete temporary passphrase files as soon as the
authorized workflow no longer needs them.

Passphrases must be nonblank UTF-8 and no larger than 16 KiB. File input removes
one final line ending and preserves all other whitespace.

## Recovery bundles and raw key exports

`export-recovery-bundle` creates one passphrase-encrypted JSON document
containing the current workspace key ring and locally available private-channel
key rings. `import-recovery-bundle` installs those content keys, but it does not
grant membership or restore the exporting device identity.

Raw `export-workspace-key` and `export-channel-key` output is unencrypted key
material. Prefer a recovery bundle for human-managed transfer. If a development
workflow requires a raw export, write it only to a protected local file, send
it through an authenticated private channel, and remove it after import. Never
commit key exports or recovery bundles.

## Portable plaintext export

Create a user-readable ZIP for offline review or migration:

```sh
cargo run -p chaft-cli -- --data-dir ./scratch/alice \
  export-portable-workspace --workspace-id <workspace-id> \
  --output ./scratch/chaft-workspace.zip
```

This archive is plaintext, contains only the selected workspace's currently
readable local state, and is not a Chaft backup. It cannot restore identity,
authorization, or keys. Read the
[portable workspace export reference](portable-workspace-export.md) before
sharing it.

## Output and errors

Most successful commands write a stable JSON object or identifier to standard
output. Warnings and errors go to standard error, so JSON output can be
redirected without mixing in diagnostics.

Paths, identifiers, input files, peer lists, metadata, and passphrases are
bounded before expensive or state-changing work. Imported events, peer data,
and artifacts remain untrusted until their relevant validation succeeds.

See the [testing guide](../development/testing.md) for smoke coverage and the
[Security Policy](https://github.com/Jurshsmith/chaft/blob/main/SECURITY.md) for
reporting vulnerabilities.
