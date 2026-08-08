import AxeBuilder from "@axe-core/playwright";
import { type Page, expect, test } from "@playwright/test";

async function openSecurityPage(page: Page) {
  const response = await page.goto("/security/", { waitUntil: "domcontentloaded" });
  expect(response?.ok()).toBe(true);
  await page.evaluate(async () => {
    await document.fonts.ready;
  });
}

test("keeps the security design on its canonical page and static with reduced motion", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  const homeResponse = await page.goto("/", { waitUntil: "domcontentloaded" });
  expect(homeResponse?.ok()).toBe(true);
  await expect(page.locator("[data-security-page-hero]")).toHaveCount(0);
  await openSecurityPage(page);

  const section = page.locator("[data-security-page-hero]");
  const copy = page.locator("[data-security-copy]");
  const visual = page.locator("[data-security-visual]");
  const image = visual.locator("img");

  await section.scrollIntoViewIfNeeded();
  await expect(section).toBeVisible();
  await expect(section).toHaveAttribute("data-security-motion", "reduced");
  await expect(section).toHaveAttribute("data-security-visible", "");
  await expect(section.getByRole("heading", { level: 1 })).toHaveText(
    "Encrypted content. Signed, verifiable history.",
  );
  await expect(section).toContainText(
    "Chaft encrypts message and attachment content before replication",
  );
  await expect(section).toContainText("treats peers and optional replicas as untrusted");
  await expect(section).toContainText(
    "No central server is authoritative for workspace history.",
  );
  await expect(section).toContainText("Metadata is not fully hidden");
  await expect(page.locator(".security-summary")).toContainText("Independent audit");
  await expect(page.locator(".security-summary")).toContainText("Not completed");
  await expect(section.getByRole("link", { name: "Report privately" })).toHaveAttribute(
    "href",
    "https://github.com/Jurshsmith/chaft/security/advisories/new",
  );
  await expect(
    section.getByRole("link", { name: "Read the security model" }),
  ).toHaveAttribute("href", "/docs/concepts/security-model/");
  await expect(
    section.getByRole("link", { name: "Read the full policy" }),
  ).toHaveAttribute("href", "https://github.com/Jurshsmith/chaft/blob/main/SECURITY.md");
  await expect(section.locator("figcaption")).toContainText(
    "Shielded tile: encrypted message and attachment content.",
  );
  await expect(section.locator("figcaption")).toContainText(
    "Outer nodes: participating devices.",
  );
  await expect(image).toBeVisible();
  await expect
    .poll(async () =>
      image.evaluate((element) => (element as HTMLImageElement).naturalWidth),
    )
    .toBeGreaterThan(0);

  const imageGeometry = await image.evaluate((element) => {
    const imageElement = element as HTMLImageElement;
    return {
      height: imageElement.naturalHeight,
      width: imageElement.naturalWidth,
    };
  });
  expect(Math.abs(imageGeometry.width / imageGeometry.height - 4 / 3)).toBeLessThan(0.02);

  const [copyBox, visualBox] = await Promise.all([copy.boundingBox(), visual.boundingBox()]);
  expect(copyBox).not.toBeNull();
  expect(visualBox).not.toBeNull();

  const viewport = page.viewportSize();
  expect(viewport).not.toBeNull();
  if ((viewport?.width ?? 0) <= 820) {
    expect((copyBox?.y ?? 0) + (copyBox?.height ?? 0)).toBeLessThanOrEqual(
      (visualBox?.y ?? 0) + 1,
    );
  } else {
    expect((copyBox?.x ?? 0) + (copyBox?.width ?? 0)).toBeLessThanOrEqual(
      (visualBox?.x ?? 0) + 1,
    );
  }

  const styles = await Promise.all(
    [copy, visual].map((locator) =>
      locator.evaluate((element) => ({
        opacity: getComputedStyle(element).opacity,
        transform: getComputedStyle(element).transform,
      })),
    ),
  );
  for (const style of styles) {
    expect(style.opacity).toBe("1");
    expect(style.transform).toBe("none");
  }

  const dimensions = await page.evaluate(() => ({
    documentWidth: document.documentElement.scrollWidth,
    viewportWidth: document.documentElement.clientWidth,
  }));
  expect(dimensions.documentWidth).toBeLessThanOrEqual(dimensions.viewportWidth);
});

test("@accessibility keeps the security page free of serious WCAG findings", async ({
  page,
}, testInfo) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  await openSecurityPage(page);

  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .analyze();

  await testInfo.attach("security-axe-results.json", {
    body: Buffer.from(JSON.stringify(results, null, 2)),
    contentType: "application/json",
  });

  const blockingFindings = results.violations.filter(
    ({ impact }) => impact === "serious" || impact === "critical",
  );
  expect(
    blockingFindings,
    blockingFindings
      .map(
        ({ id, impact, help, nodes }) =>
          `${impact ?? "unknown"} ${id}: ${help} (${nodes.length} nodes)`,
      )
      .join("\n"),
  ).toEqual([]);
});

test("reveals only the security visual once without persistent motion", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "no-preference" });
  await openSecurityPage(page);

  const section = page.locator("[data-security-page-hero]");
  const visual = page.locator("[data-security-visual]");
  await expect(section).toHaveAttribute("data-security-motion", "entrance");
  await section.scrollIntoViewIfNeeded();
  await expect(section).toHaveAttribute("data-security-visible", "");

  await expect
    .poll(async () =>
      visual.evaluate((element) => ({
        opacity: getComputedStyle(element).opacity,
        transform: getComputedStyle(element).transform,
      })),
    )
    .toEqual({ opacity: "1", transform: "none" });

  const motion = await visual.evaluate((element) => {
    const styles = getComputedStyle(element);
    return {
      animationName: styles.animationName,
      transitionDuration: styles.transitionDuration,
    };
  });
  expect(motion.animationName).toBe("none");
  expect(motion.transitionDuration).toBe("0.52s, 0.62s");

  await page.evaluate(() => window.scrollTo(0, 0));
  await expect(section).toHaveAttribute("data-security-visible", "");
});

test("keeps the complete security overview visible without JavaScript", async ({
  baseURL,
  browser,
}, testInfo) => {
  test.skip(
    testInfo.project.name !== "chromium-1440",
    "One desktop engine is sufficient for the no-JavaScript contract",
  );
  if (!baseURL) throw new Error("No-JavaScript contract requires a configured base URL");

  const context = await browser.newContext({
    baseURL,
    colorScheme: "light",
    javaScriptEnabled: false,
    reducedMotion: "reduce",
    viewport: { height: 900, width: 1440 },
  });
  const page = await context.newPage();
  const response = await page.goto("/security/", { waitUntil: "domcontentloaded" });
  expect(response?.ok()).toBe(true);

  const section = page.locator("[data-security-page-hero]");
  const copy = page.locator("[data-security-copy]");
  const visual = page.locator("[data-security-visual]");
  await section.scrollIntoViewIfNeeded();
  await expect(section).toHaveAttribute("data-security-motion", "static");
  await expect(section).not.toHaveAttribute("data-security-visible", "");
  await expect(copy).toBeVisible();
  await expect(visual).toBeVisible();

  const styles = await Promise.all(
    [copy, visual].map((locator) =>
      locator.evaluate((element) => ({
        opacity: getComputedStyle(element).opacity,
        transform: getComputedStyle(element).transform,
      })),
    ),
  );
  for (const style of styles) {
    expect(style).toEqual({ opacity: "1", transform: "none" });
  }

  await context.close();
});
