import {
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
  realpathSync,
} from "node:fs";
import { dirname, extname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const GUIDE_FRONT_MATTER_KEYS = Object.freeze([
  "title",
  "navTitle",
  "description",
  "section",
  "order",
  "audience",
  "status",
  "draft",
]);

const REQUIRED_GUIDE_FRONT_MATTER_KEYS = Object.freeze([
  "title",
  "description",
  "section",
  "order",
  "audience",
  "status",
]);

export const GUIDE_SECTIONS = Object.freeze([
  "getting-started",
  "concepts",
  "development",
  "reference",
]);

export const GUIDE_AUDIENCES = Object.freeze(["users", "contributors", "operators"]);
export const GUIDE_STATUSES = Object.freeze(["canary", "stable", "deprecated"]);

const SAFE_SECRET_INPUT_SUFFIXES = Object.freeze([
  "-file",
  "-prompt",
  "-stdin",
]);

const REPOSITORY_PATH_PREFIXES = Object.freeze([
  ".github/",
  "application/",
  "apps/",
  "bindings/",
  "domain/",
  "guides/",
  "network/",
  "runtime/",
  "security/",
  "storage/",
  "tools/",
]);

const PRIVATE_REFERENCE_PATTERNS = Object.freeze([
  {
    pattern: /\bchaft-infra\b/i,
    message: "references the private infrastructure repository",
  },
  {
    pattern: /https?:\/\/dash\.cloudflare\.com(?:\/|$)/i,
    message: "contains a private Cloudflare dashboard URL",
  },
  {
    pattern: /(?:^|[\s("'`])\/(?:Users|home)\/[^/\s]+/m,
    message: "contains an absolute user-home path",
  },
  {
    pattern: /(?:^|[\s("'`])[A-Za-z]:\\Users\\[^\\\s]+/m,
    message: "contains an absolute Windows user-home path",
  },
]);

export class DocumentationValidationError extends Error {
  constructor(issues) {
    const normalizedIssues = [...issues].sort(compareIssues);
    super(
      [
        `Public documentation validation failed with ${normalizedIssues.length} issue(s):`,
        ...normalizedIssues.map(formatIssue),
      ].join("\n"),
    );
    this.name = "DocumentationValidationError";
    this.issues = normalizedIssues;
  }
}

function compareIssues(left, right) {
  return (
    left.file.localeCompare(right.file) ||
    (left.line ?? 0) - (right.line ?? 0) ||
    left.message.localeCompare(right.message)
  );
}

function formatIssue(issue) {
  const location = issue.line ? `${issue.file}:${issue.line}` : issue.file;
  return `- ${location}: ${issue.message}`;
}

function makeIssue(file, line, message) {
  return { file, line, message };
}

function displayPath(filePath, repositoryRoot) {
  const candidate = relative(repositoryRoot, filePath);
  return candidate && !candidate.startsWith(`..${sep}`) && candidate !== ".."
    ? candidate.split(sep).join("/")
    : filePath;
}

function lineCount(source) {
  const lines = source.replaceAll("\r\n", "\n").split("\n");
  return lines.at(-1) === "" ? lines.length - 1 : lines.length;
}

function isPathWithin(root, candidate) {
  const pathFromRoot = relative(root, candidate);
  return (
    pathFromRoot === "" ||
    (!pathFromRoot.startsWith(`..${sep}`) &&
      pathFromRoot !== ".." &&
      !isAbsolute(pathFromRoot))
  );
}

function findRootFrom(startDirectory) {
  let current = resolve(startDirectory);
  if (existsSync(current) && !lstatSync(current).isDirectory()) {
    current = dirname(current);
  }

  while (true) {
    if (
      existsSync(join(current, "README.md")) &&
      existsSync(join(current, "guides", "public"))
    ) {
      return current;
    }
    const parent = dirname(current);
    if (parent === current) {
      return null;
    }
    current = parent;
  }
}

export function findRepositoryRoot(startDirectory = process.cwd()) {
  const fromRequestedStart = findRootFrom(startDirectory);
  if (fromRequestedStart) {
    return fromRequestedStart;
  }

  const fromScript = findRootFrom(dirname(fileURLToPath(import.meta.url)));
  if (fromScript) {
    return fromScript;
  }

  throw new Error(
    `could not locate a repository containing README.md and guides/public from ${startDirectory}`,
  );
}

function parseStringScalar(rawValue, key, file, line, issues) {
  const value = rawValue.trim();
  if (value === "") {
    issues.push(makeIssue(file, line, `front matter "${key}" must not be empty`));
    return "";
  }

  if (value.startsWith('"')) {
    if (!value.endsWith('"') || value.length === 1) {
      issues.push(makeIssue(file, line, `front matter "${key}" has an invalid quoted string`));
      return "";
    }
    try {
      const parsed = JSON.parse(value);
      if (typeof parsed !== "string") {
        throw new TypeError("not a string");
      }
      return parsed;
    } catch {
      issues.push(makeIssue(file, line, `front matter "${key}" has an invalid quoted string`));
      return "";
    }
  }

  if (value.startsWith("'")) {
    if (!value.endsWith("'") || value.length === 1) {
      issues.push(makeIssue(file, line, `front matter "${key}" has an invalid quoted string`));
      return "";
    }
    return value.slice(1, -1).replaceAll("''", "'");
  }

  if (/^[\[\]{}>|&*!]/.test(value)) {
    issues.push(
      makeIssue(
        file,
        line,
        `front matter "${key}" must be a single-line plain or quoted string`,
      ),
    );
    return "";
  }
  return value;
}

export function parseFrontMatter(source, { file = "<guide>" } = {}) {
  const normalizedSource = source.replaceAll("\r\n", "\n");
  const lines = normalizedSource.split("\n");
  const issues = [];

  if (lines[0] !== "---") {
    throw new DocumentationValidationError([
      makeIssue(file, 1, 'must begin with an exact "---" front matter delimiter'),
    ]);
  }

  const closingIndex = lines.indexOf("---", 1);
  if (closingIndex === -1) {
    throw new DocumentationValidationError([
      makeIssue(file, 1, 'front matter is missing its closing "---" delimiter'),
    ]);
  }

  const values = new Map();
  const sourceLines = new Map();
  for (let index = 1; index < closingIndex; index += 1) {
    const line = lines[index];
    const lineNumber = index + 1;
    if (line.trim() === "") {
      issues.push(makeIssue(file, lineNumber, "front matter must not contain blank lines"));
      continue;
    }

    const match = /^([A-Za-z][A-Za-z0-9_-]*):(?:[ \t]+(.*))?$/.exec(line);
    if (!match) {
      issues.push(
        makeIssue(file, lineNumber, "front matter entries must use one-line `key: value` syntax"),
      );
      continue;
    }

    const [, key, rawValue = ""] = match;
    if (!GUIDE_FRONT_MATTER_KEYS.includes(key)) {
      issues.push(makeIssue(file, lineNumber, `unknown front matter key "${key}"`));
      continue;
    }
    if (values.has(key)) {
      issues.push(makeIssue(file, lineNumber, `duplicate front matter key "${key}"`));
      continue;
    }
    values.set(key, rawValue);
    sourceLines.set(key, lineNumber);
  }

  for (const key of REQUIRED_GUIDE_FRONT_MATTER_KEYS) {
    if (!values.has(key)) {
      issues.push(makeIssue(file, 1, `missing required front matter key "${key}"`));
    }
  }

  const data = {};
  for (const key of ["title", "navTitle", "description", "section", "audience", "status"]) {
    if (values.has(key)) {
      data[key] = parseStringScalar(
        values.get(key),
        key,
        file,
        sourceLines.get(key),
        issues,
      );
    }
  }
  data.draft = false;

  if (values.has("order")) {
    const value = values.get("order").trim();
    if (!/^(?:0|[1-9][0-9]*)$/.test(value) || !Number.isSafeInteger(Number(value))) {
      issues.push(
        makeIssue(
          file,
          sourceLines.get("order"),
          'front matter "order" must be a non-negative safe integer',
        ),
      );
    } else {
      data.order = Number(value);
    }
  }

  if (values.has("draft")) {
    const value = values.get("draft").trim();
    if (value !== "true" && value !== "false") {
      issues.push(
        makeIssue(
          file,
          sourceLines.get("draft"),
          'front matter "draft" must be the unquoted boolean true or false',
        ),
      );
    } else {
      data.draft = value === "true";
    }
  }

  for (const [key, allowed] of [
    ["section", GUIDE_SECTIONS],
    ["audience", GUIDE_AUDIENCES],
    ["status", GUIDE_STATUSES],
  ]) {
    if (data[key] !== undefined && !allowed.includes(data[key])) {
      issues.push(
        makeIssue(
          file,
          sourceLines.get(key),
          `front matter "${key}" must be one of: ${allowed.join(", ")}`,
        ),
      );
    }
  }

  if (data.title !== undefined && data.title.trim() === "") {
    issues.push(makeIssue(file, sourceLines.get("title"), 'front matter "title" is required'));
  }
  if (data.description !== undefined && data.description.trim() === "") {
    issues.push(
      makeIssue(file, sourceLines.get("description"), 'front matter "description" is required'),
    );
  }

  if (issues.length > 0) {
    throw new DocumentationValidationError(issues);
  }

  return {
    data,
    body: lines.slice(closingIndex + 1).join("\n"),
    bodyStartLine: closingIndex + 2,
  };
}

function decodeHtmlEntities(value) {
  const named = new Map([
    ["amp", "&"],
    ["apos", "'"],
    ["gt", ">"],
    ["lt", "<"],
    ["quot", '"'],
  ]);
  return value.replace(/&(?:#([0-9]+)|#x([0-9a-f]+)|([a-z]+));/gi, (match, decimal, hex, name) => {
    if (decimal) {
      return String.fromCodePoint(Number.parseInt(decimal, 10));
    }
    if (hex) {
      return String.fromCodePoint(Number.parseInt(hex, 16));
    }
    return named.get(name.toLowerCase()) ?? match;
  });
}

function headingPlainText(value) {
  return decodeHtmlEntities(
    value
      .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
      .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
      .replace(/\[([^\]]+)\]\[[^\]]*\]/g, "$1")
      .replace(/<[^>]*>/g, "")
      .replace(/`+([^`]*)`+/g, "$1")
      .replace(/[*~_]/g, "")
      .replace(/\\([\\`*_[\]{}()#+.!-])/g, "$1"),
  ).trim();
}

export function githubSlug(value) {
  return headingPlainText(value)
    .toLocaleLowerCase("en-US")
    .replace(/[^\p{L}\p{M}\p{N}\p{Pc}\s-]/gu, "")
    .trim()
    .replace(/\s/gu, "-");
}

function uniqueHeadingSlugs(headings) {
  const used = new Set();
  return headings.map((heading) => {
    const base = githubSlug(heading.text);
    let slug = base;
    let duplicate = 0;
    while (used.has(slug)) {
      duplicate += 1;
      slug = `${base}-${duplicate}`;
    }
    used.add(slug);
    return { ...heading, slug };
  });
}

function fenceMarker(line) {
  const match = /^\s{0,3}(`{3,}|~{3,})/.exec(line);
  return match?.[1] ?? null;
}

export function extractHeadings(markdown, { lineOffset = 0 } = {}) {
  const lines = markdown.replaceAll("\r\n", "\n").split("\n");
  const headings = [];
  let activeFence = null;

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const marker = fenceMarker(line);
    if (marker) {
      if (!activeFence) {
        activeFence = marker;
      } else if (
        marker[0] === activeFence[0] &&
        marker.length >= activeFence.length
      ) {
        activeFence = null;
      }
      continue;
    }
    if (activeFence) {
      continue;
    }

    const atx = /^\s{0,3}(#{1,6})(?:[ \t]+|$)(.*)$/.exec(line);
    if (atx) {
      const rawText = atx[2].replace(/[ \t]+#+[ \t]*$/, "");
      headings.push({
        depth: atx[1].length,
        text: headingPlainText(rawText),
        line: lineOffset + index + 1,
      });
      continue;
    }

    if (
      index + 1 < lines.length &&
      line.trim() !== "" &&
      !/^\s{0,3}(?:>|[-+*]\s|\d+[.)]\s)/.test(line)
    ) {
      const setext = /^\s{0,3}(=+|-+)\s*$/.exec(lines[index + 1]);
      if (setext) {
        headings.push({
          depth: setext[1][0] === "=" ? 1 : 2,
          text: headingPlainText(line),
          line: lineOffset + index + 1,
        });
        index += 1;
      }
    }
  }

  return uniqueHeadingSlugs(headings);
}

function parseInlineLinkTarget(contents) {
  const trimmed = contents.trim();
  if (trimmed.startsWith("<")) {
    const closing = trimmed.indexOf(">");
    return closing === -1 ? null : trimmed.slice(1, closing);
  }

  let escaped = false;
  for (let index = 0; index < trimmed.length; index += 1) {
    const character = trimmed[index];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (character === "\\") {
      escaped = true;
      continue;
    }
    if (/\s/.test(character)) {
      return trimmed.slice(0, index);
    }
  }
  return trimmed || null;
}

function maskInlineCode(line) {
  return line.replace(/(`+)(.*?)\1/g, (match) => " ".repeat(match.length));
}

export function extractMarkdownLinks(markdown, { lineOffset = 0 } = {}) {
  const lines = markdown.replaceAll("\r\n", "\n").split("\n");
  const links = [];
  let activeFence = null;

  for (let index = 0; index < lines.length; index += 1) {
    const originalLine = lines[index];
    const marker = fenceMarker(originalLine);
    if (marker) {
      if (!activeFence) {
        activeFence = marker;
      } else if (
        marker[0] === activeFence[0] &&
        marker.length >= activeFence.length
      ) {
        activeFence = null;
      }
      continue;
    }
    if (activeFence) {
      continue;
    }

    const line = maskInlineCode(originalLine);
    const definition = /^\s{0,3}\[([^\]]+)\]:\s*(<[^>]+>|\S+)/.exec(line);
    if (definition) {
      links.push({
        label: definition[1],
        target: definition[2].replace(/^<|>$/g, ""),
        line: lineOffset + index + 1,
      });
    }

    let cursor = 0;
    while (cursor < line.length) {
      const closeLabel = line.indexOf("](", cursor);
      if (closeLabel === -1) {
        break;
      }
      const openLabel = line.lastIndexOf("[", closeLabel);
      if (openLabel === -1 || line[openLabel - 1] === "\\") {
        cursor = closeLabel + 2;
        continue;
      }

      let depth = 1;
      let escaped = false;
      let closing = closeLabel + 2;
      for (; closing < line.length; closing += 1) {
        const character = line[closing];
        if (escaped) {
          escaped = false;
        } else if (character === "\\") {
          escaped = true;
        } else if (character === "(") {
          depth += 1;
        } else if (character === ")") {
          depth -= 1;
          if (depth === 0) {
            break;
          }
        }
      }

      if (depth !== 0) {
        break;
      }

      const target = parseInlineLinkTarget(line.slice(closeLabel + 2, closing));
      if (target) {
        links.push({
          label: line.slice(openLabel + 1, closeLabel),
          target,
          line: lineOffset + index + 1,
        });
      }
      cursor = closing + 1;
    }
  }

  return links;
}

export function docIdFromPath(relativePath) {
  const normalized = relativePath.split("\\").join("/").replace(/^(?:\.\/)+/, "");
  if (
    !normalized.endsWith(".md") ||
    normalized.startsWith("/") ||
    normalized.split("/").some((segment) => segment === "" || segment === "." || segment === "..")
  ) {
    throw new Error(`invalid public guide path: ${relativePath}`);
  }

  const withoutExtension = normalized.slice(0, -3);
  for (const segment of withoutExtension.split("/")) {
    if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(segment)) {
      throw new Error(`public guide path segment must be a lowercase URL slug: ${segment}`);
    }
  }

  if (withoutExtension === "index") {
    return "index";
  }
  return withoutExtension;
}

export function docRouteFromId(id) {
  if (id === "index") {
    return "/docs/";
  }
  if (
    !id ||
    id.startsWith("/") ||
    id.endsWith("/") ||
    id.includes("\\") ||
    id.split("/").some((segment) => !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(segment))
  ) {
    throw new Error(`invalid public guide ID: ${id}`);
  }
  return id.endsWith("/index")
    ? `/docs/${id.slice(0, -"/index".length)}/`
    : `/docs/${id}/`;
}

function collectMarkdownFiles(directory, repositoryRoot, issues, files = []) {
  for (const entry of readdirSync(directory, { withFileTypes: true }).sort((a, b) =>
    a.name.localeCompare(b.name),
  )) {
    const entryPath = join(directory, entry.name);
    const file = displayPath(entryPath, repositoryRoot);
    if (entry.isSymbolicLink()) {
      issues.push(makeIssue(file, 0, "symbolic links are not allowed in public guides"));
    } else if (entry.isDirectory()) {
      collectMarkdownFiles(entryPath, repositoryRoot, issues, files);
    } else if (entry.isFile() && extname(entry.name).toLowerCase() === ".md") {
      files.push(entryPath);
    }
  }
  return files;
}

function splitLinkTarget(rawTarget) {
  const hashIndex = rawTarget.indexOf("#");
  const beforeFragment = hashIndex === -1 ? rawTarget : rawTarget.slice(0, hashIndex);
  const fragment = hashIndex === -1 ? null : rawTarget.slice(hashIndex + 1);
  const queryIndex = beforeFragment.indexOf("?");
  return {
    path: queryIndex === -1 ? beforeFragment : beforeFragment.slice(0, queryIndex),
    query: queryIndex === -1 ? null : beforeFragment.slice(queryIndex + 1),
    fragment,
  };
}

function decodeUrlPart(value, file, line, kind, issues) {
  if (value === null) {
    return null;
  }
  try {
    return decodeURIComponent(value).replaceAll("\\", "/");
  } catch {
    issues.push(makeIssue(file, line, `link has an invalid percent-encoded ${kind}`));
    return null;
  }
}

function isExternalTarget(target) {
  return (
    target.startsWith("//") ||
    /^[A-Za-z][A-Za-z0-9+.-]*:/.test(target)
  );
}

function validateExternalTarget(target, file, line, issues) {
  if (/^(?:javascript|data|file):/i.test(target)) {
    issues.push(makeIssue(file, line, `unsafe or local URL scheme is not allowed: ${target}`));
  }
}

function headingsForFile(filePath, headingCache) {
  if (!headingCache.has(filePath)) {
    headingCache.set(filePath, extractHeadings(readFileSync(filePath, "utf8")));
  }
  return headingCache.get(filePath);
}

function validateFragment(
  targetFile,
  rawFragment,
  file,
  line,
  issues,
  headingCache,
  repositoryRoot,
) {
  const fragment = decodeUrlPart(rawFragment, file, line, "fragment", issues);
  if (fragment === null) {
    return;
  }
  if (fragment === "") {
    issues.push(makeIssue(file, line, "link fragment must not be empty"));
    return;
  }
  if (extname(targetFile).toLowerCase() !== ".md") {
    issues.push(makeIssue(file, line, "heading fragments may only target Markdown files"));
    return;
  }

  const slugs = new Set(headingsForFile(targetFile, headingCache).map((heading) => heading.slug));
  if (!slugs.has(fragment)) {
    issues.push(
      makeIssue(
        file,
        line,
        `heading fragment "#${fragment}" does not exist in ${displayPath(
          targetFile,
          repositoryRoot,
        )}`,
      ),
    );
  }
}

function validateLocalLink({
  link,
  sourcePath,
  boundary,
  repositoryRoot,
  guidePaths,
  headingCache,
  issues,
  requireMarkdownWithinBoundary,
}) {
  const sourceFile = displayPath(sourcePath, repositoryRoot);
  const target = link.target.replace(/\\([\\()[\] ])/g, "$1");
  if (isExternalTarget(target)) {
    validateExternalTarget(target, sourceFile, link.line, issues);
    return;
  }
  if (target.startsWith("/")) {
    issues.push(makeIssue(sourceFile, link.line, `local links must be relative: ${target}`));
    return;
  }

  const parts = splitLinkTarget(target);
  if (parts.query !== null) {
    issues.push(makeIssue(sourceFile, link.line, `local links must not contain a query: ${target}`));
    return;
  }
  const decodedPath = decodeUrlPart(parts.path, sourceFile, link.line, "path", issues);
  if (decodedPath === null) {
    return;
  }

  const targetPath = decodedPath === "" ? sourcePath : resolve(dirname(sourcePath), decodedPath);
  if (
    requireMarkdownWithinBoundary &&
    extname(decodedPath).toLowerCase() === ".md" &&
    !isPathWithin(boundary, targetPath)
  ) {
    issues.push(
      makeIssue(
        sourceFile,
        link.line,
        `local Markdown target escapes guides/public: ${link.target}`,
      ),
    );
    return;
  }

  if (!isPathWithin(repositoryRoot, targetPath)) {
    issues.push(makeIssue(sourceFile, link.line, `local link escapes the repository: ${link.target}`));
    return;
  }
  if (!existsSync(targetPath)) {
    issues.push(makeIssue(sourceFile, link.line, `local link target does not exist: ${link.target}`));
    return;
  }

  const realRepositoryRoot = realpathSync(repositoryRoot);
  const realTargetPath = realpathSync(targetPath);
  if (!isPathWithin(realRepositoryRoot, realTargetPath)) {
    issues.push(
      makeIssue(sourceFile, link.line, `local link resolves outside the repository: ${link.target}`),
    );
    return;
  }
  if (
    requireMarkdownWithinBoundary &&
    extname(targetPath).toLowerCase() === ".md" &&
    guidePaths &&
    !guidePaths.has(resolve(targetPath))
  ) {
    issues.push(
      makeIssue(sourceFile, link.line, `local Markdown target is not a public guide: ${link.target}`),
    );
    return;
  }

  if (parts.fragment !== null) {
    validateFragment(
      resolve(targetPath),
      parts.fragment,
      sourceFile,
      link.line,
      issues,
      headingCache,
      repositoryRoot,
    );
  }
}

function collectPrivateReferenceIssues(source, file) {
  const issues = [];
  const lines = source.replaceAll("\r\n", "\n").split("\n");
  for (let index = 0; index < lines.length; index += 1) {
    for (const { pattern, message } of PRIVATE_REFERENCE_PATTERNS) {
      pattern.lastIndex = 0;
      if (pattern.test(lines[index])) {
        issues.push(makeIssue(file, index + 1, message));
      }
    }
  }
  return issues;
}

function extractCodeSamples(markdown) {
  const lines = markdown.replaceAll("\r\n", "\n").split("\n");
  const samples = [];
  let activeFence = null;
  let fenceStart = 0;
  let fencedLines = [];

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const marker = fenceMarker(line);
    if (marker) {
      if (!activeFence) {
        activeFence = marker;
        fenceStart = index + 2;
        fencedLines = [];
      } else if (
        marker[0] === activeFence[0] &&
        marker.length >= activeFence.length
      ) {
        samples.push({ source: fencedLines.join("\n"), line: fenceStart });
        activeFence = null;
        fencedLines = [];
      } else {
        fencedLines.push(line);
      }
      continue;
    }
    if (activeFence) {
      fencedLines.push(line);
      continue;
    }

    for (const match of line.matchAll(/(`+)(.*?)\1/g)) {
      samples.push({ source: match[2], line: index + 1 });
    }
  }

  return samples;
}

function secretFlagName(flag) {
  const normalized = flag.toLowerCase();
  if (SAFE_SECRET_INPUT_SUFFIXES.some((suffix) => normalized.endsWith(suffix))) {
    return null;
  }
  return /(?:passphrase|password|secret|token|api-key|apikey)/.test(normalized)
    ? normalized
    : null;
}

function collectSecretArgumentIssues(source, file) {
  const issues = [];
  for (const sample of extractCodeSamples(source)) {
    const argumentPattern =
      /(^|[\s\\])(--[a-z0-9][a-z0-9-]*)(?:=(?:"[^"\n]*"|'[^'\n]*'|[^\s\\]+)|(?:[ \t]+|[ \t]*\\[ \t]*\n[ \t]*)(?:"[^"\n]*"|'[^'\n]*'|[^\s\\]+))/gim;
    for (const match of sample.source.matchAll(argumentPattern)) {
      const flag = secretFlagName(match[2]);
      if (flag) {
        const precedingLines = sample.source.slice(0, match.index).match(/\n/g)?.length ?? 0;
        issues.push(
          makeIssue(
            file,
            sample.line + precedingLines,
            `secret-bearing value must not be passed directly through ${flag}`,
          ),
        );
      }
    }
  }
  return issues;
}

function parseMakeTargets(repositoryRoot) {
  const makefilePath = join(repositoryRoot, "Makefile");
  if (!existsSync(makefilePath)) {
    return new Set();
  }
  const targets = new Set();
  for (const line of readFileSync(makefilePath, "utf8").split(/\r?\n/)) {
    const match = /^([A-Za-z0-9][A-Za-z0-9_.%/-]*)(?:\s+[^:]*)?:/.exec(line);
    if (match && !match[1].includes("%")) {
      targets.add(match[1]);
    }
  }
  return targets;
}

function cleanRepositoryPathToken(token) {
  return token
    .replace(/^["'`(<]+/, "")
    .replace(/["'`)>,.:;]+$/, "")
    .replace(/^\.\/+/, "")
    .replace(/[*?].*$/, "");
}

function looksLikeRepositoryPath(token) {
  return (
    REPOSITORY_PATH_PREFIXES.some((prefix) => token.startsWith(prefix)) ||
    ["Cargo.toml", "CONTRIBUTING.md", "LICENSE", "Makefile", "README.md", "SECURITY.md"].includes(
      token,
    )
  );
}

function collectReadmeRepositoryReferenceIssues(source, repositoryRoot, file) {
  const issues = [];
  const checked = new Set();
  const makeTargets = parseMakeTargets(repositoryRoot);

  const checkPath = (rawPath, line) => {
    const repositoryPath = cleanRepositoryPathToken(rawPath);
    if (
      !repositoryPath ||
      !looksLikeRepositoryPath(repositoryPath) ||
      repositoryPath.includes("${") ||
      repositoryPath.includes("$(")
    ) {
      return;
    }
    const key = `${line}:${repositoryPath}`;
    if (checked.has(key)) {
      return;
    }
    checked.add(key);
    if (!existsSync(join(repositoryRoot, repositoryPath))) {
      issues.push(
        makeIssue(file, line, `referenced repository path does not exist: ${repositoryPath}`),
      );
    }
  };

  const checkMakeTargets = (line, lineNumber) => {
    for (const match of line.matchAll(/(?:^|[\s;&|])make(?=[ \t])/g)) {
      const tokens =
        line
          .slice(match.index + match[0].length)
          .match(/"[^"]*"|'[^']*'|[^\s;&|]+/g) ?? [];
      let skipNext = false;
      for (const token of tokens) {
        if (skipNext) {
          skipNext = false;
          continue;
        }
        if (token === "-C" || token === "--directory") {
          skipNext = true;
          continue;
        }
        if (token.startsWith("-") || /^[A-Za-z_][A-Za-z0-9_]*=/.test(token)) {
          continue;
        }
        if (!makeTargets.has(token)) {
          issues.push(
            makeIssue(file, lineNumber, `referenced Make target does not exist: ${token}`),
          );
        }
        break;
      }
    }
  };

  const lines = source.replaceAll("\r\n", "\n").split("\n");
  let activeFence = null;
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const marker = fenceMarker(line);
    if (marker) {
      activeFence = activeFence ? null : marker;
      continue;
    }

    for (const inline of line.matchAll(/`([^`\n]+)`/g)) {
      checkPath(inline[1], index + 1);
    }
    if (!activeFence) {
      continue;
    }

    const mapPath = /^\s*([A-Za-z0-9._/-]+\/)\s{2,}\S/.exec(line);
    if (mapPath) {
      checkPath(mapPath[1], index + 1);
    }
    for (const token of line.split(/\s+/)) {
      checkPath(token, index + 1);
    }

    checkMakeTargets(line, index + 1);
  }

  return issues;
}

function compareGuides(left, right) {
  return (
    GUIDE_SECTIONS.indexOf(left.data.section) - GUIDE_SECTIONS.indexOf(right.data.section) ||
    left.data.order - right.data.order ||
    left.id.localeCompare(right.id)
  );
}

export function buildPublishedPager(guides) {
  const published = guides
    .filter((guide) => !guide.data.draft)
    .slice()
    .sort(compareGuides);
  const pagerGuides = published.filter((guide) => guide.id !== "index");

  return published.map((guide) => {
    const pagerIndex = pagerGuides.findIndex((candidate) => candidate.id === guide.id);
    const previous = pagerIndex > 0 ? pagerGuides[pagerIndex - 1] : null;
    const next =
      pagerIndex !== -1 && pagerIndex < pagerGuides.length - 1
        ? pagerGuides[pagerIndex + 1]
        : null;
    return {
      id: guide.id,
      route: guide.route,
      title: guide.data.title,
      navTitle: guide.data.navTitle,
      section: guide.data.section,
      order: guide.data.order,
      previous: previous
        ? { id: previous.id, route: previous.route, title: previous.data.title }
        : null,
      next: next ? { id: next.id, route: next.route, title: next.data.title } : null,
    };
  });
}

export function verifyPagerOrder(pages) {
  const issues = [];
  const ids = new Set();
  const routes = new Set();
  const expectedPages = pages.slice().sort((left, right) => {
    return (
      GUIDE_SECTIONS.indexOf(left.section) - GUIDE_SECTIONS.indexOf(right.section) ||
      left.order - right.order ||
      left.id.localeCompare(right.id)
    );
  });
  const pagerPages = expectedPages.filter((page) => page.id !== "index");
  for (let index = 0; index < pages.length; index += 1) {
    const page = pages[index];
    if (ids.has(page.id)) {
      issues.push(makeIssue("<pager>", 0, `duplicate published page ID "${page.id}"`));
    }
    if (routes.has(page.route)) {
      issues.push(makeIssue("<pager>", 0, `duplicate published route "${page.route}"`));
    }
    ids.add(page.id);
    routes.add(page.route);
    if (expectedPages[index]?.id !== page.id) {
      issues.push(makeIssue("<pager>", 0, `published page "${page.id}" is out of order`));
    }

    const pagerIndex = pagerPages.findIndex((candidate) => candidate.id === page.id);
    const expectedPrevious =
      pagerIndex > 0 ? (pagerPages[pagerIndex - 1]?.id ?? null) : null;
    const expectedNext =
      pagerIndex !== -1 ? (pagerPages[pagerIndex + 1]?.id ?? null) : null;
    if ((page.previous?.id ?? null) !== expectedPrevious) {
      issues.push(makeIssue("<pager>", 0, `invalid previous page for "${page.id}"`));
    }
    if ((page.next?.id ?? null) !== expectedNext) {
      issues.push(makeIssue("<pager>", 0, `invalid next page for "${page.id}"`));
    }
  }
  return issues;
}

function readGuide(filePath, guidesRoot, repositoryRoot, issues) {
  const file = displayPath(filePath, repositoryRoot);
  const source = readFileSync(filePath, "utf8");
  let parsed;
  try {
    parsed = parseFrontMatter(source, { file });
  } catch (error) {
    if (error instanceof DocumentationValidationError) {
      issues.push(...error.issues);
      return null;
    }
    throw error;
  }

  const relativePath = relative(guidesRoot, filePath).split(sep).join("/");
  let id;
  let route;
  try {
    id = docIdFromPath(relativePath);
    route = docRouteFromId(id);
  } catch (error) {
    issues.push(makeIssue(file, 0, error.message));
    return null;
  }

  const headings = extractHeadings(parsed.body, {
    lineOffset: parsed.bodyStartLine - 1,
  });
  const h1Headings = headings.filter((heading) => heading.depth === 1);
  if (h1Headings.length !== 1) {
    issues.push(
      makeIssue(
        file,
        0,
        `must contain exactly one level-one heading; found ${h1Headings.length}`,
      ),
    );
  }

  issues.push(...collectPrivateReferenceIssues(source, file));
  issues.push(...collectSecretArgumentIssues(source, file));

  return {
    file,
    filePath,
    relativePath,
    id,
    route,
    data: parsed.data,
    body: parsed.body,
    bodyStartLine: parsed.bodyStartLine,
    headings,
    links: extractMarkdownLinks(parsed.body, {
      lineOffset: parsed.bodyStartLine - 1,
    }),
  };
}

export function validateGuideCollection({ repositoryRoot, guidesRoot }) {
  const issues = [];
  const markdownPaths = collectMarkdownFiles(guidesRoot, repositoryRoot, issues);
  const guides = markdownPaths
    .map((filePath) => readGuide(filePath, guidesRoot, repositoryRoot, issues))
    .filter(Boolean);

  if (!guides.some((guide) => guide.relativePath === "index.md")) {
    issues.push(makeIssue("guides/public/index.md", 0, "the public guide index is required"));
  }

  for (const [property, label] of [
    ["id", "ID"],
    ["route", "route"],
  ]) {
    const seen = new Map();
    for (const guide of guides) {
      const first = seen.get(guide[property]);
      if (first) {
        issues.push(
          makeIssue(
            guide.file,
            0,
            `duplicate guide ${label} "${guide[property]}" also used by ${first.file}`,
          ),
        );
      } else {
        seen.set(guide[property], guide);
      }
    }
  }

  const positions = new Map();
  for (const guide of guides) {
    const position = `${guide.data.section}:${guide.data.order}`;
    const first = positions.get(position);
    if (first) {
      issues.push(
        makeIssue(
          guide.file,
          0,
          `duplicate section order ${position} also used by ${first.file}`,
        ),
      );
    } else {
      positions.set(position, guide);
    }
  }

  const guidePaths = new Set(guides.map((guide) => resolve(guide.filePath)));
  const headingCache = new Map(
    guides.map((guide) => [resolve(guide.filePath), guide.headings]),
  );
  for (const guide of guides) {
    for (const link of guide.links) {
      validateLocalLink({
        link,
        sourcePath: guide.filePath,
        boundary: guidesRoot,
        repositoryRoot,
        guidePaths,
        headingCache,
        issues,
        requireMarkdownWithinBoundary: true,
      });
    }
  }

  const pages = buildPublishedPager(guides);
  issues.push(...verifyPagerOrder(pages));
  if (pages.length > 0 && pages[0].id !== "index") {
    issues.push(
      makeIssue(
        "guides/public/index.md",
        0,
        "the published guide index must be the first pager entry",
      ),
    );
  }
  if (!pages.some((page) => page.id === "index")) {
    issues.push(makeIssue("guides/public/index.md", 0, "the public guide index must be published"));
  }

  return { guides, pages, issues };
}

function normalizedLocalLinkPath(target) {
  if (isExternalTarget(target) || target.startsWith("/")) {
    return null;
  }
  const { path: targetPath } = splitLinkTarget(target);
  try {
    return decodeURIComponent(targetPath).replaceAll("\\", "/").replace(/^(?:\.\/)+/, "");
  } catch {
    return null;
  }
}

export function validateReadme({ repositoryRoot, readmePath }) {
  const source = readFileSync(readmePath, "utf8");
  const file = displayPath(readmePath, repositoryRoot);
  const issues = [];
  const readmeLineCount = lineCount(source);
  if (readmeLineCount < 200 || readmeLineCount > 300) {
    issues.push(
      makeIssue(file, 0, `must contain 200-300 lines; found ${readmeLineCount}`),
    );
  }

  issues.push(...collectPrivateReferenceIssues(source, file));
  issues.push(...collectSecretArgumentIssues(source, file));

  const links = extractMarkdownLinks(source);
  const localPaths = new Set(
    links.map((link) => normalizedLocalLinkPath(link.target)).filter(Boolean),
  );
  for (const required of ["SECURITY.md", "CONTRIBUTING.md", "guides/public/index.md"]) {
    if (!localPaths.has(required)) {
      issues.push(makeIssue(file, 0, `missing required link to ${required}`));
    }
  }
  if (
    !links.some((link) =>
      /^https:\/\/github\.com\/[^/]+\/[^/]+\/releases\/?(?:[#?].*)?$/.test(link.target),
    )
  ) {
    issues.push(makeIssue(file, 0, "missing required GitHub Releases link"));
  }

  const headingCache = new Map([[resolve(readmePath), extractHeadings(source)]]);
  for (const link of links) {
    validateLocalLink({
      link,
      sourcePath: readmePath,
      boundary: repositoryRoot,
      repositoryRoot,
      guidePaths: null,
      headingCache,
      issues,
      requireMarkdownWithinBoundary: false,
    });
  }
  issues.push(...collectReadmeRepositoryReferenceIssues(source, repositoryRoot, file));

  return { source, links, lineCount: readmeLineCount, issues };
}

export function validatePolicyDocument({ repositoryRoot, policyPath }) {
  const file = displayPath(policyPath, repositoryRoot);
  const issues = [];
  if (!existsSync(policyPath)) {
    issues.push(makeIssue(file, 0, "required policy document does not exist"));
    return {
      file,
      filePath: policyPath,
      exists: false,
      regularFile: false,
      linkCount: 0,
      issues,
    };
  }
  if (!lstatSync(policyPath).isFile()) {
    issues.push(makeIssue(file, 0, "required policy document must be a regular file"));
    return {
      file,
      filePath: policyPath,
      exists: true,
      regularFile: false,
      linkCount: 0,
      issues,
    };
  }

  const source = readFileSync(policyPath, "utf8");
  const links = extractMarkdownLinks(source);
  issues.push(...collectPrivateReferenceIssues(source, file));
  issues.push(...collectSecretArgumentIssues(source, file));

  const headingCache = new Map([[resolve(policyPath), extractHeadings(source)]]);
  for (const link of links) {
    validateLocalLink({
      link,
      sourcePath: policyPath,
      boundary: repositoryRoot,
      repositoryRoot,
      guidePaths: null,
      headingCache,
      issues,
      requireMarkdownWithinBoundary: false,
    });
  }

  return {
    file,
    filePath: policyPath,
    exists: true,
    regularFile: true,
    linkCount: links.length,
    source,
    links,
    issues,
  };
}

export function collectDocumentationValidation(options = {}) {
  const repositoryRoot = options.repositoryRoot
    ? resolve(options.repositoryRoot)
    : findRepositoryRoot(options.startDirectory);
  const guidesRoot = resolve(options.guidesRoot ?? join(repositoryRoot, "guides", "public"));
  const readmePath = resolve(options.readmePath ?? join(repositoryRoot, "README.md"));
  const securityPath = resolve(options.securityPath ?? join(repositoryRoot, "SECURITY.md"));
  const contributingPath = resolve(
    options.contributingPath ?? join(repositoryRoot, "CONTRIBUTING.md"),
  );

  if (!existsSync(guidesRoot) || !lstatSync(guidesRoot).isDirectory()) {
    throw new Error(`public guide directory does not exist: ${guidesRoot}`);
  }
  if (!existsSync(readmePath) || !lstatSync(readmePath).isFile()) {
    throw new Error(`README does not exist: ${readmePath}`);
  }

  const guideResult = validateGuideCollection({
    repositoryRoot,
    guidesRoot,
  });
  const readmeResult = validateReadme({
    repositoryRoot,
    readmePath,
  });
  const policyDocuments = [
    validatePolicyDocument({ repositoryRoot, policyPath: securityPath }),
    validatePolicyDocument({ repositoryRoot, policyPath: contributingPath }),
  ];

  return {
    repositoryRoot,
    guidesRoot,
    readmePath,
    guides: guideResult.guides,
    pages: guideResult.pages,
    readmeLineCount: readmeResult.lineCount,
    policyDocuments,
    issues: [
      ...guideResult.issues,
      ...readmeResult.issues,
      ...policyDocuments.flatMap((document) => document.issues),
    ].sort(compareIssues),
  };
}

export function validateDocumentation(options = {}) {
  const result = collectDocumentationValidation(options);
  if (result.issues.length > 0) {
    throw new DocumentationValidationError(result.issues);
  }
  return result;
}

export function runCli(argv = process.argv.slice(2)) {
  if (argv.length > 1) {
    throw new Error("usage: node apps/website/scripts/docs-validation.mjs [repository-root]");
  }
  const result = validateDocumentation(
    argv[0] ? { repositoryRoot: argv[0] } : { startDirectory: process.cwd() },
  );
  process.stdout.write(
    `Validated ${result.guides.length} public guide(s), ${
      result.pages.length
    } published route(s), a ${result.readmeLineCount}-line README, and ${
      result.policyDocuments.length
    } policy document(s).\n`,
  );
  return result;
}

const invokedAsScript =
  process.argv[1] &&
  pathToFileURL(resolve(process.argv[1])).href === pathToFileURL(fileURLToPath(import.meta.url)).href;

if (invokedAsScript) {
  try {
    runCli();
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
