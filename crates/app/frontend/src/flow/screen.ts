// What the shell draws for a flow state: `toScreen` (contract §5) and `moreModel` (the More
// sheet). Pure; the share summary and "What the app sees" are in the state's language.

import { type Copy, copyFor, fmt, list } from "../copy";
import type { ShareKey } from "../core/backend";
import type { InspectDriveOutput, InspectOutput } from "../core/types.gen";
import type { ConnectProblem, MoreModel, RollbackView, ScreenModel, WatchInfo } from "../model/screens";
import { errorGroup } from "../screens/ErrorScreen";
import { answersFor, type FlowState, type Phase, shareKeyId } from "./state";
import { confidenceOf, verdictOf, watchInfo } from "./verdict";

const PROBLEMS: readonly ConnectProblem[] = [
  "permission_denied",
  "multiple_devices",
  "garmin_folder_not_found",
  "extdata_not_found",
  "device_error",
];

/** The connect problem a failed read shows; any other code is the app's own fault. */
function problemOf(code: string): ConnectProblem {
  return PROBLEMS.find((p) => p === code) ?? "internal";
}

/** The watch the flow is on; every phase past connect has one. */
function currentWatch(state: FlowState): WatchInfo {
  if (!state.inspect) throw new Error(`flow: no watch in ${state.phase.name}`);
  return watchInfo(state.inspect);
}

/** The watch a share is about: the drive's or the one read over MTP; none from connect. */
function sharedWatch(state: FlowState, back: Phase): WatchInfo | null {
  if (back.name === "drive") return watchInfo(back.drive);
  if (back.name === "connect" || !state.inspect) return null;
  return watchInfo(state.inspect);
}

function sharedBefore(state: FlowState, source: InspectOutput | InspectDriveOutput, result: ShareKey["result"]): boolean {
  const key = { modelPart: source.device.part_number, software: source.device.software_version, result };
  return state.shared[shareKeyId(key)] ?? false;
}

function summary(state: FlowState, copy: Copy, phase: Extract<Phase, { name: "share" }>) {
  const s = copy.share.summary;
  const watch = sharedWatch(state, phase.back);
  const rows: { label: string; value: string }[] = [];
  if (watch) rows.push({ label: s.watch, value: watch.model }, { label: s.software, value: watch.software });
  const report = phase.input.report;
  if (phase.what !== "font_list" && report) {
    const last = state.lastOp;
    const count = last && "output" in last ? last.output.packs.reduce((n, pack) => n + pack.parts.length, 0) : 0;
    rows.push({ label: s.installed, value: count ? fmt(s.fonts, { count }) : s.nothing });
  }
  if (phase.what === "result") {
    const { result, views, phone } = answersFor(state);
    const results: Record<string, string> = copy.check.answer.results;
    if (result) rows.push({ label: s.result, value: results[result] ?? result });
    if (views.length) {
      rows.push({ label: s.views, value: list(state.lang, views.map((view) => copy.check.answer.views[view])) });
    }
    if (phone) rows.push({ label: s.phone, value: copy.check.answer.phone[phone] });
  }
  if (phase.what === "problem") {
    const back = phase.back;
    const value =
      back.name === "error"
        ? copy.error[errorGroup(back.failure.code)].title
        : back.name === "connect" && back.mode === "problem"
          ? copy.connect.problem[problemOf(back.failure.code)].title
          : back.name === "connect" && back.mode === "busy"
            ? (back.failure.busy?.holder.name ?? copy.connect.busy.program.unknown)
            : null;
    if (value) rows.push({ label: s.problem, value });
  }
  if (report) rows.push({ label: s.fontList, value: fmt(s.fonts, { count: report.extdata_files.length }) });
  return rows;
}

function rollbackOf(state: FlowState): RollbackView | null {
  const phase = state.phase;
  const rollback = phase.name === "error" ? phase.failure.rollback : undefined;
  if (!rollback) return null;
  const files = rollback.status === "incomplete" ? rollback.files.filter((f) => !f.done) : rollback.files;
  return { status: rollback.status, files: files.map((f) => f.path) };
}

