import { readFileSync } from "node:fs";

import AxeBuilder from "@axe-core/playwright";
import { type Locator, type Page, expect, test } from "@playwright/test";

interface PublishedAsset {
  arch: string;
  available: boolean;
  filename: string | null;
  os: string;
  sha256: string | null;
  url: string;
}

const releaseManifest = JSON.parse(
  readFileSync(new URL("../../src/data/release-manifest.json", import.meta.url), "utf8"),
) as { assets: PublishedAsset[] };
const expectedAssets = releaseManifest.assets.filter((asset) => asset.available);

function expectedAsset(os: string, arch?: string) {
  const asset = expectedAssets.find(
    (candidate) => candidate.os === os && (arch === undefined || candidate.arch === arch),
  );
  if (!asset) throw new Error(`Missing ${os}${arch ? ` ${arch}` : ""} release asset`);
  return asset;
}

async function openDownloadPage(page: Page) {
  const response = await page.goto("/download/", { waitUntil: "domcontentloaded" });
  expect(response?.ok()).toBe(true);
  await page.evaluate(async () => {
    await document.fonts.ready;
  });
  const experience = page.locator("[data-download-experience]");
  await expect(experience).toBeVisible();
  return experience;
}

function platformTab(experience: Locator, name: "Windows" | "macOS" | "Linux") {
  return experience.getByRole("tab", { name: new RegExp(`^${name}\\b`, "i") });
}

async function controlledPanel(page: Page, tab: Locator) {
  const id = await tab.getAttribute("aria-controls");
  expect(id, "Each platform tab must identify its tabpanel").toBeTruthy();
  return page.locator(`#${id}`);
}

async function expectOnlyPlatformVisible(
  experience: Locator,
  activePlatform: "windows" | "macos" | "linux",
) {
  const panels = experience.locator("[data-platform-panel]");
  await expect(panels).toHaveCount(3);

  for (const platform of ["windows", "macos", "linux"] as const) {
    const panel = experience.locator(`[data-platform-panel="${platform}"]`);
    if (platform === activePlatform) {
      await expect(panel).toBeVisible();
      await expect(panel).not.toHaveAttribute("hidden", "");
    } else {
      await expect(panel).toHaveAttribute("hidden", "");
      await expect(panel).toBeHidden();
    }
  }
}

async function expectOnlyArtifactVisible(panel: Locator, activeAssetId: string) {
  const artifacts = panel.locator("[data-download-artifact-panel]");
  await expect(artifacts).toHaveCount(2);

  for (const artifact of await artifacts.all()) {
    const assetId = await artifact.getAttribute("data-download-artifact-panel");
    if (assetId === activeAssetId) {
      await expect(artifact).toBeVisible();
      await expect(artifact).not.toHaveAttribute("hidden", "");
    } else {
      await expect(artifact).toHaveAttribute("hidden", "");
      await expect(artifact).toBeHidden();
    }
  }
}

test("keeps the selector canonical to /download/ and uses only published assets", async ({
  page,
}) => {
  const homeResponse = await page.goto("/", { waitUntil: "domcontentloaded" });
  expect(homeResponse?.ok()).toBe(true);
  await expect(page.locator("[data-download-experience]")).toHaveCount(0);

  const experience = await openDownloadPage(page);
  expect(expectedAssets).toHaveLength(4);
  await expect(experience.getByRole("heading", { level: 1 })).toHaveCount(1);
  await expect(experience.getByRole("heading", { level: 1 })).toHaveText(
    "Download Chaft for desktop.",
  );

  const tablist = experience.getByRole("tablist", { name: /operating system|platform/i });
  await expect(tablist).toBeVisible();
  await expect(tablist.getByRole("tab")).toHaveCount(3);

  for (const [name, os, arch] of [
    ["Windows", "windows", "x86_64"],
    ["macOS", "macos", "arm64"],
    ["Linux", "linux", "x86_64"],
  ] as const) {
    const tab = platformTab(experience, name);
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");
    await expectOnlyPlatformVisible(experience, os);

    const panel = await controlledPanel(page, tab);
    const asset = expectedAsset(os, arch);
    await expect(panel).toBeVisible();
    await expect(panel).toContainText(asset.filename ?? "");
    await expect(panel).toContainText(asset.sha256 ?? "");
    await expect(panel.locator(`a[href="${asset.url}"]`)).toBeVisible();
  }

  await expect(experience).not.toContainText(/\.msi|\.exe|\.deb|\.rpm/i);
});

