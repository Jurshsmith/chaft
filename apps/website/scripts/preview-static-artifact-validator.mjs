import { createHash } from "node:crypto";
import { lstatSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { renderDeploymentMarker } from "./deployment-artifact.mjs";
import { previewSlotForBranch } from "./preview-slot-contract.mjs";

const SOURCE_COMMIT = /^(?:[a-f0-9]{40}|[a-f0-9]{64})$/;
const PRODUCTION_ORIGIN = "https://chaft.ai";
const SECURITY_COPY =
  "Unaudited software. Not for sensitive or production communication.";
const ROBOTS_POLICY = ["noarchive", "nofollow", "noindex"];
const STABLE_CONTENT_SHA256 = Object.freeze({
  header: "380dc1538be4f1cebdaee78ae1a5a1d21f9e2ca3946ccf11c53f413140bd6c83",
  footer: "933a0ccac3c790e752f903bc50b2c9ebc8e6ff3314896cf9b5871fe3ee33e6d1",
  nonHero:
    "42eacb4226e69d0bafa149abb4c70dad81eb4b5df8eb695504f880ef6a3f009a",
});

function fail(message) {
  throw new Error(`preview static artifact validation failed: ${message}`);
}

function readRegularFile(root, relativePath) {
  const path = join(root, ...relativePath.split("/"));
  const state = lstatSync(path);
  if (!state.isFile() || state.isSymbolicLink()) {
    fail(`${relativePath} must be a regular file`);
  }
  return readFileSync(path, "utf8");
}

function attribute(html, tags, name) {
  for (const tag of html.matchAll(tags)) {
    const match = new RegExp(`\\b${name}=(["'])(.*?)\\1`, "i").exec(tag[0]);
    if (match) return match[2];
  }
  return null;
}

function directives(value) {
  return (value ?? "")
    .toLowerCase()
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean)
    .sort();
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
    .trim()
    .replace(/© \d{4}/g, "© YEAR");
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function element(html, tag, label) {
  const match = new RegExp(`<${tag}\\b[\\s\\S]*?<\\/${tag}>`, "i").exec(html);
  if (!match) fail(`home page is missing ${label}`);
  return match[0];
}

function elementByClass(html, tag, className, label) {
  const match = new RegExp(
    `<${tag}\\b[^>]*class=(["'])[^"']*\\b${className}\\b[^"']*\\1[^>]*>[\\s\\S]*?<\\/${tag}>`,
    "i",
  ).exec(html);
  if (!match) fail(`home page is missing ${label}`);
  return match[0];
}

function linkHrefByText(html, text) {
  for (const match of html.matchAll(/<a\b[^>]*>[\s\S]*?<\/a>/gi)) {
    const label = visibleText(match[0]).replace(/\s*→$/, "");
    if (label !== text) continue;
    return attribute(match[0], /<a\b[^>]*>/gi, "href");
  }
  return null;
}

function validateHtml(html, pathname, slot) {
  const expectedUrl = `${PRODUCTION_ORIGIN}${pathname}`;
  const canonical = attribute(
    html,
    /<link\b[^>]*\brel=(["'])canonical\1[^>]*>/gi,
    "href",
  );
  const openGraph = attribute(
    html,
    /<meta\b[^>]*\bproperty=(["'])og:url\1[^>]*>/gi,
    "content",
  );
  const robots = attribute(
    html,
    /<meta\b[^>]*\bname=(["'])robots\1[^>]*>/gi,
    "content",
  );
  if (canonical !== expectedUrl) {
    fail(`${pathname} canonical URL must be ${expectedUrl}`);
  }
  if (openGraph !== expectedUrl) {
    fail(`${pathname} Open Graph URL must be ${expectedUrl}`);
  }
  if (JSON.stringify(directives(robots)) !== JSON.stringify(ROBOTS_POLICY)) {
    fail(`${pathname} robots metadata must be noindex, nofollow, noarchive`);
  }
  if (!html.includes("Chaft Preview") || !html.includes(`Hero ${slot.slice(-1)}`)) {
    fail(`${pathname} must identify the exact Chaft Preview slot`);
  }
}

export function validatePreviewStaticArtifact({
  branch,
  distDirectory,
  expectedContentHashes = STABLE_CONTENT_SHA256,
  repository,
  sourceCommit,
}) {
  const preview = previewSlotForBranch(branch);
  if (repository !== "Jurshsmith/chaft") fail("unexpected source repository");
  if (!SOURCE_COMMIT.test(sourceCommit)) fail("source commit is malformed");
  const root = resolve(distDirectory);
  const rootState = lstatSync(root);
  if (!rootState.isDirectory() || rootState.isSymbolicLink()) {
    fail("dist directory must be a real directory");
  }

  const headers = readRegularFile(root, "_headers");
  if (
    !headers.startsWith("/*\n") ||
    !headers.includes("  X-Robots-Tag: noindex, nofollow, noarchive\n") ||
    !headers.includes(
      "/.well-known/chaft-deployment.json\n  Cache-Control: no-store\n",
    )
  ) {
    fail("_headers must apply exact Preview indexing and marker cache controls");
  }

  const robots = readRegularFile(root, "robots.txt");
  if (robots !== "User-agent: *\nDisallow: /\n") {
    fail("robots.txt must disallow all crawling without a sitemap");
  }

  const marker = readRegularFile(
    root,
    ".well-known/chaft-deployment.json",
  );
  const expectedMarker = renderDeploymentMarker({
    sourceRepository: repository,
    sourceCommit,
    siteUrl: preview.siteUrl,
  });
  if (marker !== expectedMarker) fail("deployment marker identity is not exact");

  let homeHtml;
  for (const [file, pathname] of [
    ["index.html", "/"],
    ["download/index.html", "/download/"],
    ["security/index.html", "/security/"],
  ]) {
    const html = readRegularFile(root, file);
    validateHtml(html, pathname, preview.slot);
    if (pathname === "/") homeHtml = html;
  }

  for (const [label, expectedHref] of [
    ["Download Chaft", "/download/"],
    ["Read the docs", "/docs/"],
    ["Explore the source", "https://github.com/Jurshsmith/chaft"],
  ]) {
    if (linkHrefByText(homeHtml, label) !== expectedHref) {
      fail(`${label} must retain the exact ${expectedHref} destination`);
    }
  }

  const securityWarning = elementByClass(
    homeHtml,
    "p",
    "hero__note",
    "security warning",
  );
  if (visibleText(securityWarning) !== `Canary ${SECURITY_COPY}`) {
    fail("security warning must retain its exact Canary label and safety copy");
  }

  const header = element(homeHtml, "header", "site header");
  const footer = element(homeHtml, "footer", "site footer");
  const main = element(homeHtml, "main", "main content");
  const heroPattern =
    /<section\b[^>]*class=(["'])[^"']*\bhero\b[^"']*\1[^>]*>[\s\S]*?<\/section>/i;
  const hero = heroPattern.exec(main)?.[0];
  if (!hero) fail("home page is missing the exact hero section");
  if (
    attribute(hero, /<section\b[^>]*>/gi, "data-chaft-hero") !== preview.slot
  ) {
    fail(`home page must render the exact ${preview.slot} hero`);
  }
  const actualContentHashes = {
    header: sha256(visibleText(header)),
    footer: sha256(visibleText(footer)),
    nonHero: sha256(visibleText(main.replace(heroPattern, ""))),
  };
  for (const [name, expected] of Object.entries(expectedContentHashes)) {
    if (actualContentHashes[name] !== expected) {
      fail(`${name} content changed outside the allowed hero surface`);
    }
  }

  return {
    schemaVersion: 1,
    artifactKind: "chaft-preview-static-validation",
    branch: preview.branch,
    siteUrl: preview.siteUrl,
    slot: preview.slot,
    worker: preview.worker,
    repository,
    sourceCommit,
    contentSha256: actualContentHashes,
    result: "passed",
  };
}

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index === -1 || !process.argv[index + 1]) fail(`missing ${name}`);
  return process.argv[index + 1];
}

function main() {
  const result = validatePreviewStaticArtifact({
    branch: argument("--branch"),
    distDirectory: argument("--dist"),
    repository: argument("--repository"),
    sourceCommit: argument("--commit"),
  });
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : "";
if (invokedPath === import.meta.url) main();
