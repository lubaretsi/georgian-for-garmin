import { readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

// §2.9: text ≥ 4.5:1, control boundaries (and focus rings) ≥ 3:1, both themes.

const css = readFileSync(path.join(import.meta.dirname, "tokens.css"), "utf8");

function block(selector: string): Record<string, string> {
  const start = css.indexOf(`${selector} {`);
  if (start < 0) throw new Error(`no ${selector} block in tokens.css`);
  const body = css.slice(start, css.indexOf("}", start));
  return Object.fromEntries([...body.matchAll(/--([\w-]+):\s*(#[0-9a-f]{6})/gi)].map((m) => [m[1], m[2]]));
}

function luminance(hex: string): number {
  const channel = (i: number) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
}

export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

const SURFACES = ["bg", "surface", "surface-raised"];
const TEXT: [string, string[]][] = [
  ["text", [...SURFACES, "info-bg", "warning-bg", "error-bg", "accent-soft", "code-bg"]],
  ["text-muted", [...SURFACES, "code-bg"]],
  ["on-accent", ["accent", "accent-hover"]],
  ["accent", SURFACES], // quiet buttons, disclosure summaries
  ["error-text", ["error-bg"]],
];
const BOUNDARIES: [string, string[]][] = [
  ["border", [...SURFACES, "accent-soft"]],
  ["focus", [...SURFACES, "accent-soft"]],
  ["accent", ["accent-soft"]], // the selected choice's outline
  ["success", SURFACES], // the done step's mark
  ["info-border", ["info-bg"]],
  ["warning-border", ["warning-bg"]],
  ["error-border", ["error-bg"]],
];

describe.each([
  ["light", block('[data-theme="light"]')],
  ["dark", block('[data-theme="dark"]')],
])("%s theme", (_, tokens) => {
  const color = (name: string) => {
    const value = tokens[name];
    if (!value) throw new Error(`no --${name}`);
    return value;
  };

  it.each(TEXT)("%s text reaches 4.5:1", (fg, bgs) => {
    for (const bg of bgs) expect(contrast(color(fg), color(bg)), `${fg} on ${bg}`).toBeGreaterThanOrEqual(4.5);
  });

  it.each(BOUNDARIES)("%s boundary reaches 3:1", (fg, bgs) => {
    for (const bg of bgs) expect(contrast(color(fg), color(bg)), `${fg} on ${bg}`).toBeGreaterThanOrEqual(3);
  });
});

it("dark tokens follow the system the same as [data-theme=dark]", () => {
  const media = css.slice(css.indexOf("@media (prefers-color-scheme: dark)"));
  const system = Object.fromEntries(
    [...media.slice(0, media.indexOf("}")).matchAll(/--([\w-]+):\s*(#[0-9a-f]{6})/gi)].map((m) => [m[1], m[2]]),
  );
  expect(system).toEqual(block('[data-theme="dark"]'));
});

it("no text size below 13px", () => {
  const components = readFileSync(path.join(import.meta.dirname, "components.css"), "utf8");
  const sizes = [
    ...[...css.matchAll(/--text-\w+:\s*(\d+)px/g)].map((m) => Number(m[1])),
    ...[...components.matchAll(/font-size:\s*(\d+)px/g)].map((m) => Number(m[1])),
  ];
  expect(Math.min(...sizes)).toBeGreaterThanOrEqual(13);
});
