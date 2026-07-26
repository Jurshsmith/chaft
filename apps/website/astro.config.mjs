import { fileURLToPath } from "node:url";

import { unified } from "@astrojs/markdown-remark";
import sitemap from "@astrojs/sitemap";
import { defineConfig } from "astro/config";

import remarkGitHubDocLinks from "./src/lib/remark-github-doc-links.mjs";

const repositoryRoot = fileURLToPath(new URL("../..", import.meta.url));
const publicGuidesRoot = fileURLToPath(
  new URL("../../guides/public/", import.meta.url),
);
const configuredSite = process.env.SITE_URL;
const isCi = process.env.CI === "true";

if (isCi && !configuredSite) {
  throw new Error("SITE_URL is required for CI and deployment builds");
}

const site = new URL(configuredSite ?? "http://localhost:4321");
if (
  configuredSite &&
  (site.protocol !== "https:" ||
    site.search ||
    site.hash ||
    site.username ||
    site.password)
) {
  throw new Error("SITE_URL must be an HTTPS URL without credentials, a query, or a fragment");
}

const base = site.pathname === "/" ? "/" : site.pathname.replace(/\/+$/, "");

export default defineConfig({
  output: "static",
  site: site.origin,
  base,
  integrations: [sitemap()],
  markdown: {
    processor: unified({
      remarkPlugins: [
        [remarkGitHubDocLinks, { guidesRoot: publicGuidesRoot, basePath: base }],
      ],
    }),
  },
  build: {
    inlineStylesheets: "never",
  },
  vite: {
    build: {
      assetsInlineLimit: 0,
    },
    server: {
      fs: {
        allow: [repositoryRoot],
      },
    },
  },
});
