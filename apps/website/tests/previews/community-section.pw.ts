import { expect, test, type Locator, type Page } from "@playwright/test";

type CommunityGeometry = {
  stage: { left: number; top: number; right: number; bottom: number };
  headingFrame: { centerX: number; centerY: number };
  heading: Array<{ left: number; top: number; right: number; bottom: number }>;
  avatars: Array<{
    allowTitleOverlap: boolean;
    anchorX: number;
    anchorY: number;
    centerX: number;
    centerY: number;
    finalX: number;
    finalY: number;
    profile: string;
    radius: number;
    safeInset: number;
  }>;
};

async function openCommunity(page: Page, reducedMotion: "no-preference" | "reduce") {
  await page.emulateMedia({
    colorScheme: "light",
    reducedMotion,
  });
  const response = await page.goto("/", { waitUntil: "load" });
  expect(response?.ok()).toBe(true);
  await page.evaluate(async () => {
    document.documentElement.style.scrollBehavior = "auto";
    await document.fonts.ready;
  });
  const section = page.locator("[data-community-story]");
  await expect(section).toBeVisible();
  return section;
}

async function setCommunityProgress(section: Locator, progress: number) {
  await section.evaluate((node, nextProgress) => {
    const story = node as HTMLElement;
    const top = window.scrollY + story.getBoundingClientRect().top;
    window.scrollTo(
      0,
      top + nextProgress * (story.offsetHeight - window.innerHeight),
    );
  }, progress);

  await expect.poll(async () => (
    Number(await section.getAttribute("data-community-progress"))
  ), { timeout: 10_000 }).toBeCloseTo(progress, 2);
}

async function readGeometry(section: Locator): Promise<CommunityGeometry> {
  return section.evaluate((node) => {
    const stage = node.querySelector<HTMLElement>("[data-community-stage]");
    const headingFrame = node.querySelector<HTMLElement>(".community-story__heading");
    const heading = node.querySelector<HTMLElement>("h2");
    if (!stage || !headingFrame || !heading) {
      throw new Error("Community scene is incomplete");
    }

    const stageRect = stage.getBoundingClientRect();
    const headingFrameRect = headingFrame.getBoundingClientRect();
    const headingRange = document.createRange();
    const headingRects = Array.from(heading.querySelectorAll("span")).flatMap((line) => {
      headingRange.selectNodeContents(line);
      return Array.from(headingRange.getClientRects(), (rect) => ({
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
      }));
    });
    const avatars = Array.from(
      node.querySelectorAll<HTMLElement>("[data-community-avatar]"),
      (avatar) => {
        const slot = avatar.closest<HTMLElement>("[data-community-slot]");
        if (!slot) throw new Error("Community avatar is missing its final slot");

        const avatarRect = avatar.getBoundingClientRect();
        const slotRect = slot.getBoundingClientRect();
        const transform = getComputedStyle(avatar).transform;
        const matrix = transform === "none" ? new DOMMatrix() : new DOMMatrix(transform);
        const renderedScale = Math.hypot(matrix.a, matrix.b);
        const scatterScale = Number(avatar.dataset.scatterScale ?? 1);
        const centerX = avatarRect.left + avatarRect.width / 2;
        const centerY = avatarRect.top + avatarRect.height / 2;

        return {
          allowTitleOverlap: slot.dataset.titleOverlap === "true",
          anchorX: Number(slot.dataset.anchorX ?? 0.5),
          anchorY: Number(slot.dataset.anchorY ?? 0.5),
          centerX,
          centerY,
          finalX: slotRect.left + slotRect.width / 2,
          finalY: slotRect.top + slotRect.height / 2,
          profile: avatar.dataset.profile ?? "",
          radius: slotRect.width * renderedScale / 2,
          safeInset: slotRect.width / 2 * Math.max(scatterScale, 1) + 10,
        };
      },
    );

    return {
      stage: {
        left: stageRect.left,
        top: stageRect.top,
        right: stageRect.right,
        bottom: stageRect.bottom,
      },
      headingFrame: {
        centerX: headingFrameRect.left + headingFrameRect.width / 2,
        centerY: headingFrameRect.top + headingFrameRect.height / 2,
      },
      heading: headingRects,
      avatars,
    };
  });
}

