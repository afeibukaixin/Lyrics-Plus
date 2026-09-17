import type { FontFamily } from "./types/overlay";

export const SYSTEM_FONT_FAMILY = "-apple-system";
export const SYSTEM_FONT_NAME = "System Default";

export const defaultFontFamilies: FontFamily[] = [
  { name: SYSTEM_FONT_NAME, family: SYSTEM_FONT_FAMILY },
];

export const defaultCustomFontFamilies = "system-ui, sans-serif";

export function isSystemFontFamily(family: string) {
  return [SYSTEM_FONT_FAMILY, "system-ui", "blinkmacsystemfont", "sans-serif"]
    .includes(family.trim().toLowerCase());
}

function isLegacySystemAlias(family: string) {
  return [SYSTEM_FONT_FAMILY, "blinkmacsystemfont"]
    .includes(family.trim().toLowerCase());
}

export function normalizeFontFamilies(families: FontFamily[]): FontFamily[] {
  const first = families[0];
  const primary = first?.family.trim();
  const result: FontFamily[] = [!primary || isSystemFontFamily(primary)
    ? defaultFontFamilies[0]
    : { name: first.name.trim() || primary, family: primary }];
  for (const item of families.slice(1)) {
    const family = item.family.trim();
    if (!family || isLegacySystemAlias(family)) continue;
    if (result.some((existing) => existing.family.toLowerCase() === family.toLowerCase())) continue;
    result.push({
      name: item.name.trim() || family,
      family,
    });
  }
  return result;
}

/** Split a CSS font-family stack while keeping commas inside quoted names. */
export function splitFontFamilyStack(value: string) {
  const families: string[] = [];
  let current = "";
  let quote: '"' | "'" | null = null;
  let escaped = false;
  for (const character of value) {
    if (escaped) {
      current += character;
      escaped = false;
      continue;
    }
    if (character === "\\" && quote) {
      escaped = true;
      continue;
    }
    if (quote) {
      if (character === quote) quote = null;
      else current += character;
    } else if (character === '"' || character === "'") {
      quote = character;
    } else if (character === ",") {
      const family = current.trim();
      if (family) families.push(family);
      current = "";
    } else {
      current += character;
    }
  }
  if (escaped) current += "\\";
  const family = current.trim();
  if (family) families.push(family);
  return families;
}

/** Convert a CSS stack into user-editable entries, keeping generic fallbacks. */
export function fontFamiliesFromCss(value: string): FontFamily[] {
  return splitFontFamilyStack(value)
    .map((family, index) => isSystemFontFamily(family) && index === 0
      ? { name: SYSTEM_FONT_NAME, family: SYSTEM_FONT_FAMILY }
      : { name: family, family })
    .filter((item, index) => index === 0 || !isLegacySystemAlias(item.family));
}

/** Parse only fallback entries; the first token is not a primary font. */
export function fontFallbacksFromCss(value: string): FontFamily[] {
  return splitFontFamilyStack(value)
    .filter((family) => !isLegacySystemAlias(family))
    .map((family) => ({ name: family, family }));
}

function escapeCssString(value: string) {
  return value
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"')
    .replace(/\n/g, "\\a ")
    .replace(/\r/g, "\\d ")
    .replace(/\f/g, "\\c ");
}

function fontFamilyToCss(family: string) {
  const normalized = family.toLowerCase();
  return normalized === "system-ui" || normalized === "sans-serif"
    ? normalized
    : `"${escapeCssString(family)}"`;
}

export function fontFallbacksToCss(families: FontFamily[]) {
  return families.map((item) => fontFamilyToCss(item.family)).join(", ");
}

export function fontFamiliesToCss(families: FontFamily[]) {
  const normalized = normalizeFontFamilies(families);
  const stack = normalized.map((item, index) => {
    const family = item.family.toLowerCase();
    if (index === 0 && isSystemFontFamily(family)) return SYSTEM_FONT_FAMILY;
    return fontFamilyToCss(item.family);
  });
  if (!normalized.some((item) => ["system-ui", "sans-serif"].includes(item.family.toLowerCase()))) {
    stack.push("system-ui");
  }
  if (!normalized.some((item) => item.family.toLowerCase() === "sans-serif")) {
    stack.push("sans-serif");
  }
  return stack.join(", ");
}

/** Build the CSS stack used by lyric surfaces from the persisted settings. */
export function fontFamilyStack(fontFamily: string, fontFamilies: string) {
  const parsedFamilies = fontFamiliesFromCss(fontFamily);
  const primary = parsedFamilies[0]?.family ?? fontFamily.trim();
  const primaryName = parsedFamilies[0]?.name ?? primary;
  const primaryEntry = {
    name: isSystemFontFamily(primary) ? SYSTEM_FONT_NAME : primaryName,
    family: primary,
  };
  return fontFamiliesToCss([primaryEntry, ...fontFallbacksFromCss(fontFamilies)]);
}
