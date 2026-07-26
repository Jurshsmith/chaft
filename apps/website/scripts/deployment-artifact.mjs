import { createHash } from "node:crypto";
import {
  copyFile,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  realpath,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { TextDecoder } from "node:util";

export const ARTIFACT_KIND = "chaft-website";
export const ARTIFACT_MANIFEST = "artifact-manifest.json";
export const ASSET_ROOT = "site";
export const DEPLOYMENT_MARKER = ".well-known/chaft-deployment.json";
export const MAX_ASSET_COUNT = 20_000;
export const MAX_ASSET_SIZE_BYTES = 25 * 1024 * 1024;

const VALIDATION_ORIGIN = Buffer.from("website-validation.invalid", "utf8");
const PORTABLE_SEGMENT = /^[A-Za-z0-9._~@+-]+$/;
const SHA256 = /^[a-f0-9]{64}$/;
const SOURCE_COMMIT = /^(?:[a-f0-9]{40}|[a-f0-9]{64})$/;
const WINDOWS_DEVICE = /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\..*)?$/i;

function fail(message) {
  throw new Error(message);
}

function assertExactKeys(value, expected, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }

  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) {
    fail(`${label} keys must be exactly: ${wanted.join(", ")}`);
  }
}

function isWithin(parent, candidate) {
  const rel = relative(resolve(parent), resolve(candidate));
  return rel === "" || (!isAbsolute(rel) && rel !== ".." && !rel.startsWith(`..${sep}`));
}

function comparePortablePaths(left, right) {
  return Buffer.compare(Buffer.from(left, "utf8"), Buffer.from(right, "utf8"));
}

function sensitiveSegment(segment) {
  const lower = segment.toLowerCase();
  return (
    lower === ".git" ||
    lower === "node_modules" ||
    lower === ".npmrc" ||
    lower === ".env" ||
    lower.startsWith(".env.") ||
    lower === ".dev.vars" ||
    lower.startsWith(".dev.vars.")
  );
}

export function validatePortablePath(value, label = "artifact path") {
  if (typeof value !== "string" || value.length === 0) {
    fail(`${label} must be a non-empty string`);
  }
  if (value.includes("\0") || value.includes("\\") || value.includes(":")) {
    fail(`${label} contains a forbidden character: ${value}`);
  }
  if (value.startsWith("/") || value.endsWith("/") || value.includes("//")) {
    fail(`${label} must be a normalized relative path: ${value}`);
  }

  const segments = value.split("/");
  for (const segment of segments) {
    if (
      segment === "." ||
      segment === ".." ||
      !PORTABLE_SEGMENT.test(segment) ||
      segment.endsWith(".") ||
      segment.endsWith(" ") ||
      WINDOWS_DEVICE.test(segment) ||
      sensitiveSegment(segment)
    ) {
      fail(`${label} contains a non-portable segment: ${value}`);
    }
  }

  return value;
}

function normalizeSiteUrl(value) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    fail("siteUrl must be a valid URL");
  }

  if (
    parsed.protocol !== "https:" ||
    parsed.username ||
    parsed.password ||
    parsed.search ||
    parsed.hash
  ) {
    fail("siteUrl must be HTTPS without credentials, a query, or a fragment");
  }

  const path = parsed.pathname.replace(/\/+$/, "");
  return `${parsed.origin}${path}`;
}

export function normalizeArtifactIdentity(identity) {
  assertExactKeys(
    identity,
    ["sourceRepository", "sourceCommit", "siteUrl"],
    "artifact identity",
  );

  if (
    typeof identity.sourceRepository !== "string" ||
    !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(identity.sourceRepository)
  ) {
    fail("sourceRepository must be an owner/repository name");
  }
  if (
    typeof identity.sourceCommit !== "string" ||
    !SOURCE_COMMIT.test(identity.sourceCommit)
  ) {
    fail("sourceCommit must be a lowercase full SHA-1 or SHA-256 revision");
  }

  return {
    sourceRepository: identity.sourceRepository,
    sourceCommit: identity.sourceCommit,
    siteUrl: normalizeSiteUrl(identity.siteUrl),
  };
}

