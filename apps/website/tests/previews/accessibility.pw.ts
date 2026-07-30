import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "@playwright/test";

import { openLandingPage } from "./support";

test("@accessibility has no serious or critical WCAG A/AA findings", async ({
  page,
}, testInfo) => {
  await openLandingPage(page);

  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .analyze();

  await testInfo.attach("axe-results.json", {
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
