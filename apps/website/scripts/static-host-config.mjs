import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export function deploymentBase(siteUrl) {
  const pathname = new URL(siteUrl).pathname.replace(/\/+$/, "");
  return pathname === "" ? "" : pathname;
}

export function withDeploymentBase(base, pathname) {
  if (!pathname.startsWith("/")) {
    throw new Error(`deployment path must begin with "/": ${pathname}`);
  }
  return `${base}${pathname}`;
}

export function renderHeaders(base) {
  return `/*
  Cross-Origin-Opener-Policy: same-origin
  Permissions-Policy: camera=(), geolocation=(), microphone=(), payment=(), usb=()
  Referrer-Policy: strict-origin-when-cross-origin
  X-Content-Type-Options: nosniff
  X-Frame-Options: DENY

${withDeploymentBase(base, "/.well-known/chaft-deployment.json")}
  Cache-Control: no-store

${withDeploymentBase(base, "/_astro/*")}
  Cache-Control: public, max-age=31536000, immutable

${withDeploymentBase(base, "/releases/*.json")}
  Cache-Control: public, max-age=0, must-revalidate
`;
}

export function renderRedirects(base) {
  return `${withDeploymentBase(base, "/downloads")} ${withDeploymentBase(base, "/download/")} 301
${withDeploymentBase(base, "/source")} https://github.com/Jurshsmith/chaft 302
`;
}

export function writeStaticHostConfig(distDirectory, siteUrl) {
  // Read first so a missing Astro-copied provider file fails the build rather
  // than silently producing a partially configured deployment artifact.
  readFileSync(join(distDirectory, "_headers"), "utf8");
  readFileSync(join(distDirectory, "_redirects"), "utf8");

  const base = deploymentBase(siteUrl);
  writeFileSync(join(distDirectory, "_headers"), renderHeaders(base), "utf8");
  writeFileSync(join(distDirectory, "_redirects"), renderRedirects(base), "utf8");
}
