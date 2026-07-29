import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";

import { describe, expect, it } from "vitest";

import { verifyPublicDeployment } from "./public-deployment-verifier.mjs";

const commit = "04cf7c01900335545d55ed8878cdc37dea1d6e88";
const origin = "https://chaft.ai";
const repository = "Jurshsmith/chaft";
const version = "0.1.0-canary.1";
const tag = `v${version}`;
const sensitiveUseWarning =
  "Do not use Chaft canary builds for sensitive or production communication.";
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

function html(pathname, { asset = false, warnings = false } = {}) {
  return `<!doctype html><html><head><link rel="canonical" href="${origin}${pathname}"></head>
    <body>
      ${asset ? '<script src="/_astro/site.hash.js"></script>' : ""}
      ${warnings ? `<strong>Unsigned canary</strong><p>${sensitiveUseWarning}</p>` : ""}
      <span>${tag}</span>
    </body></html>`;
}

function digest(payload) {
  return createHash("sha256").update(payload).digest("hex");
}

function trackedFile(payloads, filename, contents) {
  const payload = new TextEncoder().encode(contents);
  const url = `https://github.com/${repository}/releases/download/${tag}/${filename}`;
  payloads.set(url, payload);
  return {
    filename,
    url,
    sizeBytes: payload.byteLength,
    sha256: digest(payload),
  };
}

function asset(payloads, { arch, filename, format, os, platformLabel }) {
  const target = `${os}-${arch}`;
  const packageFile = trackedFile(payloads, filename, `package:${target}:${version}`);
  return {
    id: `${target}-${format}`,
    os,
    platformLabel,
    arch,
    format,
    ...packageFile,
    available: true,
    signingStatus: "unsigned-canary",
    evidence: {
      checksums: trackedFile(
        payloads,
        `chaft-desktop-${target}-SHA256SUMS`,
        `checksums:${target}:${packageFile.sha256}`,
      ),
      sbom: trackedFile(
        payloads,
        `chaft-desktop-${target}-sbom.cdx.json`,
        `{"bomFormat":"CycloneDX","target":"${target}"}`,
      ),
      provenance: trackedFile(
        payloads,
        `chaft-desktop-${target}-provenance.json`,
        `{"target":"${target}","tag":"${tag}"}`,
      ),
      signature: null,
      verification: trackedFile(
        payloads,
        `chaft-desktop-${target}-verification.json`,
        `{"target":"${target}","state":"unsigned-canary"}`,
      ),
    },
  };
}

