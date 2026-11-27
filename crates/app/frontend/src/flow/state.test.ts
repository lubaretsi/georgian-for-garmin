// Every transition of the journey (contract §4), with the core's real results as events.

import { describe, expect, test } from "vitest";

import type { Failure, InspectOutput } from "../core/types.gen";
import { failure, output } from "../mock/fixtures";
import type { ActionId, ActionPayload, Answers } from "../model/screens";
import { moreModel } from "./screen";
import {
  answersFor,
  type Effect,
  type FlowEvent,
  type FlowState,
  HELP_MS,
  initialState,
  POLL_MS,
  reduce,
  SLOW_MS,
  STARTING_MS,
} from "./state";

const inspect = (scenario: string, step: string) => output(scenario, step, "inspect");
const NO_DEVICE = failure("connect-problems", "inspect-no_device");
const VENU2 = inspect("venu2", "inspect");

function started(): FlowState {
  return reduce(initialState("en", "0.3.4"), { type: "start" });
}

/** The result of the poll the flow is waiting for. */
const read = (state: FlowState, result: InspectOutput | Failure): FlowState =>
  reduce(
    state,
    "code" in result
      ? { type: "inspect_failed", token: state.token, failure: result }
      : { type: "inspected", token: state.token, output: result },
  );

const act = (state: FlowState, id: ActionId, payload?: ActionPayload): FlowState =>
  reduce(state, { type: "action", id, ...(payload && { payload }) });

const run = (state: FlowState, ...events: FlowEvent[]) => events.reduce((s, e) => reduce(s, e), state);

const kinds = (state: FlowState) => state.effects.map((e) => e.kind);
const polls = (state: FlowState): Effect | undefined => state.effects.find((e) => e.kind === "inspect");

/** The Venu 2 on its watch screen. */
const atWatch = () => read(started(), VENU2);

/** The Venu 2 after install, at the restart step's first phase. */
function written(): FlowState {
  const working = act(atWatch(), "install");
  return reduce(working, { type: "op_done", op: "install", output: output("venu2", "install", "install") });
}

/** The Venu 2 back after a restart, its first read decided. */
function reconnected(result: InspectOutput): FlowState {
  return read(read(written(), NO_DEVICE), result);
}

describe("looking for the watch", () => {
  test("starts with a read and the help timer", () => {
    const state = started();
    expect(state.phase).toEqual({ name: "connect", mode: "searching", showHelp: false });
    expect(state.effects).toEqual([
      { kind: "help_timer", token: 1, delay: HELP_MS },
      { kind: "inspect", token: 1, delay: 0, slowAfter: null },
    ]);
  });

  test("no watch: keeps reading every 2 s and tries the drive", () => {
    const state = read(started(), NO_DEVICE);
    expect(state.phase.name).toBe("connect");
    expect(state.effects).toEqual([
      { kind: "inspect_drive", token: 1 },
      { kind: "inspect", token: 1, delay: POLL_MS, slowAfter: null },
    ]);
  });

  test("the help card shows after 10 s, and only for the current search", () => {
    const state = started();
    expect(reduce(state, { type: "help_due", token: 0 }).phase).toEqual(state.phase);
    expect(reduce(state, { type: "help_due", token: state.helpToken }).phase).toMatchObject({ showHelp: true });
  });

  test("a result of an older poll is dropped", () => {
    const state = started();
    expect(reduce(state, { type: "inspected", token: state.token - 1, output: VENU2 })).toEqual({ ...state, effects: [] });
  });

  test("watch found over MTP → watch, asking whether its font list was shared", () => {
    const state = atWatch();
    expect(state.phase).toEqual({ name: "watch" });
    expect(state.inspect).toBe(VENU2);
    expect(kinds(state)).toEqual(["shared_before", "inspect"]);
  });

  test("found as a USB drive → drive; unplugged → connect", () => {
    const drive = output("drive-only", "inspect_drive", "inspect_drive");
    let state = read(started(), NO_DEVICE);
    state = reduce(state, { type: "drive_inspected", token: state.token, output: drive });
    expect(state.phase).toEqual({ name: "drive", drive });
    state = read(state, NO_DEVICE);
    expect(kinds(state)).toEqual(["inspect_drive", "inspect"]);
    state = reduce(state, { type: "drive_failed", token: state.token, failure: NO_DEVICE });
    expect(state.phase).toMatchObject({ name: "connect", mode: "searching" });
    expect(kinds(state)).toEqual(["help_timer"]);
  });

  test("a drive switched to MTP → watch", () => {
    const drive = output("venu2-drive", "inspect_drive", "inspect_drive");
    let state = read(started(), NO_DEVICE);
    state = reduce(state, { type: "drive_inspected", token: state.token, output: drive });
    expect(read(state, VENU2).phase).toEqual({ name: "watch" });
  });

  test("held by another program → busy; retried every 2 s and now; released → watch", () => {
    const busy = failure("busy", "inspect-garmin_express-macos");
    let state = read(started(), busy);
    expect(state.phase).toEqual({ name: "connect", mode: "busy", failure: busy });
    expect(polls(state)).toMatchObject({ delay: POLL_MS });
    state = act(state, "retry_now");
    expect(polls(state)).toMatchObject({ delay: 0, token: 2 });
    expect(read(state, inspect("busy", "inspect-released")).phase).toEqual({ name: "watch" });
  });

  test.each([
    "inspect-permission_denied",
    "inspect-multiple_devices",
    "inspect-garmin_folder_not_found",
    "inspect-device_error",
    "inspect-internal",
  ])("%s → connect problem, still polling", (step) => {
    const problem = failure("connect-problems", step);
    const state = read(started(), problem);
    expect(state.phase).toEqual({ name: "connect", mode: "problem", failure: problem });
    expect(polls(state)).toBeDefined();
  });

  test("watch unplugged → connect", () => {
    const state = read(atWatch(), NO_DEVICE);
    expect(state.phase).toMatchObject({ name: "connect", mode: "searching", showHelp: false });
    expect(kinds(state)).toEqual(["help_timer", "inspect_drive", "inspect"]);
  });

  test("a write waiting on the watch → straight to the restart step", () => {
    const state = read(started(), inspect("venu2", "inspect-staged"));
    expect(state.phase).toMatchObject({ name: "restart", op: "install", phase: "unplug", verifiable: true });
    expect(read(started(), inspect("venu2", "inspect-restore-staged")).phase).toMatchObject({ op: "restore" });
  });
});

