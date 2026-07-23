import { fileURLToPath } from "node:url";

import sitemap from "@astrojs/sitemap";
import { defineConfig } from "astro/config";

const repositoryRoot = fileURLToPath(new URL("../..", import.meta.url));
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
