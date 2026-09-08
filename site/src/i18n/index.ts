import { en, type Dictionary } from "./en";
import { ptBR } from "./pt-BR";

export type Locale = "en" | "pt-BR";
export type { Dictionary };

/** English is the default locale and owns the bare `/` route. */
export const defaultLocale: Locale = "en";

export const dictionaries: Record<Locale, Dictionary> = {
  en,
  "pt-BR": ptBR,
};

/** Route prefix for a locale: the default locale stays unprefixed. */
export const localePath = (locale: Locale): string => (locale === defaultLocale ? "/" : "/pt-br/");

export const otherLocale = (locale: Locale): Locale => (locale === "en" ? "pt-BR" : "en");

/**
 * Fills `{name}` placeholders. Throws on an unknown or leftover placeholder so a
 * typo surfaces at build time rather than rendering "{version}" to a visitor.
 */
export const fill = (template: string, values: Record<string, string | number>): string => {
  const result = template.replace(/\{(\w+)\}/g, (_match, key: string) => {
    if (!(key in values)) throw new Error(`i18n: no value supplied for {${key}} in "${template}"`);
    return String(values[key]);
  });
  if (/\{|\}/.test(result)) throw new Error(`i18n: unbalanced braces after filling "${template}"`);
  return result;
};

/** Locale-aware number formatting: 115.4 in English, 115,4 in Portuguese. */
export const formatNumber = (locale: Locale, value: number, digits = 1): string =>
  value.toLocaleString(locale, { minimumFractionDigits: digits, maximumFractionDigits: digits });
