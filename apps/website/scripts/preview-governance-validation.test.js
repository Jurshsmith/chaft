import { describe, expect, it } from "vitest";

import { validatePreviewGovernanceDocument } from "./preview-governance-validation.mjs";

function component(overrides = "") {
  return `apiVersion: infra.chaft.dev/v1alpha1
kind: StaticWebsitePreviews
metadata:
  name: chaft-previews
  lifecycle: preview
spec:
  provider:
    name: cloudflare
    product: workers-static-assets
    accountId: "1354b55cd97e5051f32727c8dad4399d"
  source:
    repository: Jurshsmith/chaft
    trustedBranch: main
    foundationCommit: 0123456789abcdef0123456789abcdef01234567
    directory: apps/website
  deployment:
    system: github-actions
    previewEnabled: true
    candidateWorkflow: .github/workflows/website-preview.yml
    deployWorkflow: .github/workflows/deploy-website-preview.yml
    resetWorkflow: .github/workflows/reset-website-preview.yml
  worker:
    configPath: apps/website/wrangler.preview.jsonc
    wranglerVersion: "4.114.0"
    compatibilityDate: "2026-07-26"
    workersDev: false
    previewUrls: false
  domain:
    zoneName: chaft.ai
    authoritativeDns: cloudflare
  configuration:
    requiredSecretNames:
      - CHAFT_PREVIEWS_INFRA_DEPLOY_KEY
      - CLOUDFLARE_PREVIEW_API_TOKEN
      - CLOUDFLARE_PREVIEW_READ_API_TOKEN
    requiredVariableNames:
      - CHAFT_PREVIEWS_INFRA_COMMIT
      - CLOUDFLARE_ACCOUNT_ID
    secretValuesStoredHere: false
  governance:
    previewGateField: spec.deployment.previewEnabled
  ownership:
    governanceRepository: Jurshsmith/chaft-infra
    governanceRepositoryVisibility: private
    executableRepository: Jurshsmith/chaft
  previews:
    - branch: preview/landing-hero-1
      domain: hero-1.chaft.ai
      environment: chaft-preview-hero-1
      slot: hero-1
      worker: chaft-website-hero-1
    - branch: preview/landing-hero-2
      domain: hero-2.chaft.ai
      environment: chaft-preview-hero-2
      slot: hero-2
      worker: chaft-website-hero-2
    - branch: preview/landing-hero-3
      domain: hero-3.chaft.ai
      environment: chaft-preview-hero-3
      slot: hero-3
      worker: chaft-website-hero-3
    - branch: preview/landing-hero-4
      domain: hero-4.chaft.ai
      environment: chaft-preview-hero-4
      slot: hero-4
      worker: chaft-website-hero-4
${overrides}`;
}

describe("Chaft Previews governance", () => {
  it("accepts only the exact enabled preview architecture", () => {
    expect(validatePreviewGovernanceDocument(component())).toEqual({
      accountId: "1354b55cd97e5051f32727c8dad4399d",
      foundationCommit: "0123456789abcdef0123456789abcdef01234567",
      repository: "Jurshsmith/chaft-infra",
      slots: [
        {
          branch: "preview/landing-hero-1",
          domain: "hero-1.chaft.ai",
          environment: "chaft-preview-hero-1",
          slot: "hero-1",
          worker: "chaft-website-hero-1",
        },
        {
          branch: "preview/landing-hero-2",
          domain: "hero-2.chaft.ai",
          environment: "chaft-preview-hero-2",
          slot: "hero-2",
          worker: "chaft-website-hero-2",
        },
        {
          branch: "preview/landing-hero-3",
          domain: "hero-3.chaft.ai",
          environment: "chaft-preview-hero-3",
          slot: "hero-3",
          worker: "chaft-website-hero-3",
        },
        {
          branch: "preview/landing-hero-4",
          domain: "hero-4.chaft.ai",
          environment: "chaft-preview-hero-4",
          slot: "hero-4",
          worker: "chaft-website-hero-4",
        },
      ],
    });
  });

  it("rejects a disabled preview gate", () => {
    expect(() =>
      validatePreviewGovernanceDocument(
        component().replace("previewEnabled: true", "previewEnabled: false"),
      ),
    ).toThrow(/previewEnabled/);
  });

  it("rejects a production Worker substitution", () => {
    expect(() =>
      validatePreviewGovernanceDocument(
        component().replace(
          "worker: chaft-website-hero-3",
          "worker: chaft-website",
        ),
      ),
    ).toThrow(/four exact ordered preview slot identities/);
  });

  it("rejects a missing preview-only secret", () => {
    expect(() =>
      validatePreviewGovernanceDocument(
        component().replace("      - CLOUDFLARE_PREVIEW_API_TOKEN\n", ""),
      ),
    ).toThrow(/required secret list/);
  });
});