describe("install and restore", () => {
  test("install → working, which stops polling", () => {
    const state = act(atWatch(), "install");
    expect(state.phase).toEqual({ name: "working", op: "install", progress: null });
    expect(state.effects).toEqual([{ kind: "run", op: "install" }]);
    expect(state.token).toBe(atWatch().token + 1);
  });

  test("install is offered only where the verdict is install", () => {
    const installed = read(started(), inspect("venu2", "inspect-restarted"));
    expect(act(installed, "install")).toEqual({ ...installed, effects: [] });
  });

  test("progress shows while working", () => {
    const state = reduce(act(atWatch(), "install"), { type: "progress", progress: { done: 2, total: 4 } });
    expect(state.phase).toMatchObject({ progress: { done: 2, total: 4 } });
  });

  test("restore asks first; cancel goes back; confirm → working", () => {
    const watch = atWatch();
    const confirm = act(watch, "restore");
    expect(confirm.phase).toEqual({ name: "confirm_restore", back: { name: "watch" } });
    expect(act(confirm, "cancel").phase).toEqual({ name: "watch" });
    expect(act(confirm, "confirm_restore").phase).toEqual({ name: "working", op: "restore", progress: null });
  });

  test("restore isn't offered on a watch with nothing to restore", () => {
    const predicted = read(started(), inspect("predicted", "inspect"));
    expect(act(predicted, "restore").phase).toEqual({ name: "watch" });
  });

  test("written → restart, unplug, polling", () => {
    const state = written();
    expect(state.phase).toEqual({
      name: "restart",
      op: "install",
      phase: "unplug",
      verifiable: true,
      reconnected: false,
      starting: false,
      interruption: null,
    });
    expect(state.attempt).toBe(1);
    expect(polls(state)).toMatchObject({ delay: 0 });
  });

  test("failed → error; try again repeats the same operation", () => {
    const fail = failure("venu2-write-fails", "install-rollback-incomplete");
    const state = reduce(act(atWatch(), "install"), { type: "op_failed", op: "install", failure: fail });
    expect(state.phase).toEqual({ name: "error", op: "install", failure: fail });
    expect(state.effects).toEqual([]);
    expect(act(state, "try_again").effects).toEqual([{ kind: "run", op: "install" }]);
  });

  test("error → back to the watch, read again", () => {
    const fail = failure("errors", "install-conflict");
    const state = reduce(act(atWatch(), "install"), { type: "op_failed", op: "install", failure: fail });
    const back = act(state, "back");
    expect(back.phase).toEqual({ name: "watch" });
    expect(polls(back)).toMatchObject({ delay: 0 });
  });

  test("error → restore (rollback incomplete, restore first)", () => {
    const fail = failure("venu2-write-fails", "install-rollback-incomplete");
    const state = reduce(act(atWatch(), "install"), { type: "op_failed", op: "install", failure: fail });
    expect(act(state, "restore").phase).toMatchObject({ name: "confirm_restore" });
  });

  test("disconnected during the write: error, polling, then the watch again", () => {
    const fail = failure("errors", "install-no_device");
    const state = reduce(act(atWatch(), "install"), { type: "op_failed", op: "install", failure: fail });
    expect(polls(state)).toMatchObject({ delay: 0 });
    expect(read(state, NO_DEVICE).phase).toMatchObject({ name: "error" });
    expect(read(state, VENU2).phase).toEqual({ name: "watch" });
  });

  test("busy while writing → busy", () => {
    const busy = failure("busy", "inspect-garmin_express-macos");
    const state = reduce(act(atWatch(), "install"), { type: "op_failed", op: "install", failure: busy });
    expect(state.phase).toMatchObject({ name: "connect", mode: "busy" });
    expect(polls(state)).toBeDefined();
  });
});

