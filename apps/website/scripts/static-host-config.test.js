import { describe, expect, it } from "vitest";

import {
  deploymentBase,
  renderHeaders,
  renderRedirects,
  withDeploymentBase,
} from "./static-host-config.mjs";

describe("static-host configuration", () => {
  it("keeps provider routes rooted for an origin deployment", () => {
    expect(deploymentBase("https://example.com/")).toBe("");
    expect(renderHeaders("")).toContain("\n/_astro/*\n");
    expect(renderRedirects("")).toContain("/downloads /download/ 301");
  });

  it("prefixes cache rules and redirects for a path deployment", () => {
    const base = deploymentBase("https://example.com/chaft/");
    expect(base).toBe("/chaft");
    expect(renderHeaders(base)).toContain("\n/chaft/_astro/*\n");
    expect(renderHeaders(base)).toContain("\n/chaft/releases/*.json\n");
    expect(renderRedirects(base)).toContain("/chaft/downloads /chaft/download/ 301");
    expect(renderRedirects(base)).toContain("/chaft/source https://github.com/Jurshsmith/chaft 302");
  });

  it("rejects non-root-relative provider paths", () => {
    expect(() => withDeploymentBase("/chaft", "download/")).toThrow(
      /must begin with/,
    );
  });
});
