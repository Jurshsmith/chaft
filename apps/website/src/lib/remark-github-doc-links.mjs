import { realpathSync, statSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";

import { docIdFromEntry, docRoutePath } from "./docs.ts";

function isInside(root, candidate) {
  const pathFromRoot = relative(root, candidate);
  return pathFromRoot === "" || (!pathFromRoot.startsWith(`..${sep}`) && pathFromRoot !== ".." && !isAbsolute(pathFromRoot));
}

function normalizeBasePath(basePath) {
  if (typeof basePath !== "string" || !basePath.startsWith("/")) {
    throw new Error(`documentation base path must start with "/": ${basePath}`);
  }

  const normalized = basePath === "/" ? "" : basePath.replace(/\/+$/, "");
  if (
    normalized.includes("?") ||
    normalized.includes("#") ||
    normalized.split("/").some((segment) => segment === "." || segment === "..")
  ) {
    throw new Error(`invalid documentation base path: ${basePath}`);
  }
  return normalized;
}

function sourcePathFromFile(file) {
  if (typeof file?.path === "string" && file.path.length > 0) {
    return file.path;
  }
  if (Array.isArray(file?.history)) {
    const latest = file.history.at(-1);
    if (typeof latest === "string" && latest.length > 0) {
      return latest;
    }
  }
  throw new Error("could not determine the public guide source path");
}

function splitMarkdownUrl(url) {
  const fragmentIndex = url.indexOf("#");
  const rawPath = fragmentIndex === -1 ? url : url.slice(0, fragmentIndex);
  const fragment = fragmentIndex === -1 ? "" : url.slice(fragmentIndex);

  if (rawPath.includes("?")) {
    throw new Error(`public guide links cannot contain a query: ${url}`);
  }

  let decodedPath;
  try {
    decodedPath = decodeURIComponent(rawPath);
    if (fragment) {
      decodeURIComponent(fragment.slice(1));
    }
  } catch {
    throw new Error(`public guide link contains invalid URL encoding: ${url}`);
  }

  return { decodedPath, fragment };
}

export function rewritePublicGuideUrl(url, sourcePath, options) {
  if (
    typeof url !== "string" ||
    url.length === 0 ||
    url.startsWith("#") ||
    url.startsWith("//") ||
    /^[a-z][a-z\d+.-]*:/i.test(url)
  ) {
    return url;
  }

  const { decodedPath, fragment } = splitMarkdownUrl(url);
  if (!decodedPath.toLowerCase().endsWith(".md")) {
    return url;
  }
  if (decodedPath.startsWith("/") || decodedPath.includes("\0")) {
    throw new Error(`public guide Markdown links must be repository-relative: ${url}`);
  }

  const guidesRoot = realpathSync(options.guidesRoot);
  const realSourcePath = realpathSync(sourcePath);
  if (!isInside(guidesRoot, realSourcePath)) {
    throw new Error(`public guide source escapes guides/public: ${sourcePath}`);
  }

  const targetPath = resolve(dirname(realSourcePath), decodedPath);
  if (!isInside(guidesRoot, targetPath)) {
    throw new Error(`public guide link escapes guides/public: ${url}`);
  }

  let realTargetPath;
  try {
    realTargetPath = realpathSync(targetPath);
  } catch {
    throw new Error(`public guide link target does not exist: ${url}`);
  }
  if (!isInside(guidesRoot, realTargetPath) || !statSync(realTargetPath).isFile()) {
    throw new Error(`public guide link target is not a public guide file: ${url}`);
  }

  const entry = relative(guidesRoot, realTargetPath).split(sep).join("/");
  const route = docRoutePath(docIdFromEntry(entry));
  return `${normalizeBasePath(options.basePath)}${route}${fragment}`;
}

function visitLinks(node, transform) {
  if (!node || typeof node !== "object") {
    return;
  }
  if (
    (node.type === "link" || node.type === "definition") &&
    typeof node.url === "string"
  ) {
    node.url = transform(node.url);
  }
  if (Array.isArray(node.children)) {
    for (const child of node.children) {
      visitLinks(child, transform);
    }
  }
}

export default function remarkGitHubDocLinks(options) {
  if (!options || typeof options.guidesRoot !== "string") {
    throw new Error("remarkGitHubDocLinks requires an absolute guidesRoot");
  }
  normalizeBasePath(options.basePath);

  return (tree, file) => {
    const sourcePath = sourcePathFromFile(file);
    visitLinks(tree, (url) => rewritePublicGuideUrl(url, sourcePath, options));
  };
}
