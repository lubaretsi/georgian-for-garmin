import type { Lang } from "../core/backend";
import { en } from "./en";
import { ka } from "./ka";

type Widen<T> = T extends string
  ? string
  : T extends readonly (infer U)[]
    ? readonly Widen<U>[]
    : { readonly [K in keyof T]: Widen<T[K]> };

/** The catalog shape: en.ts's keys, with any text. ka.ts is checked against it. */
export type Copy = Widen<typeof en>;

export const catalogs: Record<Lang, Copy> = { ka, en };

export function copyFor(lang: Lang): Copy {
  return catalogs[lang];
}

/** Fills `{name}` placeholders. A missing value is a bug, so it throws. */
export function fmt(template: string, values: Record<string, string | number>): string {
  return template.replace(/\{(\w+)\}/g, (_, key: string) => {
    const value = values[key];
    if (value === undefined) throw new Error(`copy: no value for {${key}} in "${template}"`);
    return String(value);
  });
}

/** A text that depends on a number: English's singular and plural (Georgian uses one form). */
export type Plural = { readonly one: string; readonly other: string };

/** Fills the form of `forms` that `count` takes, as `fmt` does. */
export function fmtCount(forms: Plural, count: number, values: Record<string, string | number>): string {
  return fmt(count === 1 ? forms.one : forms.other, values);
}

/** Joins a list for a sentence: "19.05, 19.10 and 20.01" (en), "19.05, 19.10 და 20.01" (ka). */
export function list(lang: Lang, items: readonly string[]): string {
  return new Intl.ListFormat(lang, { style: "long", type: "conjunction" }).format(items);
}
