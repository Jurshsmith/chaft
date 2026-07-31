import { sitePath } from "../lib/site-path";
import {
  PREVIEW_ROBOTS_TEXT,
  productionRobotsText,
  resolveWebsiteDeployment,
} from "../lib/preview-contract.mjs";

export const prerender = true;

export function GET({ site }: { site: URL | undefined }) {
  const base = site ?? new URL("http://localhost:4321");
  const deployment = resolveWebsiteDeployment({
    deploymentMode: process.env.CHAFT_DEPLOYMENT_MODE,
    previewBranch: process.env.CHAFT_PREVIEW_BRANCH,
    siteUrl: process.env.SITE_URL ?? base,
  });
  if (deployment.isPreview) {
    return new Response(PREVIEW_ROBOTS_TEXT, {
      headers: {
        "Content-Type": "text/plain; charset=utf-8",
      },
    });
  }

  const sitemap = new URL(sitePath("/sitemap-index.xml"), base);
  return new Response(productionRobotsText(sitemap), {
    headers: {
      "Content-Type": "text/plain; charset=utf-8",
    },
  });
}
