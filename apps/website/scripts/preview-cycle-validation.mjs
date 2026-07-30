import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

import { PREVIEW_SLOTS } from "./preview-slot-contract.mjs";

const manifestUrl = new URL(
  "../previews/landing-hero/preview-cycle.json",
  import.meta.url,
);
const EXPECTED_FIGMA_FILE_URL =
  "https://www.figma.com/design/hDvRvY0J6fna6OTWWlC639/Josh-s-Chaft";
const EXPECTED_SOURCE_SECTION_NODE_ID = "164:75";
const EXPECTED_SOURCE_SECTION_URL = `${EXPECTED_FIGMA_FILE_URL}?node-id=164-75`;
const EXPECTED_FIGMA_LAST_MODIFIED_AT = "2026-07-30T11:13:58Z";
const REFERENCE_SNAPSHOT_CANONICALIZATION =
  "chaft-preview-figma-reference-v1";
const EXPECTED_REFERENCE_SNAPSHOT_SHA256 =
  "29188bdcbc245e12efe0af21ac3f58dc023763c6b61e29c82b2d7337bef99de5";
const EXPECTED_REFERENCE_NOTE =
  "Reviewed reference snapshot of the linked Figma section, visual nodes, prompt nodes, and prompt text. The digest binds this repository record; the Figma links themselves remain mutable.";
const EXPECTED_FIGMA_REFERENCES = Object.freeze([
  Object.freeze({ referenceNodeId: "173:98", promptNodeId: "173:100" }),
  Object.freeze({ referenceNodeId: "175:118", promptNodeId: "175:120" }),
  Object.freeze({ referenceNodeId: "175:122", promptNodeId: "175:124" }),
  Object.freeze({ referenceNodeId: "177:131", promptNodeId: "177:133" }),
]);
const EXPECTED_DIRECTIONS = Object.freeze([
  "A glowing earth and communication network with responsive nodes and particle dust.",
  "Particle dust forming the Chaft mark, with responsive motion around the mark.",
  "A circular particle network with communication nodes and lightweight chat activity.",
  "A full-width blue pixel connection field replacing the current side illustration.",
]);

export const EXPECTED_PREVIEW_SLOTS = Object.freeze(
  PREVIEW_SLOTS.map((slot, index) =>
    Object.freeze({
      id: slot.slot,
      branch: slot.branch,
      worker: slot.worker,
      domain: slot.domain,
      siteUrl: slot.siteUrl,
      githubEnvironment: slot.environment,
      figmaReferenceNodeId:
        EXPECTED_FIGMA_REFERENCES[index].referenceNodeId,
      figmaReferenceUrl: figmaNodeUrl(
        EXPECTED_FIGMA_REFERENCES[index].referenceNodeId,
      ),
      figmaPromptNodeId: EXPECTED_FIGMA_REFERENCES[index].promptNodeId,
      figmaPromptUrl: figmaNodeUrl(
        EXPECTED_FIGMA_REFERENCES[index].promptNodeId,
      ),
      direction: EXPECTED_DIRECTIONS[index],
    }),
  ),
);

const EXPECTED_RUNTIME_ENVIRONMENT = Object.freeze({
  deploymentModeName: "CHAFT_DEPLOYMENT_MODE",
  deploymentModeValue: "preview",
  branchName: "CHAFT_PREVIEW_BRANCH",
  siteUrlName: "SITE_URL",
});

const EXPECTED_INVARIANTS = Object.freeze({
  headline: "Team chat that runs on your devices.",
  bodyCopySha256:
    "2db41bdb5926d7968600052a638be5b5c6b5ee614013a8e2bbe7c3202693ac3b",
  primaryAction: "Download Chaft",
  secondaryAction: "Read the docs",
  sourceAction: "Explore the source",
  securityCopy:
    "Unaudited software. Not for sensitive or production communication.",
});

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function figmaNodeUrl(nodeId) {
  return `${EXPECTED_FIGMA_FILE_URL}?node-id=${nodeId.replace(":", "-")}`;
}

function referenceSnapshotPayload(manifest) {
  return {
    fileUrl: manifest.sourceDesign?.fileUrl,
    sourceSectionNodeId: manifest.sourceDesign?.sourceSectionNodeId,
    sourceSectionUrl: manifest.sourceDesign?.sourceSectionUrl,
    observedLastModifiedAt: manifest.sourceDesign?.observedLastModifiedAt,
    slots: Array.isArray(manifest.slots)
      ? manifest.slots.map((slot) => ({
          id: slot?.id,
          referenceNodeId: slot?.figmaReferenceNodeId,
          referenceUrl: slot?.figmaReferenceUrl,
          promptNodeId: slot?.figmaPromptNodeId,
          promptUrl: slot?.figmaPromptUrl,
          prompt: slot?.direction,
        }))
      : [],
  };
}

