import rawManifest from "./release-manifest.json";

const rawHistoricalManifests = import.meta.glob("./release-history/*.json", {
  eager: true,
  import: "default",
}) as Record<string, unknown>;

export const operatingSystems = ["windows", "macos", "linux"] as const;
export const releaseChannels = ["preview", "stable"] as const;
export const releaseStatuses = ["coming-soon", "published"] as const;
export const signingStatuses = [
  "pending",
  "checksummed",
  "signed",
  "notarized",
] as const;

export type OperatingSystem = (typeof operatingSystems)[number];
export type ReleaseChannel = (typeof releaseChannels)[number];
export type ReleaseStatus = (typeof releaseStatuses)[number];
export type SigningStatus = (typeof signingStatuses)[number];
export type SignatureFormat = "sig" | "asc";

const artifactFormatExtensions: Record<
  OperatingSystem,
  Readonly<Record<string, readonly string[]>>
> = {
  windows: {
    zip: [".zip"],
    msi: [".msi"],
    exe: [".exe"],
  },
  macos: {
    dmg: [".dmg"],
  },
  linux: {
    tgz: [".tgz", ".tar.gz"],
    "tar.gz": [".tar.gz"],
    appimage: [".appimage"],
  },
};

export interface ReleaseAsset {
  id: string;
  os: OperatingSystem;
  platformLabel: string;
  arch: string;
  format: string;
  filename: string | null;
  url: string;
  available: boolean;
  sizeBytes: number | null;
  sha256: string | null;
  signingStatus: SigningStatus;
  evidence: ReleaseAssetEvidence;
}

export interface ReleaseEvidenceFile {
  filename: string;
  url: string;
  sizeBytes: number;
  sha256: string;
}

export interface ReleaseSignatureEvidenceFile extends ReleaseEvidenceFile {
  format: SignatureFormat;
}

export interface ReleaseAssetEvidence {
  checksums: ReleaseEvidenceFile | null;
  sbom: ReleaseEvidenceFile | null;
  provenance: ReleaseEvidenceFile | null;
  signature: ReleaseSignatureEvidenceFile | null;
  verification: ReleaseEvidenceFile | null;
}

export interface ReleaseManifest {
  schemaVersion: 2;
  channel: ReleaseChannel;
  status: ReleaseStatus;
  version: string;
  tag: string | null;
  publishedAt: string | null;
  commit: string | null;
  releaseUrl: string;
  sourceUrl: string;
  assets: ReleaseAsset[];
}

const semanticVersionPattern = new RegExp(
  "^(0|[1-9]\\d*)\\.(0|[1-9]\\d*)\\.(0|[1-9]\\d*)" +
    "(?:-(?:(?:0|[1-9]\\d*|\\d*[A-Za-z-][0-9A-Za-z-]*)" +
    "(?:\\.(?:0|[1-9]\\d*|\\d*[A-Za-z-][0-9A-Za-z-]*))*))?" +
    "(?:\\+(?:[0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*))?$",
);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requireString(
  record: Record<string, unknown>,
  key: string,
  context: string,
): string {
  const value = record[key];
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${context}.${key} must be a non-empty string`);
  }
  return value;
}

function nullableString(
  record: Record<string, unknown>,
  key: string,
  context: string,
): string | null {
  const value = record[key];
  if (value === null) return null;
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${context}.${key} must be null or a non-empty string`);
  }
  return value;
}

function requireUrl(value: string, context: string): string {
  const parsed = new URL(value);
  if (parsed.protocol !== "https:") {
    throw new Error(`${context} must use https`);
  }
  return value;
}

function parseVersion(value: unknown): string {
  if (typeof value !== "string" || !semanticVersionPattern.test(value)) {
    throw new Error(
      "release.version must be a semantic version without a leading v (for example, 1.2.3 or 1.2.3-beta.1)",
    );
  }
  return value;
}

