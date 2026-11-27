// toScreen for each screen, and the More sheet, from flow states the reducer reached with the
// core's real results.

import { describe, expect, test } from "vitest";

import { copyFor } from "../copy";
import type { DiagnoseOutput, Failure, InspectOutput } from "../core/types.gen";
import { failure, output } from "../mock/fixtures";
import type { ActionId, ActionPayload } from "../model/screens";
import { moreModel, toScreen } from "./screen";
import { type FlowState, initialState, reduce, shareKeyId } from "./state";

const inspect = (scenario: string, step: string) => output(scenario, step, "inspect");
const NO_DEVICE = failure("connect-problems", "inspect-no_device");
const VENU2 = inspect("venu2", "inspect");
const VENU2_INFO = { model: "Venu 2", modelPart: "006-B3703-00", software: "19.05", confirmedSoftware: ["19.05"] };

const started = (lang: "ka" | "en" = "en") => reduce(initialState(lang, "0.3.4"), { type: "start" });
const read = (state: FlowState, result: InspectOutput | Failure): FlowState =>
  reduce(
    state,
    "code" in result
      ? { type: "inspect_failed", token: state.token, failure: result }
      : { type: "inspected", token: state.token, output: result },
  );
const act = (state: FlowState, id: ActionId, payload?: ActionPayload) =>
  reduce(state, { type: "action", id, ...(payload && { payload }) });
const atWatch = () => read(started(), VENU2);
const written = () => reduce(act(atWatch(), "install"), { type: "op_done", op: "install", output: output("venu2", "install", "install") });
const atResult = () => read(read(written(), NO_DEVICE), inspect("venu2", "inspect-restarted"));

