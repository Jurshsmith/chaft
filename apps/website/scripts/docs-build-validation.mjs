import {
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
  realpathSync,
} from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { deploymentMountPath } from "./deployment-artifact.mjs";

const WEBSITE_DIRECTORY = fileURLToPath(new URL("../", import.meta.url));
const REPOSITORY_ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const DOC_SECTION_ORDER = [
  "getting-started",
  "concepts",
  "development",
  "reference",
];
const DOC_SECTION_POSITION = new Map(
  DOC_SECTION_ORDER.map((section, index) => [section, index]),
);

export const DEFAULT_DOCS_DIST_DIRECTORY = join(WEBSITE_DIRECTORY, "dist");
export const DEFAULT_PUBLIC_GUIDES_DIRECTORY = join(REPOSITORY_ROOT, "guides", "public");

export class DocsBuildValidationError extends Error {
  constructor(issues) {
    const sorted = [...issues].sort(
      (left, right) =>
        left.location.localeCompare(right.location) ||
        left.message.localeCompare(right.message),
    );
    super(
      [
        `Built documentation validation failed with ${sorted.length} issue(s):`,
        ...sorted.map((issue) => `- ${issue.location}: ${issue.message}`),
      ].join("\n"),
    );
    this.name = "DocsBuildValidationError";
    this.issues = sorted;
  }
}

function issue(location, message) {
  return { location, message };
}

function pathIsWithin(root, candidate) {
  const fromRoot = relative(root, candidate);
  return (
    fromRoot === "" ||
    (!fromRoot.startsWith(`..${sep}`) && fromRoot !== ".." && !isAbsolute(fromRoot))
  );
}

function displayPath(filePath, root = REPOSITORY_ROOT) {
  const fromRoot = relative(root, filePath);
  return pathIsWithin(root, filePath) ? fromRoot.split(sep).join("/") : filePath;
}

