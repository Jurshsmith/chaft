import { describe, expect, it } from "vitest";

import {
  PREVIEW_ROBOTS_POLICY,
  PREVIEW_ROBOTS_TEXT,
  PREVIEW_SLOTS,
  PRODUCTION_ORIGIN,
  previewSlotForBranch,
  previewSlotLabel,
  resolveWebsiteDeployment,
} from "./preview-contract.mjs";

describe("Chaft Preview contract", () => {
  it("uses the exact reviewed branch, slot, Worker, and domain allowlist", () => {
    expect(PREVIEW_SLOTS).toEqual([
      {
        branch: "preview/landing-hero-1",
        slot: "hero-1",
        worker: "chaft-website-hero-1",
        domain: "hero-1.chaft.ai",
        environment: "chaft-preview-hero-1",
        siteUrl: "https://hero-1.chaft.ai",
        wranglerEnvironment: "hero-1",
      },
      {
        branch: "preview/landing-hero-2",
        slot: "hero-2",
        worker: "chaft-website-hero-2",
        domain: "hero-2.chaft.ai",
        environment: "chaft-preview-hero-2",
        siteUrl: "https://hero-2.chaft.ai",
        wranglerEnvironment: "hero-2",
      },
      {
        branch: "preview/landing-hero-3",
        slot: "hero-3",
        worker: "chaft-website-hero-3",
        domain: "hero-3.chaft.ai",
        environment: "chaft-preview-hero-3",
        siteUrl: "https://hero-3.chaft.ai",
        wranglerEnvironment: "hero-3",
      },
      {
        branch: "preview/landing-hero-4",
        slot: "hero-4",
        worker: "chaft-website-hero-4",
        domain: "hero-4.chaft.ai",
        environment: "chaft-preview-hero-4",
        siteUrl: "https://hero-4.chaft.ai",
        wranglerEnvironment: "hero-4",
      },
    ]);
    expect(new Set(PREVIEW_SLOTS.map(({ branch }) => branch)).size).toBe(4);
    expect(new Set(PREVIEW_SLOTS.map(({ slot }) => slot)).size).toBe(4);
    expect(new Set(PREVIEW_SLOTS.map(({ worker }) => worker)).size).toBe(4);
    expect(new Set(PREVIEW_SLOTS.map(({ domain }) => domain)).size).toBe(4);
    expect(PREVIEW_SLOTS.every(Object.isFrozen)).toBe(true);
    expect(Object.isFrozen(PREVIEW_SLOTS)).toBe(true);
    expect(PREVIEW_SLOTS.map(({ slot }) => previewSlotLabel(slot))).toEqual([
      "Hero 1",
      "Hero 2",
      "Hero 3",
      "Hero 4",
    ]);
  });

  it.each(PREVIEW_SLOTS)(
    "resolves $branch only for its exact Preview slot origin",
    (preview) => {
      expect(
        resolveWebsiteDeployment({
          deploymentMode: "preview",
          previewBranch: preview.branch,
          siteUrl: preview.siteUrl,
        }),
      ).toEqual({
        mode: "preview",
        isPreview: true,
        siteUrl: preview.siteUrl,
        canonicalOrigin: PRODUCTION_ORIGIN,
        preview,
      });
      expect(previewSlotForBranch(preview.branch)).toBe(preview);
    },
  );

  it("defines matching HTML/header and robots.txt indexing protections", () => {
    expect(PREVIEW_ROBOTS_POLICY).toBe("noindex, nofollow, noarchive");
    expect(PREVIEW_ROBOTS_TEXT).toBe("User-agent: *\nDisallow: /\n");
    expect(PREVIEW_ROBOTS_TEXT).not.toContain("Sitemap:");
  });

  it("rejects missing, unknown, or inexact Preview branches", () => {
    const options = {
      deploymentMode: "preview",
      siteUrl: "https://hero-1.chaft.ai",
    };
    expect(() => resolveWebsiteDeployment(options)).toThrow(
      /CHAFT_PREVIEW_BRANCH is required/,
    );
    expect(() =>
      resolveWebsiteDeployment({
        ...options,
        previewBranch: "preview/landing-hero-5",
      }),
    ).toThrow(/not assigned to a Preview slot/);
    expect(() =>
      resolveWebsiteDeployment({
        ...options,
        previewBranch: "refs/heads/preview/landing-hero-1",
      }),
    ).toThrow(/not assigned to a Preview slot/);
    expect(() =>
      resolveWebsiteDeployment({
        ...options,
        previewBranch: "preview/landing-Hero-1",
      }),
    ).toThrow(/not assigned to a Preview slot/);
  });

  it("rejects a Preview branch deployed to another slot or malformed origin", () => {
    const options = {
      deploymentMode: "preview",
      previewBranch: "preview/landing-hero-1",
    };
    expect(() =>
      resolveWebsiteDeployment({
        ...options,
        siteUrl: "https://hero-2.chaft.ai",
      }),
    ).toThrow(/must be exactly https:\/\/hero-1\.chaft\.ai/);
    expect(() =>
      resolveWebsiteDeployment({
        ...options,
        siteUrl: "https://hero-1.chaft.ai/path",
      }),
    ).toThrow(/root HTTPS origin/);
    expect(() =>
      resolveWebsiteDeployment({
        ...options,
        siteUrl: "http://hero-1.chaft.ai",
      }),
    ).toThrow(/root HTTPS origin/);
    expect(() =>
      resolveWebsiteDeployment({
        ...options,
        siteUrl: " https://hero-1.chaft.ai",
      }),
    ).toThrow(/surrounding whitespace/);
  });

  it("requires the exact production origin in explicit production mode", () => {
    expect(
      resolveWebsiteDeployment({
        deploymentMode: "production",
        siteUrl: PRODUCTION_ORIGIN,
      }),
    ).toEqual({
      mode: "production",
      isPreview: false,
      siteUrl: PRODUCTION_ORIGIN,
      canonicalOrigin: PRODUCTION_ORIGIN,
      preview: undefined,
    });
    expect(() =>
      resolveWebsiteDeployment({
        deploymentMode: "production",
        previewBranch: "preview/landing-hero-1",
        siteUrl: PRODUCTION_ORIGIN,
      }),
    ).toThrow(/CHAFT_PREVIEW_BRANCH must be unset/);
    expect(() =>
      resolveWebsiteDeployment({
        deploymentMode: "production",
        siteUrl: "https://www.chaft.ai",
      }),
    ).toThrow(/Production SITE_URL must be exactly/);
  });

  it("preserves ordinary local and validation builds when mode is unset", () => {
    expect(
      resolveWebsiteDeployment({
        siteUrl: "https://website-validation.invalid/chaft-validation",
      }),
    ).toMatchObject({
      mode: "site",
      isPreview: false,
      siteUrl: "https://website-validation.invalid",
      canonicalOrigin: "https://website-validation.invalid",
    });
    expect(
      resolveWebsiteDeployment({ siteUrl: new URL("http://localhost:4321") }),
    ).toMatchObject({
      mode: "site",
      isPreview: false,
      siteUrl: "http://localhost:4321",
    });
  });

  it("never treats a reserved Preview hostname as an ordinary site build", () => {
    expect(() =>
      resolveWebsiteDeployment({ siteUrl: "https://hero-1.chaft.ai" }),
    ).toThrow(/requires CHAFT_DEPLOYMENT_MODE=preview/);
    expect(() =>
      resolveWebsiteDeployment({ siteUrl: "https://hero-9.chaft.ai" }),
    ).toThrow(/requires CHAFT_DEPLOYMENT_MODE=preview/);
    expect(() =>
      resolveWebsiteDeployment({
        previewBranch: "preview/landing-hero-1",
        siteUrl: "https://example.com",
      }),
    ).toThrow(/requires CHAFT_DEPLOYMENT_MODE=preview/);
  });

  it("rejects unsupported or padded deployment modes", () => {
    expect(() =>
      resolveWebsiteDeployment({
        deploymentMode: "development",
        siteUrl: "https://example.com",
      }),
    ).toThrow(/must be either production or preview/);
    expect(() =>
      resolveWebsiteDeployment({
        deploymentMode: " preview",
        siteUrl: "https://hero-1.chaft.ai",
      }),
    ).toThrow(/surrounding whitespace/);
  });
});
