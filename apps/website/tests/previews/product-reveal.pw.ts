import { expect, test, type Locator, type Page } from "@playwright/test";

import { openLandingPage } from "./support";

async function getLaptopBounds(laptop: Locator) {
  return laptop.evaluate((element) => {
    const parts = [
      element.querySelector(".laptop__lid"),
      element.querySelector(".laptop__base"),
    ].filter((part): part is Element => part instanceof Element);
    const rectangles = parts.map((part) => part.getBoundingClientRect());
    const left = Math.min(...rectangles.map((rectangle) => rectangle.left));
    const right = Math.max(...rectangles.map((rectangle) => rectangle.right));
    const top = Math.min(...rectangles.map((rectangle) => rectangle.top));
    const bottom = Math.max(...rectangles.map((rectangle) => rectangle.bottom));

    return {
      bottom,
      centerX: (left + right) / 2,
      left,
      right,
      top,
      width: right - left,
    };
  });
}

async function waitForScrollFrame(page: Page) {
  await page.evaluate(
    () => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))),
  );
}

test("keeps the product proof static when reduced motion is requested", async ({
  page,
}) => {
  await openLandingPage(page);

  const section = page.locator("[data-product-reveal]");
  const laptop = page.locator("[data-reveal-laptop]");
  const image = laptop.getByRole("img");

  await expect(section).toHaveAttribute("data-reveal-motion", "reduced");
  await expect(section).not.toHaveAttribute("data-reveal-ready", "");
  await expect(laptop).toBeVisible();
  await expect(image).toBeVisible();
  await image.scrollIntoViewIfNeeded();
  await expect
    .poll(async () => image.evaluate((element) => (element as HTMLImageElement).naturalWidth))
    .toBeGreaterThan(0);
  const imageGeometry = await image.evaluate((element) => {
    const imageElement = element as HTMLImageElement;
    return {
      width: imageElement.naturalWidth,
      height: imageElement.naturalHeight,
    };
  });
  expect(Math.abs(imageGeometry.width / imageGeometry.height - 1440 / 820)).toBeLessThan(
    0.02,
  );

  const transform = await laptop.evaluate((element) => getComputedStyle(element).transform);
  expect(transform).toBe("none");
});

test("scrubs the laptop reveal from its centered opening state to full scale", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "no-preference" });
  const response = await page.goto("/", { waitUntil: "domcontentloaded" });
  expect(response?.ok()).toBe(true);
  await page.evaluate(async () => {
    await document.fonts.ready;
  });

  const section = page.locator("[data-product-reveal]");
  const laptop = page.locator("[data-reveal-laptop]");
  await expect(section).toHaveAttribute("data-reveal-motion", "scroll");
  await expect(section).toHaveAttribute("data-reveal-ready", "");
  await page.evaluate(() => {
    document.documentElement.style.scrollBehavior = "auto";
  });

  const geometry = await section.evaluate((element) => {
    const htmlElement = element as HTMLElement;
    const top = element.getBoundingClientRect().top + window.scrollY;
    return {
      top,
      range: htmlElement.offsetHeight - window.innerHeight,
    };
  });

  await page.evaluate((top) => window.scrollTo(0, top), geometry.top);
  await waitForScrollFrame(page);
  await expect(section).toHaveAttribute("data-reveal-progress", /^0\.0/);
  const openingScale = await laptop.evaluate((element) =>
    Number.parseFloat(getComputedStyle(element).getPropertyValue("--reveal-scale")),
  );
  const openingBounds = await getLaptopBounds(laptop);
  const openingCaption = await page.locator(".product-reveal__stage figcaption").boundingBox();

  await page.evaluate(
    ({ top, range }) => window.scrollTo(0, top + range * 0.55),
    geometry,
  );
  await waitForScrollFrame(page);
  await expect
    .poll(async () => Number(await section.getAttribute("data-reveal-progress")))
    .toBeGreaterThan(0.5);
  const middleScale = await laptop.evaluate((element) =>
    Number.parseFloat(getComputedStyle(element).getPropertyValue("--reveal-scale")),
  );
  const middleBounds = await getLaptopBounds(laptop);

  await page.evaluate(
    ({ top, range }) => window.scrollTo(0, top + range),
    geometry,
  );
  await waitForScrollFrame(page);
  await expect
    .poll(async () => Number(await section.getAttribute("data-reveal-progress")))
    .toBeGreaterThanOrEqual(0.99);
  const finalScale = await laptop.evaluate((element) =>
    Number.parseFloat(getComputedStyle(element).getPropertyValue("--reveal-scale")),
  );
  const finalBounds = await getLaptopBounds(laptop);
  const viewport = page.viewportSize();
  expect(viewport).not.toBeNull();

  expect(openingScale).toBeGreaterThanOrEqual(0.43);
  expect(openingScale).toBeLessThanOrEqual(0.71);
  expect(middleScale).toBeGreaterThan(openingScale);
  expect(finalScale).toBeGreaterThan(middleScale);
  expect(finalScale).toBeCloseTo(1, 2);
  expect(middleBounds.width).toBeGreaterThan(openingBounds.width);
  expect(finalBounds.width).toBeGreaterThan(middleBounds.width);
  expect(openingCaption).not.toBeNull();
  expect(openingCaption?.y).toBeGreaterThan(openingBounds.bottom);
  expect(finalBounds.width).toBeGreaterThanOrEqual((viewport?.width ?? 0) * 0.85);
  expect(finalBounds.width).toBeLessThanOrEqual((viewport?.width ?? 0) * 0.96);
  expect(finalBounds.top).toBeLessThan(openingBounds.top);
  expect(finalBounds.bottom).toBeLessThanOrEqual((viewport?.height ?? 0) + 1);

  for (const bounds of [openingBounds, middleBounds, finalBounds]) {
    expect(Math.abs(bounds.centerX - (viewport?.width ?? 0) / 2)).toBeLessThanOrEqual(1.5);
  }

  const layout = await page.evaluate(() => ({
    documentWidth: document.documentElement.scrollWidth,
    nextSectionTop: document.querySelector("#why-chaft")?.getBoundingClientRect().top,
    viewportHeight: window.innerHeight,
    viewportWidth: document.documentElement.clientWidth,
  }));
  expect(layout.documentWidth).toBeLessThanOrEqual(layout.viewportWidth);
  expect(layout.nextSectionTop).toBeGreaterThanOrEqual(layout.viewportHeight - 1);
});