function decodeHtml(value) {
  return value.replace(
    /&(?:#([0-9]+)|#x([0-9a-f]+)|([a-z]+));/gi,
    (entity, decimal, hexadecimal, named) => {
      if (decimal) {
        return String.fromCodePoint(Number.parseInt(decimal, 10));
      }
      if (hexadecimal) {
        return String.fromCodePoint(Number.parseInt(hexadecimal, 16));
      }
      return (
        {
          amp: "&",
          apos: "'",
          gt: ">",
          lt: "<",
          quot: '"',
        }[named.toLowerCase()] ?? entity
      );
    },
  );
}

function textContent(value) {
  return decodeHtml(value.replace(/<[^>]*>/g, "")).replace(/\s+/g, " ").trim();
}

function attribute(tag, name) {
  const match = new RegExp(
    `\\s${name}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|([^\\s"'=<>\\x60]+))`,
    "i",
  ).exec(tag);
  return match ? decodeHtml(match[1] ?? match[2] ?? match[3]) : null;
}

function tags(html, name) {
  return html.match(new RegExp(`<${name}\\b[^>]*>`, "gi")) ?? [];
}

function pairedContents(html, name) {
  return [...html.matchAll(new RegExp(`<${name}\\b[^>]*>([\\s\\S]*?)<\\/${name}>`, "gi"))].map(
    (match) => match[1],
  );
}

function parseScalar(raw, key, location) {
  const value = raw.trim();
  if (value === "") {
    throw new Error(`${location}: front matter "${key}" must not be empty`);
  }
  if (value.startsWith('"')) {
    try {
      const parsed = JSON.parse(value);
      if (typeof parsed !== "string") {
        throw new TypeError("not a string");
      }
      return parsed;
    } catch {
      throw new Error(`${location}: front matter "${key}" has an invalid quoted string`);
    }
  }
  if (value.startsWith("'")) {
    if (!value.endsWith("'") || value.length < 2) {
      throw new Error(`${location}: front matter "${key}" has an invalid quoted string`);
    }
    return value.slice(1, -1).replaceAll("''", "'");
  }
  if (/^[\[\]{}>|&*!]/.test(value)) {
    throw new Error(`${location}: front matter "${key}" must be a one-line scalar`);
  }
  return value;
}

export function parseGuideMetadata(source, { location = "<guide>" } = {}) {
  const lines = source.replaceAll("\r\n", "\n").split("\n");
  if (lines[0] !== "---") {
    throw new Error(`${location}: guide must start with front matter`);
  }
  const closing = lines.indexOf("---", 1);
  if (closing === -1) {
    throw new Error(`${location}: guide front matter is not closed`);
  }

  const values = new Map();
  for (let index = 1; index < closing; index += 1) {
    const line = lines[index];
    if (line.trim() === "") {
      continue;
    }
    const match = /^([A-Za-z][A-Za-z0-9_-]*):(?:[ \t]+(.*))?$/.exec(line);
    if (!match) {
      throw new Error(`${location}:${index + 1}: invalid one-line front matter entry`);
    }
    const [, key, raw = ""] = match;
    if (values.has(key)) {
      throw new Error(`${location}:${index + 1}: duplicate front matter key "${key}"`);
    }
    values.set(key, raw);
  }

  for (const key of ["title", "description", "section", "order"]) {
    if (!values.has(key)) {
      throw new Error(`${location}: missing front matter key "${key}"`);
    }
  }

  const section = parseScalar(values.get("section"), "section", location);
  if (!DOC_SECTION_POSITION.has(section)) {
    throw new Error(
      `${location}: front matter "section" must be one of ${DOC_SECTION_ORDER.join(", ")}`,
    );
  }

  const orderValue = values.get("order").trim();
  if (!/^(?:0|[1-9][0-9]*)$/.test(orderValue)) {
    throw new Error(`${location}: front matter "order" must be a non-negative integer`);
  }
  const order = Number(orderValue);
  if (!Number.isSafeInteger(order)) {
    throw new Error(`${location}: front matter "order" must be a safe integer`);
  }

  const draftValue = values.has("draft") ? values.get("draft").trim() : "false";
  if (draftValue !== "true" && draftValue !== "false") {
    throw new Error(`${location}: front matter "draft" must be true or false`);
  }

  return {
    title: parseScalar(values.get("title"), "title", location),
    description: parseScalar(values.get("description"), "description", location),
    section,
    order,
    draft: draftValue === "true",
  };
}

function documentIdFromRelativePath(relativePath) {
  const normalized = relativePath.split(sep).join("/");
  const segments = normalized.split("/");
  if (
    !normalized.endsWith(".md") ||
    normalized.startsWith("/") ||
    segments.some((segment) => !segment || segment === "." || segment === "..") ||
    segments
      .slice(0, -1)
      .some((segment) => !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(segment)) ||
    !/^[a-z0-9]+(?:-[a-z0-9]+)*\.md$/.test(segments.at(-1))
  ) {
    throw new Error(`invalid public guide path: ${relativePath}`);
  }

  return normalized.slice(0, -3);
}

function routeFromId(id) {
  if (id === "index") {
    return "/docs/";
  }
  return id.endsWith("/index")
    ? `/docs/${id.slice(0, -"/index".length)}/`
    : `/docs/${id}/`;
}

function collectGuideFiles(directory, root = directory, files = []) {
  for (const entry of readdirSync(directory, { withFileTypes: true }).sort((left, right) =>
    left.name.localeCompare(right.name),
  )) {
    const entryPath = join(directory, entry.name);
    if (entry.isSymbolicLink()) {
      throw new Error(`${displayPath(entryPath)}: symbolic links are not allowed`);
    }
    if (entry.isDirectory()) {
      collectGuideFiles(entryPath, root, files);
    } else if (entry.isFile() && entry.name.endsWith(".md")) {
      files.push(entryPath);
    }
  }
  return files;
}

function normalizeExpectedDocument(document, location) {
  if (!document || typeof document !== "object" || Array.isArray(document)) {
    throw new Error(`${location}: expected a document object`);
  }
  const { route, title, description, section, order, draft, pageTitle } = document;
  if (
    typeof route !== "string" ||
    !/^\/docs(?:\/[a-z0-9]+(?:-[a-z0-9]+)*)*\/$/.test(route)
  ) {
    throw new Error(`${location}: invalid documentation route`);
  }
  if (typeof title !== "string" || title.trim() === "") {
    throw new Error(`${location}: title must be a non-empty string`);
  }
  if (typeof description !== "string" || description.trim() === "") {
    throw new Error(`${location}: description must be a non-empty string`);
  }
  if (!DOC_SECTION_POSITION.has(section)) {
    throw new Error(`${location}: section must be one of ${DOC_SECTION_ORDER.join(", ")}`);
  }
  if (!Number.isSafeInteger(order) || order < 0) {
    throw new Error(`${location}: order must be a non-negative safe integer`);
  }
  if (typeof draft !== "boolean") {
    throw new Error(`${location}: draft must be a boolean`);
  }
  if (pageTitle !== undefined && (typeof pageTitle !== "string" || pageTitle.trim() === "")) {
    throw new Error(`${location}: pageTitle must be a non-empty string when provided`);
  }
  return {
    route,
    title,
    description,
    section,
    order,
    draft,
    pageTitle: pageTitle ?? `${title} · Chaft`,
    source: location,
  };
}

export function loadExpectedDocuments({
  guidesDirectory = DEFAULT_PUBLIC_GUIDES_DIRECTORY,
  manifestPath,
  expectedDocuments,
} = {}) {
  if (expectedDocuments !== undefined && manifestPath !== undefined) {
    throw new Error("choose expectedDocuments or manifestPath, not both");
  }

  let documents;
  if (expectedDocuments !== undefined) {
    documents = expectedDocuments.map((document, index) =>
      normalizeExpectedDocument(document, `expectedDocuments[${index}]`),
    );
  } else if (manifestPath !== undefined) {
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    const entries = Array.isArray(manifest) ? manifest : manifest?.documents;
    if (!Array.isArray(entries)) {
      throw new Error(`${manifestPath}: manifest must be an array or contain "documents"`);
    }
    documents = entries.map((document, index) =>
      normalizeExpectedDocument(document, `${manifestPath}:documents[${index}]`),
    );
  } else {
    const requestedRoot = resolve(guidesDirectory);
    if (!existsSync(requestedRoot) || !lstatSync(requestedRoot).isDirectory()) {
      throw new Error(`public guides directory does not exist: ${requestedRoot}`);
    }
    const canonicalRoot = realpathSync(requestedRoot);
    documents = collectGuideFiles(canonicalRoot).map((filePath) => {
      const relativePath = relative(canonicalRoot, filePath);
      const metadata = parseGuideMetadata(readFileSync(filePath, "utf8"), {
        location: displayPath(filePath),
      });
      return normalizeExpectedDocument(
        {
          route: routeFromId(documentIdFromRelativePath(relativePath)),
          ...metadata,
        },
        displayPath(filePath),
      );
    });
  }

  const routes = new Map();
  for (const document of documents) {
    const previous = routes.get(document.route);
    if (previous) {
      throw new Error(
        `duplicate documentation route ${document.route}: ${previous.source} and ${document.source}`,
      );
    }
    routes.set(document.route, document);
  }
  return documents;
}

export function deriveSiteLocation(siteUrl) {
  if (typeof siteUrl !== "string" || siteUrl.trim() === "") {
    throw new Error("SITE_URL is required");
  }
  const site = new URL(siteUrl);
  if (
    site.protocol !== "https:" ||
    site.username ||
    site.password ||
    site.search ||
    site.hash
  ) {
    throw new Error(
      "SITE_URL must be an HTTPS URL without credentials, a query, or a fragment",
    );
  }
  const mountPath = deploymentMountPath(siteUrl);
  const basePath = mountPath ? `/${mountPath}` : "/";
  const origin = site.origin;

  return {
    origin,
    basePath,
    sitePath(route) {
      if (!route.startsWith("/")) {
        throw new Error(`site route must start with "/": ${route}`);
      }
      return basePath === "/" ? route : `${basePath}${route}`;
    },
    canonical(route) {
      const path = basePath === "/" ? route : `${basePath}${route}`;
      return new URL(path, origin).href;
    },
  };
}

function routeOutputPath(distDirectory, route) {
  return join(distDirectory, ...route.split("/").filter(Boolean), "index.html");
}

function siteOutputRoot(distDirectory, siteLocation) {
  return siteLocation.basePath === "/"
    ? distDirectory
    : join(distDirectory, ...siteLocation.basePath.slice(1).split("/"));
}

function collectBuiltDocRoutes(outputRoot) {
  const docsDirectory = join(outputRoot, "docs");
  if (!existsSync(docsDirectory)) {
    return [];
  }
  const routes = [];
  function walk(directory) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const entryPath = join(directory, entry.name);
      if (entry.isSymbolicLink()) {
        throw new Error(`built docs output must not contain a symbolic link: ${entryPath}`);
      }
      if (entry.isDirectory()) {
        walk(entryPath);
      } else if (entry.isFile() && entry.name === "index.html") {
        const routeDirectory = relative(outputRoot, dirname(entryPath)).split(sep).join("/");
        routes.push(`/${routeDirectory}/`);
      }
    }
  }
  walk(docsDirectory);
  return routes.sort();
}

