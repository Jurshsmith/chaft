import { describe, expect, it } from "vitest";

import {
  docIdFromEntry,
  docNeighbors,
  docRoutePath,
  groupDocsBySection,
  sortPublishedDocs,
  type DocNavigationEntry,
} from "./docs";

function entry(
  id: string,
  section: DocNavigationEntry["data"]["section"],
  order: number,
  draft = false,
): DocNavigationEntry {
  return {
    id,
    data: {
      title: id,
      description: `${id} description`,
      section,
      order,
      audience: "users",
      status: "canary",
      draft,
    },
  };
}

const entries = [
  entry("reference/second", "reference", 20),
  entry("index", "getting-started", 0),
  entry("concepts/draft", "concepts", 1, true),
  entry("getting-started/first", "getting-started", 10),
  entry("reference/first", "reference", 10),
];

describe("public documentation paths", () => {
  it("derives stable ids from POSIX and Windows entry paths", () => {
    expect(docIdFromEntry("concepts/security-model.md")).toBe("concepts/security-model");
    expect(docIdFromEntry(String.raw`reference\cli.md`)).toBe("reference/cli");
  });

  it("rejects absolute, escaping, empty, and non-Markdown entry paths", () => {
    for (const value of ["/index.md", "../index.md", "concepts//page.md", "page.mdx"]) {
      expect(() => docIdFromEntry(value)).toThrow("invalid public guide entry path");
    }
  });

  it("maps ids to clean static routes", () => {
    expect(docRoutePath("index")).toBe("/docs/");
    expect(docRoutePath("concepts/index")).toBe("/docs/concepts/");
    expect(docRoutePath("concepts/security-model")).toBe(
      "/docs/concepts/security-model/",
    );
    expect(() => docRoutePath("../security")).toThrow("invalid public guide id");
  });
});

describe("public documentation navigation", () => {
  it("sorts by section and order while excluding drafts", () => {
    expect(sortPublishedDocs(entries).map(({ id }) => id)).toEqual([
      "index",
      "getting-started/first",
      "reference/first",
      "reference/second",
    ]);
  });

  it("groups sidebar entries without duplicating the docs index", () => {
    expect(
      groupDocsBySection(entries).map((group) => ({
        section: group.section,
        ids: group.entries.map(({ id }) => id),
      })),
    ).toEqual([
      { section: "getting-started", ids: ["getting-started/first"] },
      { section: "reference", ids: ["reference/first", "reference/second"] },
    ]);
  });

  it("returns deterministic previous and next entries", () => {
    expect(docNeighbors(entries, "reference/first")).toEqual({
      previous: expect.objectContaining({ id: "getting-started/first" }),
      next: expect.objectContaining({ id: "reference/second" }),
    });
    expect(docNeighbors(entries, "index")).toEqual({});
    expect(docNeighbors(entries, "missing")).toEqual({});
  });
});