function releaseFixture(mutator) {
  const payloads = new Map();
  const manifest = {
    schemaVersion: 2,
    channel: "canary",
    status: "published",
    version,
    tag,
    publishedAt: "2026-07-26T20:00:00Z",
    commit: "b".repeat(40),
    releaseUrl: `https://github.com/${repository}/releases/tag/${tag}`,
    sourceUrl: `https://github.com/${repository}`,
    releaseEvidence: {
      qtSource: trackedFile(
        payloads,
        "Chaft-Qt-6.8.4-corresponding-source.zip",
        "verified Qt corresponding source",
      ),
      qtSourceChecksums: trackedFile(
        payloads,
        "Chaft-Qt-6.8.4-corresponding-source.zip.sha256",
        "Qt source checksum sidecar",
      ),
      inventory: trackedFile(
        payloads,
        "chaft-desktop-release-inventory.json",
        '{"release":"canary","targets":4}',
      ),
      aggregateChecksums: trackedFile(
        payloads,
        "chaft-desktop-release-SHA256SUMS",
        "aggregate release checksums",
      ),
    },
    assets: [
      asset(payloads, {
        arch: "x86_64",
        filename: `Chaft-${version}-Windows-x86_64.zip`,
        format: "zip",
        os: "windows",
        platformLabel: "Windows",
      }),
      asset(payloads, {
        arch: "x86_64",
        filename: `Chaft-${version}-macOS-x86_64.dmg`,
        format: "dmg",
        os: "macos",
        platformLabel: "macOS Intel",
      }),
      asset(payloads, {
        arch: "arm64",
        filename: `Chaft-${version}-macOS-arm64.dmg`,
        format: "dmg",
        os: "macos",
        platformLabel: "macOS Apple Silicon",
      }),
      asset(payloads, {
        arch: "x86_64",
        filename: `Chaft-${version}-Linux-x86_64.AppImage`,
        format: "appimage",
        os: "linux",
        platformLabel: "Linux",
      }),
    ],
  };
  mutator?.(manifest, payloads);

  const fileRequests = new Map();
  const fetchImpl = async (input, options = {}) => {
    const url = new URL(input);
    const payload = payloads.get(url.href);
    if (payload) {
      fileRequests.set(url.href, {
        count: (fileRequests.get(url.href)?.count ?? 0) + 1,
        redirect: options.redirect,
      });
      const midpoint = Math.max(1, Math.floor(payload.byteLength / 2));
      return response(
        new ReadableStream({
          start(controller) {
            controller.enqueue(payload.subarray(0, midpoint));
            controller.enqueue(payload.subarray(midpoint));
            controller.close();
          },
        }),
      );
    }

    const { pathname } = url;
    if (pathname === "/.well-known/chaft-deployment.json") {
      return response(
        JSON.stringify({
          schemaVersion: 1,
          artifactKind: "chaft-website",
          sourceRepository: repository,
          sourceCommit: commit,
          siteUrl: origin,
        }),
        { headers: { "Cache-Control": "no-store" } },
      );
    }
    if (pathname === "/") return response(html("/", { asset: true }));
    if (pathname === "/download/") {
      return response(html("/download/", { warnings: true }));
    }
    if (pathname === "/security/") return response(html("/security/"));
    if (pathname === "/releases/") {
      return response(html("/releases/", { warnings: true }));
    }
    if (pathname === `/releases/${version}/`) {
      return response(html(`/releases/${version}/`, { warnings: true }));
    }
    if (pathname === "/definitely-not-a-page-chaft-verification") {
      return response(html(pathname), { status: 404 });
    }
    if (pathname === "/downloads") {
      return response("", { status: 301, headers: { Location: "/download/" } });
    }
    if (pathname === "/source") {
      return response("", {
        status: 302,
        headers: { Location: `https://github.com/${repository}` },
      });
    }
    if (
      pathname === "/releases/current.json" ||
      pathname === `/releases/${version}.json`
    ) {
      return response(JSON.stringify(manifest), {
        headers: {
          "Cache-Control": "public, max-age=0, must-revalidate",
          "Content-Type": "application/json; charset=utf-8",
        },
      });
    }
    if (pathname === "/robots.txt") {
      return response(`Sitemap: ${origin}/sitemap-index.xml`);
    }
    if (pathname === "/sitemap-index.xml") {
      return response(`<loc>${origin}/sitemap-0.xml</loc>`);
    }
    if (pathname === "/_astro/site.hash.js") {
      return response("export {}", {
        headers: { "Cache-Control": "public, max-age=31536000, immutable" },
      });
    }
    throw new Error(`unexpected request ${url.href}`);
  };

  return {
    fetchImpl,
    fileRequests,
    manifest,
    manifestSha256: digest(JSON.stringify(manifest)),
    payloads,
  };
}

function immutableLegacyReleaseFixture() {
  return releaseFixture((manifest, payloads) => {
    manifest.assets.splice(2, 1);
    manifest.commit = "d021e7d0ea7b143a32ab49529790abc886f0f06c";
    for (const releaseAsset of manifest.assets) {
      const target = `${releaseAsset.os}-${releaseAsset.arch}`;
      for (const evidence of Object.values(releaseAsset.evidence)) {
        if (evidence === null) continue;
        const payload = payloads.get(evidence.url);
        expect(payload).toBeDefined();
        payloads.delete(evidence.url);
        evidence.filename = evidence.filename.replace(
          `chaft-desktop-${target}-`,
          `chaft-desktop-${releaseAsset.os}-`,
        );
        evidence.url =
          `https://github.com/${repository}/releases/download/${tag}/${evidence.filename}`;
        payloads.set(evidence.url, payload);
      }
    }
  });
}

function verificationOptions(fixture, fetchImpl = fixture.fetchImpl) {
  return {
    alternateSiteUrl: "https://www.chaft.ai",
    expectedCommit: commit,
    expectedReleaseManifestSha256: fixture.manifestSha256,
    expectedReleaseStatus: fixture.manifest.status,
    expectedReleaseTag: fixture.manifest.tag,
    expectedReleaseVersion: version,
    fetchImpl,
    repository,
    siteUrl: origin,
  };
}

