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

    await expect(
      footer.getByRole("list", { name: "Chaft product principles" }).getByRole("listitem"),
    ).toHaveCount(4);
    await expect(
      footer.getByRole("link", { name: "Download the current canary" }),
    ).toHaveAttribute("href", "/download/");
    await expect(
      footer.getByText(
        "Unaudited canary software. Not for sensitive or production communication.",
        { exact: true },
      ),
    ).toBeVisible();

    for (const [label, href] of [
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
        .locator(".footer-pitch__copy")
        .evaluate((element) => getComputedStyle(element).fontFamily),
      wordmark.evaluate((element) => getComputedStyle(element).fontFamily),
      footer
        .locator(".footer-nav")
        .evaluate((element) => getComputedStyle(element).fontFamily),
    ]);
    expect(bodyFont).toContain("Chillax");
    expect(wordmarkFont).toContain("Space Grotesk");
    expect(navigationFont).toContain("Space Grotesk");
  });
});
