import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
  DocsBuildValidationError,
  deriveSiteLocation,
  loadExpectedDocuments,
  parseCliArguments,
  validateDocsBuild,
} from "./docs-build-validation.mjs";

const DOCUMENTS = [
  {
    relativePath: "index.md",
    route: "/docs/",
    title: "Documentation",
    description: "Start with the public documentation.",
    section: "getting-started",
    order: 0,
    draft: false,
  },
  {
    relativePath: "concepts/architecture.md",
    route: "/docs/concepts/architecture/",
    title: "Architecture",
    description: "Understand the current system shape.",
    section: "concepts",
    order: 1,
    draft: false,
  },
  {
    relativePath: "getting-started/install.md",
    route: "/docs/getting-started/install/",
    title: "Install Chaft",
    description: "Install Chaft on a supported platform.",
    section: "getting-started",
    order: 10,
    draft: false,
  },
  {
    relativePath: "concepts/security-model.md",
    route: "/docs/concepts/security-model/",
    title: "Security model",
    description: "Understand Chaft's security boundaries.",
    section: "concepts",
    order: 2,
    draft: false,
  },
  {
    relativePath: "concepts/unreleased.md",
    route: "/docs/concepts/unreleased/",
    title: "Unreleased design",
    description: "A draft that must not be published.",
    section: "concepts",
    order: 3,
    draft: true,
  },
];

const SECTION_ORDER = ["getting-started", "concepts", "development", "reference"];

let fixtureRoot;
let guidesDirectory;
let distDirectory;
let activeSiteUrl;

function ensureParent(filePath) {
  mkdirSync(dirname(filePath), { recursive: true });
}

function write(filePath, contents) {
  ensureParent(filePath);
  writeFileSync(filePath, contents);
}

function guideSource(document) {
  return `---
title: ${document.title}
description: ${document.description}
section: ${document.section}
order: ${document.order}
audience: users
status: preview
draft: ${document.draft}
---

# ${document.title}
`;
}

