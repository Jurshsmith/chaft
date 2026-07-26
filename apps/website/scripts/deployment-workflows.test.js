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
    expect(source.match(/^\s*false &&\s*$/gm)).toHaveLength(1);
    expect(source).toContain("if: ${{ false && needs.preflight.result == 'success' }}");
  });

  it("keeps rollback manual, explicit, and hard-disabled", async () => {
    const source = await workflow("rollback-website.yml");
    expect(source).toContain("workflow_dispatch:");
    expect(source).toContain("target_version_id:");
    expect(source).toContain("expected_source_commit:");
    expect(source).toContain("inputs.confirmation == 'rollback chaft-website'");
    expect(source.match(/^\s*false &&\s*$/gm)).toHaveLength(1);
  });

  it("serializes deploy and rollback in the same non-cancelling production queue", async () => {
    for (const name of ["deploy-website.yml", "rollback-website.yml"]) {
      const source = await workflow(name);
      expect(source).toContain("group: chaft-website-production");
      expect(source).toContain("cancel-in-progress: false");
      expect(source).toContain("queue: max");
    }
  });
});