function expectedCanonicalSet(documents, siteLocation, draft) {
  return new Set(
    documents
      .filter((document) => document.draft === draft)
      .map((document) => siteLocation.canonical(document.route)),
  );
}

function classNames(element) {
  return new Set((attribute(element, "class") ?? "").split(/\s+/).filter(Boolean));
}

function pairedElements(html, name) {
  return [
    ...html.matchAll(
      new RegExp(`(<${name}\\b[^>]*>)([\\s\\S]*?)<\\/${name}\\s*>`, "gi"),
    ),
  ].map((match) => ({ openingTag: match[1], contents: match[2] }));
}

function compareExpectedDocuments(left, right) {
  return (
    DOC_SECTION_POSITION.get(left.section) - DOC_SECTION_POSITION.get(right.section) ||
    left.order - right.order ||
    left.title.localeCompare(right.title) ||
    left.route.localeCompare(right.route)
  );
}

function expectedPagerNeighbors(documents) {
  const orderedGuides = documents
    .filter((document) => !document.draft && document.route !== "/docs/")
    .sort(compareExpectedDocuments);
  const neighbors = new Map([
    ["/docs/", { previous: undefined, next: undefined }],
  ]);
  for (const [index, document] of orderedGuides.entries()) {
    neighbors.set(document.route, {
      previous: orderedGuides[index - 1],
      next: orderedGuides[index + 1],
    });
  }
  return neighbors;
}

