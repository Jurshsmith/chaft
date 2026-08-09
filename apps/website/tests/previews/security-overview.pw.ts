import AxeBuilder from "@axe-core/playwright";
import { type Page, expect, test } from "@playwright/test";

const warning =
  "Unsigned and unaudited. Do not use Chaft canary builds for sensitive or production communication.";

async function openSecurityPage(page: Page) {
  const response = await page.goto("/security/", { waitUntil: "domcontentloaded" });
  expect(response?.ok()).toBe(true);
  await page.evaluate(async () => {
    await document.fonts.ready;
  });
  await expect(page.locator("main")).toBeVisible();
}

test("presents the current trust boundary, limits, and private reporting path", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });

  const homeResponse = await page.goto("/", { waitUntil: "domcontentloaded" });
  expect(homeResponse?.ok()).toBe(true);
  await expect(page.locator("[data-security-trust-hero]")).toHaveCount(0);

  await openSecurityPage(page);

  const hero = page.locator("[data-security-trust-hero]");
  const copy = page.locator("[data-security-trust-copy]");
  const visual = page.locator("[data-security-trust-path]");
  const rail = hero.locator(".security-boundary-rail");

  await expect(hero).toBeVisible();
  await expect(hero.getByRole("heading", { level: 1 })).toHaveText(
    "What Chaft protects today.",
  );
  await expect(hero).toContainText("encrypts message and attachment content before sync");
  await expect(hero).toContainText("verifies received events locally");
  await expect(hero.getByLabel("Canary warning")).toHaveText(warning);

  const warningOccurrences = await page.locator("body").evaluate(
    (body, exactWarning) => (body as HTMLElement).innerText.split(exactWarning).length - 1,
    warning,
  );
  expect(warningOccurrences).toBe(1);

  await expect(hero.getByRole("link", { name: "Read the security model" })).toHaveAttribute(
    "href",
    "/docs/concepts/security-model/",
  );
  await expect(hero.getByRole("link", { name: /Review current limits/ })).toHaveAttribute(
    "href",
    "#canary-limits",
  );
  await expect(hero.getByRole("link", { name: /private security advisory/i })).toHaveCount(0);

  for (const [label, value] of [
    ["Content", "Encrypted before sync"],
    ["Metadata", "Visible"],
    ["Audit", "Not completed"],
    ["Use", "Non-sensitive testing"],
  ] as const) {
    await expect(rail.locator("div").filter({ hasText: label })).toContainText(value);
  }

  await expect(visual).toBeVisible();
  await expect(visual.locator("img")).toHaveCount(0);
  await expect(visual).toContainText("Device A");
  await expect(visual).toContainText("Signed event");
  await expect(visual).toContainText("Device B");
  await expect(visual).toContainText("Optional replica");
  await expect(visual.locator("figcaption")).toContainText(
    "The peer or replica that carried it is never authoritative.",
  );

  const properties = page.locator("[data-security-properties]");
  await expect(
    properties.getByRole("heading", { name: "Current security properties" }),
  ).toBeVisible();
  await expect(properties.locator("li")).toHaveCount(5);
  for (const title of [
    "Verified locally",
    "Signed by devices",
    "Authorization fails closed",
    "Content encrypted before sync",
    "Untrusted input is bounded",
  ]) {
    await expect(properties.getByRole("heading", { name: title })).toBeVisible();
  }
  await expect(properties).not.toContainText(/\b0[1-5]\b/);

  const limits = page.locator("[data-security-limits]");
  await expect(
    limits.getByRole("heading", { name: "Why the canary is not production ready" }),
  ).toBeVisible();
  await expect(limits.locator("article")).toHaveCount(6);
  for (const title of [
    "Independent review",
    "Local device exposure",
    "Visible metadata",
    "Unsigned distribution",
    "Availability",
    "Recovery and revocation",
  ]) {
    await expect(limits.getByRole("heading", { name: title })).toBeVisible();
  }

  const report = page.locator("[data-security-report]");
  await expect(
    report.getByRole("heading", { name: "Report a vulnerability privately" }),
  ).toBeVisible();
  await expect(
    report.getByRole("link", { name: "Open a private security advisory" }),
  ).toHaveAttribute("href", "https://github.com/Jurshsmith/chaft/security/advisories/new");
  await expect(report.getByRole("link", { name: "Read the security policy" })).toHaveAttribute(
    "href",
    "https://github.com/Jurshsmith/chaft/blob/main/SECURITY.md",
  );

  const viewport = page.viewportSize();
  expect(viewport).not.toBeNull();
  const [copyBox, visualBox] = await Promise.all([copy.boundingBox(), visual.boundingBox()]);
  expect(copyBox).not.toBeNull();
  expect(visualBox).not.toBeNull();
  if ((viewport?.width ?? 0) <= 840) {
    expect((copyBox?.y ?? 0) + (copyBox?.height ?? 0)).toBeLessThanOrEqual(
      (visualBox?.y ?? 0) + 1,
    );
  } else {
    expect((copyBox?.x ?? 0) + (copyBox?.width ?? 0)).toBeLessThanOrEqual(
      (visualBox?.x ?? 0) + 1,
    );
  }

  const dimensions = await page.evaluate(() => ({
    documentWidth: document.documentElement.scrollWidth,
    viewportWidth: document.documentElement.clientWidth,
  }));
  expect(dimensions.documentWidth).toBeLessThanOrEqual(dimensions.viewportWidth);

  for (const link of [
    hero.getByRole("link", { name: "Read the security model" }),
    hero.getByRole("link", { name: /Review current limits/ }),
    report.getByRole("link", { name: "Open a private security advisory" }),
    report.getByRole("link", { name: "Read the security policy" }),
  ]) {
    const box = await link.boundingBox();
    expect(box).not.toBeNull();
    expect(box?.height ?? 0).toBeGreaterThanOrEqual(44);
  }

  const visualMotion = await visual.evaluate((element) => {
    const styles = getComputedStyle(element);
    return {
      animationName: styles.animationName,
      transitionDuration: styles.transitionDuration,
    };
  });
  expect(visualMotion).toEqual({ animationName: "none", transitionDuration: "0s" });
});

test("@accessibility keeps the complete security route free of serious WCAG findings", async ({
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

test("keeps the complete security route usable without JavaScript", async ({
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

  await expect(page.locator("[data-security-trust-hero]")).toBeVisible();
  await expect(page.locator("[data-security-trust-path]")).toBeVisible();
  await expect(page.locator("[data-security-properties]")).toBeVisible();
  await expect(page.locator("[data-security-limits]")).toBeVisible();
  await expect(page.locator("[data-security-report]")).toBeVisible();
  await expect(page.getByText(warning, { exact: true })).toBeVisible();

  await context.close();
});
