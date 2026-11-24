import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

// §2.9: text ≥ 4.5:1; control boundaries, checked indicators and the focus ring ≥ 3:1, both themes.

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

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

/** The surfaces text and controls sit on: the window, list groups and their states, fills. */
const SURFACES = ["bg", "group", "group-hover", "group-active"];
const TEXT: [string, string[]][] = [
  ["text", [...SURFACES, "fill", "fill-strong", "control-bg", "accent-soft", "success-soft", "caution-soft", "danger-soft"]],
  ["text-2", [...SURFACES, "fill", "accent-soft", "success-soft", "caution-soft", "danger-soft"]],
  ["on-accent", ["accent", "accent-hover", "accent-active"]],
  ["accent-ink", SURFACES],
  ["success-ink", ["success-soft", ...SURFACES]],
  ["caution-ink", ["caution-soft"]],
  ["danger-ink", ["danger-soft", ...SURFACES]],
];
const BOUNDARIES: [string, string[]][] = [
  ["control-border", [...SURFACES, "fill"]], // buttons, radios, checkboxes, the segmented track
  ["indicator", [...SURFACES, "accent-soft"]], // checked radios and checkboxes
  ["focus", [...SURFACES, "fill", "accent-soft"]],
  ["accent", ["bg"]], // the main button's edge, in the footer
  ["on-accent", ["indicator"]], // the check and the dot inside a checked control
  ["success", ["bg", "group"]], // the check badge, the done step
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

  it("uses no pure black or white (§10)", () => {
    for (const [name, value] of Object.entries(tokens)) expect(value.toLowerCase(), name).not.toMatch(/^#(000000|ffffff)$/);
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
  const global = readFileSync(path.join(import.meta.dirname, "global.css"), "utf8");
  const sizes = [...global.matchAll(/--text-\w+:\s*(\d+)px/g)].map((m) => Number(m[1]));
  // Arbitrary sizes in the components (`text-[17px]`) and SVG-free inline sizes.
  const dirs = ["../ui", "../screens", "../shell"].map((d) => path.join(import.meta.dirname, d));
  for (const dir of dirs) {
    for (const file of readdirSync(dir).filter((f) => f.endsWith(".tsx"))) {
      const source = readFileSync(path.join(dir, file), "utf8");
      sizes.push(...[...source.matchAll(/text-\[(\d+)px\]/g)].map((m) => Number(m[1])));
    }
  }
  expect(sizes.length).toBeGreaterThan(0);
  expect(Math.min(...sizes)).toBeGreaterThanOrEqual(13);
});
