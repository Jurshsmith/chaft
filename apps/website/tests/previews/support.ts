import { expect, type Page } from "@playwright/test";

export const previewInvariants = Object.freeze({
  headline: "Team chat that runs on your devices.",
  bodyCopySha256:
    "2db41bdb5926d7968600052a638be5b5c6b5ee614013a8e2bbe7c3202693ac3b",
  primaryAction: "Download Chaft",
  secondaryAction: "Read the docs",
  sourceAction: "Explore the source",
  securityCopy:
    "Unaudited software. Not for sensitive or production communication.",
});

export async function openLandingPage(page: Page) {
  await page.emulateMedia({
    colorScheme: "light",
    reducedMotion: "reduce",
  });
  const response = await page.goto("/", { waitUntil: "domcontentloaded" });
  expect(response?.ok()).toBe(true);
  await page.evaluate(async () => {
    await document.fonts.ready;
  });
  await expect(page.locator("main")).toBeVisible();
}