describe("toScreen", () => {
  test("connect: searching, then with help", () => {
    const state = started();
    expect(toScreen(state)).toEqual({ screen: "connect", mode: "searching", showHelp: false });
    expect(toScreen(reduce(state, { type: "help_due", token: state.helpToken }))).toEqual({
      screen: "connect",
      mode: "searching",
      showHelp: true,
    });
  });

  test("connect: busy, with the holder as the core named it", () => {
    expect(toScreen(read(started(), failure("busy", "inspect-garmin_express-windows")))).toEqual({
      screen: "connect",
      mode: "busy",
      busy: { kind: "garmin_express", name: "ExpressTray.exe", confirmed: false },
      duringRestart: false,
    });
    expect(toScreen(read(started(), failure("busy", "inspect-unknown-linux")))).toMatchObject({
      busy: { kind: "unknown", name: null, confirmed: false },
    });
  });

  test("connect: a problem, with the core's message as the detail; unknown codes are internal", () => {
    const denied = failure("connect-problems", "inspect-permission_denied");
    expect(toScreen(read(started(), denied))).toEqual({
      screen: "connect",
      mode: "problem",
      problem: "permission_denied",
      detail: denied.message,
    });
    const odd: Failure = { code: "invalid_input", message: "x" };
    expect(toScreen(read(started(), odd))).toMatchObject({ problem: "internal" });
  });

  test("drive: drive-only and MTP-capable", () => {
    const drive = (scenario: string) => {
      const state = read(started(), NO_DEVICE);
      const out = output(scenario, "inspect_drive", "inspect_drive");
      return toScreen(reduce(state, { type: "drive_inspected", token: state.token, output: out }));
    };
    expect(drive("drive-only")).toEqual({
      screen: "drive",
      watch: { model: "Example Drive Only", modelPart: "006-B9906-00", software: "13.70", confirmedSoftware: [] },
      driveOnly: true,
      issue: 16,
      reportWanted: false,
      sharedBefore: false,
      duringRestart: false,
    });
    expect(drive("venu2-drive")).toMatchObject({ driveOnly: false, issue: null });
  });

  test("watch: the verdict, issue, report wanted and shared before", () => {
    expect(toScreen(atWatch())).toEqual({
      screen: "watch",
      watch: VENU2_INFO,
      verdict: { kind: "install", confidence: "confirmed" },
      issue: null,
      reportWanted: false,
      sharedBefore: false,
    });
    const predicted = read(started(), inspect("predicted", "inspect"));
    const shared = reduce(predicted, {
      type: "shared_before",
      key: { modelPart: "006-B9907-00", software: "29.52", result: "font_list" },
      value: true,
    });
    expect(toScreen(shared)).toMatchObject({ verdict: { kind: "no_font" }, reportWanted: true, sharedBefore: true });
  });

  test("watch: confirmed on other software carries the confirmed versions", () => {
    const otherSoftware = read(started(), inspect("other-software", "inspect"));
    expect(toScreen(otherSoftware)).toMatchObject({
      watch: { software: "30.00", confirmedSoftware: ["29.52"] },
      verdict: { kind: "install", confidence: "confirmed_other_software" },
    });
  });

  test("confirm_restore and working, with progress", () => {
    expect(toScreen(act(atWatch(), "restore"))).toEqual({ screen: "confirm_restore", watch: VENU2_INFO });
    const working = act(atWatch(), "install");
    expect(toScreen(working)).toEqual({ screen: "working", op: "install", watch: VENU2_INFO, progress: null });
    expect(toScreen(reduce(working, { type: "progress", progress: { done: 1, total: 4 } }))).toMatchObject({
      progress: { done: 1, total: 4 },
    });
  });

  test("restart: phase, attempt, verifiable", () => {
    expect(toScreen(written())).toEqual({
      screen: "restart",
      op: "install",
      watch: VENU2_INFO,
      phase: "unplug",
      attempt: 1,
      verifiable: true,
      garminMode: null,
    });
    const again = read(read(written(), NO_DEVICE), inspect("venu2", "inspect-staged"));
    expect(toScreen(read(again, NO_DEVICE))).toMatchObject({ phase: "restart", attempt: 2 });
  });

  test("Garmin mode: switching, then the guidance, on connect and inside the restart (§4.2)", () => {
    const garmin = failure("garmin-mode", "inspect-garmin_mode");
    const locked = failure("garmin-mode", "switch_to_mtp-device_locked");
    const switching = read(started(), garmin);
    expect(toScreen(switching)).toEqual({ screen: "connect", mode: "switching" });
    expect(toScreen(reduce(switching, { type: "switch_failed", failure: locked }))).toEqual({
      screen: "connect",
      mode: "problem",
      problem: "device_locked",
      detail: locked.message,
    });
    const windows = failure("garmin-mode", "inspect-garmin_mode-windows");
    expect(toScreen(read(started(), windows))).toEqual({
      screen: "connect",
      mode: "problem",
      problem: "garmin_mode",
      detail: windows.message,
    });
    const restart = read(read(written(), NO_DEVICE), garmin);
    expect(toScreen(restart)).toMatchObject({ screen: "restart", phase: "restart", garminMode: "switching" });
    const timedOut = output("garmin-mode", "switch_to_mtp-timed_out", "switch_to_mtp");
    expect(toScreen(reduce(restart, { type: "switched", output: timedOut }))).toMatchObject({ garminMode: "guidance" });
    expect(toScreen(reduce(restart, { type: "switch_failed", failure: locked }))).toMatchObject({ garminMode: "locked" });
  });

  test("result: install, with the confidence", () => {
    expect(toScreen(atResult())).toEqual({ screen: "result", op: "install", watch: VENU2_INFO, confidence: "confirmed" });
  });

  test("check: answers, phone page, and the marks of the parts install writes", () => {
    let state = act(atResult(), "check");
    expect(toScreen(state)).toEqual({
      screen: "check",
      watch: VENU2_INFO,
      step: "send",
      answers: { views: [] },
      phone: null,
      markPictures: [],
    });
    const page = { url: "https://lubaretsi.github.io/georgian-for-garmin/?lang=en", qr: "data:image/svg+xml;base64,AA" };
    const mark = { part: "006-D9035-01", kind: "bitmap" as const, variant: "A", style: "bar", picture: "data:image/png;base64,AA" };
    // A part the watch lists but install doesn't write, with the same variant letter in its kind.
    const other = { part: "006-D8945-22", kind: "truetype" as const, variant: "A", style: "bar", picture: "data:image/png;base64,BB" };
    // A part this watch doesn't list.
    const unlisted = { ...mark, part: "006-D1234-01", variant: "B", picture: "data:image/png;base64,CC" };
    state = reduce(state, { type: "phone_loaded", lang: "en", page });
    state = reduce(state, { type: "marks_loaded", marks: [mark, other, unlisted] });
    state = act(act(state, "send_test_done"), "answer", { action: "answer", answers: { result: "boxes", views: [] } });
    expect(toScreen(state)).toMatchObject({
      step: "answer",
      answers: { result: "boxes", views: [] },
      phone: { url: page.url, qr: page.qr },
      markPictures: [{ variant: "A", picture: mark.picture }],
    });
  });

  test("outcome: kind, restarts, shared before, and the answer given", () => {
    let state = act(act(atResult(), "check"), "send_test_done");
    state = act(act(state, "answer", { action: "answer", answers: { result: "boxes", views: [] } }), "done");
    expect(toScreen(state)).toEqual({
      screen: "outcome",
      watch: VENU2_INFO,
      kind: "didnt_show",
      restarts: 1,
      sharedBefore: false,
      result: "boxes",
    });
    let some = act(act(atResult(), "check"), "send_test_done");
    some = act(act(some, "answer", { action: "answer", answers: { result: "some_letters", views: [] } }), "done");
    expect(toScreen(some)).toMatchObject({ kind: "didnt_show", result: "some_letters" });
  });

  describe("diagnostics", () => {
    const diagnosed = (base: FlowState, connection: DiagnoseOutput["connection"], watch: DiagnoseOutput["watch"]) =>
      reduce(act(base, "diagnostics"), {
        type: "diagnosed",
        output: {
          app: { version: "0.3.4", build: "release" },
          system: { os: "Mac OS", os_version: "15.6.1", arch: "aarch64" },
          usb: { listed: true, note: null, devices: [] },
          connection,
          drives: { listed: true, note: null, volumes: [] },
          watch,
          events: [],
          text: "Georgian for Garmin diagnostic info. It contains no serial number or watch ID.\n",
        },
      });

    test("collecting: only what the app knows", () => {
      expect(toScreen(act(started(), "diagnostics"))).toEqual({
        screen: "diagnostics",
        summary: [{ label: "App version", value: "0.3.4" }],
        text: "",
        state: "collecting",
        savedPath: null,
      });
    });

    test("ready, no watch: plain words, never the code", () => {
      const state = diagnosed(read(started(), NO_DEVICE), { ok: false, code: "no_device", message: "x", busy: null }, null);
      expect(toScreen(state)).toMatchObject({
        state: "ready",
        summary: [
          { label: "App version", value: "0.3.4" },
          { label: "System", value: "Mac OS 15.6.1 (aarch64)" },
          { label: "Watch", value: "Not found" },
          { label: "Connection", value: "No watch found" },
          { label: "Last problem", value: expect.stringMatching(/^\d\d:\d\d:\d\d — No watch found$/) },
        ],
      });
    });

    test("ready, busy: the busy screen's title, naming the holder", () => {
      const busy = failure("busy", "inspect-garmin_express-macos");
      const state = diagnosed(read(started(), busy), { ok: false, code: "device_busy", message: "x", busy: busy.busy ?? null }, null);
      expect(toScreen(state)).toMatchObject({
        summary: expect.arrayContaining([
          { label: "Connection", value: "Garmin Express is using your watch" },
          { label: "Last problem", value: expect.stringMatching(/^\d\d:\d\d:\d\d — Garmin Express is using your watch$/) },
        ]),
      });
      const unknown = diagnosed(read(started(), busy), { ok: false, code: "device_busy", message: "x", busy: null }, null);
      expect(toScreen(unknown)).toMatchObject({
        summary: expect.arrayContaining([{ label: "Connection", value: "Another program may be using your watch" }]),
      });
    });

    test("ready, a connect problem or an error: its screen's title, in the state's language", () => {
      const denied = diagnosed(started(), { ok: false, code: "permission_denied", message: "x", busy: null }, null);
      expect(toScreen(denied)).toMatchObject({
        summary: expect.arrayContaining([{ label: "Connection", value: "Linux doesn't let the app open the watch yet" }]),
      });
      const inKa = diagnosed(started("ka"), { ok: false, code: "verification_failed", message: "x", busy: null }, null);
      const ka = copyFor("ka");
      expect(toScreen(inKa)).toMatchObject({
        summary: expect.arrayContaining([{ label: ka.diagnostics.summary.connection, value: ka.error.verification_failed.title }]),
      });
    });

    test("ready, connected as a USB drive", () => {
      const state = reduce(act(started(), "diagnostics"), {
        type: "diagnosed",
        output: {
          app: { version: "0.3.4", build: "release" },
          system: { os: "Mac OS", os_version: "15.6.1", arch: "aarch64" },
          usb: { listed: true, note: null, devices: [] },
          connection: { ok: false, code: "no_device", message: "x", busy: null },
          drives: {
            listed: true,
            note: null,
            volumes: [{ volume: "/Volumes/GARMIN", model: "Forerunner 245", part_number: null, software_version: "8.20", error: null }],
          },
          watch: null,
          events: [],
          text: "x\n",
        },
      });
      expect(toScreen(state)).toMatchObject({
        summary: expect.arrayContaining([
          { label: "Watch", value: "Forerunner 245, Software 8.20" },
          { label: "Connection", value: "Connected as a USB drive" },
        ]),
      });
    });

    test("ready, watch found; then saved with its path", () => {
      const watch = read(started(), VENU2);
      let state = diagnosed(watch, { ok: true, code: null, message: null, busy: null }, VENU2.report);
      expect(toScreen(state)).toMatchObject({
        summary: [
          { label: "App version", value: "0.3.4" },
          { label: "System", value: "Mac OS 15.6.1 (aarch64)" },
          { label: "Watch", value: "Venu 2, Software 19.05" },
          { label: "Connection", value: "Connected (Venu 2)" },
          { label: "Last problem", value: "None" },
        ],
        text: expect.stringContaining("no serial number or watch ID"),
      });
      state = reduce(state, { type: "diagnostics_saved", path: "~/Downloads/d.txt" });
      expect(toScreen(state)).toMatchObject({ state: "saved", savedPath: "~/Downloads/d.txt" });
    });
  });

  test("share: summary in the state's language, the report text, the state and thanks", () => {
    let state = act(act(atResult(), "check"), "send_test_done");
    state = act(state, "answer", { action: "answer", answers: { result: "georgian", views: ["popup", "list"], phone: "android" } });
    state = act(act(state, "done"), "share", { action: "share", what: "result" });
    const link = output("venu2", "issue_link-georgian", "issue_link");
    state = reduce(state, { type: "link_ready", link });
    expect(toScreen(state)).toEqual({
      screen: "share",
      watch: VENU2_INFO,
      what: "result",
      summary: [
        { label: "Watch", value: "Venu 2" },
        { label: "Software", value: "19.05" },
        { label: "What the app installed", value: "4 fonts" },
        { label: "What the watch showed", value: "Georgian letters" },
        { label: "Where", value: `${copyFor("en").check.answer.views.popup} and ${copyFor("en").check.answer.views.list}` },
        { label: "Phone", value: "Android" },
        { label: "The watch's font list", value: expect.stringMatching(/^\d+ fonts$/) },
      ],
      reportText: link.report_text,
      state: "ready",
      thanks: null,
    });
    const opened = reduce(state, { type: "shared", shared: { complete: false, url: link.url } });
    expect(toScreen(opened)).toMatchObject({ state: "opened_paste", thanks: "tested_next" });
  });

  test("share from connect has no watch", () => {
    const busy = read(started(), failure("busy", "inspect-garmin_express-macos"));
    const state = act(busy, "share", { action: "share", what: "problem" });
    expect(toScreen(state)).toMatchObject({
      screen: "share",
      watch: null,
      summary: [{ label: "What happened", value: "Garmin Express" }],
      thanks: null,
    });
  });

  test("share of a connect problem: no watch, no watch rows", () => {
    const internal = failure("connect-problems", "inspect-internal");
    const state = act(read(started(), internal), "share", { action: "share", what: "problem" });
    expect(toScreen(state)).toMatchObject({
      screen: "share",
      watch: null,
      summary: [{ label: "What happened", value: "Something went wrong in the app" }],
    });
  });

  test("restart interrupted: the busy or drive screen, then the restart again", () => {
    const restart = read(written(), NO_DEVICE);
    const busy = read(restart, failure("busy", "inspect-garmin_express-macos"));
    expect(toScreen(busy)).toEqual({
      screen: "connect",
      mode: "busy",
      busy: { kind: "garmin_express", name: "Garmin Express", confirmed: true },
      duringRestart: true,
    });
    const drive = output("venu2-drive", "inspect_drive", "inspect_drive");
    const mounted = reduce(read(restart, NO_DEVICE), { type: "drive_inspected", token: restart.token, output: drive });
    expect(toScreen(mounted)).toMatchObject({ screen: "drive", driveOnly: false, duringRestart: true });
    expect(toScreen(read(busy, NO_DEVICE))).toMatchObject({ screen: "restart", phase: "restart", attempt: 1 });
  });

  test("error: code, rollback (what may be left) and detail", () => {
    const fail = failure("venu2-write-fails", "install-rollback-incomplete");
    const state = reduce(act(atWatch(), "install"), { type: "op_failed", op: "install", failure: fail });
    expect(toScreen(state)).toEqual({
      screen: "error",
      op: "install",
      watch: VENU2_INFO,
      code: "device_error",
      rollback: { status: "incomplete", files: ["GARMIN/ExtData/006-D9035-02.tmp", "GARMIN/ExtData/006-D9035-01.tmp"] },
      detail: fail.message,
    });
    const complete = failure("venu2-write-fails", "install-rollback-complete");
    const undone = reduce(act(atWatch(), "install"), { type: "op_failed", op: "install", failure: complete });
    expect(toScreen(undone)).toMatchObject({ rollback: { status: "complete", files: complete.rollback?.files.map((f) => f.path) } });
  });
});

