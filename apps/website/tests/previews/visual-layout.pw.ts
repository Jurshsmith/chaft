import { expect, test } from "@playwright/test";

import { openLandingPage } from "./support";

test("@visual keeps the landing page within its viewport and captures it", async ({
  page,
}, testInfo) => {
  await openLandingPage(page);

  const dimensions = await page.evaluate(() => ({
    documentWidth: document.documentElement.scrollWidth,
    viewportWidth: document.documentElement.clientWidth,
  }));
  expect(dimensions.documentWidth).toBeLessThanOrEqual(dimensions.viewportWidth);

  const hero = page.locator(".hero");
  const heading = page.locator(".hero h1");
  const actions = page.locator(".hero__actions");
  await expect(hero).toBeVisible();
  await expect(heading).toBeVisible();
  await expect(actions).toBeVisible();

  const [heroBox, headingBox, actionsBox] = await Promise.all([
    hero.boundingBox(),
    heading.boundingBox(),
    actions.boundingBox(),
  ]);
  expect(heroBox).not.toBeNull();
  expect(headingBox).not.toBeNull();
  expect(actionsBox).not.toBeNull();

  for (const [name, box] of [
    ["heading", headingBox],
    ["actions", actionsBox],
  ] as const) {
    expect(box?.x, `${name} starts outside the viewport`).toBeGreaterThanOrEqual(0);
    expect(
      (box?.x ?? 0) + (box?.width ?? 0),
      `${name} extends beyond the viewport`,
    ).toBeLessThanOrEqual(dimensions.viewportWidth + 1);
  }

  const screenshot = await page.screenshot({
    animations: "disabled",
    fullPage: true,
  });
  expect(screenshot.byteLength).toBeGreaterThan(10_000);
  await testInfo.attach(`landing-${testInfo.project.name}.png`, {
    body: screenshot,
    contentType: "image/png",
  });
});
