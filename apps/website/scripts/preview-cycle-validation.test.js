import { describe, expect, it } from "vitest";

import {
  EXPECTED_PREVIEW_SLOTS,
  readPreviewCycle,
  referenceSnapshotSha256,
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
          figmaReferenceNodeId,
          figmaReferenceUrl,
          figmaPromptNodeId,
          figmaPromptUrl,
          direction,
        }) => ({
          id,
          branch,
          worker,
          domain,
          siteUrl,
          githubEnvironment,
          figmaReferenceNodeId,
          figmaReferenceUrl,
          figmaPromptNodeId,
          figmaPromptUrl,
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

  it("pins the reviewed Figma links, node pairs, prompts, timestamp, and digest", () => {
    const checkedIn = readPreviewCycle();
    expect(referenceSnapshotSha256(checkedIn)).toBe(
      checkedIn.sourceDesign.referenceSnapshot.sha256,
    );

    const manifest = clone(readPreviewCycle());
    manifest.sourceDesign.fileUrl =
      "https://www.figma.com/design/another-file/Unreviewed";
    manifest.sourceDesign.observedLastModifiedAt = "2026-07-30T11:14:00Z";
    manifest.slots[0].figmaReferenceUrl =
      "https://www.figma.com/design/hDvRvY0J6fna6OTWWlC639/Josh-s-Chaft?node-id=173-99";
    manifest.slots[2].direction = "A different direction";

    expect(validatePreviewCycle(manifest)).toEqual(
      expect.arrayContaining([
        "sourceDesign must identify the exact reviewed Figma file, section link, and observed timestamp",
        expect.stringContaining("slots[0].figmaReferenceUrl must equal"),
        expect.stringContaining("slots[2].direction must equal"),
        "sourceDesign reference snapshot must match the exact reviewed Figma links, nodes, prompts, timestamp, and digest",
      ]),
    );
  });

  it("requires a frozen source revision before activation", () => {
    const manifest = clone(readPreviewCycle());
    manifest.status = "active";

    expect(validatePreviewCycle(manifest)).toContain(
      "baseRevision is required after the foundation stage",
    );
  });

  it("rejects duplicate Figma node identities and a stale digest", () => {
    const manifest = clone(readPreviewCycle());
    manifest.slots[1].figmaPromptNodeId =
      manifest.slots[0].figmaReferenceNodeId;

    expect(validatePreviewCycle(manifest)).toEqual(
      expect.arrayContaining([
        expect.stringContaining("slots[1].figmaPromptNodeId must equal"),
        "the four Figma reference and prompt node pairs must be unique",
        "sourceDesign reference snapshot must match the exact reviewed Figma links, nodes, prompts, timestamp, and digest",
      ]),
    );
  });

  it("rejects legacy mutable-version placeholder fields", () => {
    const manifest = clone(readPreviewCycle());
    manifest.sourceDesign.versionId = "unverified-version";
    manifest.slots[0].figmaFrameNodeId = "173:98";

    expect(validatePreviewCycle(manifest)).toContain(
      "legacy Figma versionId and figmaFrameNodeId fields are not accepted",
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