function inspectSubpathReferences(
  html,
  siteLocation,
  currentCanonical,
  outputPath,
  issues,
) {
  if (siteLocation.basePath === "/") {
    return;
  }

  const elements = html.match(/<[A-Za-z][^>]*>/g) ?? [];
  for (const element of elements) {
    for (const attributeName of ["href", "src"]) {
      const value = attribute(element, attributeName);
      if (!value || value.startsWith("#")) {
        continue;
      }

      if (
        /^[A-Za-z][A-Za-z0-9+.-]*:/.test(value) &&
        !/^https?:/i.test(value)
      ) {
        continue;
      }

      let parsed;
      try {
        parsed = new URL(value, currentCanonical);
      } catch {
        continue;
      }

      if (parsed.origin !== siteLocation.origin) {
        continue;
      }

      const isWithinBase =
        parsed.pathname === siteLocation.basePath ||
        parsed.pathname.startsWith(`${siteLocation.basePath}/`);
      if (!isWithinBase) {
        issues.push(
          issue(
            outputPath,
            `internal ${attributeName} leaks root-only path: ${value}`,
          ),
        );
      }
    }
  }
}

function inspectPager(
  document,
  html,
  expectedNeighbors,
  siteLocation,
  outputPath,
  issues,
) {
  const expected = expectedNeighbors.get(document.route) ?? {
    previous: undefined,
    next: undefined,
  };
  const expectedLinkCount = Number(Boolean(expected.previous)) + Number(Boolean(expected.next));
  const pagerOpeningTags = (html.match(/<[A-Za-z][^>]*>/g) ?? []).filter((element) =>
    classNames(element).has("docs-pager"),
  );

  if (pagerOpeningTags.length !== Number(expectedLinkCount > 0)) {
    issues.push(
      issue(
        outputPath,
        expectedLinkCount === 0
          ? `expected no .docs-pager, found ${pagerOpeningTags.length}`
          : `expected exactly one .docs-pager, found ${pagerOpeningTags.length}`,
      ),
    );
    return;
  }
  if (expectedLinkCount === 0) {
    return;
  }

  const pagers = pairedElements(html, "nav").filter(({ openingTag }) =>
    classNames(openingTag).has("docs-pager"),
  );
  if (pagers.length !== 1) {
    issues.push(issue(outputPath, ".docs-pager must be a paired <nav> element"));
    return;
  }

  const anchors = tags(pagers[0].contents, "a");
  if (anchors.length !== expectedLinkCount) {
    issues.push(
      issue(
        outputPath,
        `.docs-pager must contain exactly ${expectedLinkCount} link(s), found ${anchors.length}`,
      ),
    );
  }

  for (const direction of ["previous", "next"]) {
    const expectedDocument = expected[direction];
    const directionClass = `docs-pager__link--${direction}`;
    const directionLinks = anchors.filter((anchor) =>
      classNames(anchor).has(directionClass),
    );
    const expectedDirectionCount = expectedDocument ? 1 : 0;
    if (directionLinks.length !== expectedDirectionCount) {
      issues.push(
        issue(
          outputPath,
          `.docs-pager must contain exactly ${expectedDirectionCount} ${direction} link(s), found ${directionLinks.length}`,
        ),
      );
      continue;
    }
    if (!expectedDocument) {
      continue;
    }

    const href = attribute(directionLinks[0], "href");
    const expectedHref = siteLocation.canonical(expectedDocument.route);
    let resolvedHref;
    try {
      resolvedHref = href
        ? new URL(href, siteLocation.canonical(document.route)).href
        : null;
    } catch {
      resolvedHref = null;
    }
    if (resolvedHref !== expectedHref) {
      issues.push(
        issue(
          outputPath,
          `${direction} pager route must be ${expectedHref}, found ${href ?? "no href"}`,
        ),
      );
    }
  }
}

