import { readFileSync } from "node:fs";

import AxeBuilder from "@axe-core/playwright";
import { type Locator, type Page, expect, test } from "@playwright/test";

interface PublishedAsset {
  arch: string;
  available: boolean;
  os: string;
  url: string;
}

type PlatformId = "windows" | "macos" | "linux";
type PlatformName = "Windows" | "macOS" | "Linux";

const platformCases = [
  { arch: "x86_64", id: "windows", name: "Windows" },
  { arch: "arm64", id: "macos", name: "macOS" },
  { arch: "x86_64", id: "linux", name: "Linux" },
] as const satisfies readonly {
  arch: string;
  id: PlatformId;
  name: PlatformName;
}[];

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

async function openLandingDownloadExperience(page: Page) {
  const response = await page.goto("/", { waitUntil: "domcontentloaded" });
  expect(response?.ok()).toBe(true);
  await page.evaluate(async () => {
    await document.fonts.ready;
  });
  const experience = page.locator("[data-download-experience]");
  await expect(experience).toBeVisible();
  return experience;
}

function platformTab(experience: Locator, name: PlatformName) {
  return experience.getByRole("tab", { name: new RegExp(`^${name}\\b`, "i") });
}

async function expectPlatformIcon(tab: Locator, platform: PlatformId) {
  const icon = tab.locator(`svg[data-platform-icon="${platform}"]`);
  await expect(icon).toHaveCount(1);
  await expect(icon).toBeVisible();
  await expect(icon).toHaveAttribute("aria-hidden", "true");
  await expect(icon).toHaveAttribute("focusable", "false");
  return icon;
}

async function expectSelectedNonColorCue(tab: Locator) {
  const cue = await tab.evaluate((element) => {
    const styles = getComputedStyle(element);
    return {
      borderBottomStyle: styles.borderBottomStyle,
      borderBottomWidth: Number.parseFloat(styles.borderBottomWidth),
      boxShadow: styles.boxShadow,
    };
  });
  const hasBorderCue =
    cue.borderBottomStyle !== "none" && cue.borderBottomWidth >= 2;
  const hasInsetShadowCue =
    cue.boxShadow !== "none" && cue.boxShadow.includes("inset");
  expect(
    hasBorderCue || hasInsetShadowCue,
    "The selected platform needs a non-color visual cue",
  ).toBe(true);
}

async function expectInsetKeyboardFocusRing(tab: Locator) {
  const ring = await tab.evaluate((element) => {
    const styles = getComputedStyle(element);
    const insetSpread = Array.from(
      styles.boxShadow.matchAll(
        /0px\s+0px\s+0px\s+([\d.]+)px[^,]*\binset\b/g,
      ),
      (match) => Number.parseFloat(match[1] ?? "0"),
    ).reduce((largest, width) => Math.max(largest, width), 0);
    return {
      insetSpread,
      outlineOffset: Number.parseFloat(styles.outlineOffset),
      outlineStyle: styles.outlineStyle,
      outlineWidth: Number.parseFloat(styles.outlineWidth),
    };
  });
  const hasInsetOutline =
    ring.outlineStyle !== "none" &&
    ring.outlineWidth >= 2 &&
    ring.outlineOffset <= -ring.outlineWidth;
  expect(
    hasInsetOutline || ring.insetSpread >= 2,
    "Keyboard focus needs a component-owned inset ring of at least 2px",
  ).toBe(true);
}

async function controlledPanel(page: Page, tab: Locator) {
  const id = await tab.getAttribute("aria-controls");
  expect(id, "Each platform tab must identify its tabpanel").toBeTruthy();
  return page.locator(`#${id}`);
}