describe("the restart step", () => {
  test("still connected → stays at unplug; unplugged → restart", () => {
    const stays = read(written(), inspect("venu2", "inspect-staged"));
    expect(stays.phase).toMatchObject({ phase: "unplug" });
    const restart = read(written(), NO_DEVICE);
    expect(restart.phase).toMatchObject({ phase: "restart" });
    expect(polls(restart)).toMatchObject({ slowAfter: SLOW_MS });
  });

  test("a slow read after the restart shows verifying; a fast one doesn't", () => {
    const restart = read(written(), NO_DEVICE);
    expect(reduce(restart, { type: "inspect_slow", token: restart.token }).phase).toMatchObject({ phase: "verifying" });
    expect(reduce(restart, { type: "inspect_slow", token: restart.token - 1 }).phase).toMatchObject({ phase: "restart" });
  });

  test("reconnected and installed → result, no more polling", () => {
    const state = reconnected(inspect("venu2", "inspect-restarted"));
    expect(state.phase).toEqual({ name: "result", op: "install" });
    expect(state.effects).toEqual([]);
  });

  test("still waiting → again; the next unplug starts attempt 2", () => {
    let state = reconnected(inspect("venu2-second-restart", "inspect-not-imported"));
    expect(state.phase).toMatchObject({ phase: "again" });
    state = read(state, NO_DEVICE);
    expect(state.phase).toMatchObject({ phase: "restart", reconnected: false });
    expect(state.attempt).toBe(2);
    expect(read(state, inspect("venu2-second-restart", "inspect-restarted")).phase).toEqual({
      name: "result",
      op: "install",
    });
  });

  test("another watch → other_watch, then the right one is checked", () => {
    let state = reconnected(inspect("unconfirmed", "inspect"));
    expect(state.phase).toMatchObject({ phase: "other_watch" });
    state = read(state, NO_DEVICE);
    expect(read(state, inspect("venu2", "inspect-restarted")).phase).toMatchObject({ name: "result" });
  });

  test("`other` after the reconnect → the watch's blocked verdict, still read every 2 s", () => {
    const other = { ...inspect("errors", "inspect-other"), device: VENU2.device };
    const state = reconnected(other);
    expect(state.phase).toEqual({ name: "watch" });
    expect(kinds(state)).toContain("inspect");
  });

  test("still starting: device_error and unreadable reads keep polling for 10 s, then error", () => {
    const deviceError = failure("connect-problems", "inspect-device_error");
    let state = read(read(written(), NO_DEVICE), deviceError);
    expect(state.phase).toMatchObject({ phase: "verifying", reconnected: true, starting: true });
    expect(state.effects).toContainEqual({ kind: "starting_timer", token: 1, delay: STARTING_MS });
    state = read(state, { ...VENU2, state: "unknown" });
    expect(state.phase).toMatchObject({ phase: "verifying" });
    state = reduce(state, { type: "started_up", token: state.startingToken });
    expect(state.phase).toMatchObject({ starting: false });
    expect(read(state, deviceError).phase).toEqual({ name: "error", op: "install", failure: deviceError });
  });

  test("held after the restart: busy shows, the restart is kept, release resumes verification", () => {
    const busy = failure("busy", "inspect-garmin_express-macos");
    let state = read(read(written(), NO_DEVICE), busy);
    expect(state.phase).toMatchObject({
      name: "restart",
      op: "install",
      phase: "restart",
      interruption: { kind: "busy", failure: busy },
    });
    expect(state.attempt).toBe(1);
    expect(polls(state)).toMatchObject({ delay: POLL_MS });
    expect(polls(act(state, "retry_now"))).toMatchObject({ delay: 0 });
    state = read(state, inspect("venu2", "inspect-restarted"));
    expect(state.phase).toEqual({ name: "result", op: "install" });
  });

  test("held before the unplug: still the unplug phase", () => {
    const busy = failure("busy", "inspect-garmin_express-macos");
    expect(read(written(), busy).phase).toMatchObject({ phase: "unplug", interruption: null });
  });

  test("back as a drive after the restart: drive shows, the restart is kept, MTP resumes verification", () => {
    const drive = output("venu2-drive", "inspect_drive", "inspect_drive");
    // Not while unplugging: the watch is only leaving.
    expect(kinds(read(written(), NO_DEVICE))).toEqual(["inspect"]);
    let state = read(read(written(), NO_DEVICE), NO_DEVICE);
    expect(state.effects).toContainEqual({ kind: "inspect_drive", token: state.token });
    state = reduce(state, { type: "drive_inspected", token: state.token, output: drive });
    expect(state.phase).toMatchObject({ name: "restart", interruption: { kind: "drive", drive } });
    // Still a drive: kept through the no_device reads until inspect_drive says otherwise.
    state = read(state, NO_DEVICE);
    expect(state.phase).toMatchObject({ interruption: { kind: "drive" } });
    const unplugged = reduce(state, { type: "drive_failed", token: state.token, failure: NO_DEVICE });
    expect(unplugged.phase).toMatchObject({ name: "restart", phase: "restart", interruption: null });
    expect(read(state, inspect("venu2", "inspect-restarted")).phase).toEqual({ name: "result", op: "install" });
  });

  test("skip after an install → check", () => {
    expect(act(read(written(), NO_DEVICE), "skip_verify").phase).toEqual({ name: "check", step: "send" });
  });

  test("skip or done after a restore → watch", () => {
    const working = act(act(atWatch(), "restore"), "confirm_restore");
    const restart = read(
      reduce(working, { type: "op_done", op: "restore", output: output("venu2", "restore_stock", "restore_stock") }),
      NO_DEVICE,
    );
    expect(act(restart, "skip_verify").phase).toEqual({ name: "watch" });
    expect(act(restart, "done").phase).toEqual({ name: "watch" });
  });

  test("restore reconnected with Garmin's font → result, which keeps reading the watch", () => {
    const working = act(act(atWatch(), "restore"), "confirm_restore");
    let state = reduce(working, { type: "op_done", op: "restore", output: output("venu2", "restore_stock", "restore_stock") });
    state = read(read(state, NO_DEVICE), inspect("venu2", "inspect-restored"));
    expect(state.phase).toEqual({ name: "result", op: "restore" });
    expect(kinds(state)).toEqual(["inspect"]);
    // The same watch, still connected: the result stays.
    const same = read(state, inspect("venu2", "inspect-restored"));
    expect(same.phase).toEqual({ name: "result", op: "restore" });
    expect(kinds(same)).toEqual(["inspect"]);
    // Unplugged: the search, which also looks for a drive.
    const unplugged = read(state, NO_DEVICE);
    expect(unplugged.phase).toMatchObject({ name: "connect", mode: "searching" });
    expect(kinds(unplugged)).toEqual(expect.arrayContaining(["inspect", "inspect_drive"]));
    // Another watch: its own screen.
    const other = read(state, inspect("unconfirmed", "inspect"));
    expect(other.phase).toEqual({ name: "watch" });
    expect(other.inspect?.device.model).toBe("Example Unconfirmed");
  });

  test("an install's result doesn't read the watch: the check follows", () => {
    const state = read(read(written(), NO_DEVICE), inspect("venu2", "inspect-restarted"));
    expect(state.phase).toEqual({ name: "result", op: "install" });
    expect(kinds(state)).not.toContain("inspect");
  });

  test("not verifiable (TrueType): no polling once unplugged; check by hand", () => {
    const expected = read(started(), inspect("expected-waiting", "inspect-restarted"));
    let state = reduce(act(expected, "install"), {
      type: "op_done",
      op: "install",
      output: output("expected-waiting", "install", "install"),
    });
    expect(state.phase).toMatchObject({ verifiable: false, phase: "unplug" });
    expect(polls(state)).toBeDefined();
    state = read(state, NO_DEVICE);
    expect(state.phase).toMatchObject({ phase: "restart" });
    expect(polls(state)).toBeUndefined();
    expect(act(state, "skip_verify").phase).toEqual({ name: "check", step: "send" });
  });
});

