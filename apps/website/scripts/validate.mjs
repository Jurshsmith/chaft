import { spawn } from "node:child_process";
import { join } from "node:path";

const ROOT_VALIDATION_SITE_URL = "https://website-validation.invalid";
const PATH_VALIDATION_SITE_URL = "https://website-validation.invalid/chaft-validation";
const packageManager = process.env.npm_execpath;

if (!packageManager) {
  throw new Error("pnpm validate must run through the package manager");
}

async function run(script, { siteUrl = ROOT_VALIDATION_SITE_URL } = {}) {
  await new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [packageManager, "run", script], {
      env: {
        ...process.env,
        ASTRO_TELEMETRY_DISABLED: "1",
        SITE_URL: siteUrl,
        WRANGLER_LOG_PATH: join(process.cwd(), ".wrangler", "logs"),
        XDG_CACHE_HOME: join(process.cwd(), ".wrangler", "cache"),
        XDG_CONFIG_HOME: join(process.cwd(), ".wrangler", "config"),
      },
      stdio: "inherit",
    });
    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (signal) {
        reject(new Error(`${script} terminated by ${signal}`));
      } else if (code !== 0) {
        reject(new Error(`${script} exited with status ${code}`));
      } else {
        resolve();
      }
    });
  });
}

await run("validate:docs");
await run("validate:preview-cycle");
await run("check");
await run("test");
await run("build:validation");
await run("validate:social-preview");
await run("validate:docs-build");
await run("build:validation", { siteUrl: PATH_VALIDATION_SITE_URL });
await run("validate:social-preview", { siteUrl: PATH_VALIDATION_SITE_URL });
await run("validate:docs-build", { siteUrl: PATH_VALIDATION_SITE_URL });
await run("validate:wrangler", { siteUrl: PATH_VALIDATION_SITE_URL });
