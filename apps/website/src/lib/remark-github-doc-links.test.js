import { mkdtempSync, mkdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import remarkGitHubDocLinks, {
  rewritePublicGuideUrl,
} from "./remark-github-doc-links.mjs";

let directory;
let guidesRoot;
let source;

beforeEach(() => {
  directory = mkdtempSync(join(tmpdir(), "chaft-doc-links-"));
  guidesRoot = join(directory, "guides", "public");
  source = join(guidesRoot, "getting-started", "build.md");
  mkdirSync(join(guidesRoot, "getting-started"), { recursive: true });
  mkdirSync(join(guidesRoot, "concepts"), { recursive: true });
  writeFileSync(source, "# Build\n");
  writeFileSync(join(guidesRoot, "index.md"), "# Docs\n");
  writeFileSync(join(guidesRoot, "concepts", "security.md"), "# Key storage\n");
});

afterEach(() => {
  rmSync(directory, { recursive: true, force: true });
});

describe("GitHub-compatible public guide links", () => {
  it("rewrites relative Markdown links for root and path-prefixed sites", () => {
    expect(
      rewritePublicGuideUrl("../concepts/security.md", source, {
        guidesRoot,
        basePath: "/",
      }),
    ).toBe("/docs/concepts/security/");
    expect(
      rewritePublicGuideUrl("../index.md", source, {
        guidesRoot,
        basePath: "/chaft",
      }),
    ).toBe("/chaft/docs/");
  });

  it("preserves heading fragments", () => {
    expect(
      rewritePublicGuideUrl("../concepts/security.md#key-storage", source, {
        guidesRoot,
        basePath: "/chaft/",
      }),
    ).toBe("/chaft/docs/concepts/security/#key-storage");
  });

  it("leaves external, hash-only, and non-Markdown links untouched", () => {
    for (const url of [
      "https://github.com/Jurshsmith/chaft/blob/main/SECURITY.md",
      "mailto:security@example.com",
      "#local-heading",
      "../assets/diagram.png",
    ]) {
      expect(rewritePublicGuideUrl(url, source, { guidesRoot, basePath: "/" })).toBe(
        url,
      );
    }
  });

  it("rejects escapes, missing files, queries, and invalid URL encoding", () => {
    for (const url of [
      "../../private.md",
      "../concepts/missing.md",
      "../concepts/security.md?raw=1",
      "../concepts/%ZZ.md",
      "/concepts/security.md",
    ]) {
      expect(() =>
        rewritePublicGuideUrl(url, source, { guidesRoot, basePath: "/" }),
      ).toThrow();
    }
  });

  it("rejects symlink targets that leave the public guide root", () => {
    const outside = join(directory, "outside.md");
    const link = join(guidesRoot, "concepts", "outside.md");
    writeFileSync(outside, "# Outside\n");
    symlinkSync(outside, link);

    expect(() =>
      rewritePublicGuideUrl("../concepts/outside.md", source, {
        guidesRoot,
        basePath: "/",
      }),
    ).toThrow("not a public guide");
  });

  it("transforms inline and reference definitions in the Markdown tree", () => {
    const tree = {
      type: "root",
      children: [
        { type: "link", url: "../concepts/security.md#key-storage", children: [] },
        { type: "definition", identifier: "security", url: "../concepts/security.md" },
      ],
    };
    const transform = remarkGitHubDocLinks({ guidesRoot, basePath: "/chaft" });

    transform(tree, { path: source });

    expect(tree.children.map(({ url }) => url)).toEqual([
      "/chaft/docs/concepts/security/#key-storage",
      "/chaft/docs/concepts/security/",
    ]);
  });
});
