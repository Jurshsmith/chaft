import { describe, expect, it } from "vitest";

import legacyCanaryManifest from "./release-history/0.1.0-canary.1.json";
import rawManifest from "./release-manifest.json";
import {
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
  channel?: "canary" | "stable";
}

function comingSoonCanaryRelease(version = rawManifest.version): any {
  const sourceUrl = rawManifest.sourceUrl.replace(/\/$/, "");
  const releaseUrl = `${sourceUrl}/releases`;
  const release = {
    ...structuredClone(rawManifest),
    channel: "canary",
    status: "coming-soon",
    version,
    tag: null,
    publishedAt: null,
    commit: null,
    releaseUrl,
    sourceUrl,
    releaseEvidence: null,
    assets: rawManifest.assets.map((asset) => ({
      ...asset,
      filename: null,
      url: releaseUrl,
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
    })),
  };
  return currentTargetRelease(release);
}

function publishedRelease({
  version = "0.1.0",
  tag = `v${version}`,
  artifactVersion = version,
  artifactTag = tag ?? `v${version}`,
  publishedAt = "2026-07-18T10:00:00Z",
  channel = "stable",
}: PublishedReleaseOptions = {}): any {
  const release = {
    ...structuredClone(rawManifest),
    channel,
    status: "published",
    version,
    tag,
    publishedAt,
    commit: "a".repeat(40),
    releaseUrl: `https://github.com/Jurshsmith/chaft/releases/tag/${tag ?? "missing"}`,
    releaseEvidence: null,
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
  return currentTargetRelease(release);
}

function publishedCanaryRelease(version = "0.1.0-canary.1"): any {
  const tag = `v${version}`;
  const release = publishedRelease({
    channel: "canary",
    version,
    artifactVersion: version,
    artifactTag: tag,
  });
  const evidenceFile = (filename: string) => ({
    filename,
    url: `https://github.com/Jurshsmith/chaft/releases/download/${tag}/${filename}`,
    sizeBytes: 512,
    sha256: "d".repeat(64),
  });

  release.assets = release.assets.map((asset: any) => ({
    ...asset,
    signingStatus: "unsigned-canary",
    evidence: {
      ...asset.evidence,
      signature: null,
      verification: evidenceFile(
        `chaft-desktop-${asset.os}-${asset.arch}-verification.json`,
      ),
    },
  }));
  release.releaseEvidence = {
    qtSource: evidenceFile("Chaft-Qt-6.8.4-corresponding-source.zip"),
    qtSourceChecksums: evidenceFile(
      "Chaft-Qt-6.8.4-corresponding-source.zip.sha256",
    ),
    inventory: evidenceFile("chaft-desktop-release-inventory.json"),
    aggregateChecksums: evidenceFile(
      "chaft-desktop-release-SHA256SUMS",
    ),
  };
  return release;
}

function currentTargetRelease(input: any): any {
  const release = structuredClone(input);
  const intelIndex = release.assets.findIndex(
    (asset: any) => asset.os === "macos" && asset.arch === "x86_64",
  );
  const appleSiliconIndex = release.assets.findIndex(
    (asset: any) => asset.os === "macos" && asset.arch === "arm64",
  );
  release.assets[intelIndex].platformLabel = "macOS Intel";
  if (appleSiliconIndex === -1) {
    const appleSilicon = {
      ...structuredClone(release.assets[intelIndex]),
      id: "macos-arm64-dmg",
      arch: "arm64",
      platformLabel: "macOS Apple Silicon",
    };
    release.assets.splice(intelIndex + 1, 0, appleSilicon);
  }

  const packageNames: Record<string, string> = {
    "windows-x86_64": `Chaft-${release.version}-Windows-x86_64.zip`,
    "macos-x86_64": `Chaft-${release.version}-macOS-x86_64.dmg`,
    "macos-arm64": `Chaft-${release.version}-macOS-arm64.dmg`,
    "linux-x86_64": `Chaft-${release.version}-Linux-x86_64.AppImage`,
  };
  const metadataSuffixes: Record<string, string> = {
    checksums: "SHA256SUMS",
    sbom: "sbom.cdx.json",
    provenance: "provenance.json",
    verification: "verification.json",
  };
  for (const asset of release.assets) {
    const target = `${asset.os}-${asset.arch}`;
    const filename = packageNames[target];
    asset.id = `${target}-${asset.format}`;
    if (asset.available) {
      asset.filename = filename;
      asset.url =
        `https://github.com/Jurshsmith/chaft/releases/download/${release.tag}/${filename}`;
    }
    for (const [kind, suffix] of Object.entries(metadataSuffixes)) {
      const evidence = asset.evidence[kind];
      if (evidence === null) continue;
      const evidenceFilename = `chaft-desktop-${target}-${suffix}`;
      evidence.filename = evidenceFilename;
      evidence.url =
        `https://github.com/Jurshsmith/chaft/releases/download/${release.tag}/${evidenceFilename}`;
    }
    if (asset.evidence.signature !== null) {
      const signature = asset.evidence.signature;
      signature.filename = `${filename}.${signature.format}`;
      signature.url =
        `https://github.com/Jurshsmith/chaft/releases/download/${release.tag}/${signature.filename}`;
    }
  }
  return release;
}

describe("release manifest", () => {
  it("validates the checked-in canary manifest in its declared state", () => {
    const release = validateReleaseManifest(rawManifest);
    expect(release).toEqual(currentRelease);
    expect(release.channel).toBe("canary");
    if (release.status === "published") {
      expect(release.tag).toBe(`v${release.version}`);
      expect(
        operatingSystems.every((os) =>
          release.assets.some((asset) => asset.os === os && asset.available),
        ),
      ).toBe(true);
    } else {
      expect(release.tag).toBeNull();
      expect(release.assets.every((asset) => !asset.available)).toBe(true);
    }
  });

  it("builds a current-only collection when no history files exist", () => {
    expect(
      buildReleaseCollection(rawManifest, {}).map((release) => release.version),
    ).toEqual([currentRelease.version]);
  });

  it("contains a statically rendered option for every supported platform", () => {
    expect(new Set(currentRelease.assets.map((asset) => asset.os))).toEqual(
      new Set(operatingSystems),
    );
  });

  it("accepts the exact current four-target set with both native macOS builds", () => {
    const release = validateReleaseManifest(
      currentTargetRelease(publishedCanaryRelease()),
    );
    expect(release.assets.map((asset) => `${asset.os}-${asset.arch}`)).toEqual([
      "windows-x86_64",
      "macos-x86_64",
      "macos-arm64",
      "linux-x86_64",
    ]);
  });

  it("rejects a partial current target set that is not the immutable legacy set", () => {
    const invalid = currentTargetRelease(publishedCanaryRelease());
    invalid.assets.pop();
    expect(() => validateReleaseManifest(invalid)).toThrow(
      /legacy three-target set or the current four-target set/,
    );
  });

  it("rejects a partial current release masquerading as immutable legacy", () => {
    const invalid = currentTargetRelease(publishedCanaryRelease());
    invalid.assets.splice(2, 1);
    expect(() => validateReleaseManifest(invalid)).toThrow(
      /exact immutable published legacy release/,
    );
  });

  it("rejects a forged revision for an immutable legacy release", () => {
    const invalid = structuredClone(legacyCanaryManifest);
    invalid.commit = "b".repeat(40);
    expect(() => validateReleaseManifest(invalid)).toThrow(
      /exact immutable published legacy release/,
    );
  });

  it("accepts a current four-target release with immutable legacy history", () => {
    const current = currentTargetRelease(
      publishedCanaryRelease("0.1.0-canary.3"),
    );
    current.commit = "b".repeat(40);
    current.publishedAt = "2026-07-30T04:56:00Z";
    const releases = buildReleaseCollection(current, {
      "./release-history/0.1.0-canary.1.json": legacyCanaryManifest,
    });
    expect(releases.map((release) => release.version)).toEqual([
      "0.1.0-canary.3",
      "0.1.0-canary.1",
    ]);
  });

  it("rejects a duplicated desktop target in place of Apple Silicon", () => {
    const invalid = currentTargetRelease(publishedCanaryRelease());
    invalid.assets[2] = structuredClone(invalid.assets[1]);
    expect(() => validateReleaseManifest(invalid)).toThrow(
      /desktop target macos-x86_64 is duplicated/,
    );
  });

  it("rejects an asset id that contradicts its OS, architecture, and format", () => {
    const invalid = currentTargetRelease(publishedCanaryRelease());
    invalid.assets[2].id = "macos-x86_64-dmg";
    expect(() => validateReleaseManifest(invalid)).toThrow(
      /canonical desktop target id, OS, architecture, and format/,
    );
  });

  it.each(["1", "1.2", "v1.2.3", "01.2.3", "1.02.3", "1.2.3-01"])(
    "rejects invalid semantic version %s",
    (version) => {
      const invalid = { ...structuredClone(rawManifest), version };
      expect(() => validateReleaseManifest(invalid)).toThrow(/semantic version/);
    },
  );

  it("rejects generic prerelease and build identifiers outside the exact channels", () => {
    const valid = {
      ...structuredClone(rawManifest),
      version: "1.2.3-rc.1+build.7",
    };
    expect(() => validateReleaseManifest(valid)).toThrow(/exact X\.Y\.Z-canary\.N/);
  });

  it.each(["0", "2026-07-18", "2026-02-30T10:00:00Z", "2026-07-18T10:00:00"])(
    "rejects non-canonical publication timestamp %s",
    (publishedAt) => {
      const invalid = publishedRelease({ publishedAt });
      expect(() => validateReleaseManifest(invalid)).toThrow(/RFC 3339 date-time/);
    },
  );

  it("rejects an available artifact without final integrity metadata", () => {
    const invalid = comingSoonCanaryRelease();
    invalid.assets[0]!.available = true;
    expect(() => validateReleaseManifest(invalid)).toThrow(/filename, sizeBytes, and sha256/);
  });

  it("rejects an available artifact whose signing state is still pending", () => {
    const invalid = comingSoonCanaryRelease();
    Object.assign(invalid.assets[0]!, {
      available: true,
      filename: "Chaft-0.1.0-canary.1-Windows.zip",
      sizeBytes: 1024,
      sha256: "a".repeat(64),
    });
    expect(() => validateReleaseManifest(invalid)).toThrow(/cannot be pending/);
  });

  it("rejects a generic release-page URL for an available artifact", () => {
    const invalid = publishedCanaryRelease();
    invalid.assets[0]!.url = invalid.releaseUrl;
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
    const invalid = currentTargetRelease(publishedRelease());
    invalid.assets[2].filename = invalid.assets[1].filename;
    invalid.assets[2].url = invalid.assets[1].url;
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
    invalid.publishedAt = null;
    invalid.tag = null;
    invalid.commit = null;
    expect(() => validateReleaseManifest(invalid)).toThrow(/status must be published/);
  });

  it("rejects a published release until every platform has an available asset", () => {
    const invalid = publishedCanaryRelease();
    invalid.assets[0] = comingSoonCanaryRelease().assets[0];
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

  it("rejects stable-looking artifacts advertised as an exact canary", () => {
    const invalid = publishedCanaryRelease();
    invalid.assets[0]!.filename = "Chaft-0.1.0-Windows.zip";
    invalid.assets[0]!.url =
      "https://github.com/Jurshsmith/chaft/releases/download/v0.1.0-canary.1/Chaft-0.1.0-Windows.zip";
    expect(() => validateReleaseManifest(invalid)).toThrow(
      /filename must contain release\.version 0\.1\.0-canary\.1/,
    );
  });

  it("requires every available asset URL to use the exact release tag", () => {
    const invalid = publishedRelease();
    invalid.assets[0].url = invalid.assets[0].url.replace(
      "/v0.1.0/",
      "/v0.1.1/",
    );
    expect(() => validateReleaseManifest(invalid)).toThrow(/assets\[0\]\.url.*exact GitHub URL/);
  });

  it("accepts a coherent published cross-platform release", () => {
    const release = validateReleaseManifest(publishedRelease());
    expect(release.status).toBe("published");
    expect(release.tag).toBe("v0.1.0");
  });

  it("accepts an exact published unsigned canary with all release evidence", () => {
    const release = validateReleaseManifest(publishedCanaryRelease());
    expect(release.channel).toBe("canary");
    expect(release.version).toBe("0.1.0-canary.1");
    expect(release.assets.every((asset) => asset.signingStatus === "unsigned-canary")).toBe(true);
    expect(release.releaseEvidence?.inventory.filename).toBe(
      "chaft-desktop-release-inventory.json",
    );
  });

  it("keeps unsigned-canary outside the stable channel", () => {
    const invalid = publishedRelease();
    invalid.assets[0]!.signingStatus = "unsigned-canary";
    invalid.assets[0]!.evidence.verification = {
      filename: "chaft-desktop-windows-verification.json",
      url: "https://github.com/Jurshsmith/chaft/releases/download/v0.1.0/chaft-desktop-windows-verification.json",
      sizeBytes: 512,
      sha256: "c".repeat(64),
    };
    expect(() => validateReleaseManifest(invalid)).toThrow(
      /not sufficient for an available stable windows artifact/,
    );
  });

  it("requires every unsigned canary smoke receipt and forbids signatures", () => {
    const missingReceipt = publishedCanaryRelease();
    missingReceipt.assets[0]!.evidence.verification = null;
    expect(() => validateReleaseManifest(missingReceipt)).toThrow(
      /verification is required for unsigned-canary artifacts/,
    );

    const signed = publishedCanaryRelease();
    signed.assets[2]!.evidence.signature = {
      filename: `${signed.assets[2]!.filename}.sig`,
      url: `https://github.com/Jurshsmith/chaft/releases/download/v0.1.0-canary.1/${signed.assets[2]!.filename}.sig`,
      sizeBytes: 512,
      sha256: "e".repeat(64),
      format: "sig",
    };
    expect(() => validateReleaseManifest(signed)).toThrow(
      /signature must be null for an unsigned canary/,
    );
  });

  it("requires the exact four release-level evidence files and immutable URLs", () => {
    const missing = publishedCanaryRelease();
    missing.releaseEvidence = null;
    expect(() => validateReleaseManifest(missing)).toThrow(
      /releaseEvidence is required/,
    );

    const wrongFilename = publishedCanaryRelease();
    wrongFilename.releaseEvidence!.inventory.filename = "inventory.json";
    wrongFilename.releaseEvidence!.inventory.url =
      "https://github.com/Jurshsmith/chaft/releases/download/v0.1.0-canary.1/inventory.json";
    expect(() => validateReleaseManifest(wrongFilename)).toThrow(
      /inventory\.filename must equal chaft-desktop-release-inventory\.json/,
    );

    const wrongTag = publishedCanaryRelease();
    wrongTag.releaseEvidence!.aggregateChecksums.url =
      "https://github.com/Jurshsmith/chaft/releases/download/v0.1.0-canary.2/chaft-desktop-release-SHA256SUMS";
    expect(() => validateReleaseManifest(wrongTag)).toThrow(
      /releaseEvidence\.aggregateChecksums\.url.*exact GitHub URL/,
    );
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
      currentRelease.version,
      "0.0.9",
      "0.0.8",
    ]);
  });

  it("rejects duplicate versions across current and historical manifests", () => {
    const duplicate = publishedCanaryRelease(rawManifest.version);
    expect(() =>
      buildReleaseCollection(rawManifest, {
        [`./release-history/${rawManifest.version}.json`]: duplicate,
      }),
    ).toThrow(/appears more than once/);
  });

  it("rejects a historical manifest that has not been published", () => {
    const historical = comingSoonCanaryRelease("0.0.1-canary.1");
    expect(() =>
      buildReleaseCollection(rawManifest, {
        "./release-history/0.0.1-canary.1.json": historical,
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
