import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";

import {
  DocumentationValidationError,
  buildPublishedPager,
  collectDocumentationValidation,
  docIdFromPath,
  docRouteFromId,
  extractHeadings,
  findRepositoryRoot,
  parseFrontMatter,
  validateDocumentation,
  verifyPagerOrder,
} from "./docs-validation.mjs";

const temporaryRoots = [];
const validatorPath = fileURLToPath(new URL("./docs-validation.mjs", import.meta.url));

afterEach(() => {
  for (const root of temporaryRoots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

function write(root, relativePath, contents) {
  const destination = join(root, relativePath);
  mkdirSync(join(destination, ".."), { recursive: true });
  writeFileSync(destination, contents);
}

function guideSource(
  {
    title,
    description = `${title} documentation.`,
    section,
    order,
    audience = "users",
    status = "canary",
    draft = false,
  },
  body,
) {
  return [
    "---",
    `title: ${title}`,
    `description: ${description}`,
    `section: ${section}`,
    `order: ${order}`,
    `audience: ${audience}`,
    `status: ${status}`,
    `draft: ${draft}`,
    "---",
    "",
    body.trim(),
    "",
  ].join("\n");
}

function validReadme(extraLines = []) {
  const lines = [
    "# Fixture repository",
    "",
    "[Public guides](guides/public/index.md)",
    "[Security Policy](SECURITY.md)",
    "[Contributing](CONTRIBUTING.md)",
    "[GitHub Releases](https://github.com/example/fixture/releases)",
    "",
    "`tools/check.sh` is the local validation command.",
    "",
    "```sh",
    "tools/check.sh",
    "make website-validate",
    "```",
    "",
    ...extraLines,
  ];
  while (lines.length < 220) {
    lines.push(`Fixture documentation line ${lines.length + 1}.`);
  }
  return `${lines.join("\n")}\n`;
}

function createValidRepository() {
  const root = mkdtempSync(join(tmpdir(), "chaft-docs-validation-"));
  temporaryRoots.push(root);

  write(
    root,
    "SECURITY.md",
    `
# Security

Read the [contribution policy](CONTRIBUTING.md).

\`\`\`sh
chaft export --passphrase-prompt
chaft export --passphrase-stdin
chaft export --passphrase-file "$passphrase_file"
chaft import --identity-passphrase-prompt
chaft import --identity-passphrase-stdin
chaft import --identity-passphrase-file "$identity_passphrase_file"
\`\`\`
`,
  );
  write(
    root,
    "CONTRIBUTING.md",
    `
# Contributing

Read the [security policy](SECURITY.md).
`,
  );
  write(root, "LICENSE", "Fixture license\n");
  write(root, "Makefile", "website-validate:\n\t@true\n");
  write(root, "tools/check.sh", "#!/bin/sh\nexit 0\n");
  write(root, "README.md", validReadme());

  write(
    root,
    "guides/public/index.md",
    guideSource(
      {
        title: "Public guides",
        section: "getting-started",
        order: 0,
      },
      `
# Public guides

Read the [overview](concepts/overview.md#repeated-heading-1).
The [project site](https://example.com/docs) is external.
`,
    ),
  );
  write(
    root,
    "guides/public/getting-started/draft.md",
    guideSource(
      {
        title: "Draft guide",
        section: "getting-started",
        order: 10,
        draft: true,
      },
      "# Draft guide",
    ),
  );
  write(
    root,
    "guides/public/concepts/overview.md",
    guideSource(
      {
        title: "Overview",
        section: "concepts",
        order: 1,
        audience: "contributors",
        status: "stable",
      },
      `
# Overview

## Repeated heading

First.

## Repeated heading

Second.
`,
    ),
  );
  write(
    root,
    "guides/public/reference/cli.md",
    guideSource(
      {
        title: "CLI",
        section: "reference",
        order: 1,
        audience: "operators",
      },
      `
# CLI

Safe secret input uses indirection:

\`\`\`sh
chaft export --recovery-passphrase-stdin
chaft import --identity-passphrase-file "$passphrase_file"
\`\`\`
`,
    ),
  );

  return root;
}

function issueMessages(root) {
  return collectDocumentationValidation({ repositoryRoot: root }).issues.map(
    (issue) => issue.message,
  );
}

describe("front matter and route primitives", () => {
  it("strictly parses the supported schema and scalar types", () => {
    const parsed = parseFrontMatter(
      [
        "---",
        "title: A guide",
        "navTitle: Short guide",
        'description: "A JSON-quoted description"',
        "section: concepts",
        "order: 12",
        "audience: contributors",
        "status: deprecated",
        "---",
        "",
        "# A guide",
        "",
      ].join("\n"),
    );

    expect(parsed.data).toEqual({
      title: "A guide",
      navTitle: "Short guide",
      description: "A JSON-quoted description",
      section: "concepts",
      order: 12,
      audience: "contributors",
      status: "deprecated",
      draft: false,
    });
  });

  it("rejects unknown, duplicate, missing, enum, and incorrectly typed values", () => {
    const invalid = [
      "---",
      "title: Broken",
      "title: Duplicate",
      "description:",
      "section: unknown",
      'order: "1"',
      "audience: everybody",
      "status: final",
      'draft: "false"',
      "extra: no",
      "---",
      "# Broken",
      "",
    ].join("\n");

    expect(() => parseFrontMatter(invalid)).toThrow(DocumentationValidationError);
    try {
      parseFrontMatter(invalid);
    } catch (error) {
      const messages = error.issues.map((issue) => issue.message).join("\n");
      expect(messages).toContain('duplicate front matter key "title"');
      expect(messages).toContain('unknown front matter key "extra"');
      expect(messages).toContain('front matter "order" must be a non-negative safe integer');
      expect(messages).toContain('front matter "draft" must be the unquoted boolean');
      expect(messages).toContain('front matter "section" must be one of');
    }
  });

  it("derives stable IDs and routes while rejecting unsafe path segments", () => {
    expect(docIdFromPath("index.md")).toBe("index");
    expect(docIdFromPath("concepts/architecture.md")).toBe("concepts/architecture");
    expect(docIdFromPath("concepts/index.md")).toBe("concepts/index");
    expect(docRouteFromId("index")).toBe("/docs/");
    expect(docRouteFromId("concepts/architecture")).toBe("/docs/concepts/architecture/");
    expect(docRouteFromId("concepts/index")).toBe("/docs/concepts/");
    expect(() => docIdFromPath("../private.md")).toThrow(/invalid public guide path/);
    expect(() => docIdFromPath("Concepts/Overview.md")).toThrow(/lowercase URL slug/);
  });

  it("uses GitHub-style duplicate heading slugs", () => {
    const headings = extractHeadings(
      [
        "# Title",
        "## Repeated",
        "## Repeated",
        "## Repeated-1",
        "## A & B",
        "```md",
        "# Not a heading",
        "```",
      ].join("\n"),
    );

    expect(headings.map((heading) => heading.slug)).toEqual([
      "title",
      "repeated",
      "repeated-1",
      "repeated-1-1",
      "a--b",
    ]);
  });
});

describe("public guide collection", () => {
  it("validates a repository and computes draft-free symmetric pager order", () => {
    const root = createValidRepository();
    const result = validateDocumentation({ repositoryRoot: root });

    expect(result.guides).toHaveLength(4);
    expect(result.pages.map((page) => page.id)).toEqual([
      "index",
      "concepts/overview",
      "reference/cli",
    ]);
    expect(result.pages[0].previous).toBeNull();
    expect(result.pages[0].next).toBeNull();
    expect(result.pages[1].previous).toBeNull();
    expect(result.pages[1].next.id).toBe("reference/cli");
    expect(result.pages[2].previous.id).toBe("concepts/overview");
    expect(result.pages[2].next).toBeNull();
  });

  it("finds the repository from a nested website directory", () => {
    const root = createValidRepository();
    const nested = join(root, "apps", "website", "scripts");
    mkdirSync(nested, { recursive: true });

    expect(findRepositoryRoot(nested)).toBe(root);
  });

  it("offers a dependency-free CLI with success and failure exit codes", () => {
    const root = createValidRepository();
    const validRun = spawnSync(process.execPath, [validatorPath, root], {
      encoding: "utf8",
    });
    expect(validRun.status).toBe(0);
    expect(validRun.stdout).toContain("Validated 4 public guide(s), 3 published route(s)");

    write(root, "README.md", "# Invalid\n");
    const invalidRun = spawnSync(process.execPath, [validatorPath, root], {
      encoding: "utf8",
    });
    expect(invalidRun.status).toBe(1);
    expect(invalidRun.stderr).toContain("Public documentation validation failed");
  });

  it("rejects colliding routes and duplicate section positions", () => {
    const root = createValidRepository();
    write(
      root,
      "guides/public/concepts/overview/index.md",
      guideSource(
        {
          title: "Duplicate route",
          section: "concepts",
          order: 2,
        },
        "# Duplicate route",
      ),
    );
    write(
      root,
      "guides/public/concepts/second.md",
      guideSource(
        {
          title: "Duplicate position",
          section: "concepts",
          order: 1,
        },
        "# Duplicate position",
      ),
    );

    const messages = issueMessages(root).join("\n");
    expect(messages).toContain('duplicate guide route "/docs/concepts/overview/"');
    expect(messages).toContain("duplicate section order concepts:1");
  });

  it("requires exactly one level-one heading", () => {
    const root = createValidRepository();
    const guidePath = join(root, "guides/public/index.md");
    writeFileSync(guidePath, `${readFileSync(guidePath, "utf8")}\n# Another title\n`);

    expect(issueMessages(root)).toContain(
      "must contain exactly one level-one heading; found 2",
    );
  });

  it("validates duplicate heading fragments and reports missing fragments", () => {
    const root = createValidRepository();
    const indexPath = join(root, "guides/public/index.md");
    writeFileSync(
      indexPath,
      readFileSync(indexPath, "utf8").replace(
        "concepts/overview.md#repeated-heading-1",
        "concepts/overview.md#repeated-heading-2",
      ),
    );

    expect(issueMessages(root).join("\n")).toContain(
      'heading fragment "#repeated-heading-2" does not exist',
    );
  });

  it("confines local Markdown links to guides/public while allowing external links", () => {
    const root = createValidRepository();
    write(root, "guides/outside.md", "# Outside\n");
    const indexPath = join(root, "guides/public/index.md");
    writeFileSync(
      indexPath,
      readFileSync(indexPath, "utf8").replace(
        "concepts/overview.md#repeated-heading-1",
        "../outside.md",
      ),
    );

    expect(issueMessages(root)).toContain(
      "local Markdown target escapes guides/public: ../outside.md",
    );
  });

  it("rejects private infrastructure references and direct secret arguments", () => {
    const root = createValidRepository();
    const cliPath = join(root, "guides/public/reference/cli.md");
    writeFileSync(
      cliPath,
      `${readFileSync(cliPath, "utf8")}
The private chaft-infra repository contains this procedure.

\`\`\`sh
chaft export --passphrase hunter2
chaft import --identity-passphrase="$IDENTITY_PASSPHRASE"
chaft export --passphrase \\
  hunter2
chaft import --passphrase-path /tmp/passphrase
\`\`\`
`,
    );

    const messages = issueMessages(root).join("\n");
    expect(messages).toContain("references the private infrastructure repository");
    expect(messages).toContain(
      "secret-bearing value must not be passed directly through --passphrase",
    );
    expect(messages).toContain(
      "secret-bearing value must not be passed directly through --identity-passphrase",
    );
    expect(messages).toContain(
      "secret-bearing value must not be passed directly through --passphrase-path",
    );
  });

  it("detects a tampered pager while accepting the computed pager", () => {
    const root = createValidRepository();
    const guides = collectDocumentationValidation({ repositoryRoot: root }).guides;
    const pages = buildPublishedPager(guides);
    expect(verifyPagerOrder(pages)).toEqual([]);

    pages[1].next = null;
    expect(verifyPagerOrder(pages).map((issue) => issue.message)).toContain(
      'invalid next page for "concepts/overview"',
    );

    const reversed = pages.toReversed();
    expect(verifyPagerOrder(reversed).map((issue) => issue.message).join("\n")).toContain(
      "is out of order",
    );

    const duplicated = [...pages, { ...pages[1] }];
    const duplicateMessages = verifyPagerOrder(duplicated)
      .map((issue) => issue.message)
      .join("\n");
    expect(duplicateMessages).toContain('duplicate published page ID "concepts/overview"');
    expect(duplicateMessages).toContain(
      'duplicate published route "/docs/concepts/overview/"',
    );
  });
});

describe("README contract", () => {
  it("requires 200-300 lines and the public entry-point links", () => {
    const root = createValidRepository();
    write(root, "README.md", "# Too short\n");

    const messages = issueMessages(root).join("\n");
    expect(messages).toContain("must contain 200-300 lines; found 1");
    expect(messages).toContain("missing required link to SECURITY.md");
    expect(messages).toContain("missing required link to CONTRIBUTING.md");
    expect(messages).toContain("missing required link to guides/public/index.md");
    expect(messages).toContain("missing required GitHub Releases link");
  });

  it("validates relative links, repository paths, and Make targets", () => {
    const root = createValidRepository();
    write(
      root,
      "README.md",
      validReadme([
        "[Missing file](missing.md)",
        "`tools/missing.sh` should exist.",
        "```sh",
        "make missing-target",
        "```",
      ]),
    );

    const messages = issueMessages(root).join("\n");
    expect(messages).toContain("local link target does not exist: missing.md");
    expect(messages).toContain("referenced repository path does not exist: tools/missing.sh");
    expect(messages).toContain("referenced Make target does not exist: missing-target");
  });
});

describe("policy document contract", () => {
  it("returns evidence for both regular policy files and permits safe secret input modes", () => {
    const root = createValidRepository();
    const result = validateDocumentation({ repositoryRoot: root });

    expect(result.policyDocuments).toHaveLength(2);
    expect(
      result.policyDocuments.map(({ file, regularFile, linkCount }) => ({
        file,
        regularFile,
        linkCount,
      })),
    ).toEqual([
      { file: "SECURITY.md", regularFile: true, linkCount: 1 },
      { file: "CONTRIBUTING.md", regularFile: true, linkCount: 1 },
    ]);
  });

  it("rejects broken links and private infrastructure references in a policy", () => {
    const root = createValidRepository();
    write(
      root,
      "SECURITY.md",
      `
# Security

[Missing policy context](missing-policy-context.md)

Deployment details are in chaft-infra.
`,
    );

    const messages = issueMessages(root).join("\n");
    expect(messages).toContain(
      "local link target does not exist: missing-policy-context.md",
    );
    expect(messages).toContain("references the private infrastructure repository");
  });

  it("rejects direct generic and identity passphrase arguments in a policy", () => {
    const root = createValidRepository();
    write(
      root,
      "CONTRIBUTING.md",
      `
# Contributing

\`\`\`sh
chaft export --passphrase hunter2
chaft import --identity-passphrase="$IDENTITY_PASSPHRASE"
chaft import --identity-passphrase-file "$identity_passphrase_file"
\`\`\`
`,
    );

    const messages = issueMessages(root);
    expect(messages).toContain(
      "secret-bearing value must not be passed directly through --passphrase",
    );
    expect(messages).toContain(
      "secret-bearing value must not be passed directly through --identity-passphrase",
    );
    expect(
      messages.some((message) => message.includes("--identity-passphrase-file")),
    ).toBe(false);
  });

  it("requires SECURITY.md and CONTRIBUTING.md to exist as regular files", () => {
    const missingRoot = createValidRepository();
    rmSync(join(missingRoot, "SECURITY.md"));
    expect(issueMessages(missingRoot)).toContain("required policy document does not exist");

    const directoryRoot = createValidRepository();
    rmSync(join(directoryRoot, "CONTRIBUTING.md"));
    mkdirSync(join(directoryRoot, "CONTRIBUTING.md"));
    expect(issueMessages(directoryRoot)).toContain(
      "required policy document must be a regular file",
    );
  });

  it("rejects policy links whose symlink target escapes the repository", () => {
    const root = createValidRepository();
    const outsideRoot = mkdtempSync(join(tmpdir(), "chaft-docs-policy-outside-"));
    temporaryRoots.push(outsideRoot);
    write(outsideRoot, "outside.md", "# Outside\n");
    symlinkSync(join(outsideRoot, "outside.md"), join(root, "outside-policy.md"));
    write(
      root,
      "SECURITY.md",
      `
# Security

[Outside policy](outside-policy.md)
`,
    );

    expect(issueMessages(root)).toContain(
      "local link resolves outside the repository: outside-policy.md",
    );
  });
});
