// What the shell draws for a flow state: `toScreen` (contract §5) and `moreModel` (the More
// sheet). Pure; the share summary and "What the app sees" are in the state's language.

import { type Copy, copyFor, fmt, list } from "../copy";
import type { Lang } from "../core/backend";
import type {
  DiagnoseOutput,
  ErrorCode,
  Failure,
  Holder,
  InspectDriveOutput,
  InspectOutput,
  ShareKey,
} from "../core/types.gen";
import type { ConnectProblem, MoreModel, RollbackView, ScreenModel, WatchInfo } from "../model/screens";
import { programName } from "../screens/Connect";
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

/**
 * A failure in the summary's plain words (§2.3), from the connect and error screens' copy: the
 * busy title naming the holder when known (this very app in its own words), the connect
 * problem's or the error's title. The raw code, and `diagnose`'s notes on what this system
 * can't tell, stay in the full text.
 */
function problemWords(copy: Copy, code: ErrorCode, holder: Holder | null): string {
  if (code === "no_device") return copy.diagnostics.summary.noWatchFound;
  if (code === "device_busy") {
    const b = copy.connect.busy;
    if (holder?.kind === "this_app") return b.thisApp.title;
    const busy = { kind: holder?.kind ?? "unknown", name: holder?.name ?? null, confirmed: holder?.confirmed ?? false };
    const { program } = programName(copy, null, busy);
    return fmt(busy.confirmed ? b.title.confirmed : b.title.unconfirmed, { program });
  }
  const problem = problemOf(code);
  return problem === "internal" ? copy.error[errorGroup(code)].title : copy.connect.problem[problem].title;
}

/**
 * The diagnostics summary (§11): app version, system, the watch found or not, the connection
 * result and the last problem, in plain words. Until `diagnose` answers (`output` null), only
 * what the app itself knows.
 */
export function diagnosticsSummary(
  lang: Lang,
  version: string,
  output: DiagnoseOutput | null,
  lastProblem: FlowState["lastProblem"],
): { label: string; value: string }[] {
  const copy = copyFor(lang);
  const s = copy.diagnostics.summary;
  const rows = [{ label: s.app, value: output?.app.version ?? version }];
  if (!output) return rows;
  const { system, watch, connection } = output;
  const software = (v: string) => fmt(copy.watchCard.software, { software: v });
  // Not over MTP, but mounted as a USB drive: the drive screen's case.
  const drive = !connection.ok && connection.code === "no_device" ? (output.drives.volumes[0] ?? null) : null;
  const found = watch
    ? `${watch.model}, ${software(watch.software_version)}`
    : drive?.model
      ? drive.software_version
        ? `${drive.model}, ${software(drive.software_version)}`
        : drive.model
      : s.noWatch;
  const connected = watch ? fmt(s.connectedTo, { model: watch.model }) : s.connected;
  rows.push(
    { label: s.system, value: `${system.os} ${system.os_version} (${system.arch})` },
    { label: s.watch, value: found },
    {
      label: s.connection,
      value: connection.ok
        ? connected
        : drive
          ? s.usbDrive
          : connection.code
            ? problemWords(copy, connection.code, connection.busy?.holder ?? null)
            : s.none,
    },
    {
      label: s.lastProblem,
      value: lastProblem
        ? `${lastProblem.time.slice(11)} — ${problemWords(copy, lastProblem.code, lastProblem.holder)}`
        : s.none,
    },
  );
  return rows;
}

function rollbackOf(state: FlowState): RollbackView | null {
  const phase = state.phase;
  const rollback = phase.name === "error" ? phase.failure.rollback : undefined;
  if (!rollback) return null;
  const files = rollback.status === "incomplete" ? rollback.files.filter((f) => !f.done) : rollback.files;
  return { status: rollback.status, files: files.map((f) => f.path) };
}

/** connect/busy for a read another program refused; `duringRestart` when it interrupts one. */
function busyScreen(failure: Failure, duringRestart: boolean): ScreenModel {
  const holder = failure.busy?.holder;
  return {
    screen: "connect",
    mode: "busy",
    busy: { kind: holder?.kind ?? "unknown", name: holder?.name ?? null, confirmed: holder?.confirmed ?? false },
    duringRestart,
  };
}

function driveScreen(state: FlowState, drive: InspectDriveOutput, duringRestart: boolean): ScreenModel {
  const { support } = drive;
  return {
    screen: "drive",
    watch: watchInfo(drive),
    driveOnly: support.status === "unsupported",
    issue: support.issue ?? null,
    reportWanted: support.report_wanted,
    sharedBefore: sharedBefore(state, drive, "font_list"),
    duringRestart,
  };
}

export function toScreen(state: FlowState): ScreenModel {
  const phase = state.phase;
  switch (phase.name) {
    case "connect":
      switch (phase.mode) {
        case "searching":
          return { screen: "connect", mode: "searching", showHelp: phase.showHelp };
        case "busy":
          return busyScreen(phase.failure, false);
        case "problem":
          return { screen: "connect", mode: "problem", problem: problemOf(phase.failure.code), detail: phase.failure.message };
      }
    case "drive":
      return driveScreen(state, phase.drive, false);
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
      // Held or mounted after the restart: that screen, with the restart kept underneath (§4.1).
      if (phase.interruption?.kind === "busy") return busyScreen(phase.interruption.failure, true);
      if (phase.interruption?.kind === "drive") return driveScreen(state, phase.interruption.drive, true);
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
        result: result ?? null,
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
    case "diagnostics":
      return {
        screen: "diagnostics",
        summary: diagnosticsSummary(state.lang, state.version, phase.output, state.lastProblem),
        text: phase.output?.text ?? "",
        state: phase.state,
        savedPath: phase.savedPath,
      };
  }
}

/** The More sheet's data (§5.2, More). */
export function moreModel(state: FlowState): MoreModel {
  const phase = state.phase;
  const source = phase.name === "drive" ? phase.drive : state.inspect;
  const busy = phase.name === "working" || phase.name === "confirm_restore" || phase.name === "diagnostics";
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
