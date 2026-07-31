import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { previewSlotForBranch } from "./preview-slot-contract.mjs";

const SOURCE_COMMIT = /^(?:[a-f0-9]{40}|[a-f0-9]{64})$/;
const REQUIRED_ROBOTS_TOKENS = ["noarchive", "nofollow", "noindex"];
const PRODUCTION_ORIGIN = "https://chaft.ai";
const COMMON_HEADERS = {
  "cross-origin-opener-policy": "same-origin",
  "referrer-policy": "strict-origin-when-cross-origin",
  "x-content-type-options": "nosniff",
  "x-frame-options": "DENY",
};

function fail(message) {
  throw new Error(`preview deployment verification failed: ${message}`);
}

function assert(condition, message) {
  if (!condition) fail(message);
}

function headerTokens(headers, name) {
  return (headers.get(name) ?? "")
    .toLowerCase()
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean)
    .sort();
}

function assertCommonHeaders(response, label) {
  for (const [name, expected] of Object.entries(COMMON_HEADERS)) {
    assert(
      response.headers.get(name) === expected,
      `${label} ${name} must equal ${expected}`,
    );
  }
  assert(
    JSON.stringify(headerTokens(response.headers, "x-robots-tag")) ===
      JSON.stringify(REQUIRED_ROBOTS_TOKENS),
    `${label} X-Robots-Tag must be noindex, nofollow, noarchive`,
  );
}

async function responseBytes(response, label, maximumBytes = 5 * 1024 * 1024) {
  const bytes = new Uint8Array(await response.arrayBuffer());
  assert(bytes.byteLength <= maximumBytes, `${label} response is too large`);
  return bytes;
}

async function responseText(response, label, maximumBytes = 5 * 1024 * 1024) {
  return new TextDecoder("utf-8", { fatal: true }).decode(
    await responseBytes(response, label, maximumBytes),
  );
}

function tagAttribute(html, selectorPattern, attribute) {
  for (const match of html.matchAll(selectorPattern)) {
    const value = new RegExp(`\\b${attribute}=(["'])(.*?)\\1`, "i").exec(match[0]);
    if (value) return value[2];
  }
  return null;
}

