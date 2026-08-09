import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import sharp from "sharp";

import { deploymentMountPath } from "./deployment-artifact.mjs";

export const SOCIAL_IMAGE_FILENAME = "og-chaft-v3.png";
export const SOCIAL_IMAGE_WIDTH = 1200;
export const SOCIAL_IMAGE_HEIGHT = 630;
export const HOME_SOCIAL_TITLE =
  "Chaft — Team chat without a required central server.";

function fail(message) {
  throw new Error(`social preview validation failed: ${message}`);
}

function attribute(tag, name) {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = new RegExp(
    `(?:^|\\s)${escaped}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s>]+))`,
    "i",
  ).exec(tag);
  return match?.[1] ?? match?.[2] ?? match?.[3] ?? null;
}

function metaContent(html, selectorName, selectorValue) {
  const matches = [...html.matchAll(/<meta\b[^>]*>/gi)]
    .map((match) => match[0])
    .filter(
      (tag) =>
        attribute(tag, selectorName)?.toLowerCase() ===
        selectorValue.toLowerCase(),
    );
  if (matches.length !== 1) {
    fail(`expected one ${selectorName}="${selectorValue}" meta tag, found ${matches.length}`);
  }
  const content = attribute(matches[0], "content");
  if (content === null) fail(`${selectorValue} meta tag is missing content`);
  return content;
}

function expectedImageUrl(siteUrl) {
  const site = new URL(siteUrl);
  if (
    site.protocol !== "https:" ||
    site.username ||
    site.password ||
    site.search ||
    site.hash
  ) {
    fail("SITE_URL must be HTTPS without credentials, query, or fragment");
  }
  const base = site.pathname.replace(/\/+$/, "");
  return `${site.origin}${base}/${SOCIAL_IMAGE_FILENAME}`;
}

export async function validateSocialPreviewBuild({ distDirectory, siteUrl }) {
  const mountPath = deploymentMountPath(siteUrl);
  const outputRoot = mountPath
    ? join(resolve(distDirectory), ...mountPath.split("/"))
    : resolve(distDirectory);
  const htmlPath = join(outputRoot, "index.html");
  const imagePath = join(outputRoot, SOCIAL_IMAGE_FILENAME);

  if (!existsSync(htmlPath)) fail(`home page is missing at ${htmlPath}`);
  if (!existsSync(imagePath)) fail(`social image is missing at ${imagePath}`);

  const metadata = await sharp(imagePath).metadata();
  const pages = metadata.pages ?? 1;
  if (
    metadata.format !== "png" ||
    metadata.width !== SOCIAL_IMAGE_WIDTH ||
    metadata.height !== SOCIAL_IMAGE_HEIGHT ||
    pages !== 1
  ) {
    fail(
      `image must decode as one ${SOCIAL_IMAGE_WIDTH}x${SOCIAL_IMAGE_HEIGHT} PNG; ` +
        `received ${metadata.format} ${metadata.width}x${metadata.height} pages=${pages}`,
    );
  }

  const html = readFileSync(htmlPath, "utf8");
  const imageUrl = expectedImageUrl(siteUrl);
  const absoluteImageUrl = new URL(imageUrl);
  if (absoluteImageUrl.protocol !== "https:") fail("social image URL must be absolute HTTPS");

  const expectations = [
    ["property", "og:title", HOME_SOCIAL_TITLE],
    ["property", "og:image", imageUrl],
    ["property", "og:image:secure_url", imageUrl],
    ["property", "og:image:type", "image/png"],
    ["property", "og:image:width", String(SOCIAL_IMAGE_WIDTH)],
    ["property", "og:image:height", String(SOCIAL_IMAGE_HEIGHT)],
    ["name", "twitter:card", "summary_large_image"],
    ["name", "twitter:title", HOME_SOCIAL_TITLE],
    ["name", "twitter:image", imageUrl],
  ];
  for (const [selectorName, selectorValue, expected] of expectations) {
    const actual = metaContent(html, selectorName, selectorValue);
    if (actual !== expected) {
      fail(`${selectorValue} must be ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`);
    }
  }

  for (const [selectorName, selectorValue] of [
    ["property", "og:image:alt"],
    ["name", "twitter:image:alt"],
  ]) {
    if (!metaContent(html, selectorName, selectorValue).trim()) {
      fail(`${selectorValue} must not be empty`);
    }
  }

  return {
    format: metadata.format,
    width: metadata.width,
    height: metadata.height,
    imagePath,
    imageUrl,
  };
}

function argument(name, fallback) {
  const index = process.argv.indexOf(name);
  if (index === -1) return fallback;
  if (!process.argv[index + 1]) fail(`missing ${name}`);
  return process.argv[index + 1];
}

async function main() {
  const result = await validateSocialPreviewBuild({
    distDirectory: argument("--dist", "dist"),
    siteUrl: argument("--site-url", process.env.SITE_URL),
  });
  process.stdout.write(
    `validated ${result.width}x${result.height} ${result.format} social preview at ${result.imageUrl}\n`,
  );
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : "";
if (invokedPath === import.meta.url) await main();