function isRfc3339DateTime(value: string): boolean {
  const match = value.match(
    /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(?:Z|([+-])(\d{2}):(\d{2}))$/,
  );
  if (!match) return false;

  const [, yearText, monthText, dayText, hourText, minuteText, secondText, , offsetHourText, offsetMinuteText] = match;
  const year = Number(yearText);
  const month = Number(monthText);
  const day = Number(dayText);
  const hour = Number(hourText);
  const minute = Number(minuteText);
  const second = Number(secondText);
  const offsetHour = offsetHourText === undefined ? 0 : Number(offsetHourText);
  const offsetMinute = offsetMinuteText === undefined ? 0 : Number(offsetMinuteText);
  const daysInMonth =
    year > 0 && month >= 1 && month <= 12
      ? new Date(Date.UTC(year, month, 0)).getUTCDate()
      : 0;

  return (
    day >= 1 &&
    day <= daysInMonth &&
    hour <= 23 &&
    minute <= 59 &&
    second <= 59 &&
    offsetHour <= 23 &&
    offsetMinute <= 59 &&
    !Number.isNaN(Date.parse(value))
  );
}

function decodedPathSegments(url: URL, context: string): string[] {
  try {
    return url.pathname
      .split("/")
      .filter(Boolean)
      .map((segment) => decodeURIComponent(segment));
  } catch {
    throw new Error(`${context} contains invalid percent encoding`);
  }
}

function requireExactGitHubUrl(
  value: string,
  expectedOrigin: string,
  expectedSegments: readonly string[],
  context: string,
): void {
  const parsed = new URL(value);
  const actualSegments = decodedPathSegments(parsed, context);
  if (
    parsed.origin !== expectedOrigin ||
    parsed.search !== "" ||
    parsed.hash !== "" ||
    actualSegments.length !== expectedSegments.length ||
    actualSegments.some((segment, index) => segment !== expectedSegments[index])
  ) {
    throw new Error(
      `${context} must be the exact GitHub URL for ${expectedSegments.join("/")}`,
    );
  }
}

function filenameContainsVersion(filename: string, version: string): boolean {
  const escapedVersion = version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(
    `(?:^|[^0-9A-Za-z])${escapedVersion}(?:$|[^0-9A-Za-z])`,
  ).test(filename);
}

function validatePublishedReleaseCoherence(release: ReleaseManifest): void {
  if (release.tag === null) {
    throw new Error("release.tag is required for a published release");
  }

  const expectedTag = `v${release.version}`;
  if (release.tag !== expectedTag) {
    throw new Error(`release.tag must equal ${expectedTag}`);
  }

  const source = new URL(release.sourceUrl);
  const repositorySegments = decodedPathSegments(source, "release.sourceUrl");
  if (
    source.hostname.toLowerCase() !== "github.com" ||
    source.search !== "" ||
    source.hash !== "" ||
    repositorySegments.length !== 2
  ) {
    throw new Error("release.sourceUrl must be a GitHub repository root URL");
  }

  requireExactGitHubUrl(
    release.releaseUrl,
    source.origin,
    [...repositorySegments, "releases", "tag", release.tag],
    "release.releaseUrl",
  );

  for (const [index, asset] of release.assets.entries()) {
    if (!asset.available || asset.filename === null) continue;
    const context = `release.assets[${index}]`;
    if (!filenameContainsVersion(asset.filename, release.version)) {
      throw new Error(`${context}.filename must contain release.version ${release.version}`);
    }
    requireExactGitHubUrl(
      asset.url,
      source.origin,
      [...repositorySegments, "releases", "download", release.tag, asset.filename],
      `${context}.url`,
    );

    for (const [kind, evidence] of Object.entries(asset.evidence)) {
      if (evidence === null) continue;
      requireExactGitHubUrl(
        evidence.url,
        source.origin,
        [
          ...repositorySegments,
          "releases",
          "download",
          release.tag,
          evidence.filename,
        ],
        `${context}.evidence.${kind}.url`,
      );
    }
  }
}

function parseEnum<const T extends readonly string[]>(
  value: unknown,
  values: T,
  context: string,
): T[number] {
  if (typeof value !== "string" || !values.includes(value)) {
    throw new Error(`${context} must be one of: ${values.join(", ")}`);
  }
  return value as T[number];
}

