import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

import { PREVIEW_SLOTS } from "./preview-slot-contract.mjs";

const manifestUrl = new URL(
  "../previews/landing-hero/preview-cycle.json",
  import.meta.url,
);
const EXPECTED_FIGMA_FILE_URL =
  "https://www.figma.com/design/hDvRvY0J6fna6OTWWlC639/Josh-s-Chaft";
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

export function validatePreviewCycle(manifest) {
  const errors = [];

  if (!isRecord(manifest)) {
    return ["Preview cycle manifest must be an object"];
  }
  if (manifest.schemaVersion !== 1) {
    errors.push("schemaVersion must equal 1");
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
      if (
        slot?.figmaFrameNodeId !== null &&
        !/^\d+:\d+$/.test(slot.figmaFrameNodeId)
      ) {
        errors.push(`slots[${index}].figmaFrameNodeId is malformed`);
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

  const selection = manifest.selection;
  if (!isRecord(selection)) {
    errors.push("selection must be an object");
  } else {
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

  if (
    manifest.sourceDesign?.sourceSectionNodeId !== "164:75" ||
    manifest.sourceDesign?.fileUrl !== EXPECTED_FIGMA_FILE_URL
  ) {
    errors.push("sourceDesign must identify the exact reviewed Figma file and section");
  }
  if (
    manifest.status !== "foundation" &&
    (manifest.sourceDesign?.versionId === null ||
      manifest.slots?.some((slot) => slot.figmaFrameNodeId === null))
  ) {
    errors.push(
      "an immutable Figma version and four frame nodes are required after foundation",
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