function htmlEscape(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function renderPage(document, siteUrl, options = {}) {
  const {
    canonical,
    description = document.description,
    h1 = document.title,
    pageTitle = `${document.title} · Chaft`,
    sidebarHref,
    contentHref,
    imageSrc,
    auxiliaryHref,
    previousHref,
    nextHref,
    previousDirection = "previous",
    nextDirection = "next",
  } = options;
  const site = deriveSiteLocation(siteUrl);
  const defaultLink = site.sitePath("/docs/concepts/architecture/");
  const defaultImage = site.sitePath("/_astro/docs.png");
  const orderedGuides = DOCUMENTS.filter(
    (entry) => !entry.draft && entry.route !== "/docs/",
  ).toSorted(
    (left, right) =>
      SECTION_ORDER.indexOf(left.section) - SECTION_ORDER.indexOf(right.section) ||
      left.order - right.order ||
      left.title.localeCompare(right.title) ||
      left.route.localeCompare(right.route),
  );
  const guideIndex = orderedGuides.findIndex((entry) => entry.route === document.route);
  const defaultPrevious =
    guideIndex > 0 ? site.sitePath(orderedGuides[guideIndex - 1].route) : null;
  const defaultNext =
    guideIndex >= 0 && guideIndex < orderedGuides.length - 1
      ? site.sitePath(orderedGuides[guideIndex + 1].route)
      : null;
  const resolvedPreviousHref = Object.hasOwn(options, "previousHref")
    ? previousHref
    : defaultPrevious;
  const resolvedNextHref = Object.hasOwn(options, "nextHref")
    ? nextHref
    : defaultNext;
  const pager =
    resolvedPreviousHref !== null || resolvedNextHref !== null
      ? `<nav class="docs-pager" aria-label="Documentation pages">${
          resolvedPreviousHref === null
            ? '<span aria-hidden="true"></span>'
            : `<a class="docs-pager__link docs-pager__link--${previousDirection}" href="${htmlEscape(resolvedPreviousHref)}">Previous</a>`
        }${
          resolvedNextHref === null
            ? ""
            : `<a class="docs-pager__link docs-pager__link--${nextDirection}" href="${htmlEscape(resolvedNextHref)}">Next</a>`
        }</nav>`
      : "";
  return `<!doctype html>
<html lang="en">
  <head>
    <meta name="description" content="${htmlEscape(description)}">
    <link rel="canonical" href="${htmlEscape(canonical ?? site.canonical(document.route))}">
    <title>${htmlEscape(pageTitle)}</title>
  </head>
  <body>
    <aside class="docs-sidebar"><a href="${htmlEscape(sidebarHref ?? defaultLink)}">Sidebar</a></aside>
    <article>
      <h1>${htmlEscape(h1)}</h1>
      <img src="${htmlEscape(imageSrc ?? defaultImage)}" alt="">
      <p><a href="${htmlEscape(contentHref ?? defaultLink)}">Related guide</a></p>
      <a href="${htmlEscape(auxiliaryHref ?? "#details")}">Details</a>
      ${pager}
    </article>
  </body>
</html>
`;
}

function siteOutputDirectory(siteUrl = activeSiteUrl) {
  const site = deriveSiteLocation(siteUrl);
  return site.basePath === "/"
    ? distDirectory
    : join(distDirectory, ...site.basePath.slice(1).split("/"));
}

function outputPath(route, siteUrl = activeSiteUrl) {
  return join(
    siteOutputDirectory(siteUrl),
    ...route.split("/").filter(Boolean),
    "index.html",
  );
}

function writeSitemaps(siteUrl, includedDocuments = DOCUMENTS.filter((document) => !document.draft)) {
  const site = deriveSiteLocation(siteUrl);
  const outputRoot = siteOutputDirectory(siteUrl);
  write(
    join(outputRoot, "sitemap-index.xml"),
    `<?xml version="1.0"?><sitemapindex><sitemap><loc>${site.canonical(
      "/sitemap-0.xml",
    )}</loc></sitemap></sitemapindex>`,
  );
  write(
    join(outputRoot, "sitemap-0.xml"),
    `<?xml version="1.0"?><urlset>${includedDocuments
      .map((document) => `<url><loc>${site.canonical(document.route)}</loc></url>`)
      .join("")}<url><loc>${site.canonical("/")}</loc></url></urlset>`,
  );
}

function writeValidBuild(siteUrl) {
  activeSiteUrl = siteUrl;
  for (const document of DOCUMENTS.filter((entry) => !entry.draft)) {
    write(outputPath(document.route), renderPage(document, siteUrl));
  }
  writeSitemaps(siteUrl);
  if (deriveSiteLocation(siteUrl).basePath !== "/") {
    write(join(distDirectory, "_headers"), "/*\n  X-Frame-Options: DENY\n");
    write(join(distDirectory, "_redirects"), "");
  }
}

function validate(siteUrl, options = {}) {
  return validateDocsBuild({
    siteUrl,
    distDirectory,
    guidesDirectory,
    ...options,
  });
}

beforeEach(() => {
  fixtureRoot = mkdtempSync(join(tmpdir(), "chaft-docs-build-validation-"));
  guidesDirectory = join(fixtureRoot, "guides", "public");
  distDirectory = join(fixtureRoot, "dist");
  activeSiteUrl = "https://docs.example/";
  for (const document of DOCUMENTS) {
    write(join(guidesDirectory, document.relativePath), guideSource(document));
  }
  mkdirSync(distDirectory, { recursive: true });
});

afterEach(() => {
  rmSync(fixtureRoot, { recursive: true, force: true });
});

describe("validateDocsBuild", () => {
  it.each([
    ["root", "https://docs.example/"],
    ["path-prefixed", "https://docs.example/chaft"],
    ["multi-segment path", "https://docs.example/previews/chaft"],
  ])("accepts a complete %s deployment", (_label, siteUrl) => {
    writeValidBuild(siteUrl);

    const result = validate(siteUrl);

    expect(result.basePath).toBe(new URL(siteUrl).pathname.replace(/\/+$/, "") || "/");
    expect(result.siteOutputDirectory).toBe(siteOutputDirectory(siteUrl));
    expect(result.publishedRoutes).toEqual([
      "/docs/",
      "/docs/concepts/architecture/",
      "/docs/concepts/security-model/",
      "/docs/getting-started/install/",
    ]);
    expect(result.draftRoutes).toEqual(["/docs/concepts/unreleased/"]);
  });

  it("accepts an expected-routes manifest instead of reading guide sources", () => {
    const siteUrl = "https://docs.example/";
    writeValidBuild(siteUrl);
    const manifestPath = join(fixtureRoot, "expected-docs.json");
    write(
      manifestPath,
      `${JSON.stringify({
        documents: DOCUMENTS.map(
          ({ route, title, description, section, order, draft }) => ({
            route,
            title,
            description,
            section,
            order,
            draft,
          }),
        ),
      })}\n`,
    );

    const result = validateDocsBuild({
      siteUrl,
      distDirectory,
      manifestPath,
    });

    expect(result.publishedRoutes).toHaveLength(4);
  });

  it("defaults an omitted draft flag to false and derives a nested index route", () => {
    write(
      join(guidesDirectory, "reference", "index.md"),
      `---
title: Reference
description: Look up Chaft behavior.
section: reference
order: 5
---

# Reference
`,
    );

    const documents = loadExpectedDocuments({ guidesDirectory });

    expect(documents.find((document) => document.title === "Reference")).toMatchObject({
      route: "/docs/reference/",
      section: "reference",
      order: 5,
      draft: false,
    });
  });

  it("requires manifests to provide explicit section and order metadata", () => {
    const { order: _order, ...withoutOrder } = DOCUMENTS[0];

    expect(() =>
      loadExpectedDocuments({
        expectedDocuments: [withoutOrder],
      }),
    ).toThrow(/order must be a non-negative safe integer/);
  });

  it("rejects a missing published route", () => {
    const siteUrl = "https://docs.example/";
    writeValidBuild(siteUrl);
    rmSync(outputPath("/docs/concepts/architecture/"));

    expect(() => validate(siteUrl)).toThrow(/published documentation route is missing/);
  });

  it("rejects a built draft route", () => {
    const siteUrl = "https://docs.example/";
    writeValidBuild(siteUrl);
    const draft = DOCUMENTS.find((document) => document.draft);
    write(outputPath(draft.route), renderPage(draft, siteUrl));

    expect(() => validate(siteUrl)).toThrow(/draft documentation route/);
  });

  it.each([
    [
      "page title",
      { pageTitle: "Wrong · Chaft" },
      /page title must be "Architecture · Chaft"/,
    ],
    ["description", { description: "Wrong description" }, /description meta content/],
    ["H1", { h1: "Wrong heading" }, /H1 must be "Architecture"/],
  ])("rejects incorrect %s metadata", (_label, overrides, expectedError) => {
    const siteUrl = "https://docs.example/";
    writeValidBuild(siteUrl);
    const architecture = DOCUMENTS[1];
    write(outputPath(architecture.route), renderPage(architecture, siteUrl, overrides));

    expect(() => validate(siteUrl)).toThrow(expectedError);
  });

  it.each([
    ["page title", /<title>[\s\S]*?<\/title>/i, /expected exactly one <title>, found 0/],
    [
      "description",
      /<meta name="description"[^>]*>/i,
      /expected exactly one description meta tag, found 0/,
    ],
    ["H1", /<h1>[\s\S]*?<\/h1>/i, /expected exactly one <h1>, found 0/],
  ])("rejects an absent %s", (_label, pattern, expectedError) => {
    const siteUrl = "https://docs.example/";
    writeValidBuild(siteUrl);
    const architecture = DOCUMENTS[1];
    const pagePath = outputPath(architecture.route);
    write(pagePath, readFileSync(pagePath, "utf8").replace(pattern, ""));

    expect(() => validate(siteUrl)).toThrow(expectedError);
  });

  it("rejects root-only content, pager, and sidebar links under a path base", () => {
    const siteUrl = "https://docs.example/chaft";
    writeValidBuild(siteUrl);
    const architecture = DOCUMENTS[1];
    write(
      outputPath(architecture.route),
      renderPage(architecture, siteUrl, {
        contentHref: "/",
        previousHref: "/download/",
        sidebarHref: "https://docs.example/docs/",
      }),
    );

    try {
      validate(siteUrl);
      expect.unreachable("validation should reject root-only links");
    } catch (error) {
      expect(error).toBeInstanceOf(DocsBuildValidationError);
      expect(
        error.issues.filter((entry) => entry.message.includes("root-only path")),
      ).toHaveLength(3);
    }
  });

  it("rejects a root-only asset src under a path base", () => {
    const siteUrl = "https://docs.example/chaft";
    writeValidBuild(siteUrl);
    const architecture = DOCUMENTS[1];
    write(
      outputPath(architecture.route),
      renderPage(architecture, siteUrl, {
        imageSrc: "/_astro/docs.png",
      }),
    );

    expect(() => validate(siteUrl)).toThrow(
      /internal src leaks root-only path: \/_astro\/docs\.png/,
    );
  });

  it("rejects path-prefixed HTML left at the Cloudflare asset root", () => {
    const siteUrl = "https://docs.example/chaft";
    activeSiteUrl = "https://docs.example/";
    for (const document of DOCUMENTS.filter((entry) => !entry.draft)) {
      write(outputPath(document.route), renderPage(document, siteUrl));
    }
    writeSitemaps("https://docs.example/");
    write(join(distDirectory, "_headers"), "/*\n  X-Frame-Options: DENY\n");
    write(join(distDirectory, "_redirects"), "");

    expect(() => validate(siteUrl)).toThrow(
      /static output is not mounted exclusively beneath \/chaft|physical SITE_URL mount is missing/,
    );
  });

  it("rejects nested copies of Cloudflare control files", () => {
    const siteUrl = "https://docs.example/chaft";
    writeValidBuild(siteUrl);
    write(join(siteOutputDirectory(siteUrl), "_headers"), "nested\n");

    expect(() => validate(siteUrl)).toThrow(
      /Cloudflare control file must stay at the asset root/,
    );
  });

  it("allows external, schemed, fragment, and relative references under a path base", () => {
    const siteUrl = "https://docs.example/chaft";
    writeValidBuild(siteUrl);
    const architecture = DOCUMENTS[1];
    write(
      outputPath(architecture.route),
      renderPage(architecture, siteUrl, {
        contentHref: "#installation",
        sidebarHref: "https://other.example/docs/",
        imageSrc: "../images/docs.png",
        auxiliaryHref: "mailto:help@example.com",
      }),
    );

    expect(() => validate(siteUrl)).not.toThrow();
  });

  it("rejects a relative reference that escapes a configured path base", () => {
    const siteUrl = "https://docs.example/chaft";
    writeValidBuild(siteUrl);
    const architecture = DOCUMENTS[1];
    write(
      outputPath(architecture.route),
      renderPage(architecture, siteUrl, {
        imageSrc: "../../../../escape.png",
      }),
    );

    expect(() => validate(siteUrl)).toThrow(
      /internal src leaks root-only path: \.\.\/\.\.\/\.\.\/\.\.\/escape\.png/,
    );
  });

  it("rejects an index pager", () => {
    const siteUrl = "https://docs.example/";
    writeValidBuild(siteUrl);
    const index = DOCUMENTS[0];
    write(
      outputPath(index.route),
      renderPage(index, siteUrl, {
        nextHref: "/docs/getting-started/install/",
      }),
    );

    expect(() => validate(siteUrl)).toThrow(/expected no \.docs-pager, found 1/);
  });

  it("excludes the index from the first guide's previous neighbor", () => {
    const siteUrl = "https://docs.example/";
    writeValidBuild(siteUrl);
    const install = DOCUMENTS[2];
    write(
      outputPath(install.route),
      renderPage(install, siteUrl, {
        previousHref: "/docs/",
      }),
    );

    expect(() => validate(siteUrl)).toThrow(
      /\.docs-pager must contain exactly 1 link\(s\), found 2/,
    );
  });

  it("rejects a pager route in the wrong direction", () => {
    const siteUrl = "https://docs.example/";
    writeValidBuild(siteUrl);
    const architecture = DOCUMENTS[1];
    write(
      outputPath(architecture.route),
      renderPage(architecture, siteUrl, {
        nextDirection: "previous",
      }),
    );

    expect(() => validate(siteUrl)).toThrow(
      /\.docs-pager must contain exactly 1 previous link\(s\), found 2/,
    );
    expect(() => validate(siteUrl)).toThrow(
      /\.docs-pager must contain exactly 1 next link\(s\), found 0/,
    );
  });

  it("rejects an incorrect pager target", () => {
    const siteUrl = "https://docs.example/chaft";
    writeValidBuild(siteUrl);
    const architecture = DOCUMENTS[1];
    write(
      outputPath(architecture.route),
      renderPage(architecture, siteUrl, {
        previousHref: "/chaft/docs/",
      }),
    );

    expect(() => validate(siteUrl)).toThrow(
      /previous pager route must be https:\/\/docs\.example\/chaft\/docs\/getting-started\/install\//,
    );
  });

  it("rejects a canonical URL that omits the configured base", () => {
    const siteUrl = "https://docs.example/chaft";
    writeValidBuild(siteUrl);
    const architecture = DOCUMENTS[1];
    write(
      outputPath(architecture.route),
      renderPage(architecture, siteUrl, {
        canonical: "https://docs.example/docs/concepts/architecture/",
      }),
    );

    expect(() => validate(siteUrl)).toThrow(/canonical URL must be/);
  });

  it("rejects missing sitemap coverage and draft sitemap exposure", () => {
    const siteUrl = "https://docs.example/";
    writeValidBuild(siteUrl);
    const draft = DOCUMENTS.find((document) => document.draft);
    writeSitemaps(siteUrl, [DOCUMENTS[0], draft]);

    expect(() => validate(siteUrl)).toThrow(/sitemap is missing published documentation URL/);
    expect(() => validate(siteUrl)).toThrow(/sitemap exposes draft documentation URL/);
  });

  it("rejects a root-only sitemap location under a path base", () => {
    const siteUrl = "https://docs.example/chaft";
    writeValidBuild(siteUrl);
    write(
      join(siteOutputDirectory(siteUrl), "sitemap-index.xml"),
      '<?xml version="1.0"?><sitemapindex><sitemap><loc>https://docs.example/sitemap-0.xml</loc></sitemap></sitemapindex>',
    );

    expect(() => validate(siteUrl)).toThrow(/sitemap URL is not base-aware/);
  });
});

describe("CLI parsing", () => {
  it("accepts explicit SITE_URL and dist arguments", () => {
    expect(
      parseCliArguments([
        "--site-url",
        "https://docs.example/chaft",
        "--dist",
        "custom-dist",
      ]),
    ).toEqual({
      siteUrl: "https://docs.example/chaft",
      distDirectory: "custom-dist",
    });
  });

  it("rejects unknown or incomplete arguments", () => {
    expect(() => parseCliArguments(["--unknown"])).toThrow(/unknown argument/);
    expect(() => parseCliArguments(["--dist"])).toThrow(/missing value/);
  });
});
