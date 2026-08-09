import { expect, test } from "@playwright/test";

import { openLandingPage } from "./support";

test.describe("shared footer contract", () => {
  test.beforeEach(async ({ page }) => {
    await openLandingPage(page);
  });

  test("keeps the product message, destinations, and typography explicit", async ({
    page,
  }) => {
    const footer = page.locator(".site-footer");
    await footer.scrollIntoViewIfNeeded();
    await expect(footer).toBeVisible();

    const wordmark = footer.getByRole("link", { name: "Chaft home" });
    await expect(wordmark).toHaveText(/chaft/);
    await expect(wordmark).toHaveAttribute("href", "/");
    await expect(footer.locator(".site-footer__lead > p")).toHaveText(
      "Open-source desktop chat for small teams.",
    );

    const navigation = footer.getByRole("navigation", { name: "Footer navigation" });
    await expect(navigation.locator(".footer-nav__group")).toHaveCount(4);
    await expect(navigation.locator(".footer-nav__group > p")).toHaveText([
      "Product",
      "Learn",
      "Project",
      "Legal",
    ]);
    await expect(footer.locator(".site-footer__status")).toHaveText(
      "Canary · unsigned and unaudited",
    );

    for (const [label, href] of [
      ["Download", "/download/"],
      ["Documentation", "/docs/"],
      ["Releases", "/releases/"],
      ["Security", "/security/"],
      ["Source", "https://github.com/Jurshsmith/chaft"],
      [
        "AGPL-3.0-or-later",
        "https://github.com/Jurshsmith/chaft/blob/main/LICENSE",
      ],
    ] as const) {
      await expect(footer.getByRole("link", { name: label, exact: true })).toHaveAttribute(
        "href",
        href,
      );
    }

    const [bodyFont, wordmarkFont, navigationFont] = await Promise.all([
      footer
        .locator(".site-footer__lead > p")
        .evaluate((element) => getComputedStyle(element).fontFamily),
      wordmark.evaluate((element) => getComputedStyle(element).fontFamily),
      navigation.evaluate((element) => getComputedStyle(element).fontFamily),
    ]);
    expect(bodyFont).toContain("Space Grotesk");
    expect(wordmarkFont).toContain("Space Grotesk");
    expect(navigationFont).toContain("Space Grotesk");
  });
});