function inspectPage(
  document,
  html,
  expectedNeighbors,
  siteLocation,
  outputPath,
  issues,
) {
  const titleElements = pairedContents(html, "title");
  if (titleElements.length !== 1) {
    issues.push(issue(outputPath, `expected exactly one <title>, found ${titleElements.length}`));
  } else if (textContent(titleElements[0]) !== document.pageTitle) {
    issues.push(
      issue(
        outputPath,
        `page title must be "${document.pageTitle}", found "${textContent(titleElements[0])}"`,
      ),
    );
  }

  const descriptions = tags(html, "meta").filter(
    (tag) => attribute(tag, "name")?.toLowerCase() === "description",
  );
  if (descriptions.length !== 1) {
    issues.push(
      issue(
        outputPath,
        `expected exactly one description meta tag, found ${descriptions.length}`,
      ),
    );
  } else if (attribute(descriptions[0], "content") !== document.description) {
    issues.push(issue(outputPath, "description meta content does not match guide metadata"));
  }

  const headings = pairedContents(html, "h1");
  if (headings.length !== 1) {
    issues.push(issue(outputPath, `expected exactly one <h1>, found ${headings.length}`));
  } else if (textContent(headings[0]) !== document.title) {
    issues.push(
      issue(
        outputPath,
        `H1 must be "${document.title}", found "${textContent(headings[0])}"`,
      ),
    );
  }

  const canonicals = tags(html, "link").filter((tag) =>
    (attribute(tag, "rel") ?? "")
      .toLowerCase()
      .split(/\s+/)
      .includes("canonical"),
  );
  const expectedCanonical = siteLocation.canonical(document.route);
  if (canonicals.length !== 1) {
    issues.push(
      issue(outputPath, `expected exactly one canonical link, found ${canonicals.length}`),
    );
  } else if (attribute(canonicals[0], "href") !== expectedCanonical) {
    issues.push(
      issue(
        outputPath,
        `canonical URL must be ${expectedCanonical}, found ${attribute(canonicals[0], "href")}`,
      ),
    );
  }

  inspectPager(document, html, expectedNeighbors, siteLocation, outputPath, issues);
  inspectSubpathReferences(
    html,
    siteLocation,
    expectedCanonical,
    outputPath,
    issues,
  );
}