describe("public deployment verifier", () => {
  it("proves the complete canary production HTTP and release-file contract", async () => {
    const fixture = releaseFixture();
    const report = await verifyPublicDeployment(
      verificationOptions(fixture),
    );

    expect(report.result).toBe("passed");
    expect(report.expectedReleaseTag).toBe(tag);
    expect(report.expectedReleaseVersion).toBe(version);
    expect(report.releaseFilesVerified).toBe(24);
    expect(report.releaseFilesVerified).toBe(fixture.payloads.size);
    expect(report.checks.map((check) => check.name)).toEqual(
      expect.arrayContaining([
        "deployment-marker",
        "home",
        "download",
        "security",
        "releases",
        "release-version",
        "not-found",
        "downloads-redirect",
        "source-redirect",
        "current-release",
        "version-release",
        "robots",
        "sitemap",
        "hashed-asset",
        "alternate-home",
      ]),
    );
    expect(
      report.checks.filter((check) => check.name.startsWith("release-file:")),
    ).toHaveLength(fixture.payloads.size);
    expect([...fixture.fileRequests.values()]).toEqual(
      [...fixture.payloads].map(() => ({ count: 1, redirect: "follow" })),
    );
  });

  it("accepts the exact immutable legacy three-target canary set", async () => {
    const fixture = immutableLegacyReleaseFixture();

    const report = await verifyPublicDeployment(verificationOptions(fixture));

    expect(report.result).toBe("passed");
    expect(report.releaseFilesVerified).toBe(19);
  });

  it("rejects current target-qualified evidence masquerading as legacy", async () => {
    const fixture = releaseFixture((manifest) => {
      manifest.assets.splice(2, 1);
    });

    await expect(
      verifyPublicDeployment(verificationOptions(fixture)),
    ).rejects.toThrow(/exact immutable published legacy release/);
    expect(fixture.fileRequests.size).toBe(0);
  });

  it("rejects a three-target release with a forged legacy revision", async () => {
    const fixture = immutableLegacyReleaseFixture();
    fixture.manifest.commit = "b".repeat(40);
    fixture.manifestSha256 = digest(JSON.stringify(fixture.manifest));

    await expect(
      verifyPublicDeployment(verificationOptions(fixture)),
    ).rejects.toThrow(/exact immutable published legacy release/);
    expect(fixture.fileRequests.size).toBe(0);
  });

  it("rejects a duplicated target in place of one canonical target", async () => {
    const fixture = releaseFixture((manifest) => {
      manifest.assets[2] = structuredClone(manifest.assets[1]);
    });

    await expect(
      verifyPublicDeployment(verificationOptions(fixture)),
    ).rejects.toThrow(/target macos-x86_64 is duplicated/);
    expect(fixture.fileRequests.size).toBe(0);
  });

  it("rejects a partial current target set that is not the legacy set", async () => {
    const fixture = releaseFixture((manifest) => {
      manifest.assets.pop();
    });

    await expect(
      verifyPublicDeployment(verificationOptions(fixture)),
    ).rejects.toThrow(/legacy three-target set or the current four-target set/);
    expect(fixture.fileRequests.size).toBe(0);
  });

  it("verifies the exact coming-soon manifest without pretending downloads exist", async () => {
    const fixture = releaseFixture((manifest) => {
      manifest.status = "coming-soon";
      manifest.tag = null;
      manifest.publishedAt = null;
      manifest.commit = null;
      manifest.releaseUrl = `https://github.com/${repository}/releases`;
      manifest.releaseEvidence = null;
      manifest.assets = manifest.assets.map((releaseAsset) => ({
        ...releaseAsset,
        filename: null,
        url: `https://github.com/${repository}/releases`,
        available: false,
        sizeBytes: null,
        sha256: null,
        signingStatus: "pending",
        evidence: {
          checksums: null,
          sbom: null,
          provenance: null,
          signature: null,
          verification: null,
        },
      }));
    });
    fixture.payloads.clear();

    const report = await verifyPublicDeployment(verificationOptions(fixture));

    expect(report.result).toBe("passed");
    expect(report.expectedReleaseStatus).toBe("coming-soon");
    expect(report.expectedReleaseTag).toBeNull();
    expect(report.releaseFilesVerified).toBe(0);
    expect(fixture.fileRequests.size).toBe(0);
  });

  it("rejects a marker from a different commit", async () => {
    const fixture = releaseFixture();
    const fetchImpl = async (input, options) => {
      const value = await fixture.fetchImpl(input, options);
      if (new URL(input).pathname !== "/.well-known/chaft-deployment.json") {
        return value;
      }
      const marker = await value.json();
      marker.sourceCommit = "a".repeat(40);
      return response(JSON.stringify(marker), {
        headers: { "Cache-Control": "no-store" },
      });
    };

    await expect(
      verifyPublicDeployment(verificationOptions(fixture, fetchImpl)),
    ).rejects.toThrow(/commit/);
  });

  it("rejects version JSON that differs from the current release JSON", async () => {
    const fixture = releaseFixture();
    const fetchImpl = async (input, options) => {
      if (new URL(input).pathname !== `/releases/${version}.json`) {
        return fixture.fetchImpl(input, options);
      }
      return response(
        JSON.stringify({
          ...fixture.manifest,
          publishedAt: "2026-07-26T21:00:00Z",
        }),
        {
          headers: {
            "Cache-Control": "public, max-age=0, must-revalidate",
            "Content-Type": "application/json; charset=utf-8",
          },
        },
      );
    };

    await expect(
      verifyPublicDeployment(verificationOptions(fixture, fetchImpl)),
    ).rejects.toThrow(/current and version release JSON must be identical/);
  });

  it("rejects a release JSON body that differs from the deployed artifact", async () => {
    const fixture = releaseFixture();
    await expect(
      verifyPublicDeployment({
        ...verificationOptions(fixture),
        expectedReleaseManifestSha256: "0".repeat(64),
      }),
    ).rejects.toThrow(/deployment artifact SHA-256/);
    expect(fixture.fileRequests.size).toBe(0);
  });

  it("rejects a canonical release surface without the sensitive-use warning", async () => {
    const fixture = releaseFixture();
    const fetchImpl = async (input, options) => {
      if (new URL(input).pathname !== "/download/") {
        return fixture.fetchImpl(input, options);
      }
      return response(html("/download/"));
    };

    await expect(
      verifyPublicDeployment(verificationOptions(fixture, fetchImpl)),
    ).rejects.toThrow(/unsigned canary|sensitive-use warning/);
  });

  it.each([
    [
      "size",
      (manifest) => {
        manifest.assets[0].sizeBytes -= 1;
      },
      /exceeds manifest size|size .* does not match manifest size/,
    ],
    [
      "SHA-256",
      (manifest) => {
        manifest.assets[0].sha256 = "0".repeat(64);
      },
      /SHA-256 .* does not match manifest/,
    ],
  ])(
    "rejects a release package with the wrong manifest %s",
    async (_, mutator, error) => {
      const fixture = releaseFixture(mutator);
      await expect(
        verifyPublicDeployment(verificationOptions(fixture)),
      ).rejects.toThrow(error);
    },
  );

  it("requires the caller to bind verification to one exact canary tag", async () => {
    const fixture = releaseFixture();
    await expect(
      verifyPublicDeployment({
        ...verificationOptions(fixture),
        expectedReleaseTag: "v0.1.0-canary.2",
      }),
    ).rejects.toThrow(/must equal/);
    expect(fixture.fileRequests.size).toBe(0);
  });

  it("plumbs release tag and version through the CLI", () => {
    const result = spawnSync(
      process.execPath,
      [
        "scripts/verify-public-deployment.mjs",
        "--site-url",
        origin,
        "--repository",
        repository,
        "--commit",
        commit,
        "--release-version",
        version,
        "--release-status",
        "published",
        "--release-manifest-sha256",
        "a".repeat(64),
        "--release-tag",
        "v0.1.0-canary.2",
        "--attempts",
        "1",
      ],
      { encoding: "utf8" },
    );
    expect(result.status).toBe(1);
    expect(result.stderr).toContain(`expected release tag must equal v${version}`);
  });
});