describe("check, outcome and share", () => {
  const atResult = () => reconnected(inspect("venu2", "inspect-restarted"));

  test("result → check loads the phone page and the marks", () => {
    const state = act(atResult(), "check");
    expect(state.phase).toEqual({ name: "check", step: "send" });
    expect(state.effects).toEqual([{ kind: "phone_page", lang: "en" }, { kind: "test_marks" }]);
  });

  test("watch (installed) → check", () => {
    const installed = read(started(), inspect("venu2", "inspect-restarted"));
    expect(act(installed, "check").phase).toEqual({ name: "check", step: "send" });
  });

  function answered(result: "georgian" | "boxes" | "no_notification", from = atResult()): FlowState {
    let state = act(act(from, "check"), "send_test_done");
    expect(state.phase).toEqual({ name: "check", step: "answer" });
    state = act(state, "done");
    expect(state.phase).toMatchObject({ step: "answer" }); // nothing chosen yet
    state = act(state, "answer", { action: "answer", answers: { result, views: ["popup"] } });
    return act(state, "done");
  }

  test("Georgian on a confirmed watch → works_confirmed", () => {
    const state = answered("georgian");
    expect(state.phase).toEqual({ name: "outcome", kind: "works_confirmed" });
    expect(state.effects).toEqual([
      { kind: "shared_before", key: { modelPart: "006-B3703-00", software: "19.05", result: "georgian" } },
    ]);
  });

  test("Georgian on an unconfirmed watch → works_first", () => {
    const unconfirmed = read(started(), inspect("unconfirmed", "inspect-restarted"));
    expect(answered("georgian", unconfirmed).phase).toEqual({ name: "outcome", kind: "works_first" });
  });

  test("boxes → didnt_show; restart once more → restart, attempt 2", () => {
    const state = act(answered("boxes"), "restart_again");
    expect(state.phase).toMatchObject({ name: "restart", op: "install", phase: "unplug" });
    expect(state.attempt).toBe(2);
    expect(polls(state)).toBeDefined();
  });

  test("nothing arrived → no_notification; try again → send", () => {
    const state = answered("no_notification");
    expect(state.phase).toEqual({ name: "outcome", kind: "no_notification" });
    expect(act(state, "check").phase).toEqual({ name: "check", step: "send" });
  });

  test("outcome → restore from More", () => {
    expect(act(answered("boxes"), "restore").phase).toMatchObject({ name: "confirm_restore" });
  });

  test("answers survive leaving and coming back for the session", () => {
    const outcome = answered("georgian");
    const back = act(act(outcome, "done"), "check");
    expect(back.answers["006-B3703-00"]).toEqual({ result: "georgian", views: ["popup"] });
  });

  test("answers stay coherent: no views or test mark without letters, views in the form's order", () => {
    let state = act(act(atResult(), "check"), "send_test_done");
    const answer = (answers: Answers) => act(state, "answer", { action: "answer", answers });
    state = answer({ result: "georgian", views: ["opened", "popup"], marker: "B", phone: "android" });
    expect(answersFor(state)).toEqual({ result: "georgian", views: ["popup", "opened"], marker: "B", phone: "android" });
    state = answer({ ...answersFor(state), result: "some_letters" });
    expect(answersFor(state)).toEqual({ result: "some_letters", views: ["popup", "opened"], marker: "B", phone: "android" });
    for (const result of ["boxes", "question_marks", "no_notification"] as const) {
      const switched = answer({ ...answersFor(state), result });
      expect(answersFor(switched)).toEqual({ result, views: [], phone: "android" });
      const shared = act(act(switched, "done"), "share", { action: "share", what: "result" });
      const [effect] = shared.effects;
      if (effect?.kind !== "issue_link") throw new Error("no issue_link");
      expect(effect.input.answers).toEqual({ result, views: [], phone: "android" });
    }
  });

  test("share the result: the link, then GitHub; opened or with a paste; failed", () => {
    const outcome = answered("georgian");
    let state = act(outcome, "share", { action: "share", what: "result" });
    expect(state.phase).toMatchObject({ name: "share", what: "result", state: "ready", link: null });
    const [effect] = state.effects;
    expect(effect).toMatchObject({ kind: "issue_link" });
    if (effect?.kind !== "issue_link") throw new Error("no issue_link");
    expect(effect.input.answers).toEqual({ result: "georgian", views: ["popup"] });
    expect(effect.input.outcome).toEqual({
      operation: "install",
      status: "staged_verified",
      parts: ["006-D9035-01", "006-D9035-02", "006-D9035-03", "006-D9035-04"],
    });
    state = reduce(state, { type: "link_ready", link: output("venu2", "issue_link-georgian", "issue_link") });
    state = act(state, "share", { action: "share", what: "result" });
    expect(kinds(state)).toEqual(["share_report"]);
    const opened = reduce(state, { type: "shared", shared: { complete: true, url: "https://github.com/x" } });
    expect(opened.phase).toMatchObject({ state: "opened" });
    expect(opened.shared["006-B3703-00|19.05|georgian"]).toBe(true);
    expect(kinds(opened)).toEqual(["mark_shared"]);
    expect(reduce(state, { type: "shared", shared: { complete: false, url: "x" } }).phase).toMatchObject({
      state: "opened_paste",
    });
    const failed = reduce(state, { type: "share_failed", failure: { code: "internal", message: "no browser" } });
    expect(failed.phase).toMatchObject({ state: "failed" });
    expect(act(opened, "done").phase).toEqual({ name: "outcome", kind: "works_confirmed" });
  });

  test("share from the drive: its report, no outcome", () => {
    const drive = output("drive-only", "inspect_drive", "inspect_drive");
    let state = read(started(), NO_DEVICE);
    state = reduce(state, { type: "drive_inspected", token: state.token, output: drive });
    state = act(state, "share", { action: "share", what: "font_list" });
    expect(state.effects).toEqual([{ kind: "issue_link", input: { report: drive.report } }]);
  });

  test("share from busy: the holder alone; from a problem: the error alone", () => {
    const busy = failure("busy", "inspect-garmin_express-macos");
    const fromBusy = act(read(started(), busy), "share", { action: "share", what: "problem" });
    expect(fromBusy.effects).toEqual([{ kind: "issue_link", input: { busy: busy.busy } }]);
    const internal = failure("connect-problems", "inspect-internal");
    const fromProblem = act(read(started(), internal), "share", { action: "share", what: "problem" });
    expect(fromProblem.effects).toEqual([
      { kind: "issue_link", input: { error: { code: "internal", message: internal.message } } },
    ]);
  });

  test("share a failed write: the outcome's error", () => {
    const fail = failure("errors", "install-internal");
    const error = reduce(act(atWatch(), "install"), { type: "op_failed", op: "install", failure: fail });
    const state = act(error, "share", { action: "share", what: "problem" });
    expect(state.effects).toEqual([
      { kind: "issue_link", input: { report: VENU2.report, outcome: { operation: "install", error: "internal" } } },
    ]);
  });
});

