import { spawnSync } from "node:child_process";
import { rmSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { writeStaticHostConfig } from "./static-host-config.mjs";

const validationBuild = process.argv.includes("--validation");
const configuredSite = process.env.SITE_URL;

if (!configuredSite && !validationBuild) {
  console.error("SITE_URL is required. Set it to the production HTTPS URL before building.");
  process.exit(1);
}

const astroCli = fileURLToPath(new URL("../node_modules/astro/bin/astro.mjs", import.meta.url));
// Astro's optimized-image output names vary with `base`. Clearing the derived
// image cache prevents a root build followed by a subpath build from emitting
// HTML that references a cached filename Astro did not copy into the new dist.
rmSync(fileURLToPath(new URL("../node_modules/.astro", import.meta.url)), {
  force: true,
  recursive: true,
});
const result = spawnSync(process.execPath, [astroCli, "build"], {
  env: {
    ...process.env,
    SITE_URL: configuredSite ?? "https://website-validation.invalid",
  },
  stdio: "inherit",
});

if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);

const distDirectory = fileURLToPath(new URL("../dist", import.meta.url));
writeStaticHostConfig(
  distDirectory,
  configuredSite ?? "https://website-validation.invalid",
);
