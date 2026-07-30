import { describe, expect, it } from "vitest";

import {
  PREVIEW_SLOTS,
  assertPreviewIdentity,
  previewArtifactName,
  previewDeploymentRecordName,
  previewSlotForBranch,
  previewSlotForName,
} from "./preview-slot-contract.mjs";

describe("Chaft Previews slot contract", () => {
  it("maps the four exact branches to fixed isolated resources", () => {
    expect(PREVIEW_SLOTS).toEqual([
      {
        branch: "preview/landing-hero-1",
        domain: "hero-1.chaft.ai",
        environment: "chaft-preview-hero-1",
        siteUrl: "https://hero-1.chaft.ai",
        slot: "hero-1",
        worker: "chaft-website-hero-1",
        wranglerEnvironment: "hero-1",
      },
      {
        branch: "preview/landing-hero-2",
        domain: "hero-2.chaft.ai",
        environment: "chaft-preview-hero-2",
        siteUrl: "https://hero-2.chaft.ai",
        slot: "hero-2",
        worker: "chaft-website-hero-2",
        wranglerEnvironment: "hero-2",
      },
      {
        branch: "preview/landing-hero-3",
        domain: "hero-3.chaft.ai",
        environment: "chaft-preview-hero-3",
        siteUrl: "https://hero-3.chaft.ai",
        slot: "hero-3",
        worker: "chaft-website-hero-3",
        wranglerEnvironment: "hero-3",
      },
      {
        branch: "preview/landing-hero-4",
        domain: "hero-4.chaft.ai",
        environment: "chaft-preview-hero-4",
        siteUrl: "https://hero-4.chaft.ai",
        slot: "hero-4",
        worker: "chaft-website-hero-4",
        wranglerEnvironment: "hero-4",
      },
    ]);
  });

  it("resolves only exact branch and slot names", () => {
    expect(previewSlotForBranch("preview/landing-hero-2").slot).toBe("hero-2");
    expect(previewSlotForName("hero-4").domain).toBe("hero-4.chaft.ai");
    expect(() => previewSlotForBranch("preview/landing-hero-5")).toThrow(
      /exact Chaft Previews allowlist/,
    );
    expect(() => previewSlotForBranch("refs/heads/preview/landing-hero-1")).toThrow(
      /exact Chaft Previews allowlist/,
    );
    expect(() => previewSlotForName("hero-01")).toThrow(
      /exact Chaft Previews allowlist/,
    );
  });

  it("binds immutable artifacts and retained records to exact identities", () => {
    const commit = "0123456789abcdef0123456789abcdef01234567";
    const version = "01234567-89ab-cdef-0123-456789abcdef";
    expect(previewArtifactName("hero-3", commit)).toBe(
      `chaft-preview-hero-3-${commit}`,
    );
    expect(previewDeploymentRecordName("hero-3", version)).toBe(
      `chaft-preview-deployment-hero-3-${version}`,
    );
    expect(() => previewArtifactName("hero-3", "main")).toThrow(/source commit/);
    expect(() => previewDeploymentRecordName("hero-3", "previous")).toThrow(
      /version ID/,
    );
  });

  it("rejects any cross-slot identity substitution", () => {
    const expected = previewSlotForBranch("preview/landing-hero-1");
    expect(assertPreviewIdentity(expected)).toEqual(expected);
    expect(() =>
      assertPreviewIdentity({
        ...expected,
        worker: "chaft-website",
      }),
    ).toThrow(/worker must be chaft-website-hero-1/);
    expect(() =>
      assertPreviewIdentity({
        ...expected,
        siteUrl: "https://hero-2.chaft.ai",
      }),
    ).toThrow(/siteUrl must be https:\/\/hero-1\.chaft\.ai/);
  });
});
