import { expect, test, type Page } from "@playwright/test";

async function openCommunity(page: Page, reducedMotion: "no-preference" | "reduce") {
  await page.emulateMedia({
    colorScheme: "light",
    reducedMotion,
  });
  const response = await page.goto("/", { waitUntil: "domcontentloaded" });
  expect(response?.ok()).toBe(true);
  await page.evaluate(async () => {
    await document.fonts.ready;
  });
  const section = page.locator("[data-community-story]");
  await expect(section).toBeVisible();
  return section;
}

test("@community keeps the completed community readable with reduced motion", async ({
  page,
}) => {
  const section = await openCommunity(page, "reduce");

  await expect(section).toHaveAttribute("data-community-motion", "reduced");
  await expect(section).not.toHaveAttribute("data-community-ready", "");
  await expect(section.getByRole("heading", { level: 2 }).locator("span")).toHaveText([
    "Built for the community.",
    "Strengthened by every contribution.",
  ]);

  const avatars = section.locator("[data-community-avatar]");
  await expect(avatars).toHaveCount(10);
  const finalStyles = await avatars.evaluateAll((nodes) =>
    nodes.map((node) => {
      const styles = getComputedStyle(node);
      return { opacity: styles.opacity, transform: styles.transform };
    }),
  );
  expect(finalStyles).toEqual(
    Array.from({ length: 10 }, () => ({ opacity: "1", transform: "none" })),
  );
  const gridShape = await section.locator(".community-story__grid").evaluate((grid) => {
    const styles = getComputedStyle(grid);
    const rows = new Set(
      Array.from(grid.children, (child) =>
        Math.round((child as HTMLElement).getBoundingClientRect().top),
      ),
    );
    return {
      columns: styles.gridTemplateColumns.trim().split(/\s+/).length,
      rows: rows.size,
    };
  });
  expect(gridShape).toEqual({ columns: 5, rows: 2 });

  const dimensions = await page.evaluate(() => ({
    documentWidth: document.documentElement.scrollWidth,
    viewportWidth: document.documentElement.clientWidth,
  }));
  expect(dimensions.documentWidth).toBeLessThanOrEqual(dimensions.viewportWidth);
});

test("@community gathers contributors through reversible desktop scroll", async ({ page }) => {
  const section = await openCommunity(page, "no-preference");
  const viewport = page.viewportSize();
  expect(viewport).not.toBeNull();

  if ((viewport?.width ?? 0) <= 760 || (viewport?.height ?? 0) <= 700) {
    await expect(section).toHaveAttribute("data-community-motion", "simplified");
    await expect(section).not.toHaveAttribute("data-community-ready", "");
    return;
  }

  await expect(section).toHaveAttribute("data-community-motion", "scroll");
  await expect(section).toHaveAttribute("data-community-ready", "");

  const firstAvatar = section.locator("[data-community-avatar]").first();
  await section.evaluate((node) => window.scrollTo(0, (node as HTMLElement).offsetTop));
  await page.waitForFunction(() => {
    const story = document.querySelector<HTMLElement>("[data-community-story]");
    return Number(story?.dataset.communityProgress ?? 1) <= 0.01;
  });
  const startX = await firstAvatar.evaluate((node) =>
    (node as HTMLElement).style.getPropertyValue("--community-x"),
  );
  expect(startX).not.toBe("0.000vw");

  await section.evaluate((node) => {
    const story = node as HTMLElement;
    window.scrollTo(0, story.offsetTop + story.offsetHeight - window.innerHeight);
  });
  await page.waitForFunction(() => {
    const story = document.querySelector<HTMLElement>("[data-community-story]");
    return Number(story?.dataset.communityProgress ?? 0) >= 0.99;
  });
  await expect(firstAvatar).toHaveCSS("opacity", "1");
  const finalMotion = await firstAvatar.evaluate((node) => {
    const avatar = node as HTMLElement;
    return {
      opacity: avatar.style.getPropertyValue("--community-opacity"),
      rotation: avatar.style.getPropertyValue("--community-rotation"),
      scale: avatar.style.getPropertyValue("--community-scale"),
      x: avatar.style.getPropertyValue("--community-x"),
      y: avatar.style.getPropertyValue("--community-y"),
    };
  });
  expect(finalMotion).toEqual({
    opacity: "1.0000",
    rotation: "0.000deg",
    scale: "1.0000",
    x: "0.000vw",
    y: "0.000svh",
  });

  await section.evaluate((node) => window.scrollTo(0, (node as HTMLElement).offsetTop));
  await page.waitForFunction(() => {
    const story = document.querySelector<HTMLElement>("[data-community-story]");
    return Number(story?.dataset.communityProgress ?? 1) <= 0.01;
  });
  const reverseX = await firstAvatar.evaluate((node) =>
    (node as HTMLElement).style.getPropertyValue("--community-x"),
  );
  expect(reverseX).toBe(startX);
});
