import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { mountStaticOutput } from "./static-output-mount.mjs";

let fixtureRoot;
let distDirectory;

function write(relativePath, contents = relativePath) {
  const filePath = join(distDirectory, relativePath);
  mkdirSync(dirname(filePath), { recursive: true });
  writeFileSync(filePath, contents);
}

function read(relativePath) {
  return readFileSync(join(distDirectory, relativePath), "utf8");
}

function stagingDirectories() {
  const prefix = `.${basename(distDirectory)}-static-output-mount-`;
  return readdirSync(dirname(distDirectory)).filter((name) => name.startsWith(prefix));
}

beforeEach(() => {
  fixtureRoot = mkdtempSync(join(tmpdir(), "chaft-static-output-mount-"));
  distDirectory = join(fixtureRoot, "dist");
  mkdirSync(distDirectory);
  write("index.html", "home");
  write("docs/index.html", "docs");
  write(".metadata/empty", "");
  write(".nojekyll", "");
  write("_headers", "generated headers");
  write("_redirects", "generated redirects");
});

afterEach(() => {
  rmSync(fixtureRoot, { force: true, recursive: true });
});

describe("mountStaticOutput", () => {
  it("is a root-deployment no-op", () => {
    const result = mountStaticOutput({
      distDirectory,
      siteUrl: "https://example.com/",
    });

    expect(result).toMatchObject({
      basePath: "/",
      mountDirectory: distDirectory,
      movedEntries: [],
    });
    expect(read("index.html")).toBe("home");
    expect(read("docs/index.html")).toBe("docs");
    expect(stagingDirectories()).toEqual([]);
  });

  it("mounts every generated entry under a single-segment base", () => {
    const result = mountStaticOutput({
      distDirectory,
      siteUrl: "https://example.com/chaft/",
    });

    expect(result.basePath).toBe("/chaft");
    expect(result.mountDirectory).toBe(join(distDirectory, "chaft"));
    expect(readdirSync(distDirectory).sort()).toEqual([
      "_headers",
      "_redirects",
      "chaft",
    ]);
    expect(read("chaft/index.html")).toBe("home");
    expect(read("chaft/docs/index.html")).toBe("docs");
    expect(existsSync(join(distDirectory, "chaft", ".metadata", "empty"))).toBe(true);
    expect(existsSync(join(distDirectory, "chaft", ".nojekyll"))).toBe(true);
    expect(read("_headers")).toBe("generated headers");
    expect(read("_redirects")).toBe("generated redirects");
    expect(existsSync(join(distDirectory, "chaft", "_headers"))).toBe(false);
    expect(existsSync(join(distDirectory, "chaft", "_redirects"))).toBe(false);
  });

  it("mounts output under a multi-segment base", () => {
    mountStaticOutput({
      distDirectory,
      siteUrl: "https://example.com/previews/chaft",
    });

    expect(readdirSync(distDirectory).sort()).toEqual([
      "_headers",
      "_redirects",
      "previews",
    ]);
    expect(read("previews/chaft/index.html")).toBe("home");
    expect(read("previews/chaft/docs/index.html")).toBe("docs");
  });

  it("stages safely when the base collides with an existing output directory", () => {
    mountStaticOutput({
      distDirectory,
      siteUrl: "https://example.com/docs",
    });

    expect(read("docs/index.html")).toBe("home");
    expect(read("docs/docs/index.html")).toBe("docs");
    expect(existsSync(join(distDirectory, "index.html"))).toBe(false);
  });

  it.each([
    ["not a URL", "not-a-url"],
    ["missing authority separators", "https:example.com/chaft"],
    ["non-HTTPS", "http://example.com/chaft"],
    ["credentials", "https://user@example.com/chaft"],
    ["query", "https://example.com/chaft?preview=true"],
    ["fragment", "https://example.com/chaft#preview"],
    ["literal traversal", "https://example.com/chaft/../admin"],
    ["encoded traversal", "https://example.com/chaft/%2e%2e/admin"],
    ["encoded separator", "https://example.com/chaft%2fadmin"],
    ["encoded space", "https://example.com/chaft%20preview"],
    ["non-portable Unicode", "https://example.com/chaƒt"],
    ["backslash", "https://example.com/chaft\\admin"],
    ["reserved control file", "https://example.com/_headers/preview"],
  ])("rejects an invalid SITE_URL with %s", (_label, siteUrl) => {
    expect(() => mountStaticOutput({ distDirectory, siteUrl })).toThrow(
      /SITE_URL|Cloudflare control file/,
    );
    expect(read("index.html")).toBe("home");
    expect(stagingDirectories()).toEqual([]);
  });

  it("rejects symbolic links without changing output", () => {
    symlinkSync("index.html", join(distDirectory, "linked-index.html"));

    expect(() =>
      mountStaticOutput({
        distDirectory,
        siteUrl: "https://example.com/chaft",
      }),
    ).toThrow(/must not contain symbolic links/);
    expect(lstatSync(join(distDirectory, "linked-index.html")).isSymbolicLink()).toBe(true);
    expect(read("index.html")).toBe("home");
    expect(stagingDirectories()).toEqual([]);
  });

  it("removes same-filesystem staging after a successful mount", () => {
    mountStaticOutput({
      distDirectory,
      siteUrl: "https://example.com/docs/v1",
    });

    expect(stagingDirectories()).toEqual([]);
    expect(read("docs/v1/index.html")).toBe("home");
  });

  it("restores output and removes staging when mounting fails", () => {
    const oversizedSegment = "a".repeat(300);

    expect(() =>
      mountStaticOutput({
        distDirectory,
        siteUrl: `https://example.com/${oversizedSegment}`,
      }),
    ).toThrow();

    expect(stagingDirectories()).toEqual([]);
    expect(read("index.html")).toBe("home");
    expect(read("docs/index.html")).toBe("docs");
    expect(read("_headers")).toBe("generated headers");
    expect(read("_redirects")).toBe("generated redirects");
  });
});
