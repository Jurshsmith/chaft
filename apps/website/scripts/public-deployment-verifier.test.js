import { describe, expect, it } from "vitest";

import { verifyPublicDeployment } from "./public-deployment-verifier.mjs";

const commit = "04cf7c01900335545d55ed8878cdc37dea1d6e88";
const origin = "https://chaft.ai";
const commonHeaders = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Permissions-Policy": "camera=(), geolocation=(), microphone=(), payment=(), usb=()",
  "Referrer-Policy": "strict-origin-when-cross-origin",
  "X-Content-Type-Options": "nosniff",
  "X-Frame-Options": "DENY",
};

function response(body, { headers = {}, status = 200 } = {}) {
  return new Response(body, {
    status,
    headers: { ...commonHeaders, ...headers },
  });
}

function html(pathname, asset = false) {
  return `<!doctype html><html><head><link rel="canonical" href="${origin}${pathname}"></head>
    <body>${asset ? '<script src="/_astro/site.hash.js"></script>' : ""}</body></html>`;
}

function validFetch(url) {
  const pathname = new URL(url).pathname;
  if (pathname === "/.well-known/chaft-deployment.json") {
    return Promise.resolve(
      response(
        JSON.stringify({
          schemaVersion: 1,
          artifactKind: "chaft-website",
          sourceRepository: "Jurshsmith/chaft",
          sourceCommit: commit,
          siteUrl: origin,
        }),
        { headers: { "Cache-Control": "no-store" } },
      ),
    );
  }
  if (pathname === "/") return Promise.resolve(response(html("/", true)));
  if (pathname === "/download/") return Promise.resolve(response(html("/download/")));
  if (pathname === "/security/") return Promise.resolve(response(html("/security/")));
  if (pathname === "/definitely-not-a-page-chaft-verification") {
    return Promise.resolve(response(html(pathname), { status: 404 }));
  }
  if (pathname === "/downloads") {
    return Promise.resolve(response("", { status: 301, headers: { Location: "/download/" } }));
  }
  if (pathname === "/source") {
    return Promise.resolve(
      response("", {
        status: 302,
        headers: { Location: "https://github.com/Jurshsmith/chaft" },
      }),
    );
  }
  if (pathname === "/releases/current.json") {
    return Promise.resolve(
      response("{}", {
        headers: { "Cache-Control": "public, max-age=0, must-revalidate" },
      }),
    );
  }
  if (pathname === "/robots.txt") {
    return Promise.resolve(response(`Sitemap: ${origin}/sitemap-index.xml`));
  }
  if (pathname === "/sitemap-index.xml") {
    return Promise.resolve(response(`<loc>${origin}/sitemap-0.xml</loc>`));
  }
  if (pathname === "/_astro/site.hash.js") {
    return Promise.resolve(
      response("export {}", {
        headers: { "Cache-Control": "public, max-age=31536000, immutable" },
      }),
    );
  }
  throw new Error(`unexpected request ${url}`);
}

describe("public deployment verifier", () => {
  it("proves the complete production HTTP contract", async () => {
    const report = await verifyPublicDeployment({
      alternateSiteUrl: "https://www.chaft.ai",
      expectedCommit: commit,
      fetchImpl: validFetch,
      repository: "Jurshsmith/chaft",
      siteUrl: origin,
    });
    expect(report.result).toBe("passed");
    expect(report.checks.map((check) => check.name)).toEqual([
      "deployment-marker",
      "home",
      "download",
      "security",
      "not-found",
      "downloads-redirect",
      "source-redirect",
      "current-release",
      "robots",
      "sitemap",
      "hashed-asset",
      "alternate-home",
    ]);
  });

  it("rejects a marker from a different commit", async () => {
    const fetchImpl = async (url, options) => {
      const value = await validFetch(url, options);
      if (new URL(url).pathname !== "/.well-known/chaft-deployment.json") return value;
      const marker = await value.json();
      marker.sourceCommit = "a".repeat(40);
      return response(JSON.stringify(marker), { headers: { "Cache-Control": "no-store" } });
    };
    await expect(
      verifyPublicDeployment({
        expectedCommit: commit,
        fetchImpl,
        repository: "Jurshsmith/chaft",
        siteUrl: origin,
      }),
    ).rejects.toThrow(/commit/);
  });
});
