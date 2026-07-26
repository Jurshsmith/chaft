export const DOC_SECTION_ORDER = [
  "getting-started",
  "concepts",
  "development",
  "reference",
] as const;

export type DocSection = (typeof DOC_SECTION_ORDER)[number];
export type DocAudience = "users" | "contributors" | "operators";
export type DocStatus = "canary" | "stable" | "deprecated";

export const DOC_SECTION_LABELS: Record<DocSection, string> = {
  "getting-started": "Getting started",
  concepts: "Concepts",
  development: "Development",
  reference: "Reference",
};

export interface DocNavigationData {
  title: string;
  description: string;
  section: DocSection;
  order: number;
  audience: DocAudience;
  status: DocStatus;
  draft: boolean;
  navTitle?: string | undefined;
}

export interface DocNavigationEntry {
  id: string;
  data: DocNavigationData;
}

export interface DocSectionGroup<T extends DocNavigationEntry> {
  section: DocSection;
  label: string;
  entries: T[];
}

export interface DocNeighbors<T extends DocNavigationEntry> {
  previous?: T | undefined;
  next?: T | undefined;
}

const sectionPosition = new Map(
  DOC_SECTION_ORDER.map((section, index) => [section, index] as const),
);

function compareDocs(a: DocNavigationEntry, b: DocNavigationEntry): number {
  return (
    (sectionPosition.get(a.data.section) ?? Number.MAX_SAFE_INTEGER) -
      (sectionPosition.get(b.data.section) ?? Number.MAX_SAFE_INTEGER) ||
    a.data.order - b.data.order ||
    a.data.title.localeCompare(b.data.title) ||
    a.id.localeCompare(b.id)
  );
}

export function docIdFromEntry(entry: string): string {
  const normalized = entry.replaceAll("\\", "/").replace(/^\.\/+/, "");
  if (
    normalized.startsWith("/") ||
    !normalized.endsWith(".md") ||
    normalized.split("/").some((segment) => !segment || segment === "." || segment === "..")
  ) {
    throw new Error(`invalid public guide entry path: ${entry}`);
  }

  return normalized.slice(0, -".md".length);
}

export function docRoutePath(id: string): string {
  if (
    !id ||
    id.startsWith("/") ||
    id.endsWith("/") ||
    id.split("/").some((segment) => !segment || segment === "." || segment === "..")
  ) {
    throw new Error(`invalid public guide id: ${id}`);
  }

  if (id === "index") {
    return "/docs/";
  }
  if (id.endsWith("/index")) {
    return `/docs/${id.slice(0, -"/index".length)}/`;
  }
  return `/docs/${id}/`;
}

export function sortPublishedDocs<T extends DocNavigationEntry>(entries: readonly T[]): T[] {
  return entries.filter((entry) => !entry.data.draft).toSorted(compareDocs);
}

export function groupDocsBySection<T extends DocNavigationEntry>(
  entries: readonly T[],
): DocSectionGroup<T>[] {
  const published = sortPublishedDocs(entries).filter((entry) => entry.id !== "index");
  return DOC_SECTION_ORDER.map((section) => ({
    section,
    label: DOC_SECTION_LABELS[section],
    entries: published.filter((entry) => entry.data.section === section),
  })).filter((group) => group.entries.length > 0);
}

export function docNeighbors<T extends DocNavigationEntry>(
  entries: readonly T[],
  currentId: string,
): DocNeighbors<T> {
  const published = sortPublishedDocs(entries).filter((entry) => entry.id !== "index");
  const currentIndex = published.findIndex((entry) => entry.id === currentId);
  if (currentIndex === -1) {
    return {};
  }
  return {
    previous: published[currentIndex - 1],
    next: published[currentIndex + 1],
  };
}