test("supports the tab keyboard pattern and explicit macOS architecture choice", async ({
  page,
}) => {
  const experience = await openDownloadPage(page);
  const windows = platformTab(experience, "Windows");
  const macos = platformTab(experience, "macOS");
  const linux = platformTab(experience, "Linux");

  await windows.focus();
  await page.keyboard.press("ArrowRight");
  await expect(macos).toBeFocused();
  await expect(macos).toHaveAttribute("aria-selected", "true");
  await expectOnlyPlatformVisible(experience, "macos");

  await page.keyboard.press("End");
  await expect(linux).toBeFocused();
  await expect(linux).toHaveAttribute("aria-selected", "true");
  await expectOnlyPlatformVisible(experience, "linux");

  await page.keyboard.press("Home");
  await expect(windows).toBeFocused();
  await expect(windows).toHaveAttribute("aria-selected", "true");
  await expectOnlyPlatformVisible(experience, "windows");

  await macos.click();
  await expectOnlyPlatformVisible(experience, "macos");
  const macPanel = await controlledPanel(page, macos);
  const variants = macPanel.getByRole("group", { name: /macOS processor/i });
  const appleSilicon = variants.getByRole("button", { name: /Apple Silicon · arm64/i });
  const intel = variants.getByRole("button", { name: /Intel · x86_64/i });
  await expect(variants.getByRole("button")).toHaveCount(2);
  await expect(appleSilicon).toHaveAttribute("aria-pressed", "true");

  const appleAsset = expectedAsset("macos", "arm64");
  const intelAsset = expectedAsset("macos", "x86_64");
  await expectOnlyArtifactVisible(macPanel, "macos-arm64-dmg");
  await expect(macPanel.locator(`a[href="${appleAsset.url}"]`)).toBeVisible();

  await intel.click();
  await expect(intel).toHaveAttribute("aria-pressed", "true");
  await expect(appleSilicon).toHaveAttribute("aria-pressed", "false");
  await expectOnlyArtifactVisible(macPanel, "macos-x86_64-dmg");
  await expect(macPanel).toContainText(intelAsset.filename ?? "");
  await expect(macPanel).toContainText(intelAsset.sha256 ?? "");
  await expect(macPanel.locator(`a[href="${intelAsset.url}"]`)).toBeVisible();
});

test("uses desktop OS detection only for factual platform labeling", async ({
  page,
}, testInfo) => {
  test.skip(
    testInfo.project.name !== "chromium-1440",
    "A deterministic desktop engine is sufficient for platform detection",
  );

  await page.addInitScript(() => {
    Object.defineProperty(window.navigator, "platform", {
      configurable: true,
      get: () => "MacIntel",
    });
    Object.defineProperty(window.navigator, "userAgentData", {
      configurable: true,
      get: () => ({ mobile: false, platform: "macOS" }),
    });
  });

  const experience = await openDownloadPage(page);
  await expectOnlyPlatformVisible(experience, "macos");
  const macos = platformTab(experience, "macOS");
  await expect(macos).toHaveAttribute("aria-selected", "true");
  await expect(experience.getByText("Detected platform", { exact: true })).toHaveCount(1);
  await expect(page.locator("body")).not.toContainText(/recommended/i);

  const macPanel = await controlledPanel(page, macos);
  const variants = macPanel.getByRole("group", { name: /macOS processor/i });
  await expect(variants).not.toContainText(/detected|recommended/i);
  for (const button of await variants.getByRole("button").all()) {
    await expect(button).not.toHaveAccessibleName(/detected|recommended/i);
  }
});

test("removes selector motion for reduced-motion users and never overflows", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  const experience = await openDownloadPage(page);

  const linux = platformTab(experience, "Linux");
  await linux.click();
  const panel = await controlledPanel(page, linux);
  await expect(panel).toBeVisible();

  const motion = await panel.evaluate((element) => {
    const styles = getComputedStyle(element);
    return {
      activeAnimations: element
        .getAnimations({ subtree: true })
        .filter((animation) => animation.playState !== "finished").length,
      opacity: styles.opacity,
      transform: styles.transform,
    };
  });
  expect(motion).toEqual({ activeAnimations: 0, opacity: "1", transform: "none" });

  const dimensions = await page.evaluate(() => ({
    documentWidth: document.documentElement.scrollWidth,
    viewportWidth: document.documentElement.clientWidth,
  }));
  expect(dimensions.documentWidth).toBeLessThanOrEqual(dimensions.viewportWidth);
});

test("keeps the editorial layout relationship across responsive widths", async ({
  page,
}) => {
  const experience = await openDownloadPage(page);
  const copy = experience.locator("[data-download-copy]");
  const selector = experience.locator("[data-download-selector]");
  const [copyBox, selectorBox] = await Promise.all([
    copy.boundingBox(),
    selector.boundingBox(),
  ]);
  expect(copyBox).not.toBeNull();
  expect(selectorBox).not.toBeNull();

  const viewport = page.viewportSize();
  expect(viewport).not.toBeNull();
  if ((viewport?.width ?? 0) > 960) {
    expect((copyBox?.x ?? 0) + (copyBox?.width ?? 0)).toBeLessThanOrEqual(
      (selectorBox?.x ?? 0) + 1,
    );
    const widthRatio = (selectorBox?.width ?? 0) / (copyBox?.width ?? 1);
    expect(widthRatio).toBeGreaterThan(1.35);
    expect(widthRatio).toBeLessThan(1.65);
  } else {
    expect((copyBox?.y ?? 0) + (copyBox?.height ?? 0)).toBeLessThanOrEqual(
      (selectorBox?.y ?? 0) + 1,
    );
  }
});