function logicalPathFromSitePath(pathname, basePath) {
  if (basePath === "/") {
    return pathname;
  }
  if (pathname === basePath) {
    return "/";
  }
  return pathname.startsWith(`${basePath}/`) ? pathname.slice(basePath.length) : null;
}

function xmlLocations(xml) {
  return [...xml.matchAll(/<loc>([\s\S]*?)<\/loc>/gi)].map((match) =>
    decodeHtml(match[1].trim()),
  );
}

function inspectSitemaps(
  outputRoot,
  siteLocation,
  publishedCanonicals,
  draftCanonicals,
  issues,
) {
  const indexPath = join(outputRoot, "sitemap-index.xml");
  if (!existsSync(indexPath)) {
    issues.push(issue(indexPath, "sitemap index is missing"));
    return;
  }

  const sitemapLocations = xmlLocations(readFileSync(indexPath, "utf8"));
  if (sitemapLocations.length === 0) {
    issues.push(issue(indexPath, "sitemap index contains no sitemap locations"));
    return;
  }

  const pageLocations = new Set();
  for (const location of sitemapLocations) {
    let url;
    try {
      url = new URL(location);
    } catch {
      issues.push(issue(indexPath, `invalid sitemap URL: ${location}`));
      continue;
    }
    if (url.origin !== siteLocation.origin) {
      issues.push(issue(indexPath, `sitemap URL uses the wrong origin: ${location}`));
      continue;
    }
    const logicalPath = logicalPathFromSitePath(url.pathname, siteLocation.basePath);
    if (
      logicalPath === null ||
      !/^\/sitemap-[a-z0-9-]+\.xml$/.test(logicalPath) ||
      url.search ||
      url.hash
    ) {
      issues.push(issue(indexPath, `sitemap URL is not base-aware or valid: ${location}`));
      continue;
    }
    const sitemapPath = join(outputRoot, logicalPath.slice(1));
    if (!existsSync(sitemapPath)) {
      issues.push(issue(indexPath, `referenced sitemap is missing: ${logicalPath}`));
      continue;
    }
    for (const pageLocation of xmlLocations(readFileSync(sitemapPath, "utf8"))) {
      pageLocations.add(pageLocation);
    }
  }

  for (const canonical of publishedCanonicals) {
    if (!pageLocations.has(canonical)) {
      issues.push(issue(indexPath, `sitemap is missing published documentation URL: ${canonical}`));
    }
  }
  for (const canonical of draftCanonicals) {
    if (pageLocations.has(canonical)) {
      issues.push(issue(indexPath, `sitemap exposes draft documentation URL: ${canonical}`));
    }
  }

  const expectedDocs = publishedCanonicals;
  for (const location of pageLocations) {
    let url;
    try {
      url = new URL(location);
    } catch {
      issues.push(issue(indexPath, `invalid page URL in sitemap: ${location}`));
      continue;
    }
    if (url.origin !== siteLocation.origin) {
      continue;
    }
    const logicalPath = logicalPathFromSitePath(url.pathname, siteLocation.basePath);
    if (logicalPath === null) {
      if (url.pathname === "/docs" || url.pathname.startsWith("/docs/")) {
        issues.push(issue(indexPath, `sitemap leaks a root-only docs URL: ${location}`));
      }
      continue;
    }
    if (
      (logicalPath === "/docs" || logicalPath.startsWith("/docs/")) &&
      !expectedDocs.has(location)
    ) {
      issues.push(issue(indexPath, `sitemap contains unexpected documentation URL: ${location}`));
    }
  }
}