async function expectOnlyPlatformVisible(
  experience: Locator,
  activePlatform: PlatformId,
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

test("keeps the selector on the landing page and restores the detailed download route", async ({
  page,
}) => {
  const experience = await openLandingDownloadExperience(page);
  expect(expectedAssets).toHaveLength(4);
  await expect(page.locator("#downloads[data-download-experience]")).toHaveCount(1);
  await expect(page.locator("h1")).toHaveCount(1);
  await expect(experience.getByRole("heading", { level: 2 })).toHaveText(
    "Download Chaft.",
  );
  await expect(experience.locator("[data-download-grid]")).toHaveCount(0);
  await expect(experience).not.toContainText(/SHA-256|checksum pending/i);

  const tablist = experience.getByRole("tablist", { name: /operating system|platform/i });
  await expect(tablist).toBeVisible();
  await expect(tablist.getByRole("tab")).toHaveCount(3);
  await expect(experience.locator("svg[data-platform-icon]")).toHaveCount(3);

  const renderedHtml = await page.content();
  const attributionNotices =
    renderedHtml.match(/<!--!\s*Font Awesome Free 7\.3\.1[\s\S]*?-->/g) ?? [];
  expect(attributionNotices).toHaveLength(platformCases.length);
  for (const notice of attributionNotices) {
    expect(notice).toContain(
      "https://github.com/FortAwesome/Font-Awesome/tree/14c65a3747d0f3b751f15831fc719236aea8729d",
    );
    expect(notice).toContain(
      "CC BY 4.0 https://creativecommons.org/licenses/by/4.0/",
    );
    expect(notice).toContain("path data are unmodified");
  }

  for (const { arch, id: os, name } of platformCases) {
    const tab = platformTab(experience, name);
    await expect(tab).toHaveAccessibleName(name);
    await expectPlatformIcon(tab, os);
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");
    await expectSelectedNonColorCue(tab);
    await expectOnlyPlatformVisible(experience, os);

    const panel = await controlledPanel(page, tab);
    const asset = expectedAsset(os, arch);
    await expect(panel).toBeVisible();
    await expect(panel.locator(`a[href="${asset.url}"]`)).toBeVisible();
  }

  await expect(experience).not.toContainText(/\.msi|\.exe|\.deb|\.rpm/i);

  const downloadResponse = await page.goto("/download/", {
    waitUntil: "domcontentloaded",
  });
  expect(downloadResponse?.ok()).toBe(true);
  await expect(page.locator("[data-download-experience]")).toHaveCount(0);
  await expect(page.locator("#platforms [data-download-grid]")).toHaveCount(1);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(
    "Choose an unsigned Chaft canary.",
  );
  await expect(page.locator("[data-download-card]")).toHaveCount(expectedAssets.length);
});

test("supports the tab keyboard pattern and explicit macOS architecture choice", async ({
  page,
}) => {
  const experience = await openLandingDownloadExperience(page);
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

  const experience = await openLandingDownloadExperience(page);
  await expectOnlyPlatformVisible(experience, "macos");
  const macos = platformTab(experience, "macOS");
  const windows = platformTab(experience, "Windows");
  await expect(macos).toHaveAttribute("aria-selected", "true");
  await expect(macos).toHaveAccessibleName("macOS");
  await expect(macos.locator("[data-detected-platform]")).toBeVisible();
  await expect(experience.locator("[data-detected-platform]:visible")).toHaveCount(1);
  await expect(page.locator("body")).not.toContainText(/recommended/i);

  const macPanel = await controlledPanel(page, macos);
  const variants = macPanel.getByRole("group", { name: /macOS processor/i });
  await expect(variants).not.toContainText(/detected|recommended/i);
  for (const button of await variants.getByRole("button").all()) {
    await expect(button).not.toHaveAccessibleName(/detected|recommended/i);
  }

  await windows.click();
  await expectOnlyPlatformVisible(experience, "windows");
  await expect(experience.locator("[data-detected-platform]:visible")).toHaveCount(0);

  await macos.click();
  await expectOnlyPlatformVisible(experience, "macos");
  await expect(macos.locator("[data-detected-platform]")).toBeVisible();
  await expect(experience.locator("[data-detected-platform]:visible")).toHaveCount(1);
  await expect(macos).toHaveAccessibleName("macOS");
});

test("keeps hover semantic-neutral and exposes an unclipped keyboard focus ring", async ({
  page,
}, testInfo) => {
  test.skip(
    testInfo.project.name !== "chromium-1440",
    "One hover-capable desktop engine is sufficient for interaction-state styling",
  );

  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  const experience = await openLandingDownloadExperience(page);
  const windows = platformTab(experience, "Windows");
  const macos = platformTab(experience, "macOS");
  const linux = platformTab(experience, "Linux");

  await windows.click();
  await expectOnlyPlatformVisible(experience, "windows");
  await page.mouse.move(0, 0);
  const inactiveStyle = await linux.evaluate((element) => {
    const styles = getComputedStyle(element);
    return {
      backgroundColor: styles.backgroundColor,
      boxShadow: styles.boxShadow,
      color: styles.color,
      transform: styles.transform,
    };
  });

  await linux.hover();
  const hoverStyle = await linux.evaluate((element) => {
    const styles = getComputedStyle(element);
    return {
      backgroundColor: styles.backgroundColor,
      boxShadow: styles.boxShadow,
      color: styles.color,
      transform: styles.transform,
    };
  });
  expect(hoverStyle).not.toEqual(inactiveStyle);
  await expect(linux).toHaveAttribute("aria-selected", "false");
  await expect(windows).toHaveAttribute("aria-selected", "true");
  await expectOnlyPlatformVisible(experience, "windows");

  const detailsLink = experience.getByRole("link", { name: /Details and checksums/i });
  await detailsLink.focus();
  await page.keyboard.press("Tab");
  await expect(windows).toBeFocused();
  await expectInsetKeyboardFocusRing(windows);
  await expectSelectedNonColorCue(windows);

  await page.keyboard.press("ArrowRight");
  await expect(macos).toBeFocused();
  await expectInsetKeyboardFocusRing(macos);
  await expectSelectedNonColorCue(macos);
  await expectOnlyPlatformVisible(experience, "macos");

  await page.keyboard.press("End");
  await expect(linux).toBeFocused();
  await expectInsetKeyboardFocusRing(linux);
  await expectSelectedNonColorCue(linux);
  await expectOnlyPlatformVisible(experience, "linux");
});

test("removes selector motion for reduced-motion users and never overflows", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  const experience = await openLandingDownloadExperience(page);

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
  const experience = await openLandingDownloadExperience(page);
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
  if ((viewport?.width ?? 0) <= 560) {
    for (const { id, name } of platformCases) {
      const tab = platformTab(experience, name);
      const icon = await expectPlatformIcon(tab, id);
      const label = tab.getByText(name, { exact: true });
      const [tabBox, iconBox] = await Promise.all([
        tab.boundingBox(),
        icon.boundingBox(),
      ]);
      await expect(label).toBeVisible();
      expect(tabBox).not.toBeNull();
      expect(iconBox).not.toBeNull();
      expect(tabBox?.height ?? 0).toBeGreaterThanOrEqual(44);
      expect(tabBox?.width ?? 0).toBeGreaterThanOrEqual(44);
      expect(iconBox?.width ?? 0).toBeGreaterThanOrEqual(14);
      expect(iconBox?.width ?? 0).toBeLessThanOrEqual(22);
      expect(iconBox?.height ?? 0).toBeGreaterThanOrEqual(14);
      expect(iconBox?.height ?? 0).toBeLessThanOrEqual(22);
      expect(iconBox?.x ?? 0).toBeGreaterThanOrEqual((tabBox?.x ?? 0) - 0.5);
      expect((iconBox?.x ?? 0) + (iconBox?.width ?? 0)).toBeLessThanOrEqual(
        (tabBox?.x ?? 0) + (tabBox?.width ?? 0) + 0.5,
      );
    }
  }
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

test("keeps platform and variant swaps height-stable", async ({
  page,
}, testInfo) => {
  test.skip(
    testInfo.project.name !== "chromium-1440",
    "One desktop engine is sufficient for the component geometry contract",
  );

  const experience = await openLandingDownloadExperience(page);
  const selector = experience.locator("[data-download-selector]");
  const initialHeight = (await selector.boundingBox())?.height ?? 0;

  for (const name of ["macOS", "Linux", "Windows"] as const) {
    await platformTab(experience, name).click();
    const currentHeight = (await selector.boundingBox())?.height ?? 0;
    expect(Math.abs(currentHeight - initialHeight)).toBeLessThanOrEqual(8);
  }

  await platformTab(experience, "macOS").click();
  const macosPanel = experience.locator('[data-platform-panel="macos"]');
  await macosPanel.getByRole("button", { name: /Intel · x86_64/i }).click();
  const variantHeight = (await selector.boundingBox())?.height ?? 0;
  expect(Math.abs(variantHeight - initialHeight)).toBeLessThanOrEqual(8);
});

test("uses a short opacity-and-transform transition and settles cleanly", async ({
  page,
}, testInfo) => {
  test.skip(
    testInfo.project.name !== "chromium-1440",
    "One desktop engine is sufficient for the authored motion contract",
  );

  await page.emulateMedia({ colorScheme: "light", reducedMotion: "no-preference" });
  const experience = await openLandingDownloadExperience(page);
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
  const experience = await openLandingDownloadExperience(page);

  await expect(experience.locator("[data-platform-panel]:visible")).toHaveCount(3);
  await expect(experience.locator("[data-download-artifact-panel]:visible")).toHaveCount(4);
  const platformLinks = experience.locator("[data-platform-tab]");
  await expect(platformLinks).toHaveCount(3);
  await expect(experience.locator("svg[data-platform-icon]")).toHaveCount(3);
  for (const [index, { id, name }] of platformCases.entries()) {
    const link = platformLinks.nth(index);
    await expect(link).not.toHaveAttribute("role", "tab");
    await expect(link).not.toHaveAttribute("tabindex", /.+/);
    await expect(link).toHaveAccessibleName(name);
    await expectPlatformIcon(link, id);
  }
  await platformLinks.first().focus();
  await expect(platformLinks.first()).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(platformLinks.nth(1)).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(platformLinks.nth(2)).toBeFocused();
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

test("@accessibility has no serious or critical landing-selector findings", async ({
  page,
}, testInfo) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  await openLandingDownloadExperience(page);

  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .analyze();

  await testInfo.attach("landing-download-axe-results.json", {
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
