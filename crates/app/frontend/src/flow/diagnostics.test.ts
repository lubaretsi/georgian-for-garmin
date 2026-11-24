// Diagnostic info (contract §11): the diagnostics phase, its effects, and the session's event log.

import { describe, expect, test } from "vitest";

import type { DiagnoseOutput, Failure, InspectOutput } from "../core/types.gen";
import { failure, output } from "../mock/fixtures";
import type { ActionId, ActionPayload } from "../model/screens";
import { type FlowEvent, type FlowState, initialState, localTime, MAX_EVENTS, POLL_MS, reduce } from "./state";

const NOW = new Date(2026, 10, 24, 3, 14, 5);
const TIME = "2026-11-24T03:14:05";
const at = (state: FlowState, event: FlowEvent) => reduce(state, event, NOW);
const inspect = (scenario: string, step: string) => output(scenario, step, "inspect");
const NO_DEVICE = failure("connect-problems", "inspect-no_device");
const BUSY = failure("busy", "inspect-garmin_express-macos");
const VENU2 = inspect("venu2", "inspect");

const started = () => at(initialState("en", "0.3.4"), { type: "start" });
const read = (state: FlowState, result: InspectOutput | Failure): FlowState =>
  at(
    state,
    "code" in result
      ? { type: "inspect_failed", token: state.token, failure: result }
      : { type: "inspected", token: state.token, output: result },
  );
const act = (state: FlowState, id: ActionId, payload?: ActionPayload) =>
  at(state, { type: "action", id, ...(payload && { payload }) });
const kinds = (state: FlowState) => state.effects.map((e) => e.kind);

const DIAGNOSED: DiagnoseOutput = {
  app: { version: "0.3.4", build: "release" },
  system: { os: "Mac OS", os_version: "15.6.1", arch: "aarch64" },
  usb: { listed: true, note: null, devices: [] },
  connection: { ok: false, code: "no_device", message: NO_DEVICE.message, busy: null },
  drives: { listed: true, note: null, volumes: [] },
  watch: null,
  events: [],
  text: "Georgian for Garmin diagnostic info. It contains no serial number or watch ID.\n",
};

test("localTime has no zone", () => {
  expect(localTime(NOW)).toBe(TIME);
});

describe("the diagnostics phase", () => {
  test("opens from any screen, collecting, with the session's events", () => {
    const watch = read(started(), VENU2);
    const state = act(watch, "diagnostics");
    expect(state.phase).toEqual({ name: "diagnostics", back: { name: "watch" }, state: "collecting", output: null, savedPath: null });
    expect(state.effects).toEqual([{ kind: "diagnose", events: watch.events }]);
  });

  test("pauses polling, and Back returns to the exact phase, reading again", () => {
    const searching = read(started(), NO_DEVICE);
    const open = act(searching, "diagnostics");
    expect(open.token).toBe(searching.token + 1);
    // A poll already in flight is dropped; no new ones while it's open.
    expect(read({ ...open, token: searching.token }, VENU2).phase.name).toBe("diagnostics");
    const back = act(open, "back");
    expect(back.phase).toEqual(searching.phase);
    expect(back.effects).toContainEqual({ kind: "inspect", token: back.token, delay: 0, slowAfter: null });
  });

  test("Back from a phase that doesn't poll: no read", () => {
    const outcome = act(read(started(), VENU2), "install");
    const back = act(act(outcome, "diagnostics"), "back");
    expect(back.phase).toEqual(outcome.phase);
    expect(kinds(back)).toEqual([]);
  });

  test("not from the share screens", () => {
    const share = act(read(started(), inspect("forerunner265", "inspect")), "share", { action: "share", what: "font_list" });
    expect(act(share, "diagnostics").phase.name).toBe("share");
  });

  test("ready → save → saved → show in folder; copy → copied", () => {
    let state = at(act(read(started(), NO_DEVICE), "diagnostics"), { type: "diagnosed", output: DIAGNOSED });
    expect(state.phase).toMatchObject({ state: "ready", output: DIAGNOSED });
    expect(act(state, "show_file").effects).toEqual([]); // nothing saved yet
    state = act(state, "save_diagnostics");
    expect(state.effects).toEqual([{ kind: "save_diagnostics", text: DIAGNOSED.text }]);
    state = at(state, { type: "diagnostics_saved", path: "~/Downloads/x.txt" });
    expect(state.phase).toMatchObject({ state: "saved", savedPath: "~/Downloads/x.txt" });
    expect(act(state, "show_file").effects).toEqual([{ kind: "reveal_file", path: "~/Downloads/x.txt" }]);
    const copied = at(act(state, "copy_diagnostics"), { type: "diagnostics_copied" });
    expect(act(state, "copy_diagnostics").effects).toEqual([{ kind: "copy_text", text: DIAGNOSED.text }]);
    expect(copied.phase).toMatchObject({ state: "copied" });
  });

  test("save and copy wait for the text", () => {
    const collecting = act(read(started(), NO_DEVICE), "diagnostics");
    expect(act(collecting, "save_diagnostics").effects).toEqual([]);
    expect(act(collecting, "copy_diagnostics").effects).toEqual([]);
  });

  test("any failure → failed", () => {
    const collecting = act(read(started(), NO_DEVICE), "diagnostics");
    const failed = at(collecting, { type: "diagnostics_failed", failure: { code: "internal", message: "x" } });
    expect(failed.phase).toMatchObject({ state: "failed" });
  });

  test("an operation running behind it still finishes, and its result is there on Back", () => {
    const working = act(read(started(), VENU2), "install");
    let state = act(working, "diagnostics");
    state = at(state, { type: "op_done", op: "install", output: output("venu2", "install", "install") });
    expect(state.phase).toMatchObject({ name: "diagnostics", back: { name: "restart", phase: "unplug" } });
    expect(kinds(state)).toEqual([]); // the restart's poll waits for Back
    const back = act(state, "back");
    expect(back.phase).toMatchObject({ name: "restart", phase: "unplug" });
    expect(kinds(back)).toEqual(["inspect"]);
  });

  test("restore isn't offered from it", () => {
    const open = act(read(started(), VENU2), "diagnostics");
    expect(act(open, "restore").phase.name).toBe("diagnostics");
  });
});