function inspectPhysicalMount(distRoot, outputRoot, siteLocation, issues) {
  if (siteLocation.basePath === "/") {
    return;
  }

  const controlFiles = new Set(["_headers", "_redirects"]);
  for (const name of controlFiles) {
    const controlPath = join(distRoot, name);
    if (!existsSync(controlPath)) {
      issues.push(issue(controlPath, "Cloudflare control file is missing from the asset root"));
      continue;
    }
    const state = lstatSync(controlPath);
    if (!state.isFile() || state.isSymbolicLink()) {
      issues.push(issue(controlPath, "Cloudflare control path must be a regular file"));
    }
  }

  const segments = siteLocation.basePath.slice(1).split("/");
  let ancestor = distRoot;
  for (const segment of segments) {
    const expectedEntries = new Set([
      segment,
      ...(ancestor === distRoot ? controlFiles : []),
    ]);
    for (const entry of readdirSync(ancestor, { withFileTypes: true })) {
      if (!expectedEntries.has(entry.name)) {
        issues.push(
          issue(
            join(ancestor, entry.name),
            `static output is not mounted exclusively beneath ${siteLocation.basePath}`,
          ),
        );
      }
    }

    const next = join(ancestor, segment);
    if (!existsSync(next)) {
      issues.push(issue(next, `physical SITE_URL mount is missing: ${siteLocation.basePath}`));
      return;
    }
    const state = lstatSync(next);
    if (!state.isDirectory() || state.isSymbolicLink()) {
      issues.push(issue(next, "physical SITE_URL mount must use real directories"));
      return;
    }
    ancestor = next;
  }

  if (resolve(ancestor) !== resolve(outputRoot)) {
    issues.push(issue(outputRoot, "physical SITE_URL mount resolved to the wrong directory"));
  }
  for (const name of controlFiles) {
    const nestedControlPath = join(outputRoot, name);
    if (existsSync(nestedControlPath)) {
      issues.push(
        issue(
          nestedControlPath,
          "Cloudflare control file must stay at the asset root, not inside the public mount",
        ),
      );
    }
  }
}

