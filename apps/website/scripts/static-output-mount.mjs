import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  renameSync,
  rmSync,
  rmdirSync,
} from "node:fs";
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from "node:path";

import { deploymentMountPath } from "./deployment-artifact.mjs";

const CLOUDFLARE_CONTROL_FILES = new Set(["_headers", "_redirects"]);

function pathIsWithin(root, candidate) {
  const fromRoot = relative(root, candidate);
  return (
    fromRoot === "" ||
    (!fromRoot.startsWith(`..${sep}`) && fromRoot !== ".." && !isAbsolute(fromRoot))
  );
}

function rawPathname(siteUrl) {
  const authorityStart = siteUrl.indexOf("://") + 3;
  const pathStart = siteUrl.indexOf("/", authorityStart);
  if (pathStart === -1) {
    return "/";
  }
  const queryStart = siteUrl.indexOf("?", pathStart);
  const fragmentStart = siteUrl.indexOf("#", pathStart);
  const pathEnd = Math.min(
    ...[queryStart, fragmentStart].filter((index) => index !== -1),
    siteUrl.length,
  );
  return siteUrl.slice(pathStart, pathEnd);
}

function siteBase(siteUrl) {
  if (
    typeof siteUrl !== "string" ||
    siteUrl === "" ||
    siteUrl.trim() !== siteUrl ||
    !/^https:\/\//i.test(siteUrl)
  ) {
    throw new Error(
      "SITE_URL must be an absolute HTTPS URL without surrounding whitespace",
    );
  }

  let site;
  try {
    site = new URL(siteUrl);
  } catch {
    throw new Error(`SITE_URL is invalid: ${siteUrl}`);
  }

  const afterScheme = siteUrl.slice(siteUrl.indexOf("://") + 3);
  if (
    site.protocol !== "https:" ||
    site.username ||
    site.password ||
    site.search ||
    site.hash ||
    afterScheme.includes("?") ||
    afterScheme.includes("#")
  ) {
    throw new Error(
      "SITE_URL must be an HTTPS URL without credentials, a query, or a fragment",
    );
  }
  if (siteUrl.includes("\\")) {
    throw new Error("SITE_URL path must not contain backslashes");
  }

  const rawSegments = rawPathname(siteUrl).split("/");
  for (const rawSegment of rawSegments) {
    if (rawSegment === "") {
      continue;
    }
    let decoded;
    try {
      decoded = decodeURIComponent(rawSegment);
    } catch {
      throw new Error("SITE_URL path contains invalid percent encoding");
    }
    if (
      decoded === "." ||
      decoded === ".." ||
      decoded.includes("/") ||
      decoded.includes("\\") ||
      decoded.includes("\0")
    ) {
      throw new Error("SITE_URL path contains traversal or an encoded path separator");
    }
  }

  const mountPath = deploymentMountPath(siteUrl);
  const basePath = mountPath ? `/${mountPath}` : "/";
  const segments = mountPath ? mountPath.split("/") : [];
  return { basePath, segments };
}

function assertRegularOutputTree(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const entryPath = join(directory, entry.name);
    const stats = lstatSync(entryPath);
    if (stats.isSymbolicLink()) {
      throw new Error(`static output must not contain symbolic links: ${entryPath}`);
    }
    if (stats.isDirectory()) {
      assertRegularOutputTree(entryPath);
    } else if (!stats.isFile()) {
      throw new Error(`static output must contain only files and directories: ${entryPath}`);
    }
  }
}

function restoreStagedOutput({
  distRoot,
  stageDirectory,
  mountDirectory,
  topLevelBaseDirectory,
  mountScaffoldStarted,
  mountedEntries,
}) {
  if (mountDirectory && existsSync(mountDirectory)) {
    for (const name of mountedEntries) {
      const mountedPath = join(mountDirectory, name);
      if (existsSync(mountedPath)) {
        renameSync(mountedPath, join(stageDirectory, name));
      }
    }
  }
  if (
    mountScaffoldStarted &&
    topLevelBaseDirectory &&
    existsSync(topLevelBaseDirectory)
  ) {
    rmSync(topLevelBaseDirectory, { force: true, recursive: true });
  }
  if (existsSync(stageDirectory)) {
    for (const name of readdirSync(stageDirectory)) {
      renameSync(join(stageDirectory, name), join(distRoot, name));
    }
    rmdirSync(stageDirectory);
  }
}

/**
 * Move an Astro static build beneath SITE_URL's path base.
 *
 * Call this after Astro finishes and before writing root-level Cloudflare
 * `_headers` and `_redirects` files. Existing copies of those two control files
 * remain at the assets root and are never duplicated inside the mounted site.
 */
export function mountStaticOutput({
  distDirectory,
  siteUrl = process.env.SITE_URL,
} = {}) {
  if (typeof distDirectory !== "string" || distDirectory.trim() === "") {
    throw new Error("distDirectory is required");
  }
  const distRoot = resolve(distDirectory);
  if (!existsSync(distRoot)) {
    throw new Error(`static output directory does not exist: ${distRoot}`);
  }
  const distStats = lstatSync(distRoot);
  if (distStats.isSymbolicLink() || !distStats.isDirectory()) {
    throw new Error(`static output path must be a real directory: ${distRoot}`);
  }

  const { basePath, segments } = siteBase(siteUrl);
  assertRegularOutputTree(distRoot);
  if (basePath === "/") {
    return {
      basePath,
      distDirectory: distRoot,
      mountDirectory: distRoot,
      movedEntries: [],
    };
  }

  const mountDirectory = resolve(distRoot, ...segments);
  if (!pathIsWithin(distRoot, mountDirectory) || mountDirectory === distRoot) {
    throw new Error(`SITE_URL path base escapes static output: ${basePath}`);
  }
  if (CLOUDFLARE_CONTROL_FILES.has(segments[0])) {
    throw new Error(
      `SITE_URL path base collides with Cloudflare control file: /${segments[0]}`,
    );
  }

  const initialEntries = readdirSync(distRoot).sort();
  const mountedOutputEntries = initialEntries.filter(
    (name) => !CLOUDFLARE_CONTROL_FILES.has(name),
  );
  const stageDirectory = mkdtempSync(
    join(dirname(distRoot), `.${basename(distRoot)}-static-output-mount-`),
  );
  const mountedEntries = [];
  const topLevelBaseDirectory = join(distRoot, segments[0]);
  let mountScaffoldStarted = false;

  try {
    for (const name of mountedOutputEntries) {
      renameSync(join(distRoot, name), join(stageDirectory, name));
    }
    mountScaffoldStarted = true;
    mkdirSync(mountDirectory, { recursive: true });
    for (const name of mountedOutputEntries) {
      renameSync(join(stageDirectory, name), join(mountDirectory, name));
      mountedEntries.push(name);
    }
    rmdirSync(stageDirectory);
  } catch (error) {
    try {
      restoreStagedOutput({
        distRoot,
        stageDirectory,
        mountDirectory,
        topLevelBaseDirectory,
        mountScaffoldStarted,
        mountedEntries,
      });
    } catch (cleanupError) {
      throw new AggregateError(
        [error, cleanupError],
        "failed to mount static output and restore its staging directory",
      );
    }
    throw error;
  }

  return {
    basePath,
    distDirectory: distRoot,
    mountDirectory,
    movedEntries: mountedOutputEntries,
  };
}
