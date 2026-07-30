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
    expect(renderHeaders("")).toContain(
      "\n/.well-known/chaft-deployment.json\n  Cache-Control: no-store",
    );
    expect(renderHeaders("")).toContain("\n/_astro/*\n");
    expect(renderRedirects("")).toContain("/downloads /download/ 301");
  });

  it("adds the complete indexing policy only for a Preview build", () => {
    expect(renderHeaders("", { isPreview: true })).toContain(
      "\n  X-Robots-Tag: noindex, nofollow, noarchive\n",
    );
    expect(renderHeaders("")).not.toContain("X-Robots-Tag");
  });

  it("prefixes cache rules and redirects for a path deployment", () => {
    const base = deploymentBase("https://example.com/chaft/");
    expect(base).toBe("/chaft");
    expect(renderHeaders(base)).toContain("\n/chaft/_astro/*\n");
    expect(renderHeaders(base)).toContain(
      "\n/chaft/.well-known/chaft-deployment.json\n",
    );
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