describe("the rest", () => {
  test("a different watch drops the last operation", () => {
    const fail = failure("errors", "install-conflict");
    let state = reduce(act(atWatch(), "install"), { type: "op_failed", op: "install", failure: fail });
    state = read(act(state, "back"), inspect("unconfirmed", "inspect"));
    expect(state.lastOp).toBeNull();
  });

  test("language: saved, and the phone page reloaded on the check", () => {
    expect(kinds(reduce(atWatch(), { type: "language", lang: "ka" }))).toEqual(["save_language"]);
    const check = act(atWatch(), "check");
    expect(reduce(check, { type: "language", lang: "ka" }).effects).toEqual([
      { kind: "save_language", lang: "ka" },
      { kind: "phone_page", lang: "ka" },
    ]);
  });

  test("follow updates and which watches open their pages", () => {
    const unsupported = read(started(), inspect("font-unused", "inspect"));
    expect(act(unsupported, "open_issue").effects).toEqual([{ kind: "open_issue", issue: 6 }]);
    expect(act(unsupported, "which_watches").effects).toEqual([{ kind: "open_watch_list" }]);
  });

  test("shared before is remembered", () => {
    const key = { modelPart: "006-B3703-00", software: "19.05", result: "font_list" as const };
    const state = reduce(atWatch(), { type: "shared_before", key, value: true });
    expect(state.shared["006-B3703-00|19.05|font_list"]).toBe(true);
    // Not asked again.
    expect(kinds(read(state, VENU2))).toEqual(["inspect"]);
  });

  test("phone page and marks are kept", () => {
    const page = { url: "https://lubaretsi.github.io/georgian-for-garmin/?lang=en", qr: "data:" };
    const state = run(atWatch(), { type: "phone_loaded", lang: "en", page }, { type: "marks_loaded", marks: [] });
    expect(state.phone).toEqual({ lang: "en", page });
    expect(act(state, "check").effects).toEqual([]);
  });
});

