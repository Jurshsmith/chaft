import AxeBuilder from "@axe-core/playwright";
import { type Page, expect, test } from "@playwright/test";

async function openDocumentation(page: Page) {
  const response = await page.goto("/docs/", { waitUntil: "domcontentloaded" });
  expect(response?.ok()).toBe(true);
  await page.evaluate(async () => {
    await document.fonts.ready;
  });
  const overview = page.locator("[data-docs-overview='true']");
  await expect(overview).toBeVisible();
  return overview;
}

test.describe("documentation overview", () => {
  test("starts with clear choices and a task-ordered setup path", async ({ page }) => {
    const overview = await openDocumentation(page);
    const article = overview.locator(".docs-article");
    const prose = article.locator(".docs-prose");

    await expect(page.getByRole("heading", { level: 1 })).toHaveText("Documentation");
    await expect(prose.locator(":scope > :first-child")).toHaveJSProperty(
      "tagName",
      "H1",
    );
    await expect(article.locator(".docs-breadcrumbs, .docs-status, .docs-toc")).toHaveCount(0);
    await expect(overview.locator(".docs-sidebar--mobile")).toHaveCount(0);
    await expect(overview).not.toContainText("Chaft field manual");
    await expect(article).not.toContainText(/\bcanary\b/i);

    const choices = prose.locator("#start-here + ul > li");
    await expect(choices).toHaveCount(3);
    await expect(choices.locator("strong")).toHaveText([
      "Try the early build:",
      "Understand the risks:",
      "Contribute:",
    ]);

    const useSteps = prose.locator("#use-chaft + ol > li");
    await expect(useSteps).toHaveCount(5);
    await expect(useSteps.locator("strong")).toHaveText([
      "Install:",
      "Create or join:",
      "Invite and manage access:",
      "Keep credentials safe:",
      "Back up and export:",
    ]);

    await expect(prose.locator("#current-preview-limits + ul > li")).toHaveCount(3);
    await expect(prose.locator("#report-a-problem + ul > li")).toHaveCount(2);
    await expect(prose.getByRole("link", { name: "Open a public issue" })).toHaveAttribute(
      "href",
      "https://github.com/Jurshsmith/chaft/issues/new",
    );
    await expect(
      prose.getByRole("link", { name: "Report suspected vulnerabilities privately" }),
    ).toHaveAttribute(
      "href",
      "https://github.com/Jurshsmith/chaft/security/advisories/new",
    );

    const dimensions = await page.evaluate(() => ({
      documentWidth: document.documentElement.scrollWidth,
      viewportHeight: window.innerHeight,
      viewportWidth: document.documentElement.clientWidth,
    }));
    expect(dimensions.documentWidth).toBeLessThanOrEqual(dimensions.viewportWidth);

    if (dimensions.viewportWidth <= 390) {
      const startHere = await prose.locator("#start-here").boundingBox();
      expect(startHere, "Start here should be visible in the first mobile viewport").not.toBeNull();
      expect(startHere?.y ?? Number.POSITIVE_INFINITY).toBeLessThan(
        dimensions.viewportHeight,
      );
    }
  });

  test("keeps section anchors below the sticky header", async ({ page }) => {
    const overview = await openDocumentation(page);
    const heading = overview.locator("#current-preview-limits");
    await heading.evaluate((element) => {
      element.scrollIntoView({ behavior: "instant", block: "start" });
    });

    const [headingBox, headerBox] = await Promise.all([
      heading.boundingBox(),
      page.locator(".site-header").boundingBox(),
    ]);
    expect(headingBox).not.toBeNull();
    expect(headerBox).not.toBeNull();
    expect(headingBox?.y ?? 0).toBeGreaterThanOrEqual(
      (headerBox?.y ?? 0) + (headerBox?.height ?? 0),
    );
  });

  test("@accessibility has no serious or critical WCAG A/AA findings", async ({
    page,
  }, testInfo) => {
    test.skip(
      testInfo.project.name !== "chromium-390",
      "One representative mobile engine covers the static documentation markup",
    );
    await openDocumentation(page);

    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
      .analyze();
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
});