describe("moreModel", () => {
  test("no restore from diagnostics", () => {
    expect(moreModel(act(atWatch(), "diagnostics")).restorable).toBe(false);
  });

  test("no watch: nothing seen, no restore", () => {
    expect(moreModel(started())).toEqual({ restorable: false, seen: null, reportText: null, version: "0.3.4" });
  });

  test("a watch: what the app sees, its report, restore", () => {
    expect(moreModel(atWatch())).toEqual({
      restorable: true,
      seen: [
        { label: "Model", value: "Venu 2" },
        { label: "Part number", value: "006-B3703-00" },
        { label: "Software", value: "19.05" },
        { label: "What install writes", value: VENU2.packs.map((p) => p.name).join(", ") },
        { label: "Font state", value: "not_installed" },
      ],
      reportText: VENU2.report.text,
      version: "0.3.4",
    });
  });

  test("Restore stays in More where the screen leads with Follow updates (issue 6)", () => {
    expect(moreModel(read(started(), inspect("font-unused", "inspect"))).restorable).toBe(true);
  });

  test("no restore while writing", () => {
    expect(moreModel(act(atWatch(), "install")).restorable).toBe(false);
  });
});

describe("a drive back during a restart", () => {
  // Another watch than the one installed on, as a USB drive: everything is about the drive.
  const drive = output("drive-only", "inspect_drive", "inspect_drive");
  const atDrive = () => {
    const state = read(read(written(), NO_DEVICE), NO_DEVICE);
    return reduce(state, { type: "drive_inspected", token: state.token, output: drive });
  };

  test("its share: the drive's report, its watch, no install outcome, remembered for the drive", () => {
    const state = atDrive();
    expect(toScreen(state)).toMatchObject({ screen: "drive", duringRestart: true });
    const sharing = act(state, "share", { action: "share", what: "font_list" });
    expect(sharing.effects).toEqual([{ kind: "issue_link", input: { report: drive.report } }]);
    expect(toScreen(sharing)).toMatchObject({ screen: "share", watch: { model: drive.device.model } });
    const opened = reduce(sharing, { type: "shared", shared: { complete: true, url: "https://github.com/x" } });
    const key = { modelPart: drive.device.part_number, software: drive.device.software_version, result: "font_list" as const };
    expect(opened.shared).toEqual({ [shareKeyId(key)]: true });
  });

  test("More: what the app sees of the drive, no restore", () => {
    const more = moreModel(atDrive());
    expect(more.seen?.[0]).toEqual({ label: "Model", value: drive.device.model });
    expect(more.reportText).toBe(drive.report.text);
    expect(more.restorable).toBe(false);
  });
});
