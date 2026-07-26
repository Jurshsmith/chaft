import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const EXPECTED_REPOSITORY = "Jurshsmith/chaft-infra";
const EXPECTED_ACCOUNT_ID = "1354b55cd97e5051f32727c8dad4399d";
const EXPECTED_SITE_URL = "https://chaft.ai";

function fail(message) {
  throw new Error(`governance validation failed: ${message}`);
}

function parseScalar(value) {
  const trimmed = value.trim();
  if (trimmed === "null" || trimmed === "~") return null;
  if (trimmed === "true") return true;
  if (trimmed === "false") return false;
  if (
    (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
    (trimmed.startsWith("'") && trimmed.endsWith("'"))
  ) {
    return trimmed.slice(1, -1);
  }
  return trimmed;
}

export function parseYamlScalars(source) {
  const values = new Map();
  const stack = [];

  for (const rawLine of source.split(/\r?\n/)) {
    if (!rawLine.trim() || rawLine.trimStart().startsWith("#")) continue;
    if (rawLine.includes("\t")) fail("component.yaml must not contain tabs");

    const indent = rawLine.length - rawLine.trimStart().length;
    const line = rawLine.trim();
    if (line.startsWith("- ")) continue;

    const match = /^([A-Za-z][A-Za-z0-9]*):(?:\s+(.*))?$/.exec(line);
    if (!match) continue;

    while (stack.length > 0 && stack.at(-1).indent >= indent) stack.pop();
    const key = match[1];
    const rawValue = match[2];
    const path = [...stack.map((entry) => entry.key), key].join(".");

    if (rawValue === undefined || rawValue === "") {
      stack.push({ indent, key });
    } else {
      values.set(path, parseScalar(rawValue));
    }
  }

  return values;
}

function requireValue(values, path, expected) {
  if (!values.has(path)) fail(`missing ${path}`);
  const actual = values.get(path);
  if (actual !== expected) {
    fail(`${path} must be ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`);
  }
}

export function validateGovernanceDocument(source) {
  const values = parseYamlScalars(source);
  const expectations = {
    "apiVersion": "infra.chaft.dev/v1alpha1",
    "kind": "StaticWebsite",
    "metadata.name": "chaft-website",
    "metadata.lifecycle": "production",
    "spec.provider.name": "cloudflare",
    "spec.provider.product": "workers-static-assets",
    "spec.provider.accountId": EXPECTED_ACCOUNT_ID,
    "spec.source.repository": "Jurshsmith/chaft",
    "spec.source.branch": "main",
    "spec.source.directory": "apps/website",
    "spec.deployment.system": "github-actions",
    "spec.deployment.productionEnabled": true,
    "spec.deployment.productionBranch": "main",
    "spec.worker.name": "chaft-website",
    "spec.worker.configPath": "apps/website/wrangler.jsonc",
    "spec.worker.wranglerVersion": "4.114.0",
    "spec.worker.compatibilityDate": "2026-07-26",
    "spec.worker.workersDev": false,
    "spec.worker.previewUrls": false,
    "spec.domain.decisionStatus": "resolved",
    "spec.domain.zoneName": "chaft.ai",
    "spec.domain.canonicalHostname": "chaft.ai",
    "spec.domain.alternateHostname": "www.chaft.ai",
    "spec.configuration.secretValuesStoredHere": false,
    "spec.governance.productionGateField": "spec.deployment.productionEnabled",
    "spec.ownership.governanceRepository": "Jurshsmith/chaft-infra",
    "spec.ownership.governanceRepositoryVisibility": "private",
    "spec.ownership.executableRepository": "Jurshsmith/chaft",
  };

  for (const [path, expected] of Object.entries(expectations)) {
    requireValue(values, path, expected);
  }

  for (const requiredText of [
    "CHAFT_INFRA_DEPLOY_KEY",
    "CLOUDFLARE_API_TOKEN",
    "CLOUDFLARE_READ_API_TOKEN",
  ]) {
    if (!source.includes(`- ${requiredText}`)) {
      fail(`required secret list is missing ${requiredText}`);
    }
  }

  return {
    accountId: EXPECTED_ACCOUNT_ID,
    canonicalOrigin: EXPECTED_SITE_URL,
    repository: EXPECTED_REPOSITORY,
    worker: "chaft-website",
  };
}

function git(repositoryRoot, ...args) {
  return execFileSync("git", ["-C", repositoryRoot, ...args], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
}

export function validateGovernanceCheckout({
  componentPath,
  expectedCommit,
  repositoryRoot,
}) {
  if (!/^[a-f0-9]{40}$/.test(expectedCommit)) {
    fail("expected governance commit must be a full lowercase SHA-1");
  }

  const root = resolve(repositoryRoot);
  const head = git(root, "rev-parse", "HEAD");
  if (head !== expectedCommit) {
    fail(`checked-out main ${head} does not equal pinned commit ${expectedCommit}`);
  }

  const remote = git(root, "config", "--get", "remote.origin.url");
  if (
    remote !== "git@github.com:Jurshsmith/chaft-infra.git" &&
    remote !== "https://github.com/Jurshsmith/chaft-infra.git"
  ) {
    fail(`unexpected governance remote ${remote}`);
  }

  const document = readFileSync(resolve(componentPath), "utf8");
  return {
    ...validateGovernanceDocument(document),
    governanceCommit: head,
  };
}

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index === -1 || !process.argv[index + 1]) fail(`missing ${name}`);
  return process.argv[index + 1];
}

function main() {
  const result = validateGovernanceCheckout({
    componentPath: argument("--component"),
    expectedCommit: argument("--expected-commit"),
    repositoryRoot: argument("--repository-root"),
  });
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : "";
if (invokedPath === import.meta.url) main();
