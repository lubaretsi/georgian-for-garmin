import { describe, expect, test } from "vitest";

import type { InspectOutput } from "../core/types.gen";
import { inspectFixtures, output } from "../mock/fixtures";
import type { Verdict } from "../model/screens";
import { confidenceOf, verdictOf, verifiable, verified, watchInfo } from "./verdict";

const reason = output("font-unused", "inspect", "inspect").support.reason ?? null;
const koreanReason = output("korean-unused", "inspect", "inspect").support.reason ?? null;

/** §5.1 for every successful inspect the core produced (fixtures/core). */
const EXPECTED: Record<string, Verdict> = {
  "busy/inspect-released": { kind: "install", confidence: "confirmed" },
  "connect-problems/inspect-extdata_not_found": { kind: "unreadable" },
  "errors/inspect-other": { kind: "blocked", reason: "unknown_font" },
  "errors/inspect-restore_first": { kind: "install", confidence: "confirmed", note: "not_visible" },
  "expected-waiting/inspect": { kind: "blocked", reason: "waiting_other_font" },
  "expected-waiting/inspect-restarted": { kind: "install", confidence: "unconfirmed", note: "not_visible" },
  "expected-waiting/inspect-staged": { kind: "installed", confidence: "unconfirmed" },
  "expected-waiting/inspect-installed": { kind: "install", confidence: "unconfirmed", note: "not_visible" },
  "font-unused/inspect": { kind: "unsupported", reason, restorable: false },
  "font-unused-beta-left/inspect": { kind: "unsupported", reason, restorable: true },
  "font-unused-beta-left/inspect-restore-staged": { kind: "unsupported", reason, restorable: false },
  "font-unused-beta-left/inspect-restored": { kind: "unsupported", reason, restorable: false },
  "korean-boxes/inspect": { kind: "install", confidence: "unconfirmed" },
  "korean-boxes/inspect-staged": { kind: "restart_pending", variant: "georgian" },
  "korean-boxes/inspect-restarted": { kind: "installed", confidence: "unconfirmed" },
  "korean-boxes/inspect-restarted-again": { kind: "installed", confidence: "unconfirmed" },
  "korean-unused/inspect": { kind: "unsupported", reason: koreanReason, restorable: true },
  "korean-unused/inspect-restore-staged": { kind: "unsupported", reason: koreanReason, restorable: false },
  "korean-unused/inspect-restored": { kind: "unsupported", reason: koreanReason, restorable: false },
  "other-software/inspect": { kind: "install", confidence: "confirmed_other_software" },
  "predicted/inspect": { kind: "no_font" },
  "unconfirmed/inspect": { kind: "install", confidence: "unconfirmed" },
  "unconfirmed/inspect-staged": { kind: "restart_pending", variant: "georgian" },
  "unconfirmed/inspect-restarted": { kind: "installed", confidence: "unconfirmed" },
  "venu2/inspect": { kind: "install", confidence: "confirmed" },
  "venu2/inspect-staged": { kind: "restart_pending", variant: "georgian" },
  "venu2/inspect-restarted": { kind: "installed", confidence: "confirmed" },
  "venu2/inspect-restore-staged": { kind: "restart_pending", variant: "stock" },
  "venu2/inspect-restored": { kind: "install", confidence: "confirmed" },
  "venu2-second-restart/inspect": { kind: "install", confidence: "confirmed" },
  "venu2-second-restart/inspect-staged": { kind: "restart_pending", variant: "georgian" },
  "venu2-second-restart/inspect-not-imported": { kind: "restart_pending", variant: "georgian" },
  "venu2-second-restart/inspect-restarted": { kind: "installed", confidence: "confirmed" },
  "venu2-write-fails/inspect": { kind: "install", confidence: "confirmed" },
  "venu2-write-fails/inspect-after-complete": { kind: "install", confidence: "confirmed" },
  "venu2-write-fails/inspect-after-incomplete": { kind: "install", confidence: "confirmed", note: "partly" },
  "venu2-write-fails/inspect-restore-staged": { kind: "restart_pending", variant: "stock" },
  "venu2-write-fails/inspect-restored": { kind: "install", confidence: "confirmed" },
};

describe("verdictOf", () => {
  const all = inspectFixtures();

  test("the table names every inspect fixture", () => {
    expect(Object.keys(EXPECTED).sort()).toEqual(all.map((f) => f.name).sort());
  });

  test.each(all)("$name", ({ name, output: inspect }) => {
    expect(verdictOf(inspect)).toEqual(EXPECTED[name]);
  });

  // The rules no fixture reaches: the same watches with one field changed.
  const venu2 = output("venu2", "inspect", "inspect");
  const venu = output("korean-boxes", "inspect", "inspect");

  test("a `beta` state is install with its note", () => {
    expect(verdictOf({ ...venu2, state: "beta" })).toEqual({ kind: "install", confidence: "confirmed", note: "beta" });
  });

  test("rule 1 comes first: unsupported whatever the state", () => {
    const unsupported: InspectOutput = { ...venu2, state: "waiting", support: { ...venu2.support, status: "unsupported" } };
    expect(verdictOf(unsupported).kind).toBe("unsupported");
  });

  test("a tested target part makes it confirmed for some fonts", () => {
    const packs = venu.packs.map((pack, i) =>
      i === 0 ? { ...pack, parts: pack.parts.map((part, j) => (j === 0 ? { ...part, tested: "tested" as const } : part)) } : pack,
    );
    expect(confidenceOf({ ...venu, packs })).toBe("confirmed_some_fonts");
  });
});

describe("the restart's check", () => {
  test("install is verified by `installed`", () => {
    expect(verified("install", output("venu2", "inspect-restarted", "inspect"))).toBe(true);
    expect(verified("install", output("venu2", "inspect-staged", "inspect"))).toBe(false);
  });

  test("restore is verified when Garmin's font is in every restored part, on a supported or unsupported watch", () => {
    expect(verified("restore", output("venu2", "inspect-restored", "inspect"))).toBe(true);
    expect(verified("restore", output("venu2", "inspect-restore-staged", "inspect"))).toBe(false);
    expect(verified("restore", output("font-unused-beta-left", "inspect-restored", "inspect"))).toBe(true);
    expect(verified("restore", output("font-unused-beta-left", "inspect", "inspect"))).toBe(false);
  });

  test("only bitmap writes are verifiable", () => {
    expect(verifiable(output("venu2", "install", "install").packs)).toBe(true);
    expect(verifiable(output("expected-waiting", "install", "install").packs)).toBe(false);
    expect(verifiable(output("font-unused-beta-left", "restore_stock", "restore_stock").packs)).toBe(false);
  });
});

test("watchInfo carries the software Georgian is confirmed on", () => {
  expect(watchInfo(output("other-software", "inspect", "inspect"))).toEqual({
    model: "Example Other Software",
    modelPart: "006-B9908-00",
    software: "30.00",
    confirmedSoftware: ["29.52"],
  });
  expect(watchInfo(output("drive-only", "inspect_drive", "inspect_drive")).confirmedSoftware).toEqual([]);
});
