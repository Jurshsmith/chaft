import { describe, expect, it } from "vitest";

import rawManifest from "./release-manifest.json";
import {
  allReleases,
  buildReleaseCollection,
  currentRelease,
  formatBytes,
  operatingSystems,
  validateReleaseManifest,
} from "./releases";

interface PublishedReleaseOptions {
  version?: string;
  tag?: string | null;
  artifactVersion?: string;
  artifactTag?: string;
  publishedAt?: string;
}

function publishedRelease({
  version = "0.1.0",
  tag = `v${version}`,
  artifactVersion = version,
  artifactTag = tag ?? `v${version}`,
  publishedAt = "2026-07-18T10:00:00Z",
}: PublishedReleaseOptions = {}) {
  return {
    ...structuredClone(rawManifest),
    status: "published",
    version,
    tag,
    publishedAt,
    commit: "a".repeat(40),
    releaseUrl: `https://github.com/Jurshsmith/chaft/releases/tag/${tag ?? "missing"}`,
    assets: rawManifest.assets.map((asset) => {
      const filename = {
        windows: `Chaft-${artifactVersion}-Windows.zip`,
        macos: `Chaft-${artifactVersion}-macOS.dmg`,
        linux: `Chaft-${artifactVersion}-Linux.AppImage`,
      }[asset.os]!;
      const signingStatus = {
        windows: "signed",
        macos: "notarized",
        linux: "checksummed",
      }[asset.os]!;
      const evidenceFile = (evidenceFilename: string) => ({
        filename: evidenceFilename,
        url: `https://github.com/Jurshsmith/chaft/releases/download/${artifactTag}/${evidenceFilename}`,
        sizeBytes: 512,
        sha256: "c".repeat(64),
      });
      return {
        ...asset,
        available: true,
        filename,
        url: `https://github.com/Jurshsmith/chaft/releases/download/${artifactTag}/${filename}`,
        sizeBytes: 1024,
        sha256: "b".repeat(64),
        signingStatus,
        evidence: {
          checksums: evidenceFile(`chaft-desktop-${asset.os}-SHA256SUMS`),
          sbom: evidenceFile(`chaft-desktop-${asset.os}-sbom.cdx.json`),
          provenance: evidenceFile(`chaft-desktop-${asset.os}-provenance.json`),
          signature: null,
          verification:
            asset.os === "linux"
              ? null
              : evidenceFile(`chaft-desktop-${asset.os}-verification.json`),
        },
      };
    }),
  };
}