export function validateDocsBuild({
  siteUrl = process.env.SITE_URL,
  distDirectory = DEFAULT_DOCS_DIST_DIRECTORY,
  guidesDirectory = DEFAULT_PUBLIC_GUIDES_DIRECTORY,
  manifestPath,
  expectedDocuments,
} = {}) {
  const siteLocation = deriveSiteLocation(siteUrl);
  const distRoot = resolve(distDirectory);
  if (!existsSync(distRoot) || !lstatSync(distRoot).isDirectory()) {
    throw new Error(`dist directory does not exist: ${distRoot}`);
  }
  const outputRoot = siteOutputRoot(distRoot, siteLocation);

  const documents = loadExpectedDocuments({
    guidesDirectory,
    manifestPath,
    expectedDocuments,
  });
  const published = documents.filter((document) => !document.draft);
  const drafts = documents.filter((document) => document.draft);
  const pagerNeighbors = expectedPagerNeighbors(documents);
  const issues = [];
  const expectedPublishedRoutes = new Set(published.map((document) => document.route));
  const expectedDraftRoutes = new Set(drafts.map((document) => document.route));

  inspectPhysicalMount(distRoot, outputRoot, siteLocation, issues);

  for (const document of published) {
    const outputPath = routeOutputPath(outputRoot, document.route);
    if (!existsSync(outputPath)) {
      issues.push(issue(outputPath, `published documentation route is missing: ${document.route}`));
      continue;
    }
    inspectPage(
      document,
      readFileSync(outputPath, "utf8"),
      pagerNeighbors,
      siteLocation,
      outputPath,
      issues,
    );
  }

  for (const document of drafts) {
    const outputPath = routeOutputPath(outputRoot, document.route);
    if (existsSync(outputPath)) {
      issues.push(issue(outputPath, `draft documentation route was built: ${document.route}`));
    }
  }

  for (const route of collectBuiltDocRoutes(outputRoot)) {
    if (!expectedPublishedRoutes.has(route)) {
      const qualifier = expectedDraftRoutes.has(route) ? "draft" : "unexpected";
      issues.push(
        issue(
          routeOutputPath(outputRoot, route),
          `${qualifier} documentation route exists in built output: ${route}`,
        ),
      );
    }
  }

  inspectSitemaps(
    outputRoot,
    siteLocation,
    expectedCanonicalSet(documents, siteLocation, false),
    expectedCanonicalSet(documents, siteLocation, true),
    issues,
  );

  if (issues.length > 0) {
    throw new DocsBuildValidationError(issues);
  }

  return {
    siteUrl: new URL(siteUrl).href,
    basePath: siteLocation.basePath,
    distDirectory: distRoot,
    siteOutputDirectory: outputRoot,
    publishedRoutes: [...expectedPublishedRoutes].sort(),
    draftRoutes: [...expectedDraftRoutes].sort(),
  };
}

function usage() {
  return [
    "usage: node scripts/docs-build-validation.mjs [options]",
    "",
    "Options:",
    "  --site-url URL   Built SITE_URL (defaults to the SITE_URL environment variable)",
    "  --dist DIR       Astro dist directory",
    "  --guides DIR     Public guide source directory",
    "  --manifest FILE  Expected-document JSON manifest instead of reading guides",
    "  -h, --help       Show this help",
  ].join("\n");
}

export function parseCliArguments(argumentsList) {
  const options = {};
  for (let index = 0; index < argumentsList.length; index += 1) {
    const argument = argumentsList[index];
    if (argument === "-h" || argument === "--help") {
      return { help: true };
    }
    const key = {
      "--site-url": "siteUrl",
      "--dist": "distDirectory",
      "--guides": "guidesDirectory",
      "--manifest": "manifestPath",
    }[argument];
    if (!key) {
      throw new Error(`unknown argument: ${argument}\n${usage()}`);
    }
    const value = argumentsList[index + 1];
    if (!value || value.startsWith("--")) {
      throw new Error(`missing value for ${argument}\n${usage()}`);
    }
    options[key] = value;
    index += 1;
  }
  return options;
}

async function main() {
  const options = parseCliArguments(process.argv.slice(2));
  if (options.help) {
    console.log(usage());
    return;
  }
  const result = validateDocsBuild(options);
  console.log(
    `validated ${result.publishedRoutes.length} published documentation route(s) at base ${result.basePath}`,
  );
}

const invokedPath = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : null;
if (invokedPath === import.meta.url) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  });
}
