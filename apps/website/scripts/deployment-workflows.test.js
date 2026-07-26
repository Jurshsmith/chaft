import { readFile } from "node:fs/promises";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

const workflows = join(process.cwd(), "..", "..", ".github", "workflows");

async function workflow(name) {
  return readFile(join(workflows, name), "utf8");
}

describe("website deployment workflow safety gates", () => {
  it("keeps the candidate workflow credential-free and domain-optional on main", async () => {
    const source = await workflow("website.yml");
    expect(source).not.toContain("secrets.CLOUDFLARE");
    expect(source).toContain("vars.WEBSITE_SITE_URL != ''");
    expect(source).toContain("vars.WEBSITE_SITE_URL == ''");
    expect(source).toContain("node scripts/deployment-artifact-cli.mjs create");
    expect(source).toContain("node scripts/deployment-artifact-cli.mjs verify");
  });

  it("allows deploy candidates only from successful same-repository main runs", async () => {
    const source = await workflow("deploy-website.yml");
    expect(source).toContain("workflow_run:");
    expect(source).not.toContain("workflow_dispatch:");
    expect(source).toContain("github.event.workflow_run.conclusion == 'success'");
    expect(source).toContain("github.event.workflow_run.event == 'push'");
    expect(source).toContain("github.event.workflow_run.head_branch == 'main'");
    expect(source).toContain(
      "github.event.workflow_run.head_repository.full_name == github.repository",
    );
    expect(source).not.toMatch(/^\s*false &&\s*$/gm);
    expect(source).toContain("if: ${{ needs.preflight.result == 'success' }}");
    expect(source).toContain("/actions/runs/${SOURCE_RUN_ID}/artifacts?name=");
    expect(source).toContain("artifact.workflow_run?.head_sha");
    expect(source).toContain("/^sha256:[a-f0-9]{64}$/");
  });

  it("keeps rollback manual and binds it to explicit incident state", async () => {
    const source = await workflow("rollback-website.yml");
    expect(source).toContain("workflow_dispatch:");
    expect(source).toContain("target_version_id:");
    expect(source).toContain("expected_source_commit:");
    expect(source).toContain("failed_version_id:");
    expect(source).toContain("incident_id:");
    expect(source).toContain("inputs.confirmation == 'rollback chaft-website'");
    expect(source).not.toMatch(/^\s*false &&\s*$/gm);
    expect(source).toContain("chaft-website-deployment-${EXPECTED_SOURCE_COMMIT}");
    expect(source).toContain("retained deployment record does not authorize");
  });

  it("serializes deploy and rollback in the same non-cancelling production queue", async () => {
    for (const name of ["deploy-website.yml", "rollback-website.yml"]) {
      const source = await workflow(name);
      expect(source).toContain("group: chaft-website-production");
      expect(source).toContain("cancel-in-progress: false");
      expect(source).not.toContain("queue: max");
    }
  });

  it("proves private governance before either production mutation", async () => {
    for (const name of ["deploy-website.yml", "rollback-website.yml"]) {
      const source = await workflow(name);
      expect(source).toContain("repository: Jurshsmith/chaft-infra");
      expect(source).toContain("ssh-key: ${{ secrets.CHAFT_INFRA_DEPLOY_KEY }}");
      expect(source).toContain("node apps/website/scripts/governance-validation.mjs");
      expect(source).toContain("--expected-commit");
    }
  });

  it("captures Cloudflare state and retains structured evidence", async () => {
    const deploy = await workflow("deploy-website.yml");
    expect(deploy).toContain("deployment-before.json");
    expect(deploy).toContain("/workers/domains?service=chaft-website");
    expect(deploy).toContain("domains-after.json");
    expect(deploy).toContain("node scripts/verify-public-deployment.mjs");
    expect(deploy).toContain('"chaft-website-deployment-record"');
    expect(deploy).toContain("retention-days: 90");

    const rollback = await workflow("rollback-website.yml");
    expect(rollback).toContain("deployment-before.json");
    expect(rollback).toContain("domains-after.json");
    expect(rollback).toContain("node scripts/verify-public-deployment.mjs");
    expect(rollback).toContain('"chaft-website-rollback-record"');
    expect(rollback).toContain("retention-days: 90");
  });
});
