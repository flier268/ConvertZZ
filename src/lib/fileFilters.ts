export interface LegacyFileFilter {
  name: string;
  extensions: string[];
}

export function parseLegacyFileFilters(value: string): LegacyFileFilter[] {
  const filters: LegacyFileFilter[] = [];
  for (const match of value.matchAll(/<([^|<>]+)\|([^<>]+)>/gu)) {
    const extensions = match[2]
      .split(";")
      .map((pattern) =>
        pattern
          .trim()
          .replace(/^\*\.?/u, "")
          .replace(/^\./u, ""),
      )
      .filter((extension) => extension && extension !== "*");
    if (extensions.length)
      filters.push({ name: match[1].trim(), extensions: Array.from(new Set(extensions)) });
  }
  return filters;
}
