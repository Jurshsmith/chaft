import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const SOURCE_COMMIT = /^(?:[a-f0-9]{40}|[a-f0-9]{64})$/;
const VERSION_ID = /^[a-f0-9-]{32,64}$/i;

function fail(message) {
  throw new Error(`preview slot contract failed: ${message}`);
}

const rows = [1, 2, 3, 4].map((number) => {
  const slot = `hero-${number}`;
  return Object.freeze({
    branch: `preview/landing-hero-${number}`,
    domain: `${slot}.chaft.ai`,
    environment: `chaft-preview-${slot}`,
    siteUrl: `https://${slot}.chaft.ai`,
    slot,
    worker: `chaft-website-${slot}`,
    wranglerEnvironment: slot,
  });
});

export const PREVIEW_SLOTS = Object.freeze(rows);

function exactRow(matches, label, value) {
  if (matches.length !== 1) {
    fail(`${label} is not in the exact Chaft Previews allowlist: ${value}`);
  }
  return matches[0];
}

export function previewSlotForBranch(branch) {
  if (typeof branch !== "string" || branch.length === 0) {
    fail("branch must be a non-empty string");
  }
  return exactRow(
    PREVIEW_SLOTS.filter((row) => row.branch === branch),
    "branch",
    branch,
  );
}

export function previewSlotForName(slot) {
  if (typeof slot !== "string" || slot.length === 0) {
    fail("slot must be a non-empty string");
  }
  return exactRow(
    PREVIEW_SLOTS.filter((row) => row.slot === slot),
    "slot",
    slot,
  );
}

export function previewArtifactName(slot, sourceCommit) {
  const row = previewSlotForName(slot);
  if (typeof sourceCommit !== "string" || !SOURCE_COMMIT.test(sourceCommit)) {
    fail("source commit must be a lowercase full SHA-1 or SHA-256 revision");
  }
  return `chaft-preview-${row.slot}-${sourceCommit}`;
}

export function previewDeploymentRecordName(slot, versionId) {
  const row = previewSlotForName(slot);
  if (typeof versionId !== "string" || !VERSION_ID.test(versionId)) {
    fail("Worker version ID is malformed");
  }
  return `chaft-preview-deployment-${row.slot}-${versionId}`;
}

export function assertPreviewIdentity({
  branch,
  domain,
  environment,
  siteUrl,
  slot,
  worker,
  wranglerEnvironment,
}) {
  const expected = previewSlotForBranch(branch);
  const actual = {
    branch,
    domain,
    environment,
    siteUrl,
    slot,
    worker,
    wranglerEnvironment,
  };
  for (const [key, value] of Object.entries(expected)) {
    if (actual[key] !== value) {
      fail(`${key} must be ${value}, received ${actual[key]}`);
    }
  }
  return expected;
}

function argumentsFromCommandLine(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith("--") || value === undefined || value.startsWith("--")) {
      fail(`invalid command-line argument near ${name ?? "<end>"}`);
    }
    if (values.has(name)) fail(`duplicate command-line argument ${name}`);
    values.set(name, value);
  }
  return values;
}

function requireArgument(values, name) {
  const value = values.get(name);
  if (!value) fail(`missing ${name}`);
  return value;
}

function rejectUnexpectedArguments(values, expected) {
  for (const name of values.keys()) {
    if (!expected.has(name)) fail(`unexpected argument ${name}`);
  }
}

function main() {
  const command = process.argv[2];
  const values = argumentsFromCommandLine(process.argv.slice(3));
  let result;

  if (command === "resolve-branch") {
    rejectUnexpectedArguments(values, new Set(["--branch", "--commit"]));
    const row = previewSlotForBranch(requireArgument(values, "--branch"));
    const commit = values.get("--commit");
    result = {
      ...row,
      ...(commit ? { artifactName: previewArtifactName(row.slot, commit) } : {}),
    };
  } else if (command === "resolve-slot") {
    rejectUnexpectedArguments(values, new Set(["--slot"]));
    result = previewSlotForName(requireArgument(values, "--slot"));
  } else if (command === "verify") {
    const expected = new Set([
      "--branch",
      "--domain",
      "--environment",
      "--site-url",
      "--slot",
      "--worker",
      "--wrangler-environment",
    ]);
    rejectUnexpectedArguments(values, expected);
    result = assertPreviewIdentity({
      branch: requireArgument(values, "--branch"),
      domain: requireArgument(values, "--domain"),
      environment: requireArgument(values, "--environment"),
      siteUrl: requireArgument(values, "--site-url"),
      slot: requireArgument(values, "--slot"),
      worker: requireArgument(values, "--worker"),
      wranglerEnvironment: requireArgument(values, "--wrangler-environment"),
    });
  } else {
    fail("command must be resolve-branch, resolve-slot, or verify");
  }

  process.stdout.write(`${JSON.stringify(result)}\n`);
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : "";
if (invokedPath === import.meta.url) main();
