import { describe, expect, it } from "vitest";

import { verifyPreviewDeployment } from "./preview-public-deployment-verifier.mjs";

const commit = "0123456789abcdef0123456789abcdef01234567";
const commonHeaders = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Referrer-Policy": "strict-origin-when-cross-origin",
  "X-Content-Type-Options": "nosniff",
  "X-Frame-Options": "DENY",
  "X-Robots-Tag": "noindex, nofollow, noarchive",
};

function html(pathname, hero = "hero-1") {
  return `<!doctype html>
<html>
  <head>
    <meta name="robots" content="noindex, nofollow, noarchive" />
    <link rel="canonical" href="https://chaft.ai${pathname}" />
    <meta property="og:url" content="https://chaft.ai${pathname}" />
    <link rel="stylesheet" href="/_astro/app.AbCd1234.css" />
  </head>
  <body>
    Chaft Preview
    ${pathname === "/" ? `<section class="hero" data-chaft-hero="${hero}"></section>` : ""}
  </body>
</html>`;
}

function fixtureFetch({
  emptyAsset = false,
  binaryAsset = false,
  wrongHero = false,
  wrongWorker = false,
} = {}) {
  return async (request) => {
    const url = new URL(request);
    if (url.pathname === "/.well-known/chaft-deployment.json") {
      return Response.json(
        {
          schemaVersion: 1,
          artifactKind: "chaft-website",
          sourceRepository: "Jurshsmith/chaft",
          sourceCommit: commit,
          siteUrl: wrongWorker
            ? "https://hero-2.chaft.ai"
            : "https://hero-1.chaft.ai",
        },
        {
          headers: {
            ...commonHeaders,
            "Cache-Control": "no-store",
          },
        },
      );
    }
    if (url.pathname === "/robots.txt") {
      return new Response("User-agent: *\nDisallow: /\n", {
        headers: commonHeaders,
      });
    }
    if (url.pathname.startsWith("/_astro/")) {
      const assetBody = emptyAsset
        ? new Uint8Array()
        : binaryAsset
          ? new Uint8Array([0xff, 0xfe, 0x00, 0x01])
          : "body{}";
      return new Response(assetBody, {
        headers: {
          ...commonHeaders,
          "Cache-Control": "public, max-age=31536000, immutable",
        },
      });
    }
    if (url.pathname === "/definitely-not-a-page-chaft-preview-verification") {
      return new Response(html(url.pathname), {
        headers: commonHeaders,
        status: 404,
      });
    }
    return new Response(
      html(url.pathname, wrongHero ? "hero-2" : "hero-1"),
      { headers: commonHeaders },
    );
  };
}

describe("public Chaft Preview verification", () => {
  it("accepts the exact slot, source, noindex, and production-canonical contract", async () => {
    await expect(
      verifyPreviewDeployment({
        branch: "preview/landing-hero-1",
        expectedCommit: commit,
        fetchImpl: fixtureFetch(),
        repository: "Jurshsmith/chaft",
      }),
    ).resolves.toMatchObject({
      artifactKind: "chaft-preview-public-verification",
      branch: "preview/landing-hero-1",
      domain: "hero-1.chaft.ai",
      expectedCommit: commit,
      result: "passed",
      slot: "hero-1",
      worker: "chaft-website-hero-1",
    });
  });

  it("accepts a binary hashed Astro asset", async () => {
    await expect(
      verifyPreviewDeployment({
        branch: "preview/landing-hero-1",
        expectedCommit: commit,
        fetchImpl: fixtureFetch({ binaryAsset: true }),
        repository: "Jurshsmith/chaft",
      }),
    ).resolves.toMatchObject({
      result: "passed",
      slot: "hero-1",
    });
  });

  it("rejects an empty hashed Astro asset", async () => {
    await expect(
      verifyPreviewDeployment({
        branch: "preview/landing-hero-1",
        expectedCommit: commit,
        fetchImpl: fixtureFetch({ emptyAsset: true }),
        repository: "Jurshsmith/chaft",
      }),
    ).rejects.toThrow(/Astro asset must not be empty/);
  });

  it("rejects a marker from another preview slot", async () => {
    await expect(
      verifyPreviewDeployment({
        branch: "preview/landing-hero-1",
        expectedCommit: commit,
        fetchImpl: fixtureFetch({ wrongWorker: true }),
        repository: "Jurshsmith/chaft",
      }),
    ).rejects.toThrow(/exact preview source identity/);
  });

  it("rejects a different hero rendered on the slot", async () => {
    await expect(
      verifyPreviewDeployment({
        branch: "preview/landing-hero-1",
        expectedCommit: commit,
        fetchImpl: fixtureFetch({ wrongHero: true }),
        repository: "Jurshsmith/chaft",
      }),
    ).rejects.toThrow(/home must render the exact hero-1 hero/);
  });

  it("rejects non-allowlisted branches before making a request", async () => {
    await expect(
      verifyPreviewDeployment({
        branch: "preview/landing-hero-5",
        expectedCommit: commit,
        fetchImpl: () => {
          throw new Error("must not fetch");
        },
        repository: "Jurshsmith/chaft",
      }),
    ).rejects.toThrow(/exact Chaft Previews allowlist/);
  });
});
