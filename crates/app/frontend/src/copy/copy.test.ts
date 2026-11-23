import { describe, expect, it } from "vitest";

import { en } from "./en";
import { catalogs, fmt } from "./index";
import { ka } from "./ka";

/** Every string in a catalog by its dotted path; array items as `path.0`, `path.1`. */
function flatten(value: unknown, path = ""): Map<string, string> {
  const out = new Map<string, string>();
  if (typeof value === "string") {
    out.set(path, value);
  } else if (Array.isArray(value)) {
    value.forEach((item, i) => flatten(item, `${path}.${i}`).forEach((v, k) => out.set(k, v)));
  } else if (value && typeof value === "object") {
    for (const [key, item] of Object.entries(value)) {
      flatten(item, path ? `${path}.${key}` : key).forEach((v, k) => out.set(k, v));
    }
  }
  return out;
}

const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

/** The placeholders §7 allows. */
const ALLOWED_PLACEHOLDERS = new Set([
  "model", "software", "versions", "files", "program", "done", "total", "attempt",
  // Not in the main flow's sentences: the More sheet's app version, a marker picture's name,
  // the shared summary's font count.
  "version", "variant", "count",
]);

const flat = { ka: flatten(ka), en: flatten(en) };

describe("copy parity", () => {
  it("has the same keys in both languages", () => {
    expect([...flat.ka.keys()].sort()).toEqual([...flat.en.keys()].sort());
  });

  it("has the same placeholders per key", () => {
    for (const [key, text] of flat.en) {
      expect(placeholders(flat.ka.get(key) ?? ""), key).toEqual(placeholders(text));
    }
  });

  it("uses only the placeholders §7 names", () => {
    for (const lang of ["ka", "en"] as const) {
      for (const [key, text] of flat[lang]) {
        for (const name of placeholders(text)) {
          expect(ALLOWED_PLACEHOLDERS.has(name ?? ""), `${lang} ${key}: {${name}}`).toBe(true);
        }
      }
    }
  });

  it("has no empty strings", () => {
    for (const lang of ["ka", "en"] as const) {
      for (const [key, text] of flat[lang]) expect(text.trim(), `${lang} ${key}`).not.toBe("");
    }
  });

  it("fills placeholders and refuses a missing one", () => {
    expect(fmt("on {model}", { model: "Venu 2" })).toBe("on Venu 2");
    expect(() => fmt("on {model}", {})).toThrow();
    expect(catalogs.ka).toBe(ka);
  });
});

/**
 * §2.3: words the main flow never shows. They may appear only in the flow's own data under
 * More › What the app sees, never in the catalogs. Latin words match whole words; Georgian
 * ones match stems.
 */
const INTERNAL_WORDS: RegExp[] = [
  /\bpacks?\b/i,
  /\bparts?\b/i,
  /\bpart numbers?\b/i,
  /extdata/i,
  /\bstag(e|ed|ing)\b/i,
  /\.tmp\b/i,
  /truetype/i,
  /bitmap/i,
  /\bpx\b/i,
  /\bhandles?\b/i,
  /armenian/i,
  /hebrew/i,
  /korean/i,
  /nanum/i,
  /\bnoto\b/i,
  /პაკეტ/,
  /ნაწილ/,
  /ბიტმაპ/,
  /სომხურ/,
  /ებრაულ/,
  /კორეულ/,
];

/** §7's "Never" column. */
const NEVER_WORDS: RegExp[] = [
  /\breboot/i,
  /\btested\b/i,
  /\bchecked\b/i,
  /რეპორტ/,
  /(^|[^ა-ჰ])ქართ([^ა-ჰ]|$)/, // the cut-off language name
  /^კარგი$/, // "Done" isn't "OK"
];

/** "MTP" only where the watch shows it, with an explanation (§2.3). */
const MTP_KEYS = new Set(["connect.help.replug", "drive.mtp.body", "drive.mtp.how", "more.help.mtp"]);

describe("plain words in the main flow (§2.3, §7)", () => {
  for (const lang of ["ka", "en"] as const) {
    it(`${lang}: no internal words outside More › What the app sees`, () => {
      for (const [key, text] of flat[lang]) {
        if (key.startsWith("more.seen.labels.")) continue;
        for (const word of INTERNAL_WORDS) expect(text, `${lang} ${key}`).not.toMatch(word);
      }
    });

    it(`${lang}: none of §7's "Never" words`, () => {
      for (const [key, text] of flat[lang]) {
        for (const word of NEVER_WORDS) expect(text, `${lang} ${key}`).not.toMatch(word);
      }
    });

    it(`${lang}: MTP only where the watch shows it`, () => {
      for (const [key, text] of flat[lang]) {
        if (/\bMTP\b/.test(text)) expect(MTP_KEYS.has(key), `${lang} ${key}`).toBe(true);
      }
    });
  }

  it('ka: "ვერსია" never stands alone (it would read as the app\'s version)', () => {
    const qualified = /(სისტემის (სხვა )?|პროგრამის (მომდევნო )?|სატესტო )ვერსი/g;
    for (const [key, text] of flat.ka) {
      const all = text.match(/ვერსი/g)?.length ?? 0;
      const ok = text.match(qualified)?.length ?? 0;
      expect(ok, `ka ${key}: ${text}`).toBe(all);
    }
  });

  it('ka: "შეტყობინება" is only the phone notification, never the shared text', () => {
    for (const [key, text] of flat.ka) {
      if (/^(share\.|error\.|connect\.problem\.|actions\.(share|report|stillStuck))/.test(key)) {
        expect(text, `ka ${key}`).not.toMatch(/შეტყობინ/);
      }
    }
  });

  it("keeps the labels the docs cite", () => {
    expect(en.actions.install).toBe("Install Georgian");
    expect(ka.actions.install).toBe("ქართულის დაყენება");
    expect(en.actions.restore).toBe("Restore original font");
    expect(ka.actions.restore).toBe("ქარხნული შრიფტის დაბრუნება");
    expect(en.actions.check).toBe("Check on the watch");
    expect(ka.actions.check).toBe("საათზე შემოწმება");
  });
});
