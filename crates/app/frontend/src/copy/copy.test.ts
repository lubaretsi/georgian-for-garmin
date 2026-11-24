import { describe, expect, it } from "vitest";

import type { ErrorCode, Holder } from "../core/types.gen";
import { diagnosticsSummary } from "../flow/screen";
import { en } from "./en";
import { catalogs, fmt, fmtCount } from "./index";
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
const MTP_KEYS = new Set([
  "connect.help.replug.body",
  "drive.mtp.body",
  "drive.mtp.how",
  "drive.duringRestart", // on the drive screen, under drive.mtp.body's explanation
  "more.help.mtp",
]);

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

  /** Every error code, checked by the compiler: a new code must be added here. */
  const CODES: Record<ErrorCode, true> = {
    invalid_input: true, no_device: true, garmin_mode: true, multiple_devices: true, device_busy: true,
    permission_denied: true, device_locked: true,
    unsupported_device: true, font_unavailable: true, garmin_folder_not_found: true, extdata_not_found: true,
    conflict: true, restore_first: true, verification_failed: true, device_error: true, internal: true, cancelled: true,
  };
  const HOLDERS: (Holder | null)[] = [
    null,
    { kind: "garmin_express", name: "Garmin Express", confirmed: true },
    { kind: "other", name: "Photos", confirmed: true },
    { kind: "unknown", confirmed: false },
    { kind: "this_app", name: "Georgian for Gar", pid: 4100, confirmed: true },
  ];
  /** A raw code, as in "device_busy": the full text shows these, the summary never does. */
  const SNAKE_CASE = /\b[a-z0-9]+(_[a-z0-9]+)+\b/i;

  for (const lang of ["ka", "en"] as const) {
    it(`${lang}: the diagnostics summary has plain words for every code, connection and last problem`, () => {
      for (const code of Object.keys(CODES) as ErrorCode[]) {
        for (const holder of HOLDERS) {
          const rows = diagnosticsSummary(
            lang,
            "0.4.0",
            {
              app: { version: "0.4.0", build: "release" },
              system: { os: "macOS", os_version: "15.6.1", arch: "arm64" },
              // Notes on what the system can't tell stay in the full text, never the summary.
              usb: { listed: false, note: "this platform lists only WPD's MTP devices: usb_listing_unavailable", devices: [] },
              connection: { ok: false, code, message: code, busy: holder ? { holder } : null },
              drives: { listed: false, note: "mount table unreadable: drive_listing_unavailable", volumes: [] },
              watch: null,
              events: [],
              text: `connection ${code}\n`,
            },
            { time: "2026-11-24T03:14:05", code, holder },
          );
          for (const { label, value } of rows) {
            const where = `${lang} ${code} ${holder?.kind ?? "no holder"} ${label}: ${value}`;
            expect(value, where).not.toMatch(SNAKE_CASE);
            for (const word of [...INTERNAL_WORDS, ...NEVER_WORDS]) expect(value, where).not.toMatch(word);
          }
        }
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

  it("diagnostic info says what it doesn't hold (§11)", () => {
    expect(en.diagnostics.contents).toMatch(/no serial number or watch ID/);
    expect(ka.diagnostics.contents).toMatch(/სერიული ნომერი და საათის ID მასში არ არის/);
    // The More entry opens the screen of the same name.
    expect(en.more.diagnostics.title).toBe(en.diagnostics.title);
    expect(ka.more.diagnostics.title).toBe(ka.diagnostics.title);
  });

  it("names every confidence on the watch list's badge", () => {
    for (const catalog of [en, ka]) {
      expect(Object.keys(catalog.watchCard.status).sort()).toEqual(Object.keys(catalog.confidence).filter((k) => !k.endsWith("_unlisted")).sort());
    }
  });

  it("says one font and one file in the singular", () => {
    expect(fmtCount(en.share.summary.fonts, 1, { count: 1 })).toBe("1 font");
    expect(fmtCount(en.share.summary.fonts, 4, { count: 4 })).toBe("4 fonts");
    expect(fmtCount(en.share.summary.fonts, 0, { count: 0 })).toBe("0 fonts");
    expect(fmtCount(en.working.progress, 1, { done: 0, total: 1 })).toBe("0 of 1 file");
    expect(fmtCount(en.working.progress, 4, { done: 1, total: 4 })).toBe("1 of 4 files");
    // Georgian keeps the singular after a numeral.
    expect(fmtCount(ka.share.summary.fonts, 4, { count: 4 })).toBe("4 შრიფტი");
    expect(fmtCount(ka.working.progress, 1, { done: 0, total: 1 })).toBe("1-დან 0 ფაილი");
  });
});
