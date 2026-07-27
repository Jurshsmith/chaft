import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { afterEach, describe, expect, it } from "vitest";

import {
  HOME_SOCIAL_TITLE,
  SOCIAL_IMAGE_FILENAME,
  validateSocialPreviewBuild,
} from "./social-preview-validation.mjs";

const sourceImage = fileURLToPath(
  new URL(`../public/${SOCIAL_IMAGE_FILENAME}`, import.meta.url),
);
const fixtures = [];

function fixture(siteUrl, imageUrl) {
  const root = mkdtempSync(join(tmpdir(), "chaft-social-preview-"));
  fixtures.push(root);
  const site = new URL(siteUrl);
  const mount = site.pathname.replace(/^\/|\/$/g, "");
  const outputRoot = mount ? join(root, mount) : root;
  mkdirSync(outputRoot, { recursive: true });
  copyFileSync(sourceImage, join(outputRoot, SOCIAL_IMAGE_FILENAME));
  writeFileSync(
    join(outputRoot, "index.html"),
    `<!doctype html><html><head>
      <meta property="og:title" content="${HOME_SOCIAL_TITLE}">
      <meta property="og:image" content="${imageUrl}">
      <meta property="og:image:secure_url" content="${imageUrl}">
      <meta property="og:image:type" content="image/png">
      <meta property="og:image:width" content="1200">
      <meta property="og:image:height" content="630">
      <meta property="og:image:alt" content="Chaft logo and peer network">
      <meta name="twitter:card" content="summary_large_image">
      <meta name="twitter:title" content="${HOME_SOCIAL_TITLE}">
      <meta name="twitter:image" content="${imageUrl}">
      <meta name="twitter:image:alt" content="Chaft logo and peer network">
    </head></html>`,
  );
  return root;
}

afterEach(() => {
  for (const root of fixtures.splice(0)) {
    rmSync(root, { force: true, recursive: true });
  }
});

describe("social preview validation", () => {
  it.each([
    ["https://chaft.ai", "https://chaft.ai/og-chaft-v3.png"],
    [
      "https://example.com/chaft-preview",
      "https://example.com/chaft-preview/og-chaft-v3.png",
    ],
  ])("decodes the image and accepts the emitted absolute URL for %s", async (siteUrl, imageUrl) => {
    const distDirectory = fixture(siteUrl, imageUrl);
    await expect(
      validateSocialPreviewBuild({ distDirectory, siteUrl }),
    ).resolves.toMatchObject({
      format: "png",
      height: 630,
      imageUrl,
      width: 1200,
    });
  });

  it("rejects a root-relative social image URL", async () => {
    const siteUrl = "https://chaft.ai";
    const distDirectory = fixture(siteUrl, `/${SOCIAL_IMAGE_FILENAME}`);
    await expect(
      validateSocialPreviewBuild({ distDirectory, siteUrl }),
    ).rejects.toThrow(/og:image must be/);
  });

  it("rejects an image that cannot be decoded as PNG", async () => {
    const siteUrl = "https://chaft.ai";
    const imageUrl = `https://chaft.ai/${SOCIAL_IMAGE_FILENAME}`;
    const distDirectory = fixture(siteUrl, imageUrl);
    writeFileSync(join(distDirectory, SOCIAL_IMAGE_FILENAME), "not a png");
    await expect(
      validateSocialPreviewBuild({ distDirectory, siteUrl }),
    ).rejects.toThrow();
  });
});
