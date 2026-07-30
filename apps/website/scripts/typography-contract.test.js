import { existsSync, readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

const websiteRoot = new URL("../", import.meta.url);

function read(pathname) {
  return readFileSync(new URL(pathname, websiteRoot), "utf8");
}

describe("website typography contract", () => {
  it("loads the supported Chillax weights from Fontshare", () => {
    const layout = read("src/layouts/BaseLayout.astro");

    expect(layout).toContain(
      "https://api.fontshare.com/v2/css?f[]=chillax@400,500,600,700&display=swap",
    );
    expect(layout).toContain('href="https://api.fontshare.com"');
    expect(layout).toContain('href="https://cdn.fontshare.com"');
  });

  it("uses Chillax only for body copy and keeps Space Grotesk for UI text", () => {
    const stylesheet = read("src/styles/global.css");

    expect(stylesheet).toContain(
      '--font-ui: "Space Grotesk", "Avenir Next", Avenir, Inter, system-ui, sans-serif;',
    );
    expect(stylesheet).toContain(
      '--font-body: "Chillax", var(--font-ui);',
    );
    expect(stylesheet).toMatch(/body\s*\{[\s\S]*?font-family:\s*var\(--font-ui\);/);
    expect(stylesheet).toMatch(
      /h1,\s*h2,\s*h3\s*\{[\s\S]*?font-family:\s*var\(--font-ui\);/,
    );
    expect(stylesheet).toMatch(
      /\.body-copy,[\s\S]*?font-family:\s*var\(--font-body\);/,
    );
  });

  it("retains the licensed Space Grotesk UI font bundle", () => {
    for (const pathname of [
      "src/assets/fonts/SpaceGrotesk-Regular-latin.woff2",
      "src/assets/fonts/SpaceGrotesk-Medium-latin.woff2",
      "public/licenses/OFL-SpaceGrotesk.txt",
    ]) {
      expect(existsSync(new URL(pathname, websiteRoot))).toBe(true);
    }
  });
});