function parseEvidenceFile(
  value: unknown,
  context: string,
): ReleaseEvidenceFile {
  if (!isRecord(value)) throw new Error(`${context} must be an object`);
  const filename = requireString(value, "filename", context);
  if (filename.includes("/") || filename.includes("\\")) {
    throw new Error(`${context}.filename must be a file name, not a path`);
  }
  const sizeBytes = value.sizeBytes;
  if (
    typeof sizeBytes !== "number" ||
    !Number.isSafeInteger(sizeBytes) ||
    sizeBytes <= 0
  ) {
    throw new Error(`${context}.sizeBytes must be a positive integer`);
  }
  const sha256 = requireString(value, "sha256", context);
  if (!/^[a-f0-9]{64}$/i.test(sha256)) {
    throw new Error(`${context}.sha256 must be a 64-character hexadecimal digest`);
  }
  const url = requireUrl(requireString(value, "url", context), `${context}.url`);
  if (decodedPathSegments(new URL(url), `${context}.url`).at(-1) !== filename) {
    throw new Error(`${context}.url must point directly to its final filename`);
  }
  return { filename, url, sizeBytes, sha256 };
}

function parseAssetEvidence(
  value: unknown,
  context: string,
  os: OperatingSystem,
  artifactFilename: string | null,
  available: boolean,
  signingStatus: SigningStatus,
): ReleaseAssetEvidence {
  if (!isRecord(value)) throw new Error(`${context} must be an object`);
  const nullableEvidence = (key: string): ReleaseEvidenceFile | null => {
    const evidence = value[key];
    return evidence === null
      ? null
      : parseEvidenceFile(evidence, `${context}.${key}`);
  };

  const checksums = nullableEvidence("checksums");
  const sbom = nullableEvidence("sbom");
  const provenance = nullableEvidence("provenance");
  const verification = nullableEvidence("verification");
  const rawSignature = value.signature;
  let signature: ReleaseSignatureEvidenceFile | null = null;
  if (rawSignature !== null) {
    if (!isRecord(rawSignature)) {
      throw new Error(`${context}.signature must be null or an object`);
    }
    const file = parseEvidenceFile(rawSignature, `${context}.signature`);
    const format = parseEnum(
      rawSignature.format,
      ["sig", "asc"] as const,
      `${context}.signature.format`,
    );
    signature = { ...file, format };
  }

  const allEvidence = [checksums, sbom, provenance, signature, verification];
  if (!available && allEvidence.some((file) => file !== null)) {
    throw new Error(`${context} must remain null while the artifact is unavailable`);
  }
  if (available && (checksums === null || sbom === null || provenance === null)) {
    throw new Error(
      `${context} must include checksums, SBOM, and provenance for an available artifact`,
    );
  }
  if (
    available &&
    (signingStatus === "signed" || signingStatus === "notarized") &&
    verification === null
  ) {
    throw new Error(
      `${context}.verification is required for ${signingStatus} artifacts`,
    );
  }
  if (available && signingStatus === "checksummed" && verification !== null) {
    throw new Error(
      `${context}.verification must be null for a checksummed-only artifact`,
    );
  }
  if (available && os === "linux" && signingStatus === "signed" && signature === null) {
    throw new Error(`${context}.signature is required for signed Linux artifacts`);
  }

  const expectedMetadataNames = {
    checksums: `chaft-desktop-${os}-SHA256SUMS`,
    sbom: `chaft-desktop-${os}-sbom.cdx.json`,
    provenance: `chaft-desktop-${os}-provenance.json`,
    verification: `chaft-desktop-${os}-verification.json`,
  } as const;
  for (const [kind, expectedFilename] of Object.entries(expectedMetadataNames)) {
    const file = { checksums, sbom, provenance, verification }[
      kind as keyof typeof expectedMetadataNames
    ];
    if (file !== null && file.filename !== expectedFilename) {
      throw new Error(`${context}.${kind}.filename must equal ${expectedFilename}`);
    }
  }
  if (
    signature !== null &&
    (artifactFilename === null ||
      signature.filename !== `${artifactFilename}.${signature.format}`)
  ) {
    throw new Error(
      `${context}.signature.filename must identify the artifact's detached signature`,
    );
  }

  return { checksums, sbom, provenance, signature, verification };
}

