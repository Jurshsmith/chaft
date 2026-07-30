import { createHash } from "node:crypto";

import { expect, test } from "@playwright/test";

import {
  expectedPreviewHero,
  openLandingPage,
  previewInvariants,
} from "./support";

test.describe("landing-page Preview contract", () => {
  test.beforeEach(async ({ page }) => {
    await openLandingPage(page);
  });

  test("keeps shared product copy and actions invariant", async ({ page }) => {
    await expect(page.locator(".hero")).toHaveAttribute(
      "data-chaft-hero",
      expectedPreviewHero,
    );
    await expect(page.locator(".hero h1")).toHaveText(previewInvariants.headline);
    const bodyCopy = (await page.locator(".hero .lede").innerText())
      .replace(/\s+/g, " ")
      .trim();
    expect(
      createHash("sha256").update(bodyCopy).digest("hex"),
    ).toBe(
      previewInvariants.bodyCopySha256,
    );
    const primary = page.locator(".hero__actions").getByRole("link", {
      name: previewInvariants.primaryAction,
    });
    const secondary = page.locator(".hero__actions").getByRole("link", {
      name: previewInvariants.secondaryAction,
    });
    const source = page.locator(".hero__actions").getByRole("link", {
      name: previewInvariants.sourceAction,
    });
    await expect(primary).toBeVisible();
    await expect(primary).toHaveAttribute("href", "/download/");
    await expect(secondary).toBeVisible();
    await expect(secondary).toHaveAttribute("href", "/docs/");
    await expect(source).toBeVisible();
    await expect(source).toHaveAttribute(
      "href",
      "https://github.com/Jurshsmith/chaft",
    );
    const exactSecurityCopy = previewInvariants.securityCopy.replace(
      /[.*+?^${}()|[\]\\]/g,
      "\\$&",
    );
    await expect(page.locator(".hero__note")).toHaveText(
      new RegExp(`^Canary\\s*${exactSecurityCopy}$`),
    );
  });

  test("keeps Chillax on body copy and Space Grotesk on headings and UI", async ({
    page,
  }) => {
    const bodyCopyFont = await page
      .locator(".hero .lede")
      .evaluate((element) => getComputedStyle(element).fontFamily);
    const headingFont = await page
      .locator(".hero h1")
      .evaluate((element) => getComputedStyle(element).fontFamily);
    const navigationFont = await page
      .locator(".site-header")
      .evaluate((element) => getComputedStyle(element).fontFamily);
    const buttonFont = await page
      .locator(".hero__actions .button")
      .first()
      .evaluate((element) => getComputedStyle(element).fontFamily);

    expect(bodyCopyFont).toContain("Chillax");
    expect(headingFont).toContain("Space Grotesk");
    expect(navigationFont).toContain("Space Grotesk");
    expect(buttonFont).toContain("Space Grotesk");
  });
});