export function toScreen(state: FlowState): ScreenModel {
  const phase = state.phase;
  switch (phase.name) {
    case "connect":
      switch (phase.mode) {
        case "searching":
          return { screen: "connect", mode: "searching", showHelp: phase.showHelp };
        case "busy": {
          const holder = phase.failure.busy?.holder;
          return {
            screen: "connect",
            mode: "busy",
            busy: { kind: holder?.kind ?? "unknown", name: holder?.name ?? null, confirmed: holder?.confirmed ?? false },
          };
        }
        case "problem":
          return { screen: "connect", mode: "problem", problem: problemOf(phase.failure.code), detail: phase.failure.message };
      }
    case "drive": {
      const { support } = phase.drive;
      return {
        screen: "drive",
        watch: watchInfo(phase.drive),
        driveOnly: support.status === "unsupported",
        issue: support.issue ?? null,
        reportWanted: support.report_wanted,
        sharedBefore: sharedBefore(state, phase.drive, "font_list"),
      };
    }
    case "watch": {
      const inspect = state.inspect;
      if (!inspect) throw new Error("flow: no watch in watch");
      return {
        screen: "watch",
        watch: watchInfo(inspect),
        verdict: verdictOf(inspect),
        issue: inspect.support.issue ?? null,
        reportWanted: inspect.support.report_wanted,
        sharedBefore: sharedBefore(state, inspect, "font_list"),
      };
    }
    case "confirm_restore":
      return { screen: "confirm_restore", watch: currentWatch(state) };
    case "working":
      return { screen: "working", op: phase.op, watch: currentWatch(state), progress: phase.progress };
    case "restart":
      return {
        screen: "restart",
        op: phase.op,
        watch: currentWatch(state),
        phase: phase.phase,
        attempt: Math.max(state.attempt, 1),
        verifiable: phase.verifiable,
      };
    case "result": {
      const inspect = state.inspect;
      if (!inspect) throw new Error("flow: no watch in result");
      return { screen: "result", op: phase.op, watch: watchInfo(inspect), confidence: confidenceOf(inspect) };
    }
    case "check":
      return {
        screen: "check",
        watch: currentWatch(state),
        step: phase.step,
        answers: answersFor(state),
        phone: state.phone ? { url: state.phone.page.url, qr: state.phone.page.qr } : null,
        markPictures: (state.marks ?? []).map((mark) => ({ variant: mark.variant, picture: mark.picture })),
      };
    case "outcome": {
      const inspect = state.inspect;
      if (!inspect) throw new Error("flow: no watch in outcome");
      const result = answersFor(state).result;
      return {
        screen: "outcome",
        watch: watchInfo(inspect),
        kind: phase.kind,
        restarts: state.attempt,
        sharedBefore: result ? sharedBefore(state, inspect, result) : false,
      };
    }
    case "share": {
      const result = answersFor(state).result;
      return {
        screen: "share",
        watch: sharedWatch(state, phase.back),
        what: phase.what,
        summary: summary(state, copyFor(state.lang), phase),
        reportText: phase.link?.report_text ?? "",
        state: phase.state,
        thanks:
          phase.state === "opened" || phase.state === "opened_paste"
            ? phase.what === "result" && result === "georgian"
              ? "tested_next"
              : "we_look"
            : null,
      };
    }
    case "error":
      return {
        screen: "error",
        op: phase.op,
        watch: currentWatch(state),
        code: phase.failure.code,
        rollback: rollbackOf(state),
        detail: phase.failure.message,
      };
  }
}

/** The More sheet's data (§5.2, More). */
export function moreModel(state: FlowState): MoreModel {
  const phase = state.phase;
  const source = phase.name === "drive" ? phase.drive : state.inspect;
  const busy = phase.name === "working" || phase.name === "confirm_restore";
  const labels = copyFor(state.lang).more.seen.labels;
  const seen = source && [
    { label: labels.model, value: source.device.model },
    { label: labels.modelPart, value: source.device.part_number },
    { label: labels.software, value: source.device.software_version },
    {
      label: labels.writes,
      value: "packs" in source && source.packs.length ? source.packs.map((pack) => pack.name).join(", ") : "-",
    },
    ...("state" in source ? [{ label: labels.state, value: source.state }] : []),
  ];
  return {
    restorable: !busy && phase.name !== "drive" && (state.inspect?.restorable ?? false),
    seen,
    reportText: source?.report.text ?? null,
    version: state.version,
  };
}