describe("release manifest", () => {
  it("validates the checked-in preview manifest", () => {
    expect(validateReleaseManifest(rawManifest)).toEqual(currentRelease);
    expect(currentRelease.tag).toBeNull();
  });

  it("exposes the current manifest when no history files exist", () => {
    expect(allReleases.map((release) => release.version)).toEqual([
      currentRelease.version,
    ]);
  });

  it("contains a statically rendered option for every supported platform", () => {
    expect(new Set(currentRelease.assets.map((asset) => asset.os))).toEqual(
      new Set(operatingSystems),
    );
  });

  it.each(["1", "1.2", "v1.2.3", "01.2.3", "1.02.3", "1.2.3-01"])(
    "rejects invalid semantic version %s",
    (version) => {
      const invalid = { ...structuredClone(rawManifest), version };
      expect(() => validateReleaseManifest(invalid)).toThrow(/semantic version/);
    },
  );

  it("accepts semantic prerelease and build identifiers", () => {
    const valid = {
      ...structuredClone(rawManifest),
      version: "1.2.3-rc.1+build.7",
    };
    expect(validateReleaseManifest(valid).version).toBe("1.2.3-rc.1+build.7");
  });

  it.each(["0", "2026-07-18", "2026-02-30T10:00:00Z", "2026-07-18T10:00:00"])(
    "rejects non-canonical publication timestamp %s",
    (publishedAt) => {
      const invalid = publishedRelease({ publishedAt });
      expect(() => validateReleaseManifest(invalid)).toThrow(/RFC 3339 date-time/);
    },
  );

  it("rejects an available artifact without final integrity metadata", () => {
    const invalid = structuredClone(rawManifest);
    invalid.assets[0]!.available = true;
    expect(() => validateReleaseManifest(invalid)).toThrow(/filename, sizeBytes, and sha256/);
  });

  it("rejects an available artifact whose signing state is still pending", () => {
    const invalid = structuredClone(rawManifest);
    Object.assign(invalid.assets[0]!, {
      available: true,
      filename: "Chaft-0.1.0-dev-Windows.zip",
      sizeBytes: 1024,
      sha256: "a".repeat(64),
    });
    expect(() => validateReleaseManifest(invalid)).toThrow(/cannot be pending/);
  });

  it("rejects a generic release-page URL for an available artifact", () => {
    const invalid = structuredClone(rawManifest);
    Object.assign(invalid.assets[0]!, {
      available: true,
      filename: "Chaft-0.1.0-dev-Windows.zip",
      sizeBytes: 1024,
      sha256: "a".repeat(64),
      signingStatus: "signed",
    });
    expect(() => validateReleaseManifest(invalid)).toThrow(/point directly/);
  });

  it("rejects a package format that is unsupported for its operating system", () => {
    const invalid = structuredClone(rawManifest);
    invalid.assets[0]!.format = "dmg";
    expect(() => validateReleaseManifest(invalid)).toThrow(/format must be one of/);
  });

  it("rejects a filename whose extension contradicts its package format", () => {
    const invalid = publishedRelease();
    invalid.assets[0]!.format = "msi";
    expect(() => validateReleaseManifest(invalid)).toThrow(/filename extension must match/);
  });

  it("rejects an artifact reused for multiple published platform entries", () => {
    const invalid = publishedRelease();
    invalid.assets.push({
      ...invalid.assets[0]!,
      id: "windows-x86_64-copy",
    });
    expect(() => validateReleaseManifest(invalid)).toThrow(/filenames must be unique/);
  });

  it("requires verification evidence for signed and notarized artifacts", () => {
    const invalid = publishedRelease();
    invalid.assets[0]!.evidence.verification = null;
    expect(() => validateReleaseManifest(invalid)).toThrow(
      /verification is required for signed artifacts/,
    );
  });

  it("requires published evidence URLs to use the exact release tag", () => {
    const invalid = publishedRelease();
    const provenance = invalid.assets[0]!.evidence.provenance!;
    provenance.url = provenance.url.replace("/v0.1.0/", "/v0.1.1/");
    expect(() => validateReleaseManifest(invalid)).toThrow(
      /evidence\.provenance\.url.*exact GitHub URL/,
    );
  });

  it("rejects downloads while the release is still coming soon", () => {
    const invalid = publishedRelease();
    invalid.status = "coming-soon";
    expect(() => validateReleaseManifest(invalid)).toThrow(/status must be published/);
  });

  it("rejects a published release until every platform has an available asset", () => {
    const invalid = {
      ...structuredClone(rawManifest),
      status: "published",
      publishedAt: "2026-07-18T10:00:00Z",
    };
    expect(() => validateReleaseManifest(invalid)).toThrow(/every platform/);
  });

  it("requires the published tag to exactly match the advertised version", () => {
    const invalid = publishedRelease({ version: "0.1.0", tag: "v0.1.1" });
    expect(() => validateReleaseManifest(invalid)).toThrow(/tag must equal v0\.1\.0/);
  });

  it("requires the release URL to point at the exact GitHub tag", () => {
    const invalid = {
      ...publishedRelease(),
      releaseUrl: "https://github.com/Jurshsmith/chaft/releases",
    };
    expect(() => validateReleaseManifest(invalid)).toThrow(/release\.releaseUrl.*exact GitHub URL/);
  });

  it("rejects the previously accepted 0.1.0 artifacts advertised as 0.1.0-dev", () => {
    const invalid = publishedRelease({
      version: "0.1.0-dev",
      artifactVersion: "0.1.0",
      artifactTag: "v0.1.0-dev",
    });
    expect(() => validateReleaseManifest(invalid)).toThrow(
      /filename must contain release\.version 0\.1\.0-dev/,
    );
  });

  it("requires every available asset URL to use the exact release tag", () => {
    const invalid = publishedRelease({ artifactTag: "v0.1.1" });
    expect(() => validateReleaseManifest(invalid)).toThrow(/assets\[0\]\.url.*exact GitHub URL/);
  });

  it("accepts a coherent published cross-platform release", () => {
    const release = validateReleaseManifest(publishedRelease());
    expect(release.status).toBe("published");
    expect(release.tag).toBe("v0.1.0");
  });
});

describe("release history", () => {
  it("loads published history behind the current release in newest-first order", () => {
    const releases = buildReleaseCollection(rawManifest, {
      "./release-history/0.0.8.json": publishedRelease({
        version: "0.0.8",
        publishedAt: "2026-05-01T10:00:00Z",
      }),
      "./release-history/0.0.9.json": publishedRelease({
        version: "0.0.9",
        publishedAt: "2026-06-01T10:00:00Z",
      }),
    });

    expect(releases.map((release) => release.version)).toEqual([
      "0.1.0-dev",
      "0.0.9",
      "0.0.8",
    ]);
  });

  it("rejects duplicate versions across current and historical manifests", () => {
    const duplicate = publishedRelease({ version: rawManifest.version });
    expect(() =>
      buildReleaseCollection(rawManifest, {
        [`./release-history/${rawManifest.version}.json`]: duplicate,
      }),
    ).toThrow(/appears more than once/);
  });

  it("rejects a historical manifest that has not been published", () => {
    expect(() =>
      buildReleaseCollection(rawManifest, {
        [`./release-history/${rawManifest.version}.json`]: rawManifest,
      }),
    ).toThrow(/must be published/);
  });

  it("requires historical filenames to match their immutable version", () => {
    expect(() =>
      buildReleaseCollection(rawManifest, {
        "./release-history/latest.json": publishedRelease({ version: "0.0.9" }),
      }),
    ).toThrow(/must be named 0\.0\.9\.json/);
  });
});

describe("formatBytes", () => {
  it("uses readable binary units", () => {
    expect(formatBytes(null)).toBe("Size pending");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MB");
  });
});