describe("a watch in Garmin mode (§4.2)", () => {
  const GARMIN = failure("garmin-mode", "inspect-garmin_mode");
  const WINDOWS = failure("garmin-mode", "inspect-garmin_mode-windows");
  const SWITCHED = output("garmin-mode", "switch_to_mtp-switched", "switch_to_mtp");
  const TIMED_OUT = output("garmin-mode", "switch_to_mtp-timed_out", "switch_to_mtp");
  const LOCKED = failure("garmin-mode", "switch_to_mtp-device_locked");
  const UNSUPPORTED = failure("garmin-mode", "switch_to_mtp-unsupported_device");
  const switches = (state: FlowState) => state.effects.filter((e) => e.kind === "switch_to_mtp").length;
  const switched = (state: FlowState): FlowState => reduce(state, { type: "switched", output: SWITCHED });
  const failed = (state: FlowState, f: Failure): FlowState => reduce(state, { type: "switch_failed", failure: f });
  /** Every way the switch ends without the watch over MTP, and the guidance it leaves. */
  const FAILURES: [string, (s: FlowState) => FlowState, { locked: boolean; detail: RegExp }][] = [
    ["timed_out", (s) => reduce(s, { type: "switched", output: TIMED_OUT }), { locked: false, detail: /within 15 s/ }],
    ["device_locked", (s) => failed(s, LOCKED), { locked: true, detail: /locked/ }],
    ["unsupported_device", (s) => failed(s, UNSUPPORTED), { locked: false, detail: /A1050/ }],
    ["another error", (s) => failed(s, { code: "device_error", message: "claim failed" }), { locked: false, detail: /claim failed/ }],
  ];

  test("on connect: switched once, no reads meanwhile; switched → read now → the watch", () => {
    let state = read(started(), GARMIN);
    expect(state.phase).toEqual({ name: "connect", mode: "garmin_mode", garmin: { state: "switching", running: true } });
    expect(kinds(state)).toEqual(["switch_to_mtp"]);
    expect(state.switchTried).toBe(true);
    // A read still in flight when the switch started is dropped.
    expect(reduce(state, { type: "inspected", token: state.token - 1, output: VENU2 }).phase).toEqual(state.phase);
    state = switched(state);
    expect(state.phase).toMatchObject({ garmin: { state: "switching", running: false } });
    expect(polls(state)).toMatchObject({ delay: 0, token: state.token });
    state = read(state, VENU2);
    expect(state.phase).toEqual({ name: "watch" });
    expect(state.switchTried).toBe(false);
  });

  test.each(FAILURES)("on connect, %s → the guidance, still reading; no second switch", (_, end, guidance) => {
    let state = end(read(started(), GARMIN));
    expect(state.phase).toMatchObject({ name: "connect", mode: "garmin_mode", garmin: { state: "guidance", locked: guidance.locked } });
    const garmin = state.phase.name === "connect" && state.phase.mode === "garmin_mode" ? state.phase.garmin : null;
    expect(garmin?.state === "guidance" && garmin.detail).toMatch(guidance.detail);
    expect(polls(state)).toMatchObject({ delay: 0 });
    // No loop: further reads in Garmin mode keep the same guidance.
    for (let i = 0; i < 3; i++) {
      const next = read(state, GARMIN);
      expect(switches(next)).toBe(0);
      expect(next.phase).toEqual(state.phase);
      expect(polls(next)).toMatchObject({ delay: POLL_MS });
      state = next;
    }
    // MTP chosen on the watch: it goes on by itself.
    expect(read(state, VENU2).phase).toEqual({ name: "watch" });
  });

  test("a replug (a read with no watch) allows another switch; a read in Garmin mode doesn't", () => {
    let state = failed(read(started(), GARMIN), LOCKED);
    state = read(state, GARMIN);
    expect(switches(state)).toBe(0);
    state = read(state, NO_DEVICE);
    expect(state.phase).toMatchObject({ name: "connect", mode: "searching" });
    expect(state.switchTried).toBe(false);
    state = read(state, GARMIN);
    expect(switches(state)).toBe(1);
    expect(state.phase).toMatchObject({ garmin: { state: "switching", running: true } });
  });

  test("the watch leaving Garmin mode by itself during the switch → read now", () => {
    const state = failed(read(started(), GARMIN), NO_DEVICE);
    expect(state.phase).toMatchObject({ garmin: { state: "switching", running: false } });
    expect(polls(state)).toMatchObject({ delay: 0 });
    expect(read(state, NO_DEVICE).phase).toMatchObject({ name: "connect", mode: "searching" });
  });

  test("Windows (no_driver): straight to the guidance, never a switch", () => {
    let state = read(started(), WINDOWS);
    expect(state.phase).toEqual({
      name: "connect",
      mode: "garmin_mode",
      garmin: { state: "guidance", locked: false, detail: WINDOWS.message },
    });
    expect(kinds(state)).toEqual(["inspect"]);
    state = read(read(state, NO_DEVICE), WINDOWS);
    expect(switches(state)).toBe(0);
  });

  test("from the watch screen, and after a write cut off by an unplug", () => {
    expect(switches(read(atWatch(), GARMIN))).toBe(1);
    const failedWrite = reduce(act(atWatch(), "install"), {
      type: "op_failed",
      op: "install",
      failure: failure("errors", "install-no_device"),
    });
    expect(read(failedWrite, NO_DEVICE).phase).toMatchObject({ name: "error" });
    expect(switches(read(failedWrite, GARMIN))).toBe(1);
  });

  test("during the restart: off MTP as an unplug, switched inside the restart, verified after", () => {
    let state = read(written(), GARMIN);
    expect(state.phase).toMatchObject({
      name: "restart",
      phase: "restart",
      reconnected: false,
      interruption: { kind: "garmin_mode", garmin: { state: "switching", running: true } },
    });
    expect(kinds(state)).toEqual(["switch_to_mtp"]);
    state = switched(state);
    expect(polls(state)).toMatchObject({ delay: 0, slowAfter: SLOW_MS });
    state = read(state, inspect("venu2", "inspect-restarted"));
    expect(state.phase).toEqual({ name: "result", op: "install" });
  });

  test.each(FAILURES)("during the restart, %s → the guidance inside it, the restart kept", (_, end, guidance) => {
    let state = end(read(read(written(), NO_DEVICE), GARMIN));
    expect(state.phase).toMatchObject({
      name: "restart",
      op: "install",
      phase: "restart",
      interruption: { kind: "garmin_mode", garmin: { state: "guidance", locked: guidance.locked } },
    });
    expect(state.attempt).toBe(1);
    const again = read(state, GARMIN);
    expect(switches(again)).toBe(0);
    expect(again.phase).toEqual(state.phase);
    // Unplugged: the guidance goes; plugged in again in Garmin mode, another switch.
    state = read(again, NO_DEVICE);
    expect(state.phase).toMatchObject({ name: "restart", phase: "restart", interruption: null });
    expect(switches(read(state, GARMIN))).toBe(1);
    // MTP chosen on the watch instead: verification goes on.
    expect(read(again, inspect("venu2", "inspect-restarted")).phase).toEqual({ name: "result", op: "install" });
  });

  test("during the restart: from `again` it is the next attempt; a restart not verified never switches", () => {
    const again = read(read(written(), NO_DEVICE), inspect("venu2", "inspect-staged"));
    expect(again.phase).toMatchObject({ phase: "again" });
    const next = read(again, GARMIN);
    expect(next.attempt).toBe(again.attempt + 1);
    expect(switches(next)).toBe(1);

    const trueType = reduce(act(atWatch(), "install"), {
      type: "op_done",
      op: "install",
      output: { ...output("venu2", "install", "install"), packs: [] },
    });
    expect(trueType.phase).toMatchObject({ phase: "unplug", verifiable: false });
    const off = read(trueType, GARMIN);
    expect(off.phase).toMatchObject({ phase: "restart", interruption: null });
    expect(off.effects).toEqual([]);
  });

  test("the switch's start and end go to the event log; a failed one is the last problem", () => {
    const state = failed(read(started(), GARMIN), LOCKED);
    expect(state.events.map((e) => [e.kind, e.detail])).toEqual([
      ["poll", "garmin_mode"],
      ["operation_start", "switch_to_mtp"],
      ["operation_end", "switch_to_mtp: device_locked"],
    ]);
    expect(state.lastProblem).toMatchObject({ code: "device_locked" });
    expect(switched(read(started(), GARMIN)).events.at(-1)).toMatchObject({ detail: "switch_to_mtp: switched" });
  });

  test("a switch ending under the diagnostics screen: going back reads the watch", () => {
    let state = act(read(started(), GARMIN), "diagnostics");
    state = switched(state);
    expect(state.phase).toMatchObject({ name: "diagnostics", back: { garmin: { state: "switching", running: false } } });
    expect(polls(state)).toBeUndefined();
    state = act(state, "back");
    expect(polls(state)).toMatchObject({ delay: 0, token: state.token });
  });

  test("a result for a switch the flow has left is dropped", () => {
    const state = read(read(started(), GARMIN), NO_DEVICE);
    expect(state.phase).toMatchObject({ mode: "searching" });
    expect(switched(state).phase).toEqual(state.phase);
  });

  test("Restore isn't offered while the switch runs, on connect or in the restart; it is after", () => {
    // The Venu 2 is restorable: read once, then back in Garmin mode.
    const onConnect = read(read(atWatch(), NO_DEVICE), GARMIN);
    expect(onConnect.inspect?.restorable).toBe(true);
    expect(act(onConnect, "restore").phase).toEqual(onConnect.phase);
    expect(moreModel(onConnect).restorable).toBe(false);
    const inRestart = read(written(), GARMIN);
    expect(act(inRestart, "restore").phase).toEqual(inRestart.phase);
    expect(moreModel(inRestart).restorable).toBe(false);
    // Settled (guidance): Restore is offered again.
    const guided = failed(onConnect, LOCKED);
    expect(moreModel(guided).restorable).toBe(true);
    expect(act(guided, "restore").phase).toMatchObject({ name: "confirm_restore" });
  });

  test.each([
    ["switched", switched, { state: "switching", running: false }],
    ["device_locked", (s: FlowState) => failed(s, LOCKED), { state: "guidance", locked: true }],
    ["no_device", (s: FlowState) => failed(s, NO_DEVICE), { state: "switching", running: false }],
  ] as const)("a switch ending (%s) under a screen opened over it settles the screen underneath", (_, end, garmin) => {
    const running = read(started(), GARMIN);
    const overlays: [string, FlowState, ActionId][] = [
      ["share", act(running, "share", { action: "share", what: "problem" }), "back"],
      ["diagnostics", act(running, "diagnostics"), "back"],
      // Not reachable through the screens (Restore isn't offered then), but any screen with a
      // `back` takes the switch's result the same way.
      ["confirm_restore", { ...running, effects: [], phase: { name: "confirm_restore", back: running.phase } }, "cancel"],
    ];
    for (const [name, open, close] of overlays) {
      expect(open.phase.name, name).toBe(name);
      const ended = end(open);
      expect(ended.phase, name).toMatchObject({ name, back: { name: "connect", mode: "garmin_mode", garmin } });
      expect(polls(ended), name).toBeUndefined();
      const back = act(ended, close);
      expect(back.phase, name).toMatchObject({ name: "connect", mode: "garmin_mode", garmin });
      expect(polls(back), name).toMatchObject({ delay: 0, token: back.token });
    }
  });

  test("timers and the switch reach the restart under any screen opened over it", () => {
    const reconnected = read(read(written(), NO_DEVICE), inspect("venu2", "inspect-staged"));
    expect(reconnected.phase).toMatchObject({ phase: "again", starting: true });
    let state: FlowState = act(reconnected, "restore");
    expect(state.phase).toMatchObject({ name: "confirm_restore" });
    state = reduce(state, { type: "started_up", token: state.startingToken });
    expect(state.phase).toMatchObject({ name: "confirm_restore", back: { name: "restart", starting: false } });
    state = act(state, "cancel");
    expect(state.phase).toMatchObject({ name: "restart", phase: "again", starting: false });
  });

  test("a write that finds the watch back in Garmin mode wrote nothing: it is switched as on connect", () => {
    const working = act(atWatch(), "install");
    const state = reduce(working, { type: "op_failed", op: "install", failure: GARMIN });
    expect(state.phase).toEqual({ name: "connect", mode: "garmin_mode", garmin: { state: "switching", running: true } });
    expect(kinds(state)).toEqual(["switch_to_mtp"]);
    // Not switchable (Windows): the guidance, read once now, not twice.
    const windows = reduce(working, { type: "op_failed", op: "install", failure: WINDOWS });
    expect(windows.phase).toMatchObject({ mode: "garmin_mode", garmin: { state: "guidance" } });
    expect(kinds(windows)).toEqual(["inspect"]);
  });
});
