import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { renderDeploymentMarker } from "./deployment-artifact.mjs";
import { validatePreviewStaticArtifact } from "./preview-static-artifact-validator.mjs";

const roots = [];
const sourceCommit = "0123456789abcdef0123456789abcdef01234567";

function html(pathname) {
  const home =
    pathname === "/"
      ? `<header>Header</header>
    <main>
      <section class="hero" data-chaft-hero="hero-1">
        <a href="/download/">Download Chaft <span>→</span></a>
        <a href="/docs/">Read the docs</a>
        <a href="https://github.com/Jurshsmith/chaft">Explore the source</a>
        <p class="hero__note"><span>Canary</span> Unaudited software. Not for sensitive or production communication.</p>
      </section>
      <section>Stable</section>
    </main>
    <footer>Footer</footer>`
      : "";
  return `<!doctype html>
<html>
  <head>
    <meta name="robots" content="noindex, nofollow, noarchive" />
    <link rel="canonical" href="https://chaft.ai${pathname}" />
    <meta property="og:url" content="https://chaft.ai${pathname}" />
  </head>
  <body><aside>Chaft Preview · Hero 1</aside>${home}</body>
</html>`;
}

const expectedContentHashes = {
  header: createHash("sha256").update("Header").digest("hex"),
  footer: createHash("sha256").update("Footer").digest("hex"),
  nonHero: createHash("sha256").update("Stable").digest("hex"),
};

async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "chaft-preview-static-"));
  roots.push(root);
  for (const directory of [".well-known", "download", "security"]) {
    await mkdir(join(root, directory), { recursive: true });
  }
  await writeFile(
    join(root, "_headers"),
    `/*
  X-Robots-Tag: noindex, nofollow, noarchive

/.well-known/chaft-deployment.json
  Cache-Control: no-store
`,
  );
  await writeFile(join(root, "robots.txt"), "User-agent: *\nDisallow: /\n");
  await writeFile(
    join(root, ".well-known", "chaft-deployment.json"),
    renderDeploymentMarker({
      sourceRepository: "Jurshsmith/chaft",
      sourceCommit,
      siteUrl: "https://hero-1.chaft.ai",
    }),
  );
  await writeFile(join(root, "index.html"), html("/"));
  await writeFile(join(root, "download", "index.html"), html("/download/"));
  await writeFile(join(root, "security", "index.html"), html("/security/"));
  return root;
}

afterEach(async () => {
  await Promise.all(
    roots.splice(0).map((root) => rm(root, { force: true, recursive: true })),
  );
});

describe("trusted Preview static artifact validation", () => {
  it("accepts exact noindex, canonical, badge, and marker controls", async () => {
    const root = await fixture();
    expect(
      validatePreviewStaticArtifact({
        branch: "preview/landing-hero-1",
        distDirectory: root,
        expectedContentHashes,
        repository: "Jurshsmith/chaft",
        sourceCommit,
      }),
    ).toMatchObject({
      artifactKind: "chaft-preview-static-validation",
      result: "passed",
      slot: "hero-1",
      worker: "chaft-website-hero-1",
    });
  });

  it("rejects a crawlable artifact before deployment", async () => {
    const root = await fixture();
    await writeFile(join(root, "robots.txt"), "User-agent: *\nAllow: /\n");
    expect(() =>
      validatePreviewStaticArtifact({
        branch: "preview/landing-hero-1",
        distDirectory: root,
        expectedContentHashes,
        repository: "Jurshsmith/chaft",
        sourceCommit,
      }),
    ).toThrow(/robots\.txt must disallow/);
  });

  it("rejects a production marker substituted into a Preview artifact", async () => {
    const root = await fixture();
    await writeFile(
      join(root, ".well-known", "chaft-deployment.json"),
      renderDeploymentMarker({
        sourceRepository: "Jurshsmith/chaft",
        sourceCommit,
        siteUrl: "https://chaft.ai",
      }),
    );
    expect(() =>
      validatePreviewStaticArtifact({
        branch: "preview/landing-hero-1",
        distDirectory: root,
        expectedContentHashes,
        repository: "Jurshsmith/chaft",
        sourceCommit,
      }),
    ).toThrow(/marker identity/);
  });

  it("rejects appended or contradictory security copy", async () => {
    const root = await fixture();
    const page = await readFile(join(root, "index.html"), "utf8");
    await writeFile(
      join(root, "index.html"),
      page.replace(
        "Not for sensitive or production communication.",
        "Not for sensitive or production communication. Fully audited and production-ready.",
      ),
    );

    expect(() =>
      validatePreviewStaticArtifact({
        branch: "preview/landing-hero-1",
        distDirectory: root,
        expectedContentHashes,
        repository: "Jurshsmith/chaft",
        sourceCommit,
      }),
    ).toThrow(/security warning must retain/);
  });

  it("rejects a baseline or different slot hero", async () => {
    const root = await fixture();
    const page = await readFile(join(root, "index.html"), "utf8");
    await writeFile(
      join(root, "index.html"),
      page.replace('data-chaft-hero="hero-1"', 'data-chaft-hero="baseline"'),
    );

    expect(() =>
      validatePreviewStaticArtifact({
        branch: "preview/landing-hero-1",
        distDirectory: root,
        expectedContentHashes,
        repository: "Jurshsmith/chaft",
        sourceCommit,
      }),
    ).toThrow(/must render the exact hero-1 hero/);
  });
});
