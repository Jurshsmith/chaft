import { spawn } from "node:child_process";
import { join } from "node:path";

const VALIDATION_SITE_URL = "https://website-validation.invalid";
const packageManager = process.env.npm_execpath;

if (!packageManager) {
  throw new Error("pnpm validate must run through the package manager");
}

async function run(script) {
  await new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [packageManager, "run", script], {
      env: {
        ...process.env,
        ASTRO_TELEMETRY_DISABLED: "1",
        SITE_URL: VALIDATION_SITE_URL,
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

for (const script of ["check", "test", "build:validation", "validate:wrangler"]) {
  await run(script);
}
