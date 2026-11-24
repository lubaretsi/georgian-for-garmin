// Story data from the core's fixtures (fixtures/core, written by crates/core/tests/ui_fixtures.rs):
// real models, part numbers, software, busy holders, rollbacks and shared text.

import type { Lang } from "../../core/backend";
import type { DiagnoseOutput, InspectOutput } from "../../core/types.gen";
import { copyFor, fmtCount } from "../../copy";
import { diagnosticsSummary } from "../../flow/screen";
import { failure, fixtureFile, output } from "../../mock/fixtures";
import type { BusyView, MoreModel, RollbackView, ScreenModel, WatchInfo } from "../../model/screens";
import { PHONE_PAGE_QR } from "./phone-qr";

function watchOf(inspect: Pick<InspectOutput, "device"> & { tested?: InspectOutput["tested"] }): WatchInfo {
  return {
    model: inspect.device.model,
    modelPart: inspect.device.part_number,
    software: inspect.device.software_version,
    confirmedSoftware: inspect.tested?.verified_software ?? [],
  };
}

const venu2 = watchOf(output("venu2", "inspect", "inspect"));

/** The watches the stories use, each from its scenario's first inspect. */
export const WATCHES = {
  venu2,
  lily2Active: watchOf(output("lily2active", "inspect", "inspect")),
  instinct3: watchOf(output("instinct3", "inspect", "inspect")),
  fenix6sPro: watchOf(output("fenix6", "inspect", "inspect")),
  forerunner245: watchOf(output("forerunner245-drive", "inspect_drive", "inspect_drive")),
  forerunner265: watchOf(output("forerunner265", "inspect", "inspect")),
  venu3: watchOf(output("venu3-waiting", "inspect", "inspect")),
  forerunner165: watchOf(output("errors", "inspect-restore_first", "inspect")),
  /** Forerunner 255 on software 30.00, confirmed on 29.52 only. */
  forerunner255Newer: watchOf(output("forerunner255-newer", "inspect", "inspect")),
} satisfies Record<string, WatchInfo>;

/** The support list's reason for the fenix 6S Pro (issue 6). */
export function unsupportedReason(): { ka: string; en: string } | null {
  return output("fenix6", "inspect", "inspect").support.reason ?? null;
}

export function issueOf(scenario: string, step = "inspect"): number | null {
  return output(scenario, step, "inspect").support.issue ?? null;
}

/** A busy holder as the screen shows it, from `busy/<step>`. */
export function busyFrom(step: string): BusyView {
  const holder = failure("busy", step).busy?.holder;
  if (!holder) throw new Error(`busy/${step} has no holder`);
  return { kind: holder.kind, name: holder.name ?? null, confirmed: holder.confirmed };
}

/** The rollback of a failed write, with the file names the user would see. */
export function rollbackFrom(scenario: string, step: string): RollbackView {
  const rollback = failure(scenario, step).rollback;
  if (!rollback) throw new Error(`${scenario}/${step} has no rollback`);
  return {
    status: rollback.status,
    files: rollback.files.filter((f) => !f.done).map((f) => f.path.split("/").pop() ?? f.path),
  };
}

export function messageOf(scenario: string, step: string): string {
  return failure(scenario, step).message;
}

/** The exact text a share puts on GitHub, from an `issue_link` fixture. */
export function reportText(scenario: string, step: string): string {
  const file = fixtureFile(scenario, step);
  if (file.operation !== "issue_link" || !file.output) throw new Error(`${scenario}/${step} is no issue link`);
  return file.output.fields.find((f) => f.id === "report")?.value ?? "";
}

export function phone(lang: Lang): { url: string; qr: string } {
  return PHONE_PAGE_QR[lang];
}

/**
 * Beta marker pictures: the test word with a bar on one capital, one per font. The core
 * draws them as PNG data URLs; these are SVG data URLs of the same idea.
 */