export function renderDeploymentMarker(identity) {
  const normalized = normalizeArtifactIdentity(identity);
  return `${JSON.stringify(
    {
      schemaVersion: 1,
      artifactKind: ARTIFACT_KIND,
      ...normalized,
    },
    null,
    2,
  )}\n`;
}

async function pathState(path) {
  try {
    return await lstat(path);
  } catch (error) {
    if (error?.code === "ENOENT") return null;
    throw error;
  }
}

async function canonicalExistingDirectory(requestedPath, label) {
  const absolute = resolve(requestedPath);
  const state = await lstat(absolute);
  if (!state.isDirectory() || state.isSymbolicLink()) {
    fail(`${label} must be a real directory: ${absolute}`);
  }
  return realpath(absolute);
}

async function canonicalNewDirectory(requestedPath, label) {
  const absolute = resolve(requestedPath);
  if (await pathState(absolute)) fail(`${label} already exists: ${absolute}`);

  let ancestor = dirname(absolute);
  const missingSegments = [basename(absolute)];
  let ancestorState = await pathState(ancestor);
  while (!ancestorState) {
    missingSegments.unshift(basename(ancestor));
    const parent = dirname(ancestor);
    if (parent === ancestor) fail(`${label} has no existing directory ancestor`);
    ancestor = parent;
    ancestorState = await pathState(ancestor);
  }
  if (!ancestorState.isDirectory() || ancestorState.isSymbolicLink()) {
    fail(`${label} ancestor must be a real directory: ${ancestor}`);
  }

  const candidate = join(await realpath(ancestor), ...missingSegments);
  if (await pathState(candidate)) fail(`${label} already exists: ${candidate}`);
  return candidate;
}

async function fileRecord(root, portablePath) {
  const absolute = join(root, ...portablePath.split("/"));
  const state = await lstat(absolute);
  if (!state.isFile() || state.isSymbolicLink()) {
    fail(`artifact contains a non-regular file: ${portablePath}`);
  }
  if (!Number.isSafeInteger(state.size) || state.size > MAX_ASSET_SIZE_BYTES) {
    fail(`artifact file exceeds the ${MAX_ASSET_SIZE_BYTES}-byte limit: ${portablePath}`);
  }

  const bytes = await readFile(absolute);
  if (bytes.length > MAX_ASSET_SIZE_BYTES) {
    fail(`artifact file exceeds the ${MAX_ASSET_SIZE_BYTES}-byte limit: ${portablePath}`);
  }
  if (bytes.includes(VALIDATION_ORIGIN)) {
    fail(`artifact contains the reserved validation origin: ${portablePath}`);
  }

  return {
    path: portablePath,
    sizeBytes: bytes.length,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  };
}

async function walkRegularFiles(root) {
  const rootState = await lstat(root);
  if (!rootState.isDirectory() || rootState.isSymbolicLink()) {
    fail(`artifact root is not a real directory: ${root}`);
  }

  const files = [];
  const caseFoldedEntries = new Set();

  async function walk(current, prefix) {
    const entries = await readdir(current, { withFileTypes: true });
    entries.sort((left, right) => comparePortablePaths(left.name, right.name));
    if (prefix && entries.length === 0) {
      fail(`artifact contains an empty directory: ${prefix}`);
    }

    for (const entry of entries) {
      const portablePath = prefix ? `${prefix}/${entry.name}` : entry.name;
      validatePortablePath(portablePath);

      const folded = portablePath.toLowerCase();
      if (caseFoldedEntries.has(folded)) {
        fail(`artifact contains a case-insensitive path collision: ${portablePath}`);
      }
      caseFoldedEntries.add(folded);

      const absolute = join(current, entry.name);
      const state = await lstat(absolute);
      if (state.isSymbolicLink()) {
        fail(`artifact contains a symbolic link: ${portablePath}`);
      }
      if (state.isDirectory()) {
        await walk(absolute, portablePath);
      } else if (state.isFile()) {
        files.push(await fileRecord(root, portablePath));
        if (files.length > MAX_ASSET_COUNT) {
          fail(`artifact contains more than ${MAX_ASSET_COUNT} files`);
        }
      } else {
        fail(`artifact contains a non-regular node: ${portablePath}`);
      }
    }
  }

  await walk(root, "");
  files.sort((left, right) => comparePortablePaths(left.path, right.path));
  return files;
}

