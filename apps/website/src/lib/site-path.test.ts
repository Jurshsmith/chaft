import { describe, expect, it } from "vitest";

import { joinSiteBase } from "./site-path";

describe("joinSiteBase", () => {
  it("keeps root-hosted paths unchanged", () => {
    expect(joinSiteBase("/", "/download/")).toBe("/download/");
    expect(joinSiteBase("/", "/#why-chaft")).toBe("/#why-chaft");
  });

  it("prefixes paths and fragments for subpath deployments", () => {
    expect(joinSiteBase("/chaft", "/download/")).toBe("/chaft/download/");
    expect(joinSiteBase("/chaft/", "/#why-chaft")).toBe("/chaft/#why-chaft");
  });

  it("rejects paths that are not site-root relative", () => {
    expect(() => joinSiteBase("/", "download/")).toThrow(/must start/);
  });
});