test("keeps the progressive-enhancement handoff layout-stable", async ({
  page,
}, testInfo) => {
  test.skip(
    testInfo.project.name !== "chromium-1440",
    "One desktop engine is sufficient for the layout-shift regression contract",
  );

  await page.addInitScript(() => {
    const measuredWindow = window as Window & { __downloadLayoutShift?: number };
    measuredWindow.__downloadLayoutShift = 0;
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        const layoutShift = entry as PerformanceEntry & {
          hadRecentInput?: boolean;
          value?: number;
        };
        if (!layoutShift.hadRecentInput) {
          measuredWindow.__downloadLayoutShift =
            (measuredWindow.__downloadLayoutShift ?? 0) +
            (layoutShift.value ?? 0);
        }
      }
    }).observe({ buffered: true, type: "layout-shift" });
  });

  await openDownloadPage(page);
  await page.waitForTimeout(500);
  const cumulativeLayoutShift = await page.evaluate(
    () =>
      (window as Window & { __downloadLayoutShift?: number })
        .__downloadLayoutShift ?? 0,
  );
  expect(cumulativeLayoutShift).toBeLessThan(0.1);
});

test("uses a short opacity-and-transform transition and settles cleanly", async ({
  page,
}, testInfo) => {
  test.skip(
    testInfo.project.name !== "chromium-1440",
    "One desktop engine is sufficient for the authored motion contract",
  );

  await page.emulateMedia({ colorScheme: "light", reducedMotion: "no-preference" });
  const experience = await openDownloadPage(page);
  const linux = platformTab(experience, "Linux");
  const panel = await controlledPanel(page, linux);

  await panel.evaluate((element) => {
    const captureMotion = (event: Event) => {
      if (event.target !== element) return;
      element.removeEventListener("animationstart", captureMotion);
      const animation = element.getAnimations()[0];
      const effect = animation?.effect as KeyframeEffect | null;
      const metadataKeys = new Set([
        "composite",
        "computedOffset",
        "easing",
        "offset",
      ]);
      const properties = effect
        ? [
            ...new Set(
              effect
                .getKeyframes()
                .flatMap((frame) => Object.keys(frame))
                .filter((key) => !metadataKeys.has(key)),
            ),
          ].sort()
        : [];
      element.setAttribute(
        "data-test-motion-capture",
        JSON.stringify({
          duration: Number(effect?.getTiming().duration ?? 0),
          markerObserved: element.hasAttribute("data-entering"),
          properties,
        }),
      );
    };
    element.addEventListener("animationstart", captureMotion);
  });

  await linux.click();
  await expectOnlyPlatformVisible(experience, "linux");
  await expect(panel).toHaveAttribute("data-test-motion-capture", /.+/);
  const motion = JSON.parse(
    (await panel.getAttribute("data-test-motion-capture")) ?? "{}",
  ) as { duration: number; markerObserved: boolean; properties: string[] };
  expect(motion.markerObserved).toBe(true);
  expect(motion.properties).toEqual(["opacity", "transform"]);
  expect(motion.duration).toBeGreaterThanOrEqual(200);
  expect(motion.duration).toBeLessThanOrEqual(300);

  await expect(panel).not.toHaveAttribute("data-entering", "");
  await expect
    .poll(async () =>
      panel.evaluate((element) => ({
        opacity: getComputedStyle(element).opacity,
        transform: getComputedStyle(element).transform,
      })),
    )
    .toEqual({ opacity: "1", transform: "none" });
});

test("keeps all published packages visible without JavaScript", async ({
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
  const experience = await openDownloadPage(page);

  await expect(experience.locator("[data-platform-panel]:visible")).toHaveCount(3);
  await expect(experience.locator("[data-download-artifact-panel]:visible")).toHaveCount(4);
  const platformLinks = experience.locator("[data-platform-tab]");
  await expect(platformLinks).toHaveCount(3);
  for (const link of await platformLinks.all()) {
    await expect(link).not.toHaveAttribute("role", "tab");
    await expect(link).not.toHaveAttribute("tabindex", /.+/);
    await link.focus();
    await expect(link).toBeFocused();
  }
  await expect(experience.locator("[data-copy-checksum]:visible")).toHaveCount(0);
  const visibleAssetLinks = experience.locator("[data-download-asset]:visible");
  const visibleHrefs = await visibleAssetLinks.evaluateAll((links) =>
    links.map((link) => (link as HTMLAnchorElement).href),
  );
  expect([...new Set(visibleHrefs)].sort()).toEqual(
    expectedAssets.map((asset) => asset.url).sort(),
  );
  for (const asset of expectedAssets) {
    await expect(experience.locator(`a[href="${asset.url}"]:visible`)).toHaveCount(1);
  }

  await context.close();
});

test("@accessibility has no serious or critical download-page findings", async ({
  page,
}, testInfo) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  await openDownloadPage(page);

  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .analyze();

  await testInfo.attach("download-page-axe-results.json", {
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