describe("the session's event log", () => {
  test("polls are logged when their result changes, with the model and no identifiers", () => {
    let state = read(started(), NO_DEVICE);
    state = read(state, NO_DEVICE);
    state = read(state, VENU2);
    state = read(state, VENU2);
    expect(state.events).toEqual([
      { time: TIME, kind: "poll", detail: "no_device", model: null },
      { time: TIME, kind: "poll", detail: "not_installed", model: "Venu 2" },
    ]);
    expect(JSON.stringify(state.events)).not.toMatch(/\d{9,}|006-B3703/);
  });

  test("busy, drive, operations, answers and shares", () => {
    let state = read(started(), BUSY);
    // The holder is kept, so the summary can name the program.
    expect(state.lastProblem).toEqual({ time: TIME, code: "device_busy", holder: BUSY.busy?.holder });
    state = read(state, VENU2);
    state = act(state, "install");
    state = at(state, { type: "op_done", op: "install", output: output("venu2", "install", "install") });
    state = read(read(state, NO_DEVICE), inspect("venu2", "inspect-restarted"));
    state = act(act(state, "check"), "send_test_done");
    state = act(state, "answer", { action: "answer", answers: { result: "georgian", views: [] } });
    state = act(state, "done");
    state = act(state, "share", { action: "share", what: "result" });
    state = at(state, { type: "shared", shared: { complete: true, url: "https://github.com/x" } });
    expect(state.events.map((e) => [e.kind, e.detail])).toEqual([
      ["poll", "device_busy"],
      ["poll", "not_installed"],
      ["operation_start", "install"],
      ["operation_end", "install: staged_verified"],
      ["poll", "no_device"],
      ["poll", "installed"],
      ["answer", "result: georgian"],
      ["share", "result: opened"],
    ]);
    expect(state.lastProblem).toEqual({ time: TIME, code: "no_device", holder: null });
  });

  test("a failed operation is logged and becomes the last problem", () => {
    const fail = failure("venu2-write-fails", "install-rollback-incomplete");
    const state = at(act(read(started(), VENU2), "install"), { type: "op_failed", op: "install", failure: fail });
    expect(state.events.at(-1)).toEqual({ time: TIME, kind: "operation_end", detail: "install: device_error", model: "Venu 2" });
    expect(state.lastProblem).toEqual({ time: TIME, code: "device_error", holder: null });
  });

  test("a drive is logged as one", () => {
    let state = read(started(), NO_DEVICE);
    const drive = output("forerunner245-drive", "inspect_drive", "inspect_drive");
    state = at(state, { type: "drive_inspected", token: state.token, output: drive });
    expect(state.events.at(-1)).toEqual({ time: TIME, kind: "poll", detail: "usb_drive", model: "Forerunner 245" });
  });

  test(`keeps the last ${MAX_EVENTS}`, () => {
    let state = started();
    for (let i = 0; i < 40; i++) state = read(read(state, NO_DEVICE), VENU2);
    expect(state.events).toHaveLength(MAX_EVENTS);
    expect(state.events.at(-1)).toMatchObject({ detail: "not_installed" });
  });

  test("stale results aren't logged", () => {
    const state = started();
    expect(at(state, { type: "inspected", token: state.token - 1, output: VENU2 }).events).toEqual([]);
  });

  test("the poll interval stays 2 s", () => {
    expect(read(started(), NO_DEVICE).effects).toContainEqual({ kind: "inspect", token: 1, delay: POLL_MS, slowAfter: null });
  });
});
