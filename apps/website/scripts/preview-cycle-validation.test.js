import { describe, expect, it } from "vitest";

import {
  EXPECTED_PREVIEW_SLOTS,
  readPreviewCycle,
  validatePreviewCycle,
} from "./preview-cycle-validation.mjs";

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

describe("Chaft Previews cycle manifest", () => {
  it("accepts the checked-in four-slot foundation", () => {
    expect(validatePreviewCycle(readPreviewCycle())).toEqual([]);
  });

  it("pins each branch to one Worker, domain, and GitHub environment", () => {
    const manifest = readPreviewCycle();

    expect(
      manifest.slots.map(
        ({
          id,
          branch,
          worker,
          domain,
          siteUrl,
          githubEnvironment,
          direction,
        }) => ({
          id,
          branch,
          worker,
          domain,
          siteUrl,
          githubEnvironment,
          direction,
        }),
      ),
    ).toEqual(EXPECTED_PREVIEW_SLOTS);
  });

  it("rejects a branch-to-domain mismatch", () => {
    const manifest = clone(readPreviewCycle());
    manifest.slots[0].domain = "hero-2.chaft.ai";

    expect(validatePreviewCycle(manifest)).toContain(
      'slots[0].domain must equal "hero-1.chaft.ai"',
    );
  });

  it("rejects changes to shared copy or typography", () => {
    const manifest = clone(readPreviewCycle());
    manifest.invariants.headline = "A different headline";
    manifest.invariants.typography.bodyCopy = "Space Grotesk";

    const errors = validatePreviewCycle(manifest);
    expect(errors).toContain(
      'invariants.headline must equal "Team chat that runs on your devices."',
    );
    expect(errors).toContain(
      "typography must keep Chillax on body copy and Space Grotesk on headings and UI",
    );
  });

  it("pins the reviewed Figma file and all four direction prompts", () => {
    const manifest = clone(readPreviewCycle());
    manifest.sourceDesign.fileUrl =
      "https://www.figma.com/design/another-file/Unreviewed";
    manifest.slots[2].direction = "A different direction";

    expect(validatePreviewCycle(manifest)).toEqual(
      expect.arrayContaining([
        "sourceDesign must identify the exact reviewed Figma file and section",
        expect.stringContaining("slots[2].direction must equal"),
      ]),
    );
  });

  it("requires a frozen source revision and Figma frames before activation", () => {
    const manifest = clone(readPreviewCycle());
    manifest.status = "active";

    expect(validatePreviewCycle(manifest)).toEqual(
      expect.arrayContaining([
        "baseRevision is required after the foundation stage",
        "an immutable Figma version and four frame nodes are required after foundation",
      ]),
    );
  });

  it("does not allow a selection outside the exact Preview slots", () => {
    const manifest = clone(readPreviewCycle());
    manifest.selection.status = "selected";
    manifest.selection.selectedSlot = "hero-5";

    expect(validatePreviewCycle(manifest)).toEqual(
      expect.arrayContaining([
        "selection.selectedSlot must identify an exact Preview slot",
      ]),
    );
  });
});
