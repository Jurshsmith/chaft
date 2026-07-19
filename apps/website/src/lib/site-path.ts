function normalizeBase(base: string): string {
  const withLeadingSlash = base.startsWith("/") ? base : `/${base}`;
  return withLeadingSlash.endsWith("/") ? withLeadingSlash : `${withLeadingSlash}/`;
}

export function joinSiteBase(base: string, path: string): string {
  if (!path.startsWith("/")) {
    throw new Error(`site path must start with "/": ${path}`);
  }

  const normalizedBase = normalizeBase(base);
  return `${normalizedBase}${path.slice(1)}`;
}

export function sitePath(path: string): string {
  return joinSiteBase(import.meta.env.BASE_URL, path);
}