function expectInsideStage(geometry: CommunityGeometry) {
  for (const avatar of geometry.avatars) {
    expect(avatar.centerX - avatar.radius).toBeGreaterThanOrEqual(
      geometry.stage.left - 1.5,
    );
    expect(avatar.centerX + avatar.radius).toBeLessThanOrEqual(
      geometry.stage.right + 1.5,
    );
    expect(avatar.centerY - avatar.radius).toBeGreaterThanOrEqual(
      geometry.stage.top - 1.5,
    );
    expect(avatar.centerY + avatar.radius).toBeLessThanOrEqual(
      geometry.stage.bottom + 1.5,
    );
  }
}

function expectNoAvatarOverlap(geometry: CommunityGeometry, progress: number) {
  for (let index = 0; index < geometry.avatars.length; index += 1) {
    const avatar = geometry.avatars[index];
    if (!avatar) continue;

    for (let siblingIndex = index + 1; siblingIndex < geometry.avatars.length; siblingIndex += 1) {
      const sibling = geometry.avatars[siblingIndex];
      if (!sibling) continue;

      const distance = Math.hypot(
        avatar.centerX - sibling.centerX,
        avatar.centerY - sibling.centerY,
      );
      const pairLabel = [
        `profiles ${avatar.profile} (${avatar.centerX.toFixed(1)}, ${avatar.centerY.toFixed(1)})`,
        `and ${sibling.profile} (${sibling.centerX.toFixed(1)}, ${sibling.centerY.toFixed(1)})`,
        `overlap at progress ${progress}`,
      ].join(" ");
      expect(
        distance,
        pairLabel,
      ).toBeGreaterThanOrEqual(
        avatar.radius + sibling.radius - 1.5,
      );
    }
  }
}

function circleIntersectsRect(
  avatar: CommunityGeometry["avatars"][number],
  rect: CommunityGeometry["heading"][number],
  padding: number,
) {
  const nearestX = Math.max(
    rect.left - padding,
    Math.min(avatar.centerX, rect.right + padding),
  );
  const nearestY = Math.max(
    rect.top - padding,
    Math.min(avatar.centerY, rect.bottom + padding),
  );
  return Math.hypot(
    avatar.centerX - nearestX,
    avatar.centerY - nearestY,
  ) < avatar.radius;
}

async function sampleMotionGeometry(section: Locator) {
  const progressSamples = Array.from({ length: 21 }, (_, index) => index / 20);
  const geometries: CommunityGeometry[] = [];

  for (const progress of progressSamples) {
    await setCommunityProgress(section, progress);
    const geometry = await readGeometry(section);
    expectInsideStage(geometry);
    expectNoAvatarOverlap(geometry, progress);
    for (const avatar of geometry.avatars) {
      if (avatar.allowTitleOverlap) continue;
      const avatarLabel = [
        `profile ${avatar.profile}`,
        `at (${avatar.centerX.toFixed(1)}, ${avatar.centerY.toFixed(1)})`,
        `r=${avatar.radius.toFixed(1)}`,
      ].join(" ");
      expect(
        geometry.heading.some((line) => circleIntersectsRect(avatar, line, 8)),
        `${avatarLabel} crosses the heading at progress ${progress}`,
      ).toBe(false);
    }
    geometries.push(geometry);
  }

  return { geometries, progressSamples };
}

