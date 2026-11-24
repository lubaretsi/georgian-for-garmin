// @vitest-environment jsdom
// index.html's WebView check, run as the page runs it, over WebViews with and without each
// feature the app's build needs.

import { beforeEach, describe, expect, it, vi } from "vitest";

import html from "../index.html?raw";

const script = /<script id="webview-check">([\s\S]*?)<\/script>/.exec(html)?.[1];

const MAC = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)";
const LINUX = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)";

type Engine = {
  supports: (test: string) => boolean;
  registerProperty: boolean;
  layers: boolean;
  rangeQueries: boolean;
  userAgent: string;
};

/** Safari 16.4's engine: every feature there. */
const CURRENT: Engine = {
  supports: () => true,
  registerProperty: true,
  layers: true,
  rangeQueries: true,
  userAgent: MAC,
};

/** Runs the check in a page like index.html's, over `engine`; the message it shows, or null. */
function check(engine: Engine): HTMLElement | null {
  if (!script) throw new Error("index.html has no WebView check");
  const supports = vi.fn(engine.supports);
  const window = {
    CSS: { supports, ...(engine.registerProperty && { registerProperty: () => {} }) },
    ...(engine.layers && { CSSLayerBlockRule: class {} }),
    matchMedia: (query: string) => ({ matches: engine.rangeQueries && query === "(width >= 0px)" }),
    navigator: { userAgent: engine.userAgent },
  };
  new Function("window", "document", script)(window, document);
  return document.querySelector<HTMLElement>(".webview-too-old");
}

beforeEach(() => {
  document.body.innerHTML = '<div id="root"></div>';
});

describe("the WebView check", () => {
  it("leaves the app alone on Safari 16.4's engine and later", () => {
    expect(check(CURRENT)).toBeNull();
    expect(document.getElementById("root")).not.toBeNull();
  });

  it.each([
    ["color-mix()", { supports: (test: string) => !test.includes("color-mix") }],
    ["@container", { supports: (test: string) => !test.includes("container-type") }],
    ["@property", { registerProperty: false }],
    ["@layer", { layers: false }],
    ["media range syntax", { rangeQueries: false }],
  ] satisfies [string, Partial<Engine>][])("shows its message without %s", (feature, lacks) => {
    const message = check({ ...CURRENT, ...lacks });
    expect(message?.dataset.missing).toBe(feature);
    // The app doesn't start: main.tsx finds no #root.
    expect(document.getElementById("root")).toBeNull();
  });

  it("counts a CSS.supports that throws as missing", () => {
    const throwing = check({
      ...CURRENT,
      supports: () => {
        throw new SyntaxError("unknown");
      },
    });
    expect(throwing?.dataset.missing).toBe("color-mix(), @container");
  });

  it("names every missing feature on an old engine (Safari 15)", () => {
    const message = check({ ...CURRENT, supports: () => false, registerProperty: false, rangeQueries: false });
    expect(message?.dataset.missing).toBe("@property, color-mix(), @container, media range syntax");
  });

  it("says it in Georgian, then English, with macOS's way to update on a Mac", () => {
    const message = check({ ...CURRENT, registerProperty: false });
    const sections = [...(message?.querySelectorAll("section") ?? [])];
    expect(sections.map((s) => s.lang)).toEqual(["ka", "en"]);
    expect(sections[0]?.querySelector("h1")?.textContent).toBe("პროგრამას macOS-ის განახლება სჭირდება");
    expect(sections[1]?.textContent).toContain("Safari 16.4 or later");
    expect(sections[1]?.textContent).toContain("Software Update");
  });

  it("asks for a system update elsewhere", () => {
    const message = check({ ...CURRENT, userAgent: LINUX, registerProperty: false });
    const [ka, en] = [...(message?.querySelectorAll("section") ?? [])];
    expect(ka?.querySelector("h1")?.textContent).toBe("პროგრამას სისტემის განახლება სჭირდება");
    expect(en?.textContent).toContain("Update your system");
    expect(en?.textContent).not.toContain("Safari");
  });
});
