import { sitePath } from "../lib/site-path";

export const prerender = true;

export function GET({ site }: { site: URL | undefined }) {
  const base = site ?? new URL("http://localhost:4321");
  const sitemap = new URL(sitePath("/sitemap-index.xml"), base);
  return new Response(`User-agent: *\nAllow: /\nSitemap: ${sitemap}\n`, {
    headers: {
      "Content-Type": "text/plain; charset=utf-8",
    },
  });
}