function parseAsset(value: unknown, index: number): ReleaseAsset {
  const context = `release.assets[${index}]`;
  if (!isRecord(value)) throw new Error(`${context} must be an object`);

  const available = value.available;
  if (typeof available !== "boolean") {
    throw new Error(`${context}.available must be a boolean`);
  }

  const sizeBytes = value.sizeBytes;
  if (
    sizeBytes !== null &&
    (typeof sizeBytes !== "number" || !Number.isSafeInteger(sizeBytes) || sizeBytes <= 0)
  ) {
    throw new Error(`${context}.sizeBytes must be null or a positive integer`);
  }

  const sha256 = nullableString(value, "sha256", context);
  if (sha256 !== null && !/^[a-f0-9]{64}$/i.test(sha256)) {
    throw new Error(`${context}.sha256 must be a 64-character hexadecimal digest`);
  }

  const filename = nullableString(value, "filename", context);
  if (filename !== null && (filename.includes("/") || filename.includes("\\"))) {
    throw new Error(`${context}.filename must be a file name, not a path`);
  }
  if (available && (filename === null || sizeBytes === null || sha256 === null)) {
    throw new Error(`${context} must include filename, sizeBytes, and sha256 when available`);
  }

  const os = parseEnum(value.os, operatingSystems, `${context}.os`);
  const format = requireString(value, "format", context).toLowerCase();
  const allowedExtensions = artifactFormatExtensions[os][format];
  if (!allowedExtensions) {
    throw new Error(
      `${context}.format must be one of: ${Object.keys(artifactFormatExtensions[os]).join(", ")}`,
    );
  }
  if (
    filename !== null &&
    !allowedExtensions.some((extension) =>
      filename.toLowerCase().endsWith(extension),
    )
  ) {
    throw new Error(
      `${context}.filename extension must match ${os} ${format} format`,
    );
  }
  const url = requireUrl(requireString(value, "url", context), `${context}.url`);
  const signingStatus = parseEnum(
    value.signingStatus,
    signingStatuses,
    `${context}.signingStatus`,
  );
  if (available && signingStatus === "pending") {
    throw new Error(`${context}.signingStatus cannot be pending when available`);
  }
  if (!available && signingStatus !== "pending") {
    throw new Error(`${context}.signingStatus must be pending while unavailable`);
  }
  if (available) {
    const requiredSigningState: Record<OperatingSystem, readonly SigningStatus[]> = {
      windows: ["signed"],
      macos: ["notarized"],
      linux: ["checksummed", "signed"],
    };
    if (!requiredSigningState[os].includes(signingStatus)) {
      throw new Error(
        `${context}.signingStatus is not sufficient for an available ${os} artifact`,
      );
    }
    if (decodedPathSegments(new URL(url), `${context}.url`).at(-1) !== filename) {
      throw new Error(`${context}.url must point directly to its final filename`);
    }
  }
  const evidence = parseAssetEvidence(
    value.evidence,
    `${context}.evidence`,
    os,
    filename,
    available,
    signingStatus,
  );

  return {
    id: requireString(value, "id", context),
    os,
    platformLabel: requireString(value, "platformLabel", context),
    arch: requireString(value, "arch", context),
    format,
    filename,
    url,
    available,
    sizeBytes,
    sha256,
    signingStatus,
    evidence,
  };
}

