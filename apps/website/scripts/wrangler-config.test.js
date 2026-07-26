import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

const configPath = fileURLToPath(new URL("../wrangler.jsonc", import.meta.url));

describe("Cloudflare Worker configuration", () => {
  it("is route-less, asset-only, and disabled on public preview hosts", () => {
    const config = JSON.parse(readFileSync(configPath, "utf8"));

    expect(config).toEqual({
      $schema: "./node_modules/wrangler/config-schema.json",
      name: "chaft-website",
      compatibility_date: "2026-07-26",
      workers_dev: false,
      preview_urls: false,
      assets: {
        directory: "./dist",
        html_handling: "auto-trailing-slash",
        not_found_handling: "404-page",
        run_worker_first: false,
      },
    });
    expect(config).not.toHaveProperty("main");
    expect(config).not.toHaveProperty("route");
    expect(config).not.toHaveProperty("routes");
    expect(config.assets).not.toHaveProperty("binding");
  });
});
