import { expect, test } from "@playwright/test";

import { expectedPreviewHero, openLandingPage } from "./support";

const canaryWarning =
  "Unsigned and unaudited. Do not use Chaft canary builds for sensitive or production communication.";

test.describe("production landing-page product story", () => {
  test.skip(
    expectedPreviewHero !== undefined,
    "Preview branches have their own hero contract",
  );

  test.beforeEach(async ({ page }) => {
    await openLandingPage(page);
  });

  test("leads with the product, its everyday use, and the operating model", async ({
    page,
  }) => {
    const hero = page.locator(".hero[data-chaft-hero='baseline']");
    await expect(hero).toBeVisible();
    await expect(hero.getByRole("heading", { level: 1 })).toHaveText(
      "Team chat without a required central server.",
    );

    await expect(
      hero.getByRole("link", { name: /(?:download|view) the early build/i }),
    ).toHaveAttribute("href", "/download/");
    await expect(hero.getByRole("link", { name: "See how it works" })).toHaveAttribute(
      "href",
      "#how-it-works",
    );

    const workspace = hero.getByRole("img", {
      name: /Chaft desktop workspace showing rooms, threaded replies, an attachment, member access, search, and current synchronization status/i,
    });
    await expect(workspace).toBeVisible();
    await expect(hero.locator(".workspace-figure figcaption")).toHaveText(
      "Representative view of the current desktop preview.",
    );

    const product = page.locator("#product");
    await expect(product.getByRole("heading", { level: 2 })).toHaveText(
      "The everyday parts of team chat, kept together.",
    );
    await expect(product.locator(".moment-card")).toHaveCount(3);
    await expect(product.locator(".moment-card h3")).toHaveText([
      "Rooms and replies",
      "Files and local search",
      "Invitations and access",
    ]);

    const operatingModel = page.locator("#how-it-works");
    await expect(operatingModel.getByRole("heading", { level: 2 })).toHaveText(
      "Create, invite, then sync.",
    );
    await expect(operatingModel.locator(".sync-steps > li")).toHaveCount(3);
    await expect(operatingModel.locator(".sync-steps h3")).toHaveText([
      "Create",
      "Invite",
      "Sync",
    ]);
  });

  test("states the early-build limitation once and keeps product proof responsive", async ({
    page,
  }) => {
    const warning = page.locator("main .canary-notice__warning");
    await expect(warning).toHaveCount(1);
    await expect(warning).toContainText(canaryWarning);
    await expect(
      page.getByRole("link", { name: "Open a public issue" }),
    ).toHaveAttribute("href", "https://github.com/Jurshsmith/chaft/issues");
    await expect(
      page.getByRole("link", { name: "Use the private reporting process" }),
    ).toHaveAttribute(
      "href",
      "https://github.com/Jurshsmith/chaft/blob/main/SECURITY.md",
    );

    const layout = await page.evaluate(() => ({
      documentWidth: document.documentElement.scrollWidth,
      viewportWidth: document.documentElement.clientWidth,
    }));
    expect(layout.documentWidth).toBeLessThanOrEqual(layout.viewportWidth);

    const viewport = page.viewportSize();
    if (viewport && viewport.width <= 680) {
      const carousel = page.locator("#product .moment-grid");
      const carouselDimensions = await carousel.evaluate((element) => ({
        clientWidth: element.clientWidth,
        scrollWidth: element.scrollWidth,
      }));
      expect(carouselDimensions.scrollWidth).toBeGreaterThan(
        carouselDimensions.clientWidth,
      );
      await expect(carousel.locator(".moment-card").first()).toBeVisible();
    }
  });
});
