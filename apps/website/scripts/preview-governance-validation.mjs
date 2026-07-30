import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { parseYamlScalars } from "./governance-validation.mjs";
import { PREVIEW_SLOTS } from "./preview-slot-contract.mjs";

const EXPECTED_REPOSITORY = "Jurshsmith/chaft-infra";
const EXPECTED_ACCOUNT_ID = "1354b55cd97e5051f32727c8dad4399d";

function fail(message) {
  throw new Error(`preview governance validation failed: ${message}`);
}

function requireValue(values, path, expected) {
  if (!values.has(path)) fail(`missing ${path}`);
  const actual = values.get(path);
  if (actual !== expected) {
    fail(`${path} must be ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`);
  }
}

function previewRows(source) {
  const match = /^  previews:\n(?<body>(?:^    .*(?:\n|$))+)/m.exec(source);
  if (!match?.groups?.body) fail("missing spec.previews");

  const rows = [];
  let current = null;
  for (const line of match.groups.body.split(/\r?\n/)) {
    if (!line) continue;
    const first = /^    - branch: ([^\s]+)$/.exec(line);
    if (first) {
      if (current) rows.push(current);
      current = { branch: first[1] };
      continue;
    }
    const field = /^      ([a-z][A-Za-z0-9]*): ([^\s]+)$/.exec(line);
    if (!field || !current) fail(`invalid spec.previews row: ${line.trim()}`);
    if (Object.hasOwn(current, field[1])) {
      fail(`duplicate spec.previews field ${field[1]}`);
    }
    current[field[1]] = field[2];
  }
  if (current) rows.push(current);
  return rows;
}

export function validatePreviewGovernanceDocument(source) {
  const values = parseYamlScalars(source);
  const expectations = {
    apiVersion: "infra.chaft.dev/v1alpha1",
    kind: "StaticWebsitePreviews",
    "metadata.name": "chaft-previews",
    "metadata.lifecycle": "preview",
    "spec.provider.name": "cloudflare",
    "spec.provider.product": "workers-static-assets",
    "spec.provider.accountId": EXPECTED_ACCOUNT_ID,
    "spec.source.repository": "Jurshsmith/chaft",
    "spec.source.trustedBranch": "main",
    "spec.source.directory": "apps/website",
    "spec.deployment.system": "github-actions",
    "spec.deployment.previewEnabled": true,
    "spec.deployment.candidateWorkflow": ".github/workflows/website-preview.yml",
    "spec.deployment.deployWorkflow": ".github/workflows/deploy-website-preview.yml",
    "spec.deployment.resetWorkflow": ".github/workflows/reset-website-preview.yml",
    "spec.worker.configPath": "apps/website/wrangler.preview.jsonc",
    "spec.worker.wranglerVersion": "4.114.0",
    "spec.worker.compatibilityDate": "2026-07-26",
    "spec.worker.workersDev": false,
    "spec.worker.previewUrls": false,
    "spec.domain.zoneName": "chaft.ai",
    "spec.domain.authoritativeDns": "cloudflare",
    "spec.configuration.secretValuesStoredHere": false,
    "spec.governance.previewGateField": "spec.deployment.previewEnabled",
    "spec.ownership.governanceRepository": EXPECTED_REPOSITORY,
    "spec.ownership.governanceRepositoryVisibility": "private",
    "spec.ownership.executableRepository": "Jurshsmith/chaft",
  };

  for (const [path, expected] of Object.entries(expectations)) {
    requireValue(values, path, expected);
  }

  const foundationCommit = values.get("spec.source.foundationCommit");
  if (
    typeof foundationCommit !== "string" ||
    !/^[a-f0-9]{40}$/.test(foundationCommit)
  ) {
    fail("spec.source.foundationCommit must be a full lowercase SHA-1");
  }

  const actualRows = previewRows(source);
  const expectedRows = PREVIEW_SLOTS.map((row) => ({
    branch: row.branch,
    domain: row.domain,
    environment: row.environment,
    slot: row.slot,
    worker: row.worker,
  }));
  if (JSON.stringify(actualRows) !== JSON.stringify(expectedRows)) {
    fail("spec.previews must contain the four exact ordered preview slot identities");
  }

  for (const requiredText of [
    "CHAFT_PREVIEWS_INFRA_DEPLOY_KEY",
    "CLOUDFLARE_PREVIEW_API_TOKEN",
    "CLOUDFLARE_PREVIEW_READ_API_TOKEN",
  ]) {
    if (!source.includes(`- ${requiredText}`)) {
      fail(`required secret list is missing ${requiredText}`);
    }
  }

  for (const requiredText of [
    "CHAFT_PREVIEWS_INFRA_COMMIT",
    "CLOUDFLARE_ACCOUNT_ID",
  ]) {
    if (!source.includes(`- ${requiredText}`)) {
      fail(`required variable list is missing ${requiredText}`);
    }
  }

  return {
    accountId: EXPECTED_ACCOUNT_ID,
    foundationCommit,
    repository: EXPECTED_REPOSITORY,
    slots: expectedRows,
  };
}

function git(repositoryRoot, ...args) {
  return execFileSync("git", ["-C", repositoryRoot, ...args], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
}

export function validatePreviewGovernanceCheckout({
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
    ...validatePreviewGovernanceDocument(document),
    governanceCommit: head,
  };
}

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index === -1 || !process.argv[index + 1]) fail(`missing ${name}`);
  return process.argv[index + 1];
}

function main() {
  const result = validatePreviewGovernanceCheckout({
    componentPath: argument("--component"),
    expectedCommit: argument("--expected-commit"),
    repositoryRoot: argument("--repository-root"),
  });
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : "";
if (invokedPath === import.meta.url) main();