export function validateReleaseManifest(value: unknown): ReleaseManifest {
  if (!isRecord(value)) throw new Error("release manifest must be an object");
  if (value.schemaVersion !== 2) {
    throw new Error("release.schemaVersion must equal 2");
  }
  if (!Array.isArray(value.assets) || value.assets.length === 0) {
    throw new Error("release.assets must contain at least one asset");
  }

  const assets = value.assets.map(parseAsset);
  for (const os of operatingSystems) {
    if (!assets.some((asset) => asset.os === os)) {
      throw new Error(`release.assets must include a ${os} option`);
    }
  }
  if (new Set(assets.map((asset) => asset.id)).size !== assets.length) {
    throw new Error("release asset ids must be unique");
  }
  const availableAssets = assets.filter((asset) => asset.available);
  const availableFilenames = availableAssets.map((asset) => asset.filename);
  if (new Set(availableFilenames).size !== availableFilenames.length) {
    throw new Error("available release asset filenames must be unique");
  }
  const availableUrls = availableAssets.map((asset) => asset.url);
  if (new Set(availableUrls).size !== availableUrls.length) {
    throw new Error("available release asset URLs must be unique");
  }

  const status = parseEnum(value.status, releaseStatuses, "release.status");
  const publishedAt = nullableString(value, "publishedAt", "release");
  if (status === "published" && publishedAt === null) {
    throw new Error("release.publishedAt is required for a published release");
  }
  if (publishedAt !== null && !isRfc3339DateTime(publishedAt)) {
    throw new Error(
      "release.publishedAt must be an RFC 3339 date-time with a timezone",
    );
  }
  if (status !== "published" && assets.some((asset) => asset.available)) {
    throw new Error("release.status must be published before an asset can be available");
  }
  if (
    status === "published" &&
    operatingSystems.some((os) => !assets.some((asset) => asset.os === os && asset.available))
  ) {
    throw new Error("a published release must include an available asset for every platform");
  }

  const commit = nullableString(value, "commit", "release");
  if (commit !== null && !/^[a-f0-9]{40,64}$/i.test(commit)) {
    throw new Error("release.commit must be a 40-to-64-character hexadecimal revision");
  }
  if (status === "published" && commit === null) {
    throw new Error("release.commit is required for a published release");
  }

  const version = parseVersion(value.version);
  const tag = nullableString(value, "tag", "release");
  if (tag !== null && tag !== `v${version}`) {
    throw new Error(`release.tag must equal v${version}`);
  }

  const release: ReleaseManifest = {
    schemaVersion: 2,
    channel: parseEnum(value.channel, releaseChannels, "release.channel"),
    status,
    version,
    tag,
    publishedAt,
    commit,
    releaseUrl: requireUrl(
      requireString(value, "releaseUrl", "release"),
      "release.releaseUrl",
    ),
    sourceUrl: requireUrl(
      requireString(value, "sourceUrl", "release"),
      "release.sourceUrl",
    ),
    assets,
  };

  if (release.status === "published") {
    validatePublishedReleaseCoherence(release);
  }

  return release;
}

function historyFilename(path: string): string {
  return path.split(/[\\/]/).at(-1) ?? path;
}

export function buildReleaseCollection(
  currentValue: unknown,
  historicalValues: Readonly<Record<string, unknown>> = {},
): readonly ReleaseManifest[] {
  const current = validateReleaseManifest(currentValue);
  const historical = Object.entries(historicalValues).map(([path, value]) => {
    let release: ReleaseManifest;
    try {
      release = validateReleaseManifest(value);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      throw new Error(`invalid historical release manifest ${path}: ${message}`);
    }

    if (release.status !== "published") {
      throw new Error(`historical release manifest ${path} must be published`);
    }
    if (historyFilename(path) !== `${release.version}.json`) {
      throw new Error(
        `historical release manifest ${path} must be named ${release.version}.json`,
      );
    }
    return release;
  });

  const releases = [current, ...historical];
  const versions = new Set<string>();
  for (const release of releases) {
    if (versions.has(release.version)) {
      throw new Error(`release version ${release.version} appears more than once`);
    }
    versions.add(release.version);
  }

  historical.sort((left, right) => {
    const publishedDifference =
      Date.parse(right.publishedAt ?? "") - Date.parse(left.publishedAt ?? "");
    return (
      publishedDifference ||
      right.version.localeCompare(left.version, undefined, { numeric: true })
    );
  });

  return Object.freeze([current, ...historical]);
}

export function formatBytes(bytes: number | null): string {
  if (bytes === null) return "Size pending";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`;
}

export const currentRelease = validateReleaseManifest(rawManifest);
export const allReleases = buildReleaseCollection(
  rawManifest,
  rawHistoricalManifests,
);