function requireProviderFiles(files) {
  const paths = new Set(files.map((file) => file.path));
  for (const required of ["404.html", "_headers", "_redirects"]) {
    if (!paths.has(required)) fail(`artifact is missing required file: ${required}`);
  }
}

function validateManifestRows(rows) {
  if (!Array.isArray(rows)) fail("artifact manifest files must be an array");
  if (rows.length > MAX_ASSET_COUNT) {
    fail(`artifact manifest contains more than ${MAX_ASSET_COUNT} files`);
  }

  const exact = new Set();
  const folded = new Set();
  let previous = null;

  for (const [index, row] of rows.entries()) {
    assertExactKeys(row, ["path", "sha256", "sizeBytes"], `manifest file ${index}`);
    validatePortablePath(row.path, `manifest file ${index} path`);
    if (
      !Number.isSafeInteger(row.sizeBytes) ||
      row.sizeBytes < 0 ||
      row.sizeBytes > MAX_ASSET_SIZE_BYTES
    ) {
      fail(`manifest file ${index} has an invalid size`);
    }
    if (typeof row.sha256 !== "string" || !SHA256.test(row.sha256)) {
      fail(`manifest file ${index} has an invalid SHA-256 digest`);
    }
    if (exact.has(row.path)) fail(`manifest contains a duplicate path: ${row.path}`);
    exact.add(row.path);

    const lower = row.path.toLowerCase();
    if (folded.has(lower)) {
      fail(`manifest contains a case-insensitive path collision: ${row.path}`);
    }
    folded.add(lower);

    if (previous !== null && comparePortablePaths(previous, row.path) >= 0) {
      fail("manifest file paths must be strictly sorted by UTF-8 bytes");
    }
    if (previous !== null && row.path.startsWith(`${previous}/`)) {
      fail(`manifest contains a file/directory prefix collision: ${previous}`);
    }
    previous = row.path;
  }

  for (const row of rows) {
    const segments = row.path.split("/");
    for (let index = 1; index < segments.length; index += 1) {
      const prefix = segments.slice(0, index).join("/");
      if (exact.has(prefix)) {
        fail(`manifest contains a file/directory prefix collision: ${prefix}`);
      }
    }
  }

  requireProviderFiles(rows);
  if (!exact.has(DEPLOYMENT_MARKER)) {
    fail(`manifest is missing the deployment marker: ${DEPLOYMENT_MARKER}`);
  }
}

function compareFileLists(expected, actual, label) {
  if (expected.length !== actual.length) {
    fail(`${label} file count differs: expected ${expected.length}, received ${actual.length}`);
  }
  for (let index = 0; index < expected.length; index += 1) {
    const wanted = expected[index];
    const received = actual[index];
    if (
      wanted.path !== received.path ||
      wanted.sizeBytes !== received.sizeBytes ||
      wanted.sha256 !== received.sha256
    ) {
      fail(
        `${label} mismatch at ${wanted.path}: expected ${wanted.sizeBytes}/${wanted.sha256}, ` +
          `received ${received.path} ${received.sizeBytes}/${received.sha256}`,
      );
    }
  }
}

async function copyFileList(sourceRoot, destinationRoot, files) {
  for (const file of files) {
    const source = join(sourceRoot, ...file.path.split("/"));
    const destination = join(destinationRoot, ...file.path.split("/"));
    const state = await lstat(source);
    if (!state.isFile() || state.isSymbolicLink()) {
      fail(`source changed before copy: ${file.path}`);
    }
    await mkdir(dirname(destination), { recursive: true });
    await copyFile(source, destination);
  }
}

function artifactManifest(files) {
  return {
    schemaVersion: 1,
    artifactKind: ARTIFACT_KIND,
    algorithm: "sha256",
    assetRoot: ASSET_ROOT,
    markerPath: DEPLOYMENT_MARKER,
    files,
  };
}