export function markPictures(): { variant: string; picture: string }[] {
  const word = "ᲛᲗᲐᲕᲠᲣᲚᲘ";
  return ["a", "b", "c"].map((variant, i) => ({
    variant,
    picture: `data:image/svg+xml;charset=utf-8,${encodeURIComponent(
      `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 40" width="200" height="40">` +
        `<rect width="200" height="40" fill="#000"/>` +
        `<text x="8" y="29" font-size="22" fill="#fff" font-family="sans-serif">${word}</text>` +
        `<rect x="${14 + i * 46}" y="6" width="18" height="3" fill="#fff"/></svg>`,
    )}`,
  }));
}

/** The shared summary as the flow builds it (toScreen), in the story's language. */
export function shareSummary(
  lang: Lang,
  what: "result" | "font_list" | "problem",
  watch: WatchInfo | null,
): { label: string; value: string }[] {
  const c = copyFor(lang);
  const s = c.share.summary;
  const base = watch
    ? [
        { label: s.watch, value: watch.model },
        { label: s.software, value: watch.software },
      ]
    : [];
  switch (what) {
    case "result":
      return [
        ...base,
        { label: s.installed, value: fmtCount(s.fonts, 4, { count: 4 }) },
        { label: s.result, value: c.check.answer.results.boxes },
        { label: s.views, value: [c.check.answer.views.popup, c.check.answer.views.list].join(", ") },
        { label: s.phone, value: c.check.answer.phone.android },
        { label: s.fontList, value: fmtCount(s.fonts, 38, { count: 38 }) },
      ];
    case "font_list":
      return [...base, { label: s.installed, value: s.nothing }, { label: s.fontList, value: fmtCount(s.fonts, 12, { count: 12 }) }];
    case "problem":
      return [...base, { label: s.problem, value: messageOf("errors", "install-internal") }];
  }
}

/** The More sheet's data for a watch (the flow builds it; internal words are allowed here). */
export function moreFor(lang: Lang, watch: WatchInfo | null, restorable: boolean): MoreModel {
  const l = copyFor(lang).more.seen.labels;
  return {
    restorable,
    seen: watch
      ? [
          { label: l.model, value: watch.model },
          { label: l.modelPart, value: watch.modelPart },
          { label: l.software, value: watch.software },
          { label: l.writes, value: "armenian-noto-sans-regular: 006-D9035-01, -02, -03, -04" },
          { label: l.state, value: "not_installed" },
        ]
      : null,
    reportText: watch ? reportText("venu2", "issue_link-georgian") : null,
    version: "0.4.0",
  };
}

/** Which computer the diagnostics stories describe. */
export type DiagnosticsCase = "no_watch" | "watch" | "busy";

/**
 * The diagnostics summary built by the flow's own `diagnosticsSummary` (toScreen's), and a
 * `text` in the mock's format (the core's `diagnose` writes the real one), for a Mac with or
 * without a Venu 2.
 */
export function diagnosticsSample(lang: Lang, found: DiagnosticsCase): { summary: { label: string; value: string }[]; text: string } {
  const busy = failure("busy", "inspect-garmin_express-macos");
  const diagnosed: DiagnoseOutput = {
    app: { version: "0.4.0", build: "release" },
    system: { os: "macOS", os_version: "15.6.1", arch: "arm64" },
    usb: { listed: true, note: null, devices: [] },
    connection:
      found === "watch"
        ? { ok: true, code: null, message: null, busy: null }
        : found === "busy"
          ? { ok: false, code: "device_busy", message: busy.message, busy: busy.busy ?? null }
          : { ok: false, code: "no_device", message: "no watch found", busy: null },
    drives: { listed: true, note: null, volumes: [] },
    watch: found === "watch" ? output("venu2", "inspect", "inspect").report : null,
    events: [],
    text: "",
  };
  const lastProblem =
    found === "busy" ? { time: "2026-11-24T03:14:05", code: "device_busy" as const, holder: busy.busy?.holder ?? null } : null;
  const summary = diagnosticsSummary(lang, "0.4.0", diagnosed, lastProblem);
  const usb =
    found === "no_watch"
      ? ["usb no Garmin device"]
      : ["usb", `  0x4CDA mtp${found === "busy" ? ' held by garmin_express "Garmin Express"' : ""}`];
  const text = [
    "Georgian for Garmin diagnostic info. It contains no serial number or watch ID.",
    "app 0.4.0 (2026-11-20)",
    "system macOS 15.6.1 arm64",
    ...usb,
    `connection ${found === "watch" ? "ok" : found === "busy" ? "device_busy: Garmin Express is using the watch" : "no_device: no watch found"}`,
    "drives none",
    found === "watch" ? `watch\n  model ${venu2.model}\n  software ${venu2.software}\n  state not_installed` : "watch none",
    "events 3",
    "  2026-11-24T03:13:58 poll no_device",
    `  2026-11-24T03:14:05 poll ${found === "busy" ? "device_busy" : found === "watch" ? "watch Venu 2" : "no_device"}`,
    "  2026-11-24T03:14:07 diagnostics open",
  ].join("\n");
  return { summary, text: `${text}\n` };
}

/** A model built per language (the share summary is in the story's language). */
export type StoryModel = ScreenModel | ((lang: Lang) => ScreenModel);

export function resolveModel(model: StoryModel, lang: Lang): ScreenModel {
  return typeof model === "function" ? model(lang) : model;
}