export function referenceSnapshotSha256(manifest) {
  const bytes = `${REFERENCE_SNAPSHOT_CANONICALIZATION}\n${JSON.stringify(
    referenceSnapshotPayload(manifest),
  )}`;
  return createHash("sha256").update(bytes, "utf8").digest("hex");
}

function compareExactRecord(actual, expected, label, errors) {
  if (!isRecord(actual)) {
    errors.push(`${label} must be an object`);
    return;
  }

  for (const [key, value] of Object.entries(expected)) {
    if (actual[key] !== value) {
      errors.push(`${label}.${key} must equal ${JSON.stringify(value)}`);
    }
  }
}

function compareExactKeys(actual, expected, label, errors) {
  if (!isRecord(actual)) return;
  const actualKeys = Object.keys(actual).sort();
  const expectedKeys = [...expected].sort();
  if (JSON.stringify(actualKeys) !== JSON.stringify(expectedKeys)) {
    errors.push(`${label} must contain only its exact schema v2 fields`);
  }
}

export function validatePreviewCycle(manifest) {
  const errors = [];

  if (!isRecord(manifest)) {
    return ["Preview cycle manifest must be an object"];
  }
  compareExactKeys(
    manifest,
    [
      "$schema",
      "schemaVersion",
      "systemName",
      "name",
      "title",
      "status",
      "baseRevision",
      "sourceDesign",
      "runtimeEnvironment",
      "invariants",
      "governance",
      "slots",
      "selection",
    ],
    "manifest",
    errors,
  );
  if (manifest.schemaVersion !== 2) {
    errors.push("schemaVersion must equal 2");
  }
  if (manifest.systemName !== "Chaft Previews") {
    errors.push('systemName must equal "Chaft Previews"');
  }
  if (manifest.name !== "landing-hero") {
    errors.push('name must equal "landing-hero"');
  }
  if (
    !["foundation", "ready", "active", "review", "selected", "closed"].includes(
      manifest.status,
    )
  ) {
    errors.push("status is unsupported");
  }
  if (
    manifest.baseRevision !== null &&
    !/^[a-f0-9]{40}$/.test(manifest.baseRevision)
  ) {
    errors.push("baseRevision must be null or a full 40-character Git commit");
  }
  if (manifest.status !== "foundation" && manifest.baseRevision === null) {
    errors.push("baseRevision is required after the foundation stage");
  }

  compareExactRecord(
    manifest.runtimeEnvironment,
    EXPECTED_RUNTIME_ENVIRONMENT,
    "runtimeEnvironment",
    errors,
  );
  compareExactRecord(
    manifest.invariants,
    EXPECTED_INVARIANTS,
    "invariants",
    errors,
  );
  compareExactKeys(
    manifest.runtimeEnvironment,
    Object.keys(EXPECTED_RUNTIME_ENVIRONMENT),
    "runtimeEnvironment",
    errors,
  );
  compareExactKeys(
    manifest.invariants,
    [...Object.keys(EXPECTED_INVARIANTS), "typography"],
    "invariants",
    errors,
  );
  compareExactKeys(
    manifest.invariants?.typography,
    ["bodyCopy", "headingsNavigationButtonsAndLabels"],
    "invariants.typography",
    errors,
  );

  if (
    manifest.invariants?.typography?.bodyCopy !== "Chillax" ||
    manifest.invariants?.typography?.headingsNavigationButtonsAndLabels !==
      "Space Grotesk"
  ) {
    errors.push(
      "typography must keep Chillax on body copy and Space Grotesk on headings and UI",
    );
  }

  if (!Array.isArray(manifest.slots) || manifest.slots.length !== 4) {
    errors.push("slots must contain exactly four entries");
  } else {
    const seenValues = new Set();
    manifest.slots.forEach((slot, index) => {
      const expected = EXPECTED_PREVIEW_SLOTS[index];
      compareExactRecord(slot, expected, `slots[${index}]`, errors);
      compareExactKeys(
        slot,
        [...Object.keys(expected), "status"],
        `slots[${index}]`,
        errors,
      );

      for (const key of [
        "id",
        "branch",
        "worker",
        "domain",
        "siteUrl",
        "githubEnvironment",
      ]) {
        const identity = `${key}:${slot?.[key]}`;
        if (seenValues.has(identity)) {
          errors.push(`slots contain duplicate ${key} value ${slot?.[key]}`);
        }
        seenValues.add(identity);
      }

      if (
        !["foundation", "ready", "active", "review", "selected", "closed"].includes(
          slot?.status,
        )
      ) {
        errors.push(`slots[${index}].status is unsupported`);
      }
    });
  }

  if (
    manifest.governance?.componentPath !==
      "cloudflare/website-previews/component.yaml" ||
    manifest.governance?.productionWorker !== "chaft-website" ||
    JSON.stringify(manifest.governance?.productionDomains) !==
      JSON.stringify(["chaft.ai", "www.chaft.ai"]) ||
    manifest.governance?.productionMustRemainUnchanged !== true ||
    manifest.governance?.workersDevEnabled !== false ||
    manifest.governance?.cloudflarePreviewUrlsEnabled !== false
  ) {
    errors.push("governance must preserve the exact production and Preview boundaries");
  }
  compareExactKeys(
    manifest.governance,
    [
      "componentPath",
      "productionWorker",
      "productionDomains",
      "productionMustRemainUnchanged",
      "workersDevEnabled",
      "cloudflarePreviewUrlsEnabled",
    ],
    "governance",
    errors,
  );

  const selection = manifest.selection;
  if (!isRecord(selection)) {
    errors.push("selection must be an object");
  } else {
    compareExactKeys(
      selection,
      [
        "status",
        "selectedSlot",
        "decisionRecord",
        "productionRevision",
      ],
      "selection",
      errors,
    );
    if (
      !["not-started", "reviewing", "selected", "promoted", "closed"].includes(
        selection.status,
      )
    ) {
      errors.push("selection.status is unsupported");
    }
    if (
      selection.selectedSlot !== null &&
      !EXPECTED_PREVIEW_SLOTS.some((slot) => slot.id === selection.selectedSlot)
    ) {
      errors.push("selection.selectedSlot must identify an exact Preview slot");
    }
    if (
      ["selected", "promoted", "closed"].includes(selection.status) &&
      selection.selectedSlot === null
    ) {
      errors.push("selection.selectedSlot is required after selection");
    }
    if (
      selection.productionRevision !== null &&
      !/^[a-f0-9]{40}$/.test(selection.productionRevision)
    ) {
      errors.push(
        "selection.productionRevision must be null or a full 40-character Git commit",
      );
    }
  }

  const sourceDesign = manifest.sourceDesign;
  compareExactKeys(
    sourceDesign,
    [
      "fileUrl",
      "sourceSectionNodeId",
      "sourceSectionUrl",
      "observedLastModifiedAt",
      "referenceSnapshot",
      "note",
    ],
    "sourceDesign",
    errors,
  );
  compareExactKeys(
    sourceDesign?.referenceSnapshot,
    ["canonicalization", "sha256"],
    "sourceDesign.referenceSnapshot",
    errors,
  );
  if (
    sourceDesign?.fileUrl !== EXPECTED_FIGMA_FILE_URL ||
    sourceDesign?.sourceSectionNodeId !== EXPECTED_SOURCE_SECTION_NODE_ID ||
    sourceDesign?.sourceSectionUrl !== EXPECTED_SOURCE_SECTION_URL ||
    sourceDesign?.observedLastModifiedAt !== EXPECTED_FIGMA_LAST_MODIFIED_AT ||
    sourceDesign?.note !== EXPECTED_REFERENCE_NOTE
  ) {
    errors.push(
      "sourceDesign must identify the exact reviewed Figma file, section link, and observed timestamp",
    );
  }
  if (
    !isRecord(sourceDesign?.referenceSnapshot) ||
    sourceDesign.referenceSnapshot.canonicalization !==
      REFERENCE_SNAPSHOT_CANONICALIZATION ||
    sourceDesign.referenceSnapshot.sha256 !==
      EXPECTED_REFERENCE_SNAPSHOT_SHA256 ||
    referenceSnapshotSha256(manifest) !== EXPECTED_REFERENCE_SNAPSHOT_SHA256
  ) {
    errors.push(
      "sourceDesign reference snapshot must match the exact reviewed Figma links, nodes, prompts, timestamp, and digest",
    );
  }
  const figmaNodeIds = Array.isArray(manifest.slots)
    ? manifest.slots.flatMap((slot) => [
        slot?.figmaReferenceNodeId,
        slot?.figmaPromptNodeId,
      ])
    : [];
  if (
    figmaNodeIds.length !== 8 ||
    new Set(figmaNodeIds).size !== 8 ||
    figmaNodeIds.some((value) => !/^\d+:\d+$/.test(value))
  ) {
    errors.push("the four Figma reference and prompt node pairs must be unique");
  }
  if (
    Object.hasOwn(sourceDesign ?? {}, "versionId") ||
    manifest.slots?.some((slot) => Object.hasOwn(slot ?? {}, "figmaFrameNodeId"))
  ) {
    errors.push(
      "legacy Figma versionId and figmaFrameNodeId fields are not accepted",
    );
  }

  return errors;
}

export function readPreviewCycle(url = manifestUrl) {
  return JSON.parse(readFileSync(url, "utf8"));
}

function main() {
  const manifest = readPreviewCycle();
  const errors = validatePreviewCycle(manifest);
  if (errors.length > 0) {
    for (const error of errors) {
      console.error(`- ${error}`);
    }
    process.exitCode = 1;
    return;
  }
  console.log(
    `Validated ${manifest.systemName} ${manifest.title}: ${manifest.slots.length} exact Preview slots.`,
  );
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  main();
}