export async function createDeploymentArtifact({
  sourceDirectory,
  artifactDirectory,
  identity,
}) {
  const normalizedIdentity = normalizeArtifactIdentity(identity);
  const requestedSource = resolve(sourceDirectory);
  const requestedOutput = resolve(artifactDirectory);

  if (
    isWithin(requestedSource, requestedOutput) ||
    isWithin(requestedOutput, requestedSource)
  ) {
    fail("source and artifact directories must not overlap");
  }
  const source = await canonicalExistingDirectory(sourceDirectory, "source directory");
  const output = await canonicalNewDirectory(artifactDirectory, "artifact directory");
  if (isWithin(source, output) || isWithin(output, source)) {
    fail("source and artifact directories must not overlap");
  }
  await mkdir(dirname(output), { recursive: true });

  const sourceFiles = await walkRegularFiles(source);
  requireProviderFiles(sourceFiles);
  if (sourceFiles.some((file) => file.path === DEPLOYMENT_MARKER)) {
    fail(`source directory already contains reserved marker: ${DEPLOYMENT_MARKER}`);
  }

  const parent = dirname(output);
  const temporary = await mkdtemp(join(parent, `.${basename(output)}-`));

  try {
    const site = join(temporary, ASSET_ROOT);
    await mkdir(site);
    await copyFileList(source, site, sourceFiles);

    const marker = join(site, ...DEPLOYMENT_MARKER.split("/"));
    await mkdir(dirname(marker), { recursive: true });
    await writeFile(marker, renderDeploymentMarker(normalizedIdentity), {
      encoding: "utf8",
      flag: "wx",
    });

    const stagedFiles = await walkRegularFiles(site);
    const stagedSourceFiles = stagedFiles.filter((file) => file.path !== DEPLOYMENT_MARKER);
    compareFileLists(sourceFiles, stagedSourceFiles, "staged source");

    const manifest = artifactManifest(stagedFiles);
    await writeFile(
      join(temporary, ARTIFACT_MANIFEST),
      `${JSON.stringify(manifest, null, 2)}\n`,
      { encoding: "utf8", flag: "wx" },
    );
    await rename(temporary, output);
    return manifest;
  } catch (error) {
    await rm(temporary, { force: true, recursive: true });
    throw error;
  }
}

async function readManifest(artifactDirectory) {
  const path = join(artifactDirectory, ARTIFACT_MANIFEST);
  const state = await lstat(path);
  if (!state.isFile() || state.isSymbolicLink() || state.size > 10 * 1024 * 1024) {
    fail("artifact manifest must be a regular file no larger than 10 MiB");
  }

  const bytes = await readFile(path);
  let text;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    fail("artifact manifest is not valid UTF-8");
  }
  let value;
  try {
    value = JSON.parse(text);
  } catch {
    fail("artifact manifest is not valid JSON");
  }

  assertExactKeys(
    value,
    ["algorithm", "artifactKind", "assetRoot", "files", "markerPath", "schemaVersion"],
    "artifact manifest",
  );
  if (
    value.schemaVersion !== 1 ||
    value.artifactKind !== ARTIFACT_KIND ||
    value.algorithm !== "sha256" ||
    value.assetRoot !== ASSET_ROOT ||
    value.markerPath !== DEPLOYMENT_MARKER
  ) {
    fail("artifact manifest metadata is unsupported");
  }
  validateManifestRows(value.files);
  const canonical = artifactManifest(
    value.files.map(({ path, sizeBytes, sha256 }) => ({ path, sizeBytes, sha256 })),
  );
  if (text !== `${JSON.stringify(canonical, null, 2)}\n`) {
    fail("artifact manifest must use the canonical generated JSON encoding");
  }
  return { manifest: value, bytes };
}

