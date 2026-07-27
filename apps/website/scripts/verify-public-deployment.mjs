import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";

import { verifyPublicDeployment } from "./public-deployment-verifier.mjs";

function argument(name, fallback) {
  const index = process.argv.indexOf(name);
  if (index === -1) return fallback;
  if (!process.argv[index + 1]) throw new Error(`missing value for ${name}`);
  return process.argv[index + 1];
}

const attempts = Number(argument("--attempts", "1"));
const delayMs = Number(argument("--delay-ms", "5000"));
const output = argument("--output");
const options = {
  alternateSiteUrl: argument("--alternate-site-url"),
  expectedCommit: argument("--commit"),
  expectedReleaseManifestSha256: argument("--release-manifest-sha256"),
  expectedReleaseStatus: argument("--release-status"),
  expectedReleaseTag: argument("--release-tag"),
  expectedReleaseVersion: argument("--release-version"),
  repository: argument("--repository"),
  siteUrl: argument("--site-url"),
};

if (!Number.isInteger(attempts) || attempts < 1 || attempts > 20) {
  throw new Error("--attempts must be an integer from 1 to 20");
}
if (!Number.isInteger(delayMs) || delayMs < 0 || delayMs > 30_000) {
  throw new Error("--delay-ms must be an integer from 0 to 30000");
}

let lastError;
for (let attempt = 1; attempt <= attempts; attempt += 1) {
  try {
    const report = {
      ...(await verifyPublicDeployment(options)),
      attempt,
      maximumAttempts: attempts,
    };
    const serialized = `${JSON.stringify(report, null, 2)}\n`;
    if (output) await writeFile(resolve(output), serialized, "utf8");
    process.stdout.write(serialized);
    process.exit(0);
  } catch (error) {
    lastError = error;
    if (attempt < attempts) {
      process.stderr.write(`verification attempt ${attempt} failed: ${error.message}\n`);
      await new Promise((resolveDelay) => setTimeout(resolveDelay, delayMs));
    }
  }
}

throw lastError;