function canonicalHref(html) {
  return tagAttribute(
    html,
    /<link\b[^>]*\brel=(["'])canonical\1[^>]*>/gi,
    "href",
  );
}

function openGraphUrl(html) {
  return tagAttribute(
    html,
    /<meta\b[^>]*\bproperty=(["'])og:url\1[^>]*>/gi,
    "content",
  );
}

function robotsMeta(html) {
  return tagAttribute(
    html,
    /<meta\b[^>]*\bname=(["'])robots\1[^>]*>/gi,
    "content",
  );
}

function staticAssetHref(html) {
  const match = /(?:src|href)=(["'])([^"']*\/_astro\/[^"']+)\1/i.exec(html);
  return match?.[2] ?? null;
}

function assertRobotsValue(value, label) {
  const tokens = (value ?? "")
    .toLowerCase()
    .split(",")
    .map((token) => token.trim())
    .filter(Boolean)
    .sort();
  assert(
    JSON.stringify(tokens) === JSON.stringify(REQUIRED_ROBOTS_TOKENS),
    `${label} robots directive must be noindex, nofollow, noarchive`,
  );
}

export async function verifyPreviewDeployment({
  branch,
  expectedCommit,
  fetchImpl = fetch,
  repository,
}) {
  const preview = previewSlotForBranch(branch);
  assert(repository === "Jurshsmith/chaft", "unexpected source repository");
  assert(
    typeof expectedCommit === "string" && SOURCE_COMMIT.test(expectedCommit),
    "expected commit must be a lowercase full SHA-1 or SHA-256 revision",
  );

  const checks = [];
  const request = async (pathname, label, expectedStatus = 200) => {
    const response = await fetchImpl(`${preview.siteUrl}${pathname}`, {
      redirect: "manual",
      signal: AbortSignal.timeout(10_000),
    });
    assert(response.status === expectedStatus, `${label} must return ${expectedStatus}`);
    checks.push({ detail: pathname, name: label, status: response.status });
    return response;
  };

  const markerResponse = await request(
    "/.well-known/chaft-deployment.json",
    "deployment-marker",
  );
  assertCommonHeaders(markerResponse, "deployment marker");
  assert(
    headerTokens(markerResponse.headers, "cache-control").includes("no-store"),
    "deployment marker must use no-store",
  );
  const marker = JSON.parse(
    await responseText(markerResponse, "deployment marker", 64 * 1024),
  );
  const expectedMarker = {
    schemaVersion: 1,
    artifactKind: "chaft-website",
    sourceRepository: repository,
    sourceCommit: expectedCommit,
    siteUrl: preview.siteUrl,
  };
  assert(
    JSON.stringify(marker) === JSON.stringify(expectedMarker),
    "deployment marker does not match the exact preview source identity",
  );

  let homeHtml;
  for (const [pathname, label] of [
    ["/", "home"],
    ["/download/", "download"],
    ["/security/", "security"],
  ]) {
    const response = await request(pathname, label);
    assertCommonHeaders(response, label);
    const html = await responseText(response, label);
    const productionUrl = `${PRODUCTION_ORIGIN}${pathname}`;
    assert(
      canonicalHref(html) === productionUrl,
      `${label} canonical URL must be ${productionUrl}`,
    );
    assert(
      openGraphUrl(html) === productionUrl,
      `${label} Open Graph URL must be ${productionUrl}`,
    );
    assertRobotsValue(robotsMeta(html), label);
    if (label === "home") homeHtml = html;
  }
  assert(
    tagAttribute(
      homeHtml,
      /<section\b[^>]*\bclass=(["'])[^"']*\bhero\b[^"']*\1[^>]*>/gi,
      "data-chaft-hero",
    ) === preview.slot,
    `home must render the exact ${preview.slot} hero`,
  );

  const notFound = await request(
    "/definitely-not-a-page-chaft-preview-verification",
    "not-found",
    404,
  );
  assertCommonHeaders(notFound, "not-found");
  assertRobotsValue(
    robotsMeta(await responseText(notFound, "not-found")),
    "not-found",
  );

  const robotsResponse = await request("/robots.txt", "robots");
  const robots = await responseText(robotsResponse, "robots", 64 * 1024);
  assert(
    robots === "User-agent: *\nDisallow: /\n",
    "robots.txt must disallow all crawling without a sitemap",
  );

  const assetHref = staticAssetHref(homeHtml);
  assert(assetHref, "home page must reference a hashed Astro asset");
  const assetUrl = new URL(assetHref, preview.siteUrl);
  assert(
    assetUrl.origin === preview.siteUrl &&
      assetUrl.pathname.startsWith("/_astro/"),
    "home page references an invalid Astro asset",
  );
  const asset = await fetchImpl(assetUrl, {
    redirect: "manual",
    signal: AbortSignal.timeout(10_000),
  });
  assert(asset.status === 200, "referenced Astro asset must return 200");
  for (const [name, expected] of Object.entries(COMMON_HEADERS)) {
    assert(
      asset.headers.get(name) === expected,
      `Astro asset ${name} must equal ${expected}`,
    );
  }
  const cacheControl = headerTokens(asset.headers, "cache-control");
  assert(
    cacheControl.includes("immutable") &&
      cacheControl.includes("max-age=31536000") &&
      cacheControl.includes("public"),
    "Astro asset must use one-year immutable caching",
  );
  const assetBytes = await responseBytes(
    asset,
    "Astro asset",
    25 * 1024 * 1024,
  );
  assert(assetBytes.byteLength > 0, "Astro asset must not be empty");
  checks.push({
    detail: assetUrl.pathname,
    name: "hashed-asset",
    status: asset.status,
  });

  return {
    schemaVersion: 1,
    artifactKind: "chaft-preview-public-verification",
    verifiedAt: new Date().toISOString(),
    branch: preview.branch,
    domain: preview.domain,
    slot: preview.slot,
    worker: preview.worker,
    siteUrl: preview.siteUrl,
    repository,
    expectedCommit,
    checks,
    result: "passed",
  };
}

function argumentsFromCommandLine(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index];
    const value = argv[index + 1];
    if (!name?.startsWith("--") || value === undefined || value.startsWith("--")) {
      fail(`invalid command-line argument near ${name ?? "<end>"}`);
    }
    if (values.has(name)) fail(`duplicate command-line argument ${name}`);
    values.set(name, value);
  }
  return values;
}

function required(values, name) {
  const value = values.get(name);
  if (!value) fail(`missing ${name}`);
  return value;
}

async function main() {
  const values = argumentsFromCommandLine(process.argv.slice(2));
  const allowed = new Set([
    "--attempts",
    "--branch",
    "--commit",
    "--delay-ms",
    "--output",
    "--repository",
  ]);
  for (const name of values.keys()) {
    if (!allowed.has(name)) fail(`unexpected argument ${name}`);
  }

  const attempts = Number(values.get("--attempts") ?? "1");
  const delayMs = Number(values.get("--delay-ms") ?? "0");
  if (!Number.isSafeInteger(attempts) || attempts < 1 || attempts > 20) {
    fail("attempts must be an integer from 1 to 20");
  }
  if (!Number.isSafeInteger(delayMs) || delayMs < 0 || delayMs > 60_000) {
    fail("delay-ms must be an integer from 0 to 60000");
  }

  const options = {
    branch: required(values, "--branch"),
    expectedCommit: required(values, "--commit"),
    repository: required(values, "--repository"),
  };
  let lastError;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      const result = await verifyPreviewDeployment(options);
      const output = required(values, "--output");
      await writeFile(output, `${JSON.stringify(result, null, 2)}\n`, {
        encoding: "utf8",
        flag: "wx",
      });
      process.stdout.write(`${JSON.stringify(result)}\n`);
      return;
    } catch (error) {
      lastError = error;
      if (attempt < attempts && delayMs > 0) {
        await new Promise((resolveDelay) => setTimeout(resolveDelay, delayMs));
      }
    }
  }
  throw lastError;
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : "";
if (invokedPath === import.meta.url) {
  main().catch((error) => {
    process.stderr.write(`${error.stack ?? error}\n`);
    process.exitCode = 1;
  });
}
