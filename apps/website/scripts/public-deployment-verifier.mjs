import { createHash } from "node:crypto";

import { productionRobotsText } from "../src/lib/preview-contract.mjs";

const COMMON_HEADERS = {
  "cross-origin-opener-policy": "same-origin",
  "referrer-policy": "strict-origin-when-cross-origin",
  "x-content-type-options": "nosniff",
  "x-frame-options": "DENY",
};

const CANARY_VERSION_PATTERN =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)-canary\.([1-9]\d*)$/;
const RELEASE_STATUSES = new Set(["coming-soon", "published"]);
const RELEASE_FILE_TIMEOUT_MS = 120_000;
const REQUIRED_ASSET_EVIDENCE = [
  "checksums",
  "sbom",
  "provenance",
  "verification",
];
const REQUIRED_RELEASE_EVIDENCE = [
  "qtSource",
  "qtSourceChecksums",
  "inventory",
  "aggregateChecksums",
];
const DESKTOP_TARGETS = new Map([
  [
    "windows-x86_64",
    { id: "windows-x86_64-zip", os: "windows", arch: "x86_64", format: "zip" },
  ],
  [
    "macos-x86_64",
    { id: "macos-x86_64-dmg", os: "macos", arch: "x86_64", format: "dmg" },
  ],
  [
    "macos-arm64",
    { id: "macos-arm64-dmg", os: "macos", arch: "arm64", format: "dmg" },
  ],
  [
    "linux-x86_64",
    {
      id: "linux-x86_64-appimage",
      os: "linux",
      arch: "x86_64",
      format: "appimage",
    },
  ],
]);
const LEGACY_DESKTOP_TARGET_SET = [
  "windows-x86_64",
  "macos-x86_64",
  "linux-x86_64",
].sort();
const CURRENT_DESKTOP_TARGET_SET = [...DESKTOP_TARGETS.keys()].sort();
const IMMUTABLE_LEGACY_RELEASES = new Map([
  [
    "0.1.0-canary.1",
    {
      tag: "v0.1.0-canary.1",
      commit: "d021e7d0ea7b143a32ab49529790abc886f0f06c",
    },
  ],
  [
    "0.1.0-canary.2",
    {
      tag: "v0.1.0-canary.2",
      commit: "f21f308d3be377da78ca2123a66996aa563ba825",
    },
  ],
]);
const SENSITIVE_USE_WARNING =
  "Do not use Chaft canary builds for sensitive or production communication.";

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function normalizedOrigin(siteUrl) {
  const url = new URL(siteUrl);
  assert(
    url.protocol === "https:" &&
      !url.username &&
      !url.password &&
      url.pathname === "/" &&
      !url.search &&
      !url.hash,
    "site URL must be a pathless HTTPS origin",
  );
  return url.origin;
}

