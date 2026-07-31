import { PREVIEW_SLOTS as REVIEWED_PREVIEW_SLOTS } from "../../scripts/preview-slot-contract.mjs";

export const PRODUCTION_ORIGIN = "https://chaft.ai";
export const PREVIEW_ROBOTS_POLICY = "noindex, nofollow, noarchive";
export const PREVIEW_ROBOTS_TEXT = "User-agent: *\nDisallow: /\n";
export const PRODUCTION_ROBOTS_CONTENT_SIGNAL =
  "search=yes, ai-train=no, use=reference";
export const PRODUCTION_ROBOTS_RIGHTS_NOTICE = [
  "# The Content-Signal directives below state Chaft's permitted content uses.",
  "# Any restriction reserves all applicable rights, including under Article 4",
  "# of Directive (EU) 2019/790.",
].join("\n");
export const PRODUCTION_ROBOTS_BLOCKED_CRAWLERS = Object.freeze([
  "Amazonbot",
  "Applebot-Extended",
  "Bytespider",
  "CCBot",
  "ClaudeBot",
  "CloudflareBrowserRenderingCrawler",
  "Google-Extended",
  "GPTBot",
  "meta-externalagent",
]);

export function productionRobotsText(sitemap) {
  const sitemapUrl = sitemap instanceof URL ? sitemap : new URL(sitemap);
  const blockedCrawlers = PRODUCTION_ROBOTS_BLOCKED_CRAWLERS.flatMap(
    (crawler) => [`User-agent: ${crawler}`, "Disallow: /", ""],
  );
  return [
    PRODUCTION_ROBOTS_RIGHTS_NOTICE,
    "",
    "User-agent: *",
    `Content-Signal: ${PRODUCTION_ROBOTS_CONTENT_SIGNAL}`,
    "Allow: /",
    "",
    ...blockedCrawlers,
    `Sitemap: ${sitemapUrl.href}`,
    "",
  ].join("\n");
}

// The trusted deployment tooling owns the exact allowlist. Website metadata,
// indexing controls, and the deployment workflow all consume the same frozen
// rows instead of maintaining parallel mappings.
export const PREVIEW_SLOTS = REVIEWED_PREVIEW_SLOTS;

const previewSlotsByBranch = new Map(
  PREVIEW_SLOTS.map((slot) => [slot.branch, slot]),
);
const previewOrigins = new Set(PREVIEW_SLOTS.map((slot) => slot.siteUrl));

/**
 * @typedef {(typeof PREVIEW_SLOTS)[number]} PreviewSlot
 * @typedef {{
 *   mode: "site" | "production" | "preview",
 *   isPreview: boolean,
 *   siteUrl: string,
 *   canonicalOrigin: string,
 *   preview: PreviewSlot | undefined,
 * }} WebsiteDeployment
 */

function optionalEnvironmentValue(value, name) {
  if (value === undefined || value === null || value === "") return undefined;
  if (typeof value !== "string" || value.trim() !== value) {
    throw new Error(`${name} must not contain surrounding whitespace`);
  }
  return value;
}

function parseSiteUrl(value) {
  const rawValue =
    value instanceof URL ? value.href : optionalEnvironmentValue(value, "SITE_URL");
  if (!rawValue) {
    throw new Error("SITE_URL is required to resolve the website deployment mode");
  }

  let site;
  try {
    site = new URL(rawValue);
  } catch {
    throw new Error(`SITE_URL is invalid: ${rawValue}`);
  }
  return site;
}

function assertRootHttpsUrl(site, label) {
  if (
    site.protocol !== "https:" ||
    site.username ||
    site.password ||
    site.pathname !== "/" ||
    site.search ||
    site.hash
  ) {
    throw new Error(
      `${label} must be a root HTTPS origin without credentials, a query, or a fragment`,
    );
  }
}

/**
 * @param {"site" | "production" | "preview"} mode
 * @param {URL} site
 * @param {PreviewSlot | undefined} [preview]
 * @returns {Readonly<WebsiteDeployment>}
 */
function frozenDeployment(mode, site, preview = undefined) {
  return Object.freeze({
    mode,
    isPreview: mode === "preview",
    siteUrl: site.origin,
    canonicalOrigin: mode === "preview" ? PRODUCTION_ORIGIN : site.origin,
    preview,
  });
}

export function previewSlotForBranch(branch) {
  return previewSlotsByBranch.get(branch);
}

export function previewSlotLabel(slot) {
  const number = slot.match(/^hero-([1-4])$/u)?.[1];
  if (!number) {
    throw new Error(`Preview slot is not in the exact allowlist: ${slot}`);
  }
  return `Hero ${number}`;
}

export function resolveWebsiteDeployment({
  deploymentMode,
  previewBranch,
  siteUrl,
}) {
  const mode = optionalEnvironmentValue(
    deploymentMode,
    "CHAFT_DEPLOYMENT_MODE",
  );
  const branch = optionalEnvironmentValue(
    previewBranch,
    "CHAFT_PREVIEW_BRANCH",
  );
  const site = parseSiteUrl(siteUrl);

  if (mode === "preview") {
    if (!branch) {
      throw new Error(
        "CHAFT_PREVIEW_BRANCH is required when CHAFT_DEPLOYMENT_MODE=preview",
      );
    }

    const preview = previewSlotForBranch(branch);
    if (!preview) {
      throw new Error(
        `CHAFT_PREVIEW_BRANCH is not assigned to a Preview slot: ${branch}`,
      );
    }

    assertRootHttpsUrl(site, "Preview SITE_URL");
    if (site.origin !== preview.siteUrl) {
      throw new Error(
        `Preview SITE_URL for ${branch} must be exactly ${preview.siteUrl}`,
      );
    }
    return frozenDeployment("preview", site, preview);
  }

  if (mode === "production") {
    if (branch) {
      throw new Error(
        "CHAFT_PREVIEW_BRANCH must be unset when CHAFT_DEPLOYMENT_MODE=production",
      );
    }
    assertRootHttpsUrl(site, "Production SITE_URL");
    if (site.origin !== PRODUCTION_ORIGIN) {
      throw new Error(
        `Production SITE_URL must be exactly ${PRODUCTION_ORIGIN}`,
      );
    }
    return frozenDeployment("production", site);
  }

  if (mode) {
    throw new Error(
      "CHAFT_DEPLOYMENT_MODE must be either production or preview when set",
    );
  }
  if (branch) {
    throw new Error(
      "CHAFT_PREVIEW_BRANCH requires CHAFT_DEPLOYMENT_MODE=preview",
    );
  }
  if (
    previewOrigins.has(site.origin) ||
    /^hero-[a-z0-9-]+\.chaft\.ai$/u.test(site.hostname)
  ) {
    throw new Error(
      "A Chaft Preview slot SITE_URL requires CHAFT_DEPLOYMENT_MODE=preview",
    );
  }

  // Preserve the existing local and validation build behavior when no
  // deployment mode is configured. Only the reserved Preview origins require
  // an explicit mode.
  return frozenDeployment("site", site);
}