async function validateArtifactRoot(artifactDirectory) {
  const state = await lstat(artifactDirectory);
  if (!state.isDirectory() || state.isSymbolicLink()) {
    fail("deployment artifact root must be a real directory");
  }

  const entries = await readdir(artifactDirectory, { withFileTypes: true });
  entries.sort((left, right) => comparePortablePaths(left.name, right.name));
  if (
    entries.length !== 2 ||
    entries[0].name !== ARTIFACT_MANIFEST ||
    !entries[0].isFile() ||
    entries[1].name !== ASSET_ROOT ||
    !entries[1].isDirectory()
  ) {
    fail(`deployment artifact root must contain only ${ARTIFACT_MANIFEST} and ${ASSET_ROOT}/`);
  }

  for (const entry of entries) {
    const state = await lstat(join(artifactDirectory, entry.name));
    if (state.isSymbolicLink()) fail(`deployment artifact root contains a symlink: ${entry.name}`);
  }
}

export async function verifyDeploymentArtifact({ artifactDirectory, expectedIdentity }) {
  const requestedArtifact = resolve(artifactDirectory);
  const identity = normalizeArtifactIdentity(expectedIdentity);
  await validateArtifactRoot(requestedArtifact);
  const artifact = await realpath(requestedArtifact);

  const { manifest, bytes } = await readManifest(artifact);
  const actual = await walkRegularFiles(join(artifact, ASSET_ROOT));
  compareFileLists(manifest.files, actual, "artifact");

  const marker = await readFile(
    join(artifact, ASSET_ROOT, ...DEPLOYMENT_MARKER.split("/")),
    "utf8",
  );
  const expectedMarker = renderDeploymentMarker(identity);
  if (marker !== expectedMarker) fail("deployment marker does not match the expected identity");

  return {
    manifest,
    manifestSha256: createHash("sha256").update(bytes).digest("hex"),
  };
}

export async function installDeploymentArtifact({
  artifactDirectory,
  destinationDirectory,
  expectedIdentity,
}) {
  const requestedArtifact = resolve(artifactDirectory);
  const requestedDestination = resolve(destinationDirectory);
  if (
    isWithin(requestedArtifact, requestedDestination) ||
    isWithin(requestedDestination, requestedArtifact)
  ) {
    fail("artifact and destination directories must not overlap");
  }
  await validateArtifactRoot(requestedArtifact);
  const artifact = await realpath(requestedArtifact);
  const destination = await canonicalNewDirectory(
    destinationDirectory,
    "deployment destination",
  );
  if (isWithin(artifact, destination) || isWithin(destination, artifact)) {
    fail("artifact and destination directories must not overlap");
  }
  await mkdir(dirname(destination), { recursive: true });

  const verified = await verifyDeploymentArtifact({
    artifactDirectory: artifact,
    expectedIdentity,
  });

  const parent = dirname(destination);
  const temporary = await mkdtemp(join(parent, `.${basename(destination)}-install-`));

  try {
    await copyFileList(join(artifact, ASSET_ROOT), temporary, verified.manifest.files);
    const installed = await walkRegularFiles(temporary);
    compareFileLists(verified.manifest.files, installed, "installed artifact");
    await rename(temporary, destination);
    return verified;
  } catch (error) {
    await rm(temporary, { force: true, recursive: true });
    throw error;
  }
}

export async function compareInstalledDeploymentArtifact({
  artifactDirectory,
  destinationDirectory,
  expectedIdentity,
}) {
  const requestedArtifact = resolve(artifactDirectory);
  const requestedDestination = resolve(destinationDirectory);
  if (
    isWithin(requestedArtifact, requestedDestination) ||
    isWithin(requestedDestination, requestedArtifact)
  ) {
    fail("artifact and destination directories must not overlap");
  }

  await validateArtifactRoot(requestedArtifact);
  const artifact = await realpath(requestedArtifact);
  const destination = await canonicalExistingDirectory(
    requestedDestination,
    "deployment destination",
  );
  if (isWithin(artifact, destination) || isWithin(destination, artifact)) {
    fail("artifact and destination directories must not overlap");
  }
  const verified = await verifyDeploymentArtifact({
    artifactDirectory: artifact,
    expectedIdentity,
  });
  const installed = await walkRegularFiles(destination);
  compareFileLists(verified.manifest.files, installed, "installed artifact");
  return verified;
}