function canonicalHref(html) {
  for (const tag of html.matchAll(/<link\b[^>]*>/gi)) {
    if (!/\brel=(["'])canonical\1/i.test(tag[0])) continue;
    const href = /\bhref=(["'])(.*?)\1/i.exec(tag[0]);
    if (href) return href[2];
  }
  return null;
}

function staticAssetHref(html) {
  const match = /(?:src|href)=(["'])([^"']*\/_astro\/[^"']+)\1/i.exec(html);
  return match?.[2] ?? null;
}

function visibleText(html) {
  return html
    .replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, " ")
    .replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/&nbsp;/gi, " ")
    .replace(/&amp;/gi, "&")
    .replace(/&#39;|&apos;/gi, "'")
    .replace(/&quot;/gi, '"')
    .replace(/\s+/g, " ")
    .trim();
}

function assertCanaryWarnings(html, label) {
  const text = visibleText(html).toLowerCase();
  assert(
    text.includes("unsigned canary"),
    `${label} must identify the unsigned canary`,
  );
  assert(
    text.includes(SENSITIVE_USE_WARNING.toLowerCase()),
    `${label} must include the sensitive-use warning`,
  );
}

function decodedPathSegments(url, label) {
  try {
    return url.pathname
      .split("/")
      .filter(Boolean)
      .map((segment) => decodeURIComponent(segment));
  } catch {
    throw new Error(`${label} contains invalid percent encoding`);
  }
}

function assertExactGitHubUrl(value, expectedSegments, label) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error(`${label} must be a valid URL`);
  }
  const actualSegments = decodedPathSegments(url, label);
  assert(
    url.origin === "https://github.com" &&
      !url.username &&
      !url.password &&
      !url.search &&
      !url.hash &&
      actualSegments.length === expectedSegments.length &&
      actualSegments.every((segment, index) => segment === expectedSegments[index]),
    `${label} must be the exact immutable GitHub URL for ${expectedSegments.join("/")}`,
  );
  return url;
}

function releaseFile(file, { expectedTag, label, repository }) {
  assert(
    file && typeof file === "object" && !Array.isArray(file),
    `${label} must be an object`,
  );
  assert(
    typeof file.filename === "string" &&
      file.filename.length > 0 &&
      !file.filename.includes("/") &&
      !file.filename.includes("\\"),
    `${label}.filename must be a file name`,
  );
  assert(
    Number.isSafeInteger(file.sizeBytes) && file.sizeBytes > 0,
    `${label}.sizeBytes must be a positive integer`,
  );
  assert(
    typeof file.sha256 === "string" && /^[a-f0-9]{64}$/i.test(file.sha256),
    `${label}.sha256 must be a SHA-256 digest`,
  );
  const repositorySegments = repository.split("/");
  assertExactGitHubUrl(
    file.url,
    [
      ...repositorySegments,
      "releases",
      "download",
      expectedTag,
      file.filename,
    ],
    `${label}.url`,
  );
  return {
    filename: file.filename,
    url: file.url,
    sizeBytes: file.sizeBytes,
    sha256: file.sha256.toLowerCase(),
    references: [label],
  };
}

function canonicalDesktopTargetSet(manifest) {
  const { assets } = manifest;
  assert(
    Array.isArray(assets),
    "release assets must be an array",
  );

  const targetNames = new Set();
  const assetIds = new Set();
  for (const [index, asset] of assets.entries()) {
    const label = `release.assets[${index}]`;
    assert(
      asset && typeof asset === "object" && !Array.isArray(asset),
      `${label} must be an object`,
    );
    const targetName = `${asset.os}-${asset.arch}`;
    const expected = DESKTOP_TARGETS.get(targetName);
    assert(
      expected !== undefined,
      `${label} must identify a canonical desktop target`,
    );
    assert(
      asset.id === expected.id,
      `${label}.id must equal ${expected.id}`,
    );
    assert(
      asset.os === expected.os &&
        asset.arch === expected.arch &&
        asset.format === expected.format,
      `${label} must match the ${targetName} target contract`,
    );
    assert(!targetNames.has(targetName), `${label} target ${targetName} is duplicated`);
    assert(!assetIds.has(asset.id), `${label}.id is duplicated`);
    targetNames.add(targetName);
    assetIds.add(asset.id);
  }

  const actual = [...targetNames].sort();
  const isLegacy =
    JSON.stringify(actual) === JSON.stringify(LEGACY_DESKTOP_TARGET_SET);
  const isCurrent =
    JSON.stringify(actual) === JSON.stringify(CURRENT_DESKTOP_TARGET_SET);
  assert(
    isLegacy || isCurrent,
    "release assets must contain exactly the legacy three-target set or the current four-target set",
  );
  const targetSet = isCurrent ? "current" : "legacy";
  if (targetSet === "legacy") {
    const immutableIdentity = IMMUTABLE_LEGACY_RELEASES.get(manifest.version);
    assert(
      manifest.status === "published" &&
        immutableIdentity !== undefined &&
        manifest.tag === immutableIdentity.tag &&
        manifest.commit === immutableIdentity.commit,
      "the legacy three-target set is accepted only for an exact immutable published legacy release",
    );
  }

  const metadataSuffixes = {
    checksums: "SHA256SUMS",
    sbom: "sbom.cdx.json",
    provenance: "provenance.json",
    verification: "verification.json",
  };
  for (const [index, asset] of assets.entries()) {
    const metadataScope =
      targetSet === "legacy" ? asset.os : `${asset.os}-${asset.arch}`;
    for (const [kind, suffix] of Object.entries(metadataSuffixes)) {
      const evidence = asset.evidence?.[kind];
      if (evidence === null || evidence === undefined) continue;
      assert(
        evidence.filename === `chaft-desktop-${metadataScope}-${suffix}`,
        `release.assets[${index}].evidence.${kind}.filename must bind to ${metadataScope}`,
      );
    }
  }
  return targetSet;
}

function collectCanaryReleaseFiles(
  manifest,
  {
    expectedReleaseStatus,
    expectedReleaseTag,
    expectedReleaseVersion,
    repository,
  },
) {
  assert(
    manifest && typeof manifest === "object" && !Array.isArray(manifest),
    "release JSON must be an object",
  );
  assert(manifest.schemaVersion === 2, "release schemaVersion must be 2");
  assert(manifest.channel === "canary", "release channel must be canary");
  assert(
    manifest.status === expectedReleaseStatus,
    "release status does not match the deployment artifact",
  );
  assert(manifest.version === expectedReleaseVersion, "release version does not match");

  const repositorySegments = repository.split("/");
  assertExactGitHubUrl(manifest.sourceUrl, repositorySegments, "release source URL");
  const desktopTargetSet = canonicalDesktopTargetSet(manifest);

  if (expectedReleaseStatus === "coming-soon") {
    assert(expectedReleaseTag === null, "coming-soon release tag must be null");
    assert(manifest.tag === null, "coming-soon manifest tag must be null");
    assert(manifest.publishedAt === null, "coming-soon publishedAt must be null");
    assert(manifest.commit === null, "coming-soon commit must be null");
    assert(
      manifest.releaseEvidence === null,
      "coming-soon releaseEvidence must be null",
    );
    assertExactGitHubUrl(
      manifest.releaseUrl,
      [...repositorySegments, "releases"],
      "release URL",
    );

    const evidenceKeys = [
      "checksums",
      "sbom",
      "provenance",
      "signature",
      "verification",
    ];
    for (const [index, asset] of manifest.assets.entries()) {
      const label = `release.assets[${index}]`;
      assert(asset.available === false, `${label}.available must be false`);
      assert(asset.filename === null, `${label}.filename must be null`);
      assert(asset.sizeBytes === null, `${label}.sizeBytes must be null`);
      assert(asset.sha256 === null, `${label}.sha256 must be null`);
      assert(asset.signingStatus === "pending", `${label}.signingStatus must be pending`);
      assertExactGitHubUrl(
        asset.url,
        [...repositorySegments, "releases"],
        `${label}.url`,
      );
      assert(
        asset.evidence &&
          typeof asset.evidence === "object" &&
          !Array.isArray(asset.evidence),
        `${label}.evidence must be an object`,
      );
      assert(
        JSON.stringify(Object.keys(asset.evidence).sort()) ===
          JSON.stringify([...evidenceKeys].sort()),
        `${label}.evidence keys changed`,
      );
      assert(
        evidenceKeys.every((key) => asset.evidence[key] === null),
        `${label}.evidence must remain entirely null`,
      );
    }
    return [];
  }

  assert(manifest.tag === expectedReleaseTag, "release tag does not match");
  assert(
    typeof manifest.publishedAt === "string" && manifest.publishedAt.length > 0,
    "published canary must include publishedAt",
  );
  assert(
    typeof manifest.commit === "string" && /^[a-f0-9]{40}$/.test(manifest.commit),
    "published canary must include a full lowercase commit",
  );
  assertExactGitHubUrl(
    manifest.releaseUrl,
    [...repositorySegments, "releases", "tag", expectedReleaseTag],
    "release URL",
  );

  const declarations = [];
  for (const [index, asset] of manifest.assets.entries()) {
    const label = `release.assets[${index}]`;
    assert(asset.available === true, `${label} must be available`);
    assert(
      asset.signingStatus === "unsigned-canary",
      `${label}.signingStatus must be unsigned-canary`,
    );
    assert(
      typeof asset.filename === "string" && asset.filename.includes(expectedReleaseVersion),
      `${label}.filename must contain ${expectedReleaseVersion}`,
    );
    declarations.push(
      releaseFile(asset, {
        expectedTag: expectedReleaseTag,
        label: `${label}.package`,
        repository,
      }),
    );

    assert(
      asset.evidence && typeof asset.evidence === "object" && !Array.isArray(asset.evidence),
      `${label}.evidence must be an object`,
    );
    assert(
      JSON.stringify(Object.keys(asset.evidence).sort()) ===
        JSON.stringify(
          [
            "checksums",
            "sbom",
            "provenance",
            "signature",
            "verification",
          ].sort(),
        ),
      `${label}.evidence keys changed`,
    );
    assert(
      asset.evidence.signature === null,
      `${label}.evidence.signature must be null`,
    );
    for (const key of REQUIRED_ASSET_EVIDENCE) {
      assert(
        asset.evidence[key] !== null && asset.evidence[key] !== undefined,
        `${label}.evidence.${key} is required`,
      );
    }
    for (const [key, evidence] of Object.entries(asset.evidence)) {
      if (evidence === null) continue;
      declarations.push(
        releaseFile(evidence, {
          expectedTag: expectedReleaseTag,
          label: `${label}.evidence.${key}`,
          repository,
        }),
      );
    }
  }
  assert(
    manifest.releaseEvidence &&
      typeof manifest.releaseEvidence === "object" &&
      !Array.isArray(manifest.releaseEvidence),
    "releaseEvidence must be an object",
  );
  assert(
    JSON.stringify(Object.keys(manifest.releaseEvidence).sort()) ===
      JSON.stringify([...REQUIRED_RELEASE_EVIDENCE].sort()),
    "releaseEvidence keys changed",
  );
  for (const key of REQUIRED_RELEASE_EVIDENCE) {
    assert(
      manifest.releaseEvidence[key] !== null &&
        manifest.releaseEvidence[key] !== undefined,
      `releaseEvidence.${key} is required`,
    );
  }
  for (const [key, evidence] of Object.entries(manifest.releaseEvidence)) {
    if (evidence === null) continue;
    declarations.push(
      releaseFile(evidence, {
        expectedTag: expectedReleaseTag,
        label: `releaseEvidence.${key}`,
        repository,
      }),
    );
  }

  const uniqueFiles = new Map();
  for (const declaration of declarations) {
    const existing = uniqueFiles.get(declaration.url);
    if (!existing) {
      uniqueFiles.set(declaration.url, declaration);
      continue;
    }
    assert(
      existing.filename === declaration.filename &&
        existing.sizeBytes === declaration.sizeBytes &&
        existing.sha256 === declaration.sha256,
      `release URL ${declaration.url} has conflicting manifest metadata`,
    );
    existing.references.push(...declaration.references);
  }
  const expectedReleaseFileCount = desktopTargetSet === "current" ? 24 : 19;
  assert(
    uniqueFiles.size === expectedReleaseFileCount,
    `published canary must expose exactly ${expectedReleaseFileCount} release files`,
  );
  return [...uniqueFiles.values()];
}

async function verifyReleaseFile(file, fetchImpl) {
  const response = await fetchImpl(file.url, {
    redirect: "follow",
    signal: AbortSignal.timeout(RELEASE_FILE_TIMEOUT_MS),
  });
  assert(response.status === 200, `${file.filename} must download with status 200`);
  assert(response.body, `${file.filename} response body is missing`);

  const hash = createHash("sha256");
  let sizeBytes = 0;
  for await (const chunk of response.body) {
    const bytes =
      chunk instanceof Uint8Array
        ? chunk
        : new Uint8Array(chunk);
    sizeBytes += bytes.byteLength;
    assert(
      sizeBytes <= file.sizeBytes,
      `${file.filename} exceeds manifest size ${file.sizeBytes}`,
    );
    hash.update(bytes);
  }
  assert(
    sizeBytes === file.sizeBytes,
    `${file.filename} size ${sizeBytes} does not match manifest size ${file.sizeBytes}`,
  );
  const sha256 = hash.digest("hex");
  assert(
    sha256 === file.sha256,
    `${file.filename} SHA-256 ${sha256} does not match manifest ${file.sha256}`,
  );
  return { response, sizeBytes, sha256 };
}

function headerIncludes(headers, name, expected) {
  return (headers.get(name) ?? "").toLowerCase().includes(expected.toLowerCase());
}

function assertCommonHeaders(response, label) {
  for (const [name, expected] of Object.entries(COMMON_HEADERS)) {
    assert(response.headers.get(name) === expected, `${label} is missing ${name}: ${expected}`);
  }
  assert(
    headerIncludes(response.headers, "permissions-policy", "camera=()") &&
      headerIncludes(response.headers, "permissions-policy", "microphone=()") &&
      headerIncludes(response.headers, "permissions-policy", "payment=()"),
    `${label} has an unexpected permissions-policy`,
  );
}

async function body(response, label) {
  const text = await response.text();
  assert(!text.includes("website-validation.invalid"), `${label} contains validation-origin data`);
  return text;
}

export async function verifyPublicDeployment({
  alternateSiteUrl,
  expectedCommit,
  expectedReleaseManifestSha256,
  expectedReleaseStatus,
  expectedReleaseTag,
  expectedReleaseVersion,
  fetchImpl = fetch,
  repository,
  siteUrl,
}) {
  assert(/^[a-f0-9]{40}$/.test(expectedCommit), "expected commit must be a full SHA-1");
  assert(
    typeof expectedReleaseVersion === "string" &&
      CANARY_VERSION_PATTERN.test(expectedReleaseVersion),
    "expected release version must be a canary semantic version without a leading v",
  );
  assert(
    RELEASE_STATUSES.has(expectedReleaseStatus),
    "expected release status must be coming-soon or published",
  );
  assert(
    /^[a-f0-9]{64}$/.test(expectedReleaseManifestSha256),
    "expected release manifest SHA-256 must be a lowercase digest",
  );
  assert(
    expectedReleaseStatus === "published"
      ? expectedReleaseTag === `v${expectedReleaseVersion}`
      : expectedReleaseTag === null || expectedReleaseTag === undefined,
    expectedReleaseStatus === "published"
      ? `expected release tag must equal v${expectedReleaseVersion}`
      : "coming-soon release tag must be omitted",
  );
  assert(repository === "Jurshsmith/chaft", "unexpected source repository");
  const origin = normalizedOrigin(siteUrl);
  const alternateOrigin = alternateSiteUrl ? normalizedOrigin(alternateSiteUrl) : null;
  if (alternateOrigin) assert(alternateOrigin !== origin, "alternate origin must differ");
  const checks = [];

  const record = (name, response, detail) => {
    checks.push({ name, status: response.status, detail });
  };

  const markerResponse = await fetchImpl(`${origin}/.well-known/chaft-deployment.json`, {
    redirect: "manual",
    signal: AbortSignal.timeout(10_000),
  });
  assert(markerResponse.status === 200, "deployment marker must return 200");
  assertCommonHeaders(markerResponse, "deployment marker");
  assert(
    headerIncludes(markerResponse.headers, "cache-control", "no-store"),
    "deployment marker must use no-store",
  );
  const markerText = await body(markerResponse, "deployment marker");
  const marker = JSON.parse(markerText);
  const keys = Object.keys(marker).sort();
  const expectedKeys = [
    "artifactKind",
    "schemaVersion",
    "siteUrl",
    "sourceCommit",
    "sourceRepository",
  ].sort();
  assert(JSON.stringify(keys) === JSON.stringify(expectedKeys), "deployment marker shape changed");
  assert(marker.schemaVersion === 1, "deployment marker schemaVersion must be 1");
  assert(marker.artifactKind === "chaft-website", "deployment marker artifactKind changed");
  assert(marker.sourceRepository === repository, "deployment marker repository does not match");
  assert(marker.sourceCommit === expectedCommit, "deployment marker commit does not match");
  assert(marker.siteUrl === origin, "deployment marker site URL does not match");
  record("deployment-marker", markerResponse, expectedCommit);

  const pages = [
    ["/", 200, "home", "/"],
    ["/download/", 200, "download", "/download/"],
    ["/security/", 200, "security", "/security/"],
    ["/releases/", 200, "releases", "/releases/"],
    [
      `/releases/${expectedReleaseVersion}/`,
      200,
      "release-version",
      `/releases/${expectedReleaseVersion}/`,
    ],
    ["/definitely-not-a-page-chaft-verification", 404, "not-found", null],
  ];
  let homeHtml = "";
  for (const [pathname, status, label, expectedCanonical] of pages) {
    const response = await fetchImpl(`${origin}${pathname}`, {
      redirect: "manual",
      signal: AbortSignal.timeout(10_000),
    });
    assert(response.status === status, `${label} must return ${status}`);
    assertCommonHeaders(response, label);
    const html = await body(response, label);
    if (expectedCanonical) {
      assert(
        canonicalHref(html) === `${origin}${expectedCanonical}`,
        `${label} canonical URL does not match ${origin}${expectedCanonical}`,
      );
    }
    if (
      expectedReleaseStatus === "published" &&
      ["download", "releases", "release-version"].includes(label)
    ) {
      assertCanaryWarnings(html, label);
    }
    if (["download", "releases", "release-version"].includes(label)) {
      const visibleReleaseIdentity =
        expectedReleaseTag ?? `v${expectedReleaseVersion}`;
      assert(
        visibleText(html).includes(visibleReleaseIdentity),
        `${label} must identify ${visibleReleaseIdentity}`,
      );
    }
    if (label === "home") homeHtml = html;
    record(label, response, pathname);
  }
  assert(
    /\bdata-chaft-hero=(["'])baseline\1/i.test(homeHtml),
    "production home must render the baseline hero",
  );

  for (const [pathname, expectedStatus, expectedLocation, label] of [
    ["/downloads", 301, `${origin}/download/`, "downloads-redirect"],
    ["/source", 302, "https://github.com/Jurshsmith/chaft", "source-redirect"],
  ]) {
    const response = await fetchImpl(`${origin}${pathname}`, {
      redirect: "manual",
      signal: AbortSignal.timeout(10_000),
    });
    assert(response.status === expectedStatus, `${label} must return ${expectedStatus}`);
    assert(
      new URL(response.headers.get("location"), origin).href === expectedLocation,
      `${label} location does not match`,
    );
    record(label, response, expectedLocation);
  }

  const currentReleaseResponse = await fetchImpl(
    `${origin}/releases/current.json`,
    {
      redirect: "manual",
      signal: AbortSignal.timeout(10_000),
    },
  );
  assert(
    currentReleaseResponse.status === 200,
    "current release JSON must return 200",
  );
  assertCommonHeaders(currentReleaseResponse, "current release JSON");
  assert(
    headerIncludes(currentReleaseResponse.headers, "cache-control", "max-age=0") &&
      headerIncludes(currentReleaseResponse.headers, "cache-control", "must-revalidate"),
    "current release JSON must revalidate",
  );
  assert(
    headerIncludes(currentReleaseResponse.headers, "content-type", "application/json"),
    "current release JSON must use application/json",
  );
  const currentReleaseText = await body(
    currentReleaseResponse,
    "current release JSON",
  );
  const currentReleaseSha256 = createHash("sha256")
    .update(currentReleaseText)
    .digest("hex");
  assert(
    currentReleaseSha256 === expectedReleaseManifestSha256,
    "current release JSON does not match the deployment artifact SHA-256",
  );
  const currentRelease = JSON.parse(currentReleaseText);
  record("current-release", currentReleaseResponse, "/releases/current.json");

  const versionReleasePath = `/releases/${expectedReleaseVersion}.json`;
  const versionReleaseResponse = await fetchImpl(
    `${origin}${versionReleasePath}`,
    {
      redirect: "manual",
      signal: AbortSignal.timeout(10_000),
    },
  );
  assert(
    versionReleaseResponse.status === 200,
    "version release JSON must return 200",
  );
  assertCommonHeaders(versionReleaseResponse, "version release JSON");
  assert(
    headerIncludes(versionReleaseResponse.headers, "cache-control", "max-age=0") &&
      headerIncludes(versionReleaseResponse.headers, "cache-control", "must-revalidate"),
    "version release JSON must revalidate",
  );
  assert(
    headerIncludes(versionReleaseResponse.headers, "content-type", "application/json"),
    "version release JSON must use application/json",
  );
  const versionRelease = JSON.parse(
    await body(versionReleaseResponse, "version release JSON"),
  );
  assert(
    JSON.stringify(versionRelease) === JSON.stringify(currentRelease),
    "current and version release JSON must be identical",
  );
  record("version-release", versionReleaseResponse, versionReleasePath);

  const releaseFiles = collectCanaryReleaseFiles(currentRelease, {
    expectedReleaseStatus,
    expectedReleaseTag,
    expectedReleaseVersion,
    repository,
  });
  for (const file of releaseFiles) {
    const verified = await verifyReleaseFile(file, fetchImpl);
    checks.push({
      name: `release-file:${file.filename}`,
      status: verified.response.status,
      detail: `${file.sizeBytes} bytes sha256:${file.sha256}`,
    });
  }

  for (const [pathname, label] of [
    ["/robots.txt", "robots"],
    ["/sitemap-index.xml", "sitemap"],
  ]) {
    const response = await fetchImpl(`${origin}${pathname}`, {
      redirect: "manual",
      signal: AbortSignal.timeout(10_000),
    });
    assert(response.status === 200, `${label} must return 200`);
    // The zone-level managed robots feature must remain off. Production and
    // Preview indexing policies are exact source-controlled Worker bytes.
    assertCommonHeaders(response, label);
    const text = await body(response, label);
    if (label === "robots") {
      assert(
        text === productionRobotsText(`${origin}/sitemap-index.xml`),
        "robots does not match the exact source-controlled production policy",
      );
    } else {
      assert(
        text.includes(origin),
        `${label} does not reference the production origin`,
      );
    }
    record(label, response, pathname);
  }

  const assetHref = staticAssetHref(homeHtml);
  assert(assetHref, "home page does not reference a hashed Astro asset");
  const assetUrl = new URL(assetHref, origin);
  assert(assetUrl.origin === origin && assetUrl.pathname.startsWith("/_astro/"), "invalid asset URL");
  const asset = await fetchImpl(assetUrl, {
    redirect: "manual",
    signal: AbortSignal.timeout(10_000),
  });
  assert(asset.status === 200, "referenced Astro asset must return 200");
  assertCommonHeaders(asset, "Astro asset");
  assert(
    headerIncludes(asset.headers, "cache-control", "max-age=31536000") &&
      headerIncludes(asset.headers, "cache-control", "immutable"),
    "Astro asset must use one-year immutable caching",
  );
  await body(asset, "Astro asset");
  record("hashed-asset", asset, assetUrl.pathname);

  if (alternateOrigin) {
    const alternateHome = await fetchImpl(`${alternateOrigin}/`, {
      redirect: "manual",
      signal: AbortSignal.timeout(10_000),
    });
    assert(alternateHome.status === 200, "alternate hostname must return 200");
    assertCommonHeaders(alternateHome, "alternate hostname");
    const alternateHtml = await body(alternateHome, "alternate hostname");
    assert(
      canonicalHref(alternateHtml) === `${origin}/`,
      "alternate hostname must retain the apex canonical URL",
    );
    record("alternate-home", alternateHome, alternateOrigin);
  }

  return {
    schemaVersion: 1,
    artifactKind: "chaft-website-public-verification",
    verifiedAt: new Date().toISOString(),
    siteUrl: origin,
    alternateSiteUrl: alternateOrigin,
    repository,
    expectedCommit,
    expectedReleaseManifestSha256,
    expectedReleaseStatus,
    expectedReleaseTag: expectedReleaseTag ?? null,
    expectedReleaseVersion,
    releaseFilesVerified: releaseFiles.length,
    checks,
    result: "passed",
  };
}
