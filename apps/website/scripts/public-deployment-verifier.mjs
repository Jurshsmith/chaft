const COMMON_HEADERS = {
  "cross-origin-opener-policy": "same-origin",
  "referrer-policy": "strict-origin-when-cross-origin",
  "x-content-type-options": "nosniff",
  "x-frame-options": "DENY",
};

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function normalizedOrigin(siteUrl) {
  const url = new URL(siteUrl);
  assert(
    url.protocol === "https:" &&
      !url.username &&
      !url.password &&
      url.pathname === "/" &&
      !url.search &&
      !url.hash,
    "site URL must be a pathless HTTPS origin",
  );
  return url.origin;
}

function canonicalHref(html) {
  for (const tag of html.matchAll(/<link\b[^>]*>/gi)) {
    if (!/\brel=(["'])canonical\1/i.test(tag[0])) continue;
    const href = /\bhref=(["'])(.*?)\1/i.exec(tag[0]);
    if (href) return href[2];
  }
  return null;
}

function staticAssetHref(html) {
  const match = /(?:src|href)=(["'])([^"']*\/_astro\/[^"']+)\1/i.exec(html);
  return match?.[2] ?? null;
}

function headerIncludes(headers, name, expected) {
  return (headers.get(name) ?? "").toLowerCase().includes(expected.toLowerCase());
}

function assertCommonHeaders(response, label) {
  for (const [name, expected] of Object.entries(COMMON_HEADERS)) {
    assert(response.headers.get(name) === expected, `${label} is missing ${name}: ${expected}`);
  }
  assert(
    headerIncludes(response.headers, "permissions-policy", "camera=()") &&
      headerIncludes(response.headers, "permissions-policy", "microphone=()") &&
      headerIncludes(response.headers, "permissions-policy", "payment=()"),
    `${label} has an unexpected permissions-policy`,
  );
}

async function body(response, label) {
  const text = await response.text();
  assert(!text.includes("website-validation.invalid"), `${label} contains validation-origin data`);
  return text;
}

export async function verifyPublicDeployment({
  alternateSiteUrl,
  expectedCommit,
  fetchImpl = fetch,
  repository,
  siteUrl,
}) {
  assert(/^[a-f0-9]{40}$/.test(expectedCommit), "expected commit must be a full SHA-1");
  assert(repository === "Jurshsmith/chaft", "unexpected source repository");
  const origin = normalizedOrigin(siteUrl);
  const alternateOrigin = alternateSiteUrl ? normalizedOrigin(alternateSiteUrl) : null;
  if (alternateOrigin) assert(alternateOrigin !== origin, "alternate origin must differ");
  const checks = [];

  const record = (name, response, detail) => {
    checks.push({ name, status: response.status, detail });
  };

  const markerResponse = await fetchImpl(`${origin}/.well-known/chaft-deployment.json`, {
    redirect: "manual",
    signal: AbortSignal.timeout(10_000),
  });
  assert(markerResponse.status === 200, "deployment marker must return 200");
  assertCommonHeaders(markerResponse, "deployment marker");
  assert(
    headerIncludes(markerResponse.headers, "cache-control", "no-store"),
    "deployment marker must use no-store",
  );
  const markerText = await body(markerResponse, "deployment marker");
  const marker = JSON.parse(markerText);
  const keys = Object.keys(marker).sort();
  const expectedKeys = [
    "artifactKind",
    "schemaVersion",
    "siteUrl",
    "sourceCommit",
    "sourceRepository",
  ].sort();
  assert(JSON.stringify(keys) === JSON.stringify(expectedKeys), "deployment marker shape changed");
  assert(marker.schemaVersion === 1, "deployment marker schemaVersion must be 1");
  assert(marker.artifactKind === "chaft-website", "deployment marker artifactKind changed");
  assert(marker.sourceRepository === repository, "deployment marker repository does not match");
  assert(marker.sourceCommit === expectedCommit, "deployment marker commit does not match");
  assert(marker.siteUrl === origin, "deployment marker site URL does not match");
  record("deployment-marker", markerResponse, expectedCommit);

  const pages = [
    ["/", 200, "home", "/"],
    ["/download/", 200, "download", "/download/"],
    ["/security/", 200, "security", "/security/"],
    ["/definitely-not-a-page-chaft-verification", 404, "not-found", null],
  ];
  let homeHtml = "";
  for (const [pathname, status, label, expectedCanonical] of pages) {
    const response = await fetchImpl(`${origin}${pathname}`, {
      redirect: "manual",
      signal: AbortSignal.timeout(10_000),
    });
    assert(response.status === status, `${label} must return ${status}`);
    assertCommonHeaders(response, label);
    const html = await body(response, label);
    if (expectedCanonical) {
      assert(
        canonicalHref(html) === `${origin}${expectedCanonical}`,
        `${label} canonical URL does not match ${origin}${expectedCanonical}`,
      );
    }
    if (label === "home") homeHtml = html;
    record(label, response, pathname);
  }

  for (const [pathname, expectedStatus, expectedLocation, label] of [
    ["/downloads", 301, `${origin}/download/`, "downloads-redirect"],
    ["/source", 302, "https://github.com/Jurshsmith/chaft", "source-redirect"],
  ]) {
    const response = await fetchImpl(`${origin}${pathname}`, {
      redirect: "manual",
      signal: AbortSignal.timeout(10_000),
    });
    assert(response.status === expectedStatus, `${label} must return ${expectedStatus}`);
    assert(
      new URL(response.headers.get("location"), origin).href === expectedLocation,
      `${label} location does not match`,
    );
    record(label, response, expectedLocation);
  }

  const currentRelease = await fetchImpl(`${origin}/releases/current.json`, {
    redirect: "manual",
    signal: AbortSignal.timeout(10_000),
  });
  assert(currentRelease.status === 200, "current release JSON must return 200");
  assertCommonHeaders(currentRelease, "current release JSON");
  assert(
    headerIncludes(currentRelease.headers, "cache-control", "max-age=0") &&
      headerIncludes(currentRelease.headers, "cache-control", "must-revalidate"),
    "current release JSON must revalidate",
  );
  JSON.parse(await body(currentRelease, "current release JSON"));
  record("current-release", currentRelease, "/releases/current.json");

  for (const [pathname, label] of [
    ["/robots.txt", "robots"],
    ["/sitemap-index.xml", "sitemap"],
  ]) {
    const response = await fetchImpl(`${origin}${pathname}`, {
      redirect: "manual",
      signal: AbortSignal.timeout(10_000),
    });
    assert(response.status === 200, `${label} must return 200`);
    // Cloudflare's zone-level managed robots feature prepends content signals
    // outside the Static Assets response path, so it does not preserve the
    // Worker `_headers` rules. The sitemap remains a normal static response.
    if (label !== "robots") assertCommonHeaders(response, label);
    const text = await body(response, label);
    assert(text.includes(origin), `${label} does not reference the production origin`);
    record(label, response, pathname);
  }

  const assetHref = staticAssetHref(homeHtml);
  assert(assetHref, "home page does not reference a hashed Astro asset");
  const assetUrl = new URL(assetHref, origin);
  assert(assetUrl.origin === origin && assetUrl.pathname.startsWith("/_astro/"), "invalid asset URL");
  const asset = await fetchImpl(assetUrl, {
    redirect: "manual",
    signal: AbortSignal.timeout(10_000),
  });
  assert(asset.status === 200, "referenced Astro asset must return 200");
  assertCommonHeaders(asset, "Astro asset");
  assert(
    headerIncludes(asset.headers, "cache-control", "max-age=31536000") &&
      headerIncludes(asset.headers, "cache-control", "immutable"),
    "Astro asset must use one-year immutable caching",
  );
  await body(asset, "Astro asset");
  record("hashed-asset", asset, assetUrl.pathname);

  if (alternateOrigin) {
    const alternateHome = await fetchImpl(`${alternateOrigin}/`, {
      redirect: "manual",
      signal: AbortSignal.timeout(10_000),
    });
    assert(alternateHome.status === 200, "alternate hostname must return 200");
    assertCommonHeaders(alternateHome, "alternate hostname");
    const alternateHtml = await body(alternateHome, "alternate hostname");
    assert(
      canonicalHref(alternateHtml) === `${origin}/`,
      "alternate hostname must retain the apex canonical URL",
    );
    record("alternate-home", alternateHome, alternateOrigin);
  }

  return {
    schemaVersion: 1,
    artifactKind: "chaft-website-public-verification",
    verifiedAt: new Date().toISOString(),
    siteUrl: origin,
    alternateSiteUrl: alternateOrigin,
    repository,
    expectedCommit,
    checks,
    result: "passed",
  };
}
