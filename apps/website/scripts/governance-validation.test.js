import { describe, expect, it } from "vitest";

import {
  parseYamlScalars,
  validateGovernanceDocument,
} from "./governance-validation.mjs";

function component(overrides = "") {
  return `apiVersion: infra.chaft.dev/v1alpha1
kind: StaticWebsite
metadata:
  name: chaft-website
  lifecycle: production
spec:
  provider:
    name: cloudflare
    product: workers-static-assets
    accountId: "1354b55cd97e5051f32727c8dad4399d"
  source:
    repository: Jurshsmith/chaft
    branch: main
    directory: apps/website
  deployment:
    system: github-actions
    productionEnabled: true
    productionBranch: main
  worker:
    name: chaft-website
    configPath: apps/website/wrangler.jsonc
    wranglerVersion: "4.114.0"
    compatibilityDate: "2026-07-26"
    workersDev: false
    previewUrls: false
  domain:
    decisionStatus: resolved
    zoneName: chaft.ai
    canonicalHostname: chaft.ai
    alternateHostname: www.chaft.ai
  configuration:
    requiredSecretNames:
      - CHAFT_INFRA_DEPLOY_KEY
      - CLOUDFLARE_API_TOKEN
      - CLOUDFLARE_READ_API_TOKEN
    secretValuesStoredHere: false
  governance:
    productionGateField: spec.deployment.productionEnabled
  ownership:
    governanceRepository: Jurshsmith/chaft-infra
    governanceRepositoryVisibility: private
    executableRepository: Jurshsmith/chaft
${overrides}`;
}

describe("website deployment governance", () => {
  it("reads the scalar paths used by the fail-closed gate", () => {
    const values = parseYamlScalars(component());
    expect(values.get("spec.deployment.productionEnabled")).toBe(true);
    expect(values.get("spec.domain.canonicalHostname")).toBe("chaft.ai");
  });

  it("accepts the exact production contract", () => {
    expect(validateGovernanceDocument(component())).toEqual({
      accountId: "1354b55cd97e5051f32727c8dad4399d",
      canonicalOrigin: "https://chaft.ai",
      repository: "Jurshsmith/chaft-infra",
      worker: "chaft-website",
    });
  });

  it("rejects disabled production even if the rest of the document is valid", () => {
    expect(() =>
      validateGovernanceDocument(
        component().replace("productionEnabled: true", "productionEnabled: false"),
      ),
    ).toThrow(/productionEnabled/);
  });

  it("rejects a different canonical hostname", () => {
    expect(() =>
      validateGovernanceDocument(
        component().replace("canonicalHostname: chaft.ai", "canonicalHostname: other.example"),
      ),
    ).toThrow(/canonicalHostname/);
  });
});
