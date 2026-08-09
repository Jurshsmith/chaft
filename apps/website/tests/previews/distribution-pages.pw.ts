import { readFileSync } from "node:fs";

import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

interface ReleaseManifest {
  assets: Array<{
    arch: string;
    available: boolean;
    id: string;
    os: "windows" | "macos" | "linux";
    url: string;
  }>;
  version: string;
}

const release = JSON.parse(
  readFileSync(new URL("../../src/data/release-manifest.json", import.meta.url), "utf8"),
) as ReleaseManifest;
const safetyWarning =
  "Do not use Chaft canary builds for sensitive or production communication.";

test("groups the detailed download page into three operating-system choices", async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(window.navigator, "platform", {
      configurable: true,
      get: () => "MacIntel",
    });
    Object.defineProperty(window.navigator, "userAgentData", {
      configurable: true,
      get: () => ({ mobile: false, platform: "macOS" }),
    });
    Object.defineProperty(window.navigator, "userAgent", {
      configurable: true,
      get: () => "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit Safari",
    });
    Object.defineProperty(window.navigator, "maxTouchPoints", {
      configurable: true,
      get: () => 0,
    });
  });

  const response = await page.goto("/download/", { waitUntil: "domcontentloaded" });
  expect(response?.ok()).toBe(true);

  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Download Chaft.");
  const warning = page.locator("[data-release-warning]");
  await expect(warning).toHaveCount(1);
  await expect(warning).toContainText(safetyWarning);

  const grid = page.locator("[data-download-grid]");
  const cards = grid.locator("[data-download-card]");
  await expect(cards).toHaveCount(3);
  for (const os of ["windows", "macos", "linux"] as const) {
    await expect(grid.locator(`[data-download-card][data-os="${os}"]`)).toHaveCount(1);
  }

  const macos = grid.locator('[data-download-card][data-os="macos"]');
  await expect(macos.locator("[data-download-option]")).toHaveCount(2);
  await expect(macos).toContainText("Apple Silicon · arm64");
  await expect(macos).toContainText("Intel · x86_64");
  await expect(grid.locator('[data-download-card][data-platform-match="true"]')).toHaveCount(1);
  await expect(macos.locator("[data-recommended]")).toBeVisible();
  await expect(grid.locator("[data-recommended]:visible")).toHaveCount(1);

  for (const asset of release.assets.filter(({ available }) => available)) {
    const option = grid.locator(`[data-download-option="${asset.id}"]`);
    await expect(option).toHaveCount(1);
    await expect(option.locator(`a[href="${asset.url}"]`)).toBeVisible();
    await expect(option.locator(".download-option__summary strong")).toContainText(asset.arch);
    await expect(option.getByText("Verify this package", { exact: true })).toBeVisible();
  }
});

test("presents a current-first release history and preserves JSON records", async ({
  page,
  request,
}) => {
  const response = await page.goto("/releases/", { waitUntil: "domcontentloaded" });
  expect(response?.ok()).toBe(true);

  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Releases");
  await expect(page.locator("[data-release-warning]")).toHaveCount(1);
  await expect(page.locator("[data-release-warning]")).toContainText(safetyWarning);
  const rows = page.locator("[data-release-row]");
  await expect(rows).toHaveCount(3);
  await expect(rows.first()).toHaveAttribute("data-current", "true");
  await expect(rows.first()).toContainText(`v${release.version}`);
  await expect(rows.first()).toContainText("4 packages");

  const currentJson = await request.get("/releases/current.json");
  expect(currentJson.ok()).toBe(true);
  expect((await currentJson.json()).version).toBe(release.version);

  const versionJson = await request.get(`/releases/${release.version}.json`);
  expect(versionJson.ok()).toBe(true);
  expect((await versionJson.json()).assets).toHaveLength(release.assets.length);
});

test("keeps release detail scannable by system and expands evidence on demand", async ({
  page,
}) => {
  const response = await page.goto(`/releases/${release.version}/`, {
    waitUntil: "domcontentloaded",
  });
  expect(response?.ok()).toBe(true);

  await expect(page.getByRole("heading", { level: 1 })).toHaveText(
    `Chaft v${release.version}`,
  );
  await expect(page.locator(".release-summary > div")).toHaveCount(5);
  await expect(page.locator("[data-release-warning]")).toHaveCount(1);
  await expect(page.locator("[data-release-warning]")).toContainText(safetyWarning);

  await expect(page.locator("[data-release-platform]")).toHaveCount(3);
  await expect(page.locator('[data-release-platform="macos"] [data-release-artifact]')).toHaveCount(2);
  await expect(page.locator("[data-release-artifact]")).toHaveCount(release.assets.length);

  const firstArtifact = page.locator("[data-release-artifact]").first();
  const evidence = firstArtifact.getByText("Checksums and build evidence", {
    exact: true,
  });
  await evidence.click();
  await expect(firstArtifact.getByText("SHA-256", { exact: true })).toBeVisible();
  await expect(firstArtifact.getByRole("link", { name: "SBOM", exact: true })).toBeVisible();
});

test("@accessibility distribution pages have no serious or critical findings", async ({
  page,
}, testInfo) => {
  test.skip(
    testInfo.project.name !== "chromium-1440",
    "One desktop engine is sufficient for the route-level accessibility audit",
  );

  for (const path of ["/download/", "/releases/", `/releases/${release.version}/`]) {
    const response = await page.goto(path, { waitUntil: "domcontentloaded" });
    expect(response?.ok()).toBe(true);
    const results = await new AxeBuilder({ page })
      .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
      .analyze();
    const blockingFindings = results.violations.filter(
      ({ impact }) => impact === "serious" || impact === "critical",
    );
    expect(
      blockingFindings,
      `${path}: ${blockingFindings
        .map(({ id, impact, help }) => `${impact ?? "unknown"} ${id}: ${help}`)
        .join("\n")}`,
    ).toEqual([]);
  }
});