test("@community keeps the completed community readable with reduced motion", async ({
  page,
}) => {
  const section = await openCommunity(page, "reduce");

  await expect(section).toHaveAttribute("data-community-motion", "reduced");
  await expect(section).not.toHaveAttribute("data-community-layout", "");
  await expect(section).not.toHaveAttribute("data-community-ready", "");
  await expect(section.getByRole("heading", { level: 2 }).locator("span")).toHaveText([
    "Built for the community.",
    "Strengthened by every contribution.",
  ]);

  const avatars = section.locator("[data-community-avatar]");
  await expect(avatars).toHaveCount(10);
  await expect(section.locator(".community-avatar__art use")).toHaveCount(10);
  const hrefs = await section.locator(".community-avatar__art use").evaluateAll((nodes) =>
    nodes.map((node) => node.getAttribute("href")),
  );
  expect(hrefs.every((href) => (
    href?.includes("/assets/community-avatars.svg#community-avatar-")
  ))).toBe(true);
  const spriteUrl = new URL(hrefs[0] ?? "", page.url());
  spriteUrl.hash = "";
  const spriteResponse = await page.request.get(spriteUrl.href);
  expect(spriteResponse.ok()).toBe(true);
  expect(spriteResponse.headers()["content-type"]).toContain("image/svg+xml");

  const finalStyles = await avatars.evaluateAll((nodes) =>
    nodes.map((node) => {
      const styles = getComputedStyle(node);
      return { opacity: styles.opacity, transform: styles.transform };
    }),
  );
  expect(finalStyles).toEqual(
    Array.from({ length: 10 }, () => ({ opacity: "1", transform: "none" })),
  );

  const gridShape = await section.locator("[data-community-grid]").evaluate((grid) => {
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

test("@community follows safe stage anchors and gathers without collisions", async ({
  page,
}) => {
  test.slow();
  const section = await openCommunity(page, "no-preference");
  const viewport = page.viewportSize();
  expect(viewport).not.toBeNull();

  if ((viewport?.width ?? 0) <= 1279 || (viewport?.height ?? 0) <= 767) {
    await expect(section).toHaveAttribute("data-community-motion", "simplified");
    await expect(section).not.toHaveAttribute("data-community-layout", "");
    await expect(section).not.toHaveAttribute("data-community-ready", "");
    const styles = await section.locator("[data-community-avatar]").evaluateAll((nodes) =>
      nodes.map((node) => ({
        opacity: getComputedStyle(node).opacity,
        transform: getComputedStyle(node).transform,
      })),
    );
    expect(styles).toEqual(
      Array.from({ length: 10 }, () => ({ opacity: "1", transform: "none" })),
    );
    return;
  }

  await expect(section).toHaveAttribute("data-community-motion", "scroll");
  await expect(section).toHaveAttribute("data-community-layout", "");
  await expect(section).toHaveAttribute("data-community-ready", "");

  const { geometries, progressSamples } = await sampleMotionGeometry(section);

  const start = geometries[0];
  expect(start).toBeDefined();
  if (!start) return;

  const stageCenterX = (start.stage.left + start.stage.right) / 2;
  const expectedHeadingCenterY = start.stage.top
    + (start.stage.bottom - start.stage.top) * 0.32;
  expect(start.headingFrame.centerX).toBeCloseTo(stageCenterX, 0);
  expect(start.headingFrame.centerY).toBeCloseTo(expectedHeadingCenterY, 0);

  for (const avatar of start.avatars) {
    const expectedX = start.stage.left + Math.min(
      Math.max(
        (start.stage.right - start.stage.left) * avatar.anchorX,
        avatar.safeInset,
      ),
      start.stage.right - start.stage.left - avatar.safeInset,
    );
    const expectedY = start.stage.top + Math.min(
      Math.max(
        (start.stage.bottom - start.stage.top) * avatar.anchorY,
        avatar.safeInset,
      ),
      start.stage.bottom - start.stage.top - avatar.safeInset,
    );
    expect(avatar.centerX).toBeCloseTo(expectedX, 0);
    expect(avatar.centerY).toBeCloseTo(expectedY, 0);
  }

  for (let sampleIndex = 1; sampleIndex < geometries.length; sampleIndex += 1) {
    const previous = geometries[sampleIndex - 1];
    const current = geometries[sampleIndex];
    if (!previous || !current) continue;

    current.avatars.forEach((avatar, avatarIndex) => {
      const prior = previous.avatars[avatarIndex];
      if (!prior) return;
      const previousDistance = Math.hypot(
        prior.centerX - prior.finalX,
        prior.centerY - prior.finalY,
      );
      const currentDistance = Math.hypot(
        avatar.centerX - avatar.finalX,
        avatar.centerY - avatar.finalY,
      );
      expect(
        currentDistance,
        `profile ${avatar.profile} moves away from its final slot between progress ${progressSamples[sampleIndex - 1]} and ${progressSamples[sampleIndex]}`,
      ).toBeLessThanOrEqual(previousDistance + 1.5);
    });
  }

  const end = geometries.at(-1);
  expect(end).toBeDefined();
  if (!end) return;

  end.avatars.forEach((avatar) => {
    expect(avatar.centerX).toBeCloseTo(avatar.finalX, 1);
    expect(avatar.centerY).toBeCloseTo(avatar.finalY, 1);
  });

  const endMotion = await section.locator("[data-community-avatar]").evaluateAll((nodes) =>
    nodes.map((node) => ({
      opacity: (node as HTMLElement).style.getPropertyValue("--community-opacity"),
      transform: (node as HTMLElement).style.getPropertyValue("--community-transform"),
    })),
  );
  expect(endMotion).toEqual(
    Array.from({ length: 10 }, () => ({
      opacity: "1.0000",
      transform: "translate3d(0.00px, 0.00px, 0) rotate(0.00deg) scale(1.0000)",
    })),
  );

  const startMotion = await (async () => {
    await setCommunityProgress(section, 0);
    return section.locator("[data-community-avatar]").evaluateAll((nodes) =>
      nodes.map((node) => ({
        opacity: (node as HTMLElement).style.getPropertyValue("--community-opacity"),
        transform: (node as HTMLElement).style.getPropertyValue("--community-transform"),
      })),
    );
  })();

  expect(startMotion.every((motion) => (
    motion.transform !== "translate3d(0.00px, 0.00px, 0) rotate(0.00deg) scale(1.0000)"
  ))).toBe(true);
});

test("@community switches cleanly across motion viewport boundaries", async ({
  page,
}) => {
  test.skip(test.info().project.name !== "chromium-1440");
  test.slow();

  await page.setViewportSize({ width: 1280, height: 768 });
  const section = await openCommunity(page, "no-preference");
  await expect(section).toHaveAttribute("data-community-motion", "scroll");
  await expect(section).toHaveAttribute("data-community-ready", "");

  await page.setViewportSize({ width: 1279, height: 768 });
  await expect(section).toHaveAttribute("data-community-motion", "simplified");
  await expect(section).not.toHaveAttribute("data-community-ready", "");

  await page.setViewportSize({ width: 1280, height: 768 });
  await expect(section).toHaveAttribute("data-community-motion", "scroll");
  await expect(section).toHaveAttribute("data-community-ready", "");

  await page.setViewportSize({ width: 1280, height: 767 });
  await expect(section).toHaveAttribute("data-community-motion", "simplified");
  await expect(section).not.toHaveAttribute("data-community-ready", "");

  await page.setViewportSize({ width: 1280, height: 768 });
  await expect(section).toHaveAttribute("data-community-motion", "scroll");
  await expect(section).toHaveAttribute("data-community-ready", "");
  await sampleMotionGeometry(section);
});

test("@community remains complete without JavaScript", async ({ browser }) => {
  test.skip(test.info().project.name !== "chromium-1440");

  const context = await browser.newContext({
    javaScriptEnabled: false,
    viewport: { width: 1440, height: 900 },
  });
  const page = await context.newPage();
  const response = await page.goto("/", { waitUntil: "load" });
  expect(response?.ok()).toBe(true);

  const section = page.locator("[data-community-story]");
  await expect(section).toHaveAttribute("data-community-motion", "static");
  await expect(section).not.toHaveAttribute("data-community-layout", "");
  await expect(section).not.toHaveAttribute("data-community-ready", "");
  await expect(section.getByRole("heading", { level: 2 })).toBeVisible();
  await expect(section.locator("[data-community-avatar]")).toHaveCount(10);
  await expect(section.locator(".community-avatar__art use")).toHaveCount(10);

  const styles = await section.locator("[data-community-avatar]").evaluateAll((nodes) =>
    nodes.map((node) => ({
      opacity: getComputedStyle(node).opacity,
      transform: getComputedStyle(node).transform,
    })),
  );
  expect(styles).toEqual(
    Array.from({ length: 10 }, () => ({ opacity: "1", transform: "none" })),
  );

  const overflow = await page.evaluate(() => ({
    documentWidth: document.documentElement.scrollWidth,
    viewportWidth: document.documentElement.clientWidth,
  }));
  expect(overflow.documentWidth).toBeLessThanOrEqual(overflow.viewportWidth);

  await context.close();
});
