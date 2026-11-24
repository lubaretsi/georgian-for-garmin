import { describe, expect, test } from "vitest";

import type { InspectOutput } from "../core/types.gen";
import { inspectFixtures, output } from "../mock/fixtures";
import type { Verdict } from "../model/screens";
import { confidenceOf, verdictOf, verifiable, verified, watchInfo } from "./verdict";

const reason = output("fenix6", "inspect", "inspect").support.reason ?? null;
const instinct3Reason = output("instinct3", "inspect", "inspect").support.reason ?? null;

/** §5.1 for every successful inspect the core produced (fixtures/core). */
const EXPECTED: Record<string, Verdict> = {
  "busy/inspect-released": { kind: "install", confidence: "confirmed" },
  "connect-problems/inspect-extdata_not_found": { kind: "unreadable" },
  "errors/inspect-other": { kind: "blocked", reason: "unknown_font" },
  "errors/inspect-restore_first": { kind: "install", confidence: "confirmed", note: "not_visible" },
  "fenix6/inspect": { kind: "unsupported", reason, restorable: false },
  "fenix6-beta-left/inspect": { kind: "unsupported", reason, restorable: true },
  "fenix6-beta-left/inspect-restore-staged": { kind: "unsupported", reason, restorable: false },
  "fenix6-beta-left/inspect-restored": { kind: "unsupported", reason, restorable: false },
  "forerunner255-newer/inspect": { kind: "install", confidence: "confirmed_other_software" },
  "forerunner265/inspect": { kind: "no_font" },
  "instinct3/inspect": { kind: "unsupported", reason: instinct3Reason, restorable: true },
  "instinct3/inspect-restore-staged": { kind: "unsupported", reason: instinct3Reason, restorable: false },
  "instinct3/inspect-restored": { kind: "unsupported", reason: instinct3Reason, restorable: false },
  "lily2active/inspect": { kind: "install", confidence: "unconfirmed" },
  "lily2active/inspect-staged": { kind: "restart_pending", variant: "georgian" },
  "lily2active/inspect-restarted": { kind: "installed", confidence: "unconfirmed" },
  "venu-boxes/inspect": { kind: "install", confidence: "unconfirmed" },
  "venu-boxes/inspect-staged": { kind: "restart_pending", variant: "georgian" },
  "venu-boxes/inspect-restarted": { kind: "installed", confidence: "unconfirmed" },
  "venu-boxes/inspect-restarted-again": { kind: "installed", confidence: "unconfirmed" },
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
  "venu3-waiting/inspect": { kind: "blocked", reason: "waiting_other_font" },
  "venu3-waiting/inspect-restarted": { kind: "install", confidence: "unconfirmed", note: "not_visible" },
  "venu3-waiting/inspect-staged": { kind: "installed", confidence: "unconfirmed" },
  "venu3-waiting/inspect-installed": { kind: "install", confidence: "unconfirmed", note: "not_visible" },
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
  const venu = output("venu-boxes", "inspect", "inspect");

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
    expect(verified("restore", output("fenix6-beta-left", "inspect-restored", "inspect"))).toBe(true);
    expect(verified("restore", output("fenix6-beta-left", "inspect", "inspect"))).toBe(false);
  });

  test("only bitmap writes are verifiable", () => {
    expect(verifiable(output("venu2", "install", "install").packs)).toBe(true);
    expect(verifiable(output("venu3-waiting", "install", "install").packs)).toBe(false);
    expect(verifiable(output("fenix6-beta-left", "restore_stock", "restore_stock").packs)).toBe(false);
  });
});

test("watchInfo carries the software Georgian is confirmed on", () => {
  expect(watchInfo(output("forerunner255-newer", "inspect", "inspect"))).toEqual({
    model: "Forerunner 255",
    modelPart: "006-B3992-00",
    software: "30.00",
    confirmedSoftware: ["29.52"],
  });
  expect(watchInfo(output("forerunner245-drive", "inspect_drive", "inspect_drive")).confirmedSoftware).toEqual([]);
});
