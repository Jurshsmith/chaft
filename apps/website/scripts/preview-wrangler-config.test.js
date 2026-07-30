import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { PREVIEW_SLOTS } from "./preview-slot-contract.mjs";

const configPath = fileURLToPath(
  new URL("../wrangler.preview.jsonc", import.meta.url),
);

describe("Chaft Previews Worker configuration", () => {
  it("contains only the four fixed asset-only preview slots", () => {
    const config = JSON.parse(readFileSync(configPath, "utf8"));

    expect(config.$schema).toBe("./node_modules/wrangler/config-schema.json");
    expect(config.name).toBe("chaft-website-preview-template");
    expect(config.compatibility_date).toBe("2026-07-26");
    expect(config.workers_dev).toBe(false);
    expect(config.preview_urls).toBe(false);
    expect(config.assets).toEqual({
      directory: "./dist",
      html_handling: "auto-trailing-slash",
      not_found_handling: "404-page",
      run_worker_first: false,
    });
    expect(config).not.toHaveProperty("main");
    expect(config).not.toHaveProperty("routes");
    expect(Object.keys(config.env).sort()).toEqual(
      PREVIEW_SLOTS.map((row) => row.wranglerEnvironment).sort(),
    );

    for (const row of PREVIEW_SLOTS) {
      expect(config.env[row.wranglerEnvironment]).toEqual({
        name: row.worker,
        routes: [
          {
            pattern: row.domain,
            custom_domain: true,
          },
        ],
      });
    }
  });

  it("cannot name or route the production Worker and hostnames", () => {
    const config = JSON.parse(readFileSync(configPath, "utf8"));
    const encoded = JSON.stringify(config);
    expect(encoded).not.toContain('"name":"chaft-website"');
    expect(encoded).not.toContain('"pattern":"chaft.ai"');
    expect(encoded).not.toContain('"pattern":"www.chaft.ai"');
  });
});
