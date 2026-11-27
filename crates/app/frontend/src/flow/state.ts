// The journey (contract §4) as a pure reducer: `reduce(FlowState, FlowEvent) => FlowState`.
// The state's `effects` are what the effect runner (effects.ts) does after this transition:
// backend calls and timers. Every effect's result comes back as an event; results of polls the
// flow has moved on from carry an old `token` and are dropped.

import type { Lang } from "../core/backend";
import type {
  CheckUpdateOutput,
  Event as DiagnoseEvent,
  DiagnoseOutput,
  ErrorCode,
  Failure,
  Holder,
  InspectDriveOutput,
  InspectOutput,
  IssueLink,
  IssueLinkInput,
  MarkPicture,
  PhonePage,
  Progress,
  ShareKey,
  Shared,
  StageOutput,
  SwitchOutput,
} from "../core/types.gen";
import { type ActionId, type ActionPayload, type Answers, type Op, sawLetters, VIEWS } from "../model/screens";
import { verdictOf, verifiable, verified } from "./verdict";

/** How long the app waits between polls, and before the connect help card (§4). */
export const POLL_MS = 2000;
export const HELP_MS = 10_000;
/** A read after the restart taking longer than this shows "Checking the watch…" (§4.1). */
export const SLOW_MS = 400;
/** After the watch reconnects, how long `device_error` and `extdata_not_found` mean it is still starting. */
export const STARTING_MS = 10_000;
/** The session's event log keeps this many, for diagnostics (§11). */
export const MAX_EVENTS = 50;

export type RestartPhase = "unplug" | "restart" | "verifying" | "again" | "other_watch";
export type ShareWhat = "result" | "font_list" | "problem";
export type OutcomeKind = "works_confirmed" | "works_first" | "didnt_show" | "no_notification";

export type Phase =
  | { name: "connect"; mode: "searching"; showHelp: boolean }
  | { name: "connect"; mode: "busy"; failure: Failure }
  | { name: "connect"; mode: "problem"; failure: Failure }
  /** The watch in Garmin mode (§4.2): being switched, or what to do on the watch. */
  | { name: "connect"; mode: "garmin_mode"; garmin: GarminModeState }
  | { name: "drive"; drive: InspectDriveOutput }
  | { name: "watch" }
  | { name: "confirm_restore"; back: Phase }
  | { name: "working"; op: Op; progress: Progress | null }
  | {
      name: "restart";
      op: Op;
      phase: RestartPhase;
      verifiable: boolean;
      /** The watch answered since it was unplugged. */
      reconnected: boolean;
      /** Within `STARTING_MS` of that. */
      starting: boolean;
      /**
       * Reconnected held by another program, or as a USB drive: its screen shows, and the
       * restart (op, attempt, verification) goes on once the watch is free over MTP (§4.1).
       */
      interruption: RestartInterruption | null;
    }
  | { name: "result"; op: Op }
  | { name: "check"; step: "send" | "answer" }
  | { name: "outcome"; kind: OutcomeKind }
  | { name: "share"; what: ShareWhat; input: IssueLinkInput; link: IssueLink | null; state: ShareState; back: Phase }
  | { name: "error"; op: Op; failure: Failure }
  /** From More; `back` is the phase it returns to, kept moving while this is open (§11). */
  | {
      name: "diagnostics";
      back: Phase;
      state: DiagnosticsState;
      output: DiagnoseOutput | null;
      savedPath: string | null;
    };

export type DiagnosticsState = "collecting" | "ready" | "saved" | "copied" | "failed";

export type RestartInterruption =
  | { kind: "busy"; failure: Failure }
  | { kind: "drive"; drive: InspectDriveOutput }
  | { kind: "garmin_mode"; garmin: GarminModeState };

/**
 * The watch in Garmin mode (§4.2). `switching`: the switch to MTP runs (`running`), or it
 * switched and the watch is being read. `guidance`: what to do on the watch, after a switch that
 * didn't work (`locked`: the watch said it is locked) or where none can run; `detail` says why.
 */
export type GarminModeState =
  | { state: "switching"; running: boolean }
  | { state: "guidance"; locked: boolean; detail: string };

export type ShareState = "ready" | "opened" | "opened_paste" | "failed";

export type Effect =
  /** A read of the watch after `delay`; `slowAfter`: when to say it is slow (`inspect_slow`). */
  | { kind: "inspect"; token: number; delay: number; slowAfter: number | null }
  | { kind: "inspect_drive"; token: number }
  | { kind: "help_timer"; token: number; delay: number }
  | { kind: "starting_timer"; token: number; delay: number }
  | { kind: "run"; op: Op }
  | { kind: "issue_link"; input: IssueLinkInput }
  | { kind: "share_report"; input: IssueLinkInput }
  | { kind: "open_issue"; issue: number }
  | { kind: "open_watch_list" }
  | { kind: "phone_page"; lang: Lang }
  | { kind: "test_marks" }
  | { kind: "shared_before"; key: ShareKey }
  | { kind: "mark_shared"; key: ShareKey }
  | { kind: "save_language"; lang: Lang }
  | { kind: "diagnose"; events: DiagnoseEvent[] }
  | { kind: "save_diagnostics"; text: string }
  | { kind: "copy_text"; text: string }
  | { kind: "reveal_file"; path: string }
  | { kind: "switch_to_mtp" }
  /** The latest release (§12): asked once per run, in the background. */
  | { kind: "check_update" }
  | { kind: "open_update" };

export type FlowState = {
  phase: Phase;
  lang: Lang;
  version: string;
  /** The last watch read over MTP; null before any. */
  inspect: InspectOutput | null;
  /** The last install or restore on that watch, for the report's outcome. */
  lastOp: { op: Op; output: StageOutput } | { op: Op; failure: Failure } | null;
  /** Restarts asked for since the last write: the attempt the restart step shows. */
  attempt: number;
  /** The check's answers for the session, by the watch's part number (§5.2, Answers survive). */
  answers: Record<string, Answers>;
  /** What `sharedBefore` said, by `shareKeyId`. */
  shared: Record<string, boolean>;
  /** The phone test page, in the language it was loaded in. */
  phone: { lang: Lang; page: PhonePage } | null;
  marks: MarkPicture[] | null;
  /** The current poll; a result carrying another token is dropped. */
  token: number;
  /** The current connect search's help timer. */
  helpToken: number;
  /** The current reconnect's still-starting timer. */
  startingToken: number;
  /** The session's last `MAX_EVENTS` events, oldest first, with no identifiers (§11). */
  events: DiagnoseEvent[];
  /** The last failure the app met, for the diagnostics summary; `holder` when another program held the watch. */
  lastProblem: { time: string; code: ErrorCode; holder: Holder | null } | null;
  /**
   * The switch to MTP ran for the watch's current appearance in Garmin mode (§4.2): it runs
   * again only after a read finds no watch or the watch over MTP.
   */
  switchTried: boolean;
  /** The update check (§12) was asked for this run: it is never asked again. */
  updateAsked: boolean;
  /** A newer published version, once the check found one; null otherwise, failures included. */
  update: string | null;
  effects: Effect[];
};

export type FlowEvent =
  | { type: "start" }
  | { type: "inspected"; token: number; output: InspectOutput }
  | { type: "inspect_failed"; token: number; failure: Failure }
  | { type: "drive_inspected"; token: number; output: InspectDriveOutput }
  | { type: "drive_failed"; token: number; failure: Failure }
  | { type: "help_due"; token: number }
  | { type: "inspect_slow"; token: number }
  | { type: "started_up"; token: number }
  | { type: "progress"; progress: Progress }
  | { type: "op_done"; op: Op; output: StageOutput }
  | { type: "op_failed"; op: Op; failure: Failure }
  | { type: "phone_loaded"; lang: Lang; page: PhonePage }
  | { type: "marks_loaded"; marks: MarkPicture[] }
  | { type: "link_ready"; link: IssueLink }
  | { type: "link_failed"; failure: Failure }
  | { type: "shared"; shared: Shared }
  | { type: "share_failed"; failure: Failure }
  | { type: "shared_before"; key: ShareKey; value: boolean }
  | { type: "language"; lang: Lang }
  | { type: "diagnosed"; output: DiagnoseOutput }
  | { type: "diagnostics_saved"; path: string }
  | { type: "diagnostics_copied" }
  | { type: "diagnostics_failed"; failure: Failure }
  | { type: "switched"; output: SwitchOutput }
  | { type: "switch_failed"; failure: Failure }
  | { type: "update_checked"; output: CheckUpdateOutput }
  | { type: "action"; id: ActionId; payload?: ActionPayload };

export function initialState(lang: Lang, version: string): FlowState {
  return {
    phase: { name: "connect", mode: "searching", showHelp: false },
    lang,
    version,
    inspect: null,
    lastOp: null,
    attempt: 0,
    answers: {},
    shared: {},
    phone: null,
    marks: null,
    token: 0,
    helpToken: 0,
    startingToken: 0,
    switchTried: false,
    updateAsked: false,
    update: null,
    events: [],
    lastProblem: null,
    effects: [],
  };
}

export function shareKeyId(key: ShareKey): string {
  return `${key.modelPart}|${key.software}|${key.result}`;
}

/** The answers given for the watch the flow is on. */
export function answersFor(state: FlowState): Answers {
  const part = state.inspect?.device.part_number;
  return (part && state.answers[part]) || { views: [] };
}

/**
 * `answers` as the report takes them: views in the form's order, and none, nor a test mark,
 * without letters, so switching to boxes drops what was picked for letters.
 */
function coherent(answers: Answers): Answers {
  if (!sawLetters(answers.result)) {
    const { result, phone } = answers;
    return { views: [], ...(result && { result }), ...(phone && { phone }) };
  }
  return { ...answers, views: VIEWS.filter((view) => answers.views.includes(view)) };
}

/** The drive on screen: the drive phase's, or one that came back during a restart (§4.1). */
export function shownDrive(phase: Phase): InspectDriveOutput | null {
  if (phase.name === "drive") return phase.drive;
  return phase.name === "restart" && phase.interruption?.kind === "drive" ? phase.interruption.drive : null;
}

function switchRunning(garmin: GarminModeState): boolean {
  return garmin.state === "switching" && garmin.running;
}

/**
 * Whether the switch to MTP runs in `phase` or in a screen under it (§4.2): the core holds the
 * watch, so the app neither reads it nor starts a write meanwhile.
 */
export function switching(phase: Phase): boolean {
  if (phase.name === "connect" && phase.mode === "garmin_mode") return switchRunning(phase.garmin);
  if (phase.name === "restart" && phase.interruption?.kind === "garmin_mode") return switchRunning(phase.interruption.garmin);
  return "back" in phase ? switching(phase.back) : false;
}

/** Whether the app reads the watch every 2 s in `phase` (§4, Polling). */
export function polls(phase: Phase): boolean {
  switch (phase.name) {
    case "connect":
      // Not while the switch to MTP runs: the core holds the watch (§4.2).
      return !switching(phase);
    case "drive":
    case "watch":
      return true;
    case "restart":
      if (switching(phase)) return false;
      return phase.phase === "unplug" || phase.verifiable;
    case "result":
      // After a restore nothing follows on screen: the app looks for the unplug or another watch.
      return phase.op === "restore";
    case "error":
      // The watch was disconnected during the write: the app looks for it again.
      return phase.failure.code === "no_device";
    default:
      return false;
  }
}

/** Starts a new search for a watch: a fresh help timer (§4, the help card after 10 s). */
function searching(state: FlowState): FlowState {
  if (state.phase.name === "connect" && state.phase.mode === "searching") return state;
  const helpToken = state.helpToken + 1;
  return {
    ...state,
    phase: { name: "connect", mode: "searching", showHelp: false },
    helpToken,
    effects: [...state.effects, { kind: "help_timer", token: helpToken, delay: HELP_MS }],
  };
}

/**
 * Moves to `phase`, starting a poll right away if it polls and the last one didn't, and
 * dropping polls in flight if it doesn't.
 */
function go(state: FlowState, phase: Phase, effects: Effect[] = []): FlowState {
  const before = polls(state.phase);
  const next: FlowState = { ...state, phase, effects: [...state.effects, ...effects] };
  if (polls(phase) && !before) return pollNow(next);
  if (!polls(phase) && before) return { ...next, token: next.token + 1 };
  return next;
}

/** Reads after the restart say when they are slow: the watch answered and is being read. */
function slowAfter(phase: Phase): number | null {
  return phase.name === "restart" && phase.verifiable && phase.phase === "restart" ? SLOW_MS : null;
}

/** A new poll now, replacing any in flight. */
function pollNow(state: FlowState): FlowState {
  const token = state.token + 1;
  return withEffects({ ...state, token }, { kind: "inspect", token, delay: 0, slowAfter: slowAfter(state.phase) });
}

/** The next poll of the current one, after `delay`, if the phase polls. */
function pollNext(state: FlowState, delay = POLL_MS): FlowState {
  if (!polls(state.phase)) return state;
  return withEffects(state, { kind: "inspect", token: state.token, delay, slowAfter: slowAfter(state.phase) });
}

function keyFor(inspect: InspectOutput | InspectDriveOutput, result: ShareKey["result"]): ShareKey {
  return { modelPart: inspect.device.part_number, software: inspect.device.software_version, result };
}

/** Asks whether this watch's font list was shared before, unless known. */
function askSharedBefore(state: FlowState, source: InspectOutput | InspectDriveOutput): Effect[] {
  const key = keyFor(source, "font_list");
  return shareKeyId(key) in state.shared ? [] : [{ kind: "shared_before", key }];
}

/**
 * The watch's screen for a read: straight to the restart step if a write waits (§5.2). Another
 * watch than the last one starts afresh: no last operation, no restarts.
 */
function showWatch(state: FlowState, inspect: InspectOutput): FlowState {
  const same = state.inspect?.device.part_number === inspect.device.part_number;
  const next: FlowState = same ? { ...state, inspect } : { ...state, inspect, lastOp: null, attempt: 0 };
  const verdict = verdictOf(inspect);
  const effects = askSharedBefore(state, inspect);
  if (verdict.kind === "restart_pending") {
    const op: Op = verdict.variant === "georgian" ? "install" : "restore";
    const restart: Phase = {
      name: "restart",
      op,
      phase: "unplug",
      verifiable: true,
      reconnected: false,
      starting: false,
      interruption: null,
    };
    return go({ ...next, attempt: Math.max(next.attempt, 1) }, restart, effects);
  }
  return go(next, { name: "watch" }, effects);
}

/** Back to the watch's screen with a fresh read, or to the search if there is no watch. */
function home(state: FlowState): FlowState {
  if (!state.inspect) {
    const next = searching(state);
    return polls(state.phase) ? next : pollNow(next);
  }
  const next = go(state, { name: "watch" });
  // Entering a polling phase already reads the watch now; else replace the poll in flight.
  return polls(state.phase) ? pollNow(next) : next;
}

/** A failed read while looking for or at a watch (§4, §6). */
function readFailed(state: FlowState, failure: Failure): FlowState {
  switch (failure.code) {
    case "no_device": {
      const next = searching(state);
      return pollNext({ ...next, effects: [...next.effects, { kind: "inspect_drive", token: next.token }] });
    }
    case "device_busy":
      return pollNext(go(state, { name: "connect", mode: "busy", failure }));
    case "garmin_mode": {
      const phase = state.phase;
      const current = phase.name === "connect" && phase.mode === "garmin_mode" ? phase.garmin : null;
      const next = garminRead(state, failure, current);
      const moved = go(next.state, { name: "connect", mode: "garmin_mode", garmin: next.garmin }, next.effects);
      // From a phase that didn't read (a write that found the watch in Garmin mode), `go` reads now.
      return polls(phase) ? pollNext(moved) : moved;
    }
    default:
      return pollNext(go(state, { name: "connect", mode: "problem", failure }));
  }
}

/**
 * A `garmin_mode` read (§4.2): the switch to MTP, once per appearance of the watch in Garmin
 * mode, where it can run; otherwise the guidance, keeping what an earlier one said.
 */
function garminRead(
  state: FlowState,
  failure: Failure,
  current: GarminModeState | null,
): { state: FlowState; garmin: GarminModeState; effects: Effect[] } {
  if (failure.garmin_mode?.switchable && !state.switchTried) {
    return {
      state: { ...state, switchTried: true },
      garmin: { state: "switching", running: true },
      effects: [{ kind: "switch_to_mtp" }],
    };
  }
  if (current?.state === "guidance") return { state, garmin: current, effects: [] };
  return { state, garmin: { state: "guidance", locked: false, detail: failure.message }, effects: [] };
}

type SwitchEvent = Extract<FlowEvent, { type: "switched" | "switch_failed" }>;

/** Where the switch's result leaves the watch (§4.2): read again, or the guidance. */
function afterSwitch(event: SwitchEvent): GarminModeState {
  if (event.type === "switched") {
    const { status, waited_seconds, product, protocols } = event.output;
    if (status === "switched") return { state: "switching", running: false };
    const seen = [product?.description, protocols.join(", ")].filter(Boolean).join("; ");
    const detail = `switch_to_mtp: the watch didn't come back over MTP within ${waited_seconds} s${seen ? ` (${seen})` : ""}.`;
    return { state: "guidance", locked: false, detail };
  }
  const { code, message } = event.failure;
  // Gone from Garmin mode by itself (MTP chosen on the watch, or unplugged): the read says which.
  if (code === "no_device") return { state: "switching", running: false };
  return { state: "guidance", locked: code === "device_locked", detail: message };
}

/** The switch's result, where it runs: on connect or inside the restart step (§4.2). */
function switchEnded(state: FlowState, garmin: GarminModeState): FlowState {
  const phase = state.phase;
  if (phase.name === "connect" && phase.mode === "garmin_mode" && switchRunning(phase.garmin)) {
    return go(state, { ...phase, garmin });
  }
  if (phase.name === "restart" && phase.interruption?.kind === "garmin_mode" && switchRunning(phase.interruption.garmin)) {
    return go(state, { ...phase, interruption: { kind: "garmin_mode", garmin } });
  }
  return state;
}

type Restart = Extract<Phase, { name: "restart" }>;

/** The watch answered after the restart: the first answer starts the still-starting time. */
function answered(state: FlowState, phase: Restart, restartPhase: RestartPhase): FlowState {
  if (phase.reconnected) return { ...state, phase: { ...phase, phase: restartPhase } };
  const startingToken = state.startingToken + 1;
  return withEffects(
    { ...state, startingToken, phase: { ...phase, phase: restartPhase, reconnected: true, starting: true } },
    { kind: "starting_timer", token: startingToken, delay: STARTING_MS },
  );
}

/** A read during the restart step (§4.1): decided on the first one after the reconnect. */
function restartRead(state: FlowState, interrupted: Restart, inspect: InspectOutput): FlowState {
  // Free over MTP: whatever held or mounted it is over.
  const phase: Restart = { ...interrupted, interruption: null };
  state = { ...state, phase };
  if (phase.phase === "unplug" || phase.phase === "again" || !phase.verifiable) return pollNext(state);
  if (state.inspect && inspect.device.part_number !== state.inspect.device.part_number) {
    return pollNext(answered(state, phase, "other_watch"));
  }
  // A watch still starting may not show its fonts yet.
  if (inspect.state === "unknown" && (phase.starting || !phase.reconnected)) {
    return pollNext(answered(state, phase, "verifying"));
  }
  const next = { ...state, inspect };
  if (verified(phase.op, inspect)) return pollNext(go(next, { name: "result", op: phase.op }));
  if (inspect.state === "waiting" || inspect.state === "stock_waiting") {
    return pollNext(answered(next, phase, "again"));
  }
  // `other` or anything else: the watch's own screen says what it is (§4.1).
  return pollNext(go(next, { name: "watch" }));
}

function restartFailed(state: FlowState, phase: Restart, failure: Failure): FlowState {
  if (failure.code === "garmin_mode") {
    // Off MTP as on an unplug (from `again`, the next attempt), with the switch or the guidance
    // inside the restart (§4.1, §4.2). A restart the app doesn't verify needs no switch.
    const attempt = phase.phase === "again" ? state.attempt + 1 : state.attempt;
    const off: Restart = { ...phase, phase: "restart", reconnected: false, starting: false, interruption: null };
    if (!off.verifiable) return pollNext({ ...state, attempt, phase: off });
    const current = phase.interruption?.kind === "garmin_mode" ? phase.interruption.garmin : null;
    const next = garminRead({ ...state, attempt }, failure, current);
    const restart: Restart = { ...off, interruption: { kind: "garmin_mode", garmin: next.garmin } };
    return pollNext(go(next.state, restart, next.effects));
  }
  if (failure.code === "no_device") {
    // Unplugged: from `again`, that starts the next attempt (§4.1). A drive is looked for as on
    // connect; a drive already found stays until inspect_drive says otherwise.
    const attempt = phase.phase === "again" ? state.attempt + 1 : state.attempt;
    const interruption = phase.interruption?.kind === "drive" ? phase.interruption : null;
    const unplugged: Restart = { ...phase, phase: "restart", reconnected: false, starting: false, interruption };
    const next = pollNext({ ...state, attempt, phase: unplugged });
    return phase.phase === "unplug" ? next : withEffects(next, { kind: "inspect_drive", token: state.token });
  }
  if (failure.code === "device_busy" && phase.phase !== "unplug") {
    return pollNext({ ...state, phase: { ...phase, interruption: { kind: "busy", failure } } });
  }
  if (phase.phase === "unplug" || phase.phase === "again" || !phase.verifiable || failure.code === "device_busy") {
    return pollNext(state);
  }
  if (failure.code === "device_error" || failure.code === "extdata_not_found") {
    const next = answered(state, phase, phase.phase === "other_watch" ? "other_watch" : "verifying");
    if (next.phase.name === "restart" && next.phase.starting) return pollNext(next);
  }
  return go(state, { name: "error", op: phase.op, failure });
}

/**
 * issue_link's input for sharing `what` about the watch the flow is on (§5.2, share). From
 * connect there is no watch read: busy shares the holder, a problem its error (§6).
 */
function shareInput(state: FlowState, what: ShareWhat): IssueLinkInput {
  const phase = state.phase;
  if (phase.name === "connect") {
    if (phase.mode === "busy" && phase.failure.busy) return { busy: phase.failure.busy };
    if (phase.mode === "problem") return { error: { code: phase.failure.code, message: phase.failure.message } };
    return {};
  }
  const drive = shownDrive(phase);
  const report = drive ? drive.report : state.inspect?.report;
  if (!report) return {};
  const input: IssueLinkInput = { report };
  if (phase.name === "restart" && phase.interruption?.kind === "busy" && phase.interruption.failure.busy) {
    input.busy = phase.interruption.failure.busy;
  }
  const last = state.lastOp;
  if (last && !drive) {
    const operation = last.op === "install" ? "install" : "restore_stock";
    input.outcome =
      "output" in last
        ? {
            operation,
            status: last.output.status,
            parts: last.output.packs.flatMap((pack) => pack.parts.map((part) => part.part)),
          }
        : { operation, error: last.failure.code };
  }
  if (what === "result") {
    const { result, views, phone, marker } = answersFor(state);
    input.answers = { views, ...(result && { result }), ...(phone && { phone }), ...(marker && { marker }) };
  }
  return input;
}

function startShare(state: FlowState, what: ShareWhat): FlowState {
  const input = shareInput(state, what);
  const back = state.phase.name === "share" ? state.phase.back : state.phase;
  return go(state, { name: "share", what, input, link: null, state: "ready", back }, [{ kind: "issue_link", input }]);
}

/** The check's first step, loading the phone page and the test marks unless loaded. */
function startCheck(state: FlowState): FlowState {
  const effects: Effect[] = [];
  if (state.phone?.lang !== state.lang) effects.push({ kind: "phone_page", lang: state.lang });
  if (!state.marks) effects.push({ kind: "test_marks" });
  return go(state, { name: "check", step: "send" }, effects);
}

function startOp(state: FlowState, op: Op): FlowState {
  // Never beside a switch to MTP (§4.2): Install and Restore aren't offered then.
  if (switching(state.phase)) return state;
  return go(state, { name: "working", op, progress: null }, [{ kind: "run", op }]);
}

/** The restart step after a write, or another restart from the outcome (§4.1). */
function startRestart(state: FlowState, op: Op, attempt: number, isVerifiable: boolean): FlowState {
  const restart: Phase = {
    name: "restart",
    op,
    phase: "unplug",
    verifiable: isVerifiable,
    reconnected: false,
    starting: false,
    interruption: null,
  };
  return go({ ...state, attempt }, restart);
}

function outcomeKind(state: FlowState): OutcomeKind {
  const result = answersFor(state).result;
  if (result === "no_notification") return "no_notification";
  if (result !== "georgian") return "didnt_show";
  return state.inspect?.support.status === "tested" ? "works_confirmed" : "works_first";
}

function act(state: FlowState, id: ActionId, payload: ActionPayload | undefined): FlowState {
  const phase = state.phase;
  if (id === "which_watches") return withEffects(state, { kind: "open_watch_list" });
  if (id === "open_update") return state.update ? withEffects(state, { kind: "open_update" }) : state;
  if (id === "diagnostics") {
    // From More on every screen but share (§11).
    if (phase.name === "share" || phase.name === "diagnostics") return state;
    const diagnostics: Phase = { name: "diagnostics", back: phase, state: "collecting", output: null, savedPath: null };
    return go(state, diagnostics, [{ kind: "diagnose", events: state.events }]);
  }
  // Available from More on every screen but these.
  if (id === "restore") {
    const busy = phase.name === "working" || phase.name === "confirm_restore" || phase.name === "diagnostics";
    if (!state.inspect?.restorable || busy || switching(phase)) return state;
    return go(state, { name: "confirm_restore", back: phase });
  }
  if (id === "share" && payload?.action === "share") {
    if (phase.name === "share") return withEffects(state, { kind: "share_report", input: phase.input });
    return startShare(state, payload.what);
  }
  switch (phase.name) {
    case "connect":
      return id === "retry_now" ? pollNow(state) : state;
    case "drive": {
      const issue = phase.drive.support.issue;
      return id === "open_issue" && issue ? withEffects(state, { kind: "open_issue", issue }) : state;
    }
    case "watch": {
      const verdict = state.inspect && verdictOf(state.inspect);
      switch (id) {
        case "install":
          return verdict?.kind === "install" ? startOp(state, "install") : state;
        case "check":
          return startCheck(state);
        case "retry_now":
          return pollNow(state);
        case "open_issue": {
          const issue = state.inspect?.support.issue;
          return issue ? withEffects(state, { kind: "open_issue", issue }) : state;
        }
        default:
          return state;
      }
    }
    case "confirm_restore":
      if (id === "confirm_restore") return startOp(state, "restore");
      if (id === "cancel" || id === "back") return go(state, phase.back);
      return state;
    case "restart": {
      if (id === "skip_verify") return phase.op === "install" ? startCheck(state) : home(state);
      if (id === "done") return home(state);
      if (id === "retry_now") return pollNow(state);
      const issue = phase.interruption?.kind === "drive" ? phase.interruption.drive.support.issue : null;
      if (id === "open_issue" && issue) return withEffects(state, { kind: "open_issue", issue });
      return state;
    }
    case "result":
      if (id === "check") return startCheck(state);
      if (id === "done") return home(state);
      return state;
    case "check":
      if (phase.step === "send") {
        if (id === "send_test_done") return go(state, { name: "check", step: "answer" });
        if (id === "back") return home(state);
        return state;
      }
      if (id === "answer" && payload?.action === "answer") {
        const part = state.inspect?.device.part_number;
        return part ? { ...state, answers: { ...state.answers, [part]: coherent(payload.answers) } } : state;
      }
      if (id === "back") return go(state, { name: "check", step: "send" });
      if (id === "done" && answersFor(state).result && state.inspect) {
        const result = answersFor(state).result;
        const key = result && keyFor(state.inspect, result);
        const effects: Effect[] = key && !(shareKeyId(key) in state.shared) ? [{ kind: "shared_before", key }] : [];
        return go(state, { name: "outcome", kind: outcomeKind(state) }, effects);
      }
      return state;
    case "outcome":
      if (id === "done") return home(state);
      if (id === "check") return startCheck(state);
      if (id === "restart_again") {
        const last = state.lastOp;
        const isVerifiable = last && "output" in last ? verifiable(last.output.packs) : true;
        return startRestart(state, "install", state.attempt + 1, isVerifiable);
      }
      return state;
    case "share":
      if (id === "done" || id === "back" || id === "cancel") return go(state, phase.back);
      return state;
    case "error":
      if (id === "try_again") return startOp(state, phase.op);
      if (id === "back") return home(state);
      return state;
    case "working":
      return state;
    case "diagnostics": {
      const text = phase.output?.text;
      if (id === "back") return go(state, phase.back);
      if (id === "save_diagnostics" && text) return withEffects(state, { kind: "save_diagnostics", text });
      if (id === "copy_diagnostics" && text) return withEffects(state, { kind: "copy_text", text });
      if (id === "show_file" && phase.savedPath) return withEffects(state, { kind: "reveal_file", path: phase.savedPath });
      return state;
    }
  }
}

function withEffects(state: FlowState, ...effects: Effect[]): FlowState {
  return { ...state, effects: [...state.effects, ...effects] };
}

/** Local time without a zone, as the event log shows it: "2026-11-24T03:14:05". */
export function localTime(date: Date): string {
  const two = (n: number) => String(n).padStart(2, "0");
  return (
    `${date.getFullYear()}-${two(date.getMonth() + 1)}-${two(date.getDate())}` +
    `T${two(date.getHours())}:${two(date.getMinutes())}:${two(date.getSeconds())}`
  );
}

/**
 * The events a screen opened over another (its `back`) handles itself; every other one goes on
 * underneath: an install finishes, a timer fires, the switch to MTP ends (§4.2, §11).
 */
type Own = Partial<Record<FlowEvent["type"], true>>;
const OWN_EVENTS: Record<"diagnostics" | "share" | "confirm_restore", Own> = {
  diagnostics: { action: true, diagnosed: true, diagnostics_saved: true, diagnostics_copied: true, diagnostics_failed: true },
  share: { action: true, link_ready: true, link_failed: true, shared: true, share_failed: true },
  confirm_restore: { action: true },
};

/**
 * The flow's one transition function. `now` stamps the event log; it is the only input that
 * isn't the state or the event.
 */
export function reduce(previous: FlowState, event: FlowEvent, now: Date = new Date(0)): FlowState {
  return logged(previous, event, step(previous, event), localTime(now));
}

/** `event` applied to the screen it belongs to: the one on top, or the one under it. */
function step(state: FlowState, event: FlowEvent): FlowState {
  const phase = state.phase;
  if ("back" in phase && !OWN_EVENTS[phase.name][event.type]) return inBackground(state, phase, event);
  return transition(state, event);
}

/**
 * `event` applied to the phase under the open screen, so returning shows where it got to. The
 * watch isn't read meanwhile (polling pauses, §11); going back reads it again.
 */
function inBackground(state: FlowState, overlay: Extract<Phase, { back: Phase }>, event: FlowEvent): FlowState {
  const inner = step({ ...state, phase: overlay.back }, event);
  return {
    ...inner,
    phase: { ...overlay, back: inner.phase },
    effects: inner.effects.filter((e) => e.kind !== "inspect" && e.kind !== "inspect_drive"),
  };
}

/** `next` with `event` added to the session's log, when it is one the log keeps (§11). */
function logged(previous: FlowState, event: FlowEvent, next: FlowState, time: string): FlowState {
  const model = previous.inspect?.device.model ?? null;
  const entries: DiagnoseEvent[] = [];
  let problem: Failure | null = null;
  const poll = (detail: string, polled: string | null) => {
    const last = [...previous.events].reverse().find((e) => e.kind === "poll");
    // A poll is logged when its result changes, so 2 s polls don't push everything else out.
    if (last?.detail !== detail || last.model !== polled) entries.push({ time, kind: "poll", detail, model: polled });
  };
  switch (event.type) {
    case "inspected":
      if (event.token === previous.token) poll(event.output.state, event.output.device.model);
      break;
    case "inspect_failed":
      if (event.token === previous.token) {
        poll(event.failure.code, null);
        problem = event.failure;
        if (next.effects.some((e) => e.kind === "switch_to_mtp")) {
          entries.push({ time, kind: "operation_start", detail: "switch_to_mtp", model });
        }
      }
      break;
    case "switched":
      entries.push({ time, kind: "operation_end", detail: `switch_to_mtp: ${event.output.status}`, model });
      break;
    case "switch_failed":
      entries.push({ time, kind: "operation_end", detail: `switch_to_mtp: ${event.failure.code}`, model });
      problem = event.failure;
      break;
    case "drive_inspected":
      if (event.token === previous.token) poll("usb_drive", event.output.device.model);
      break;
    case "op_done":
    case "op_failed":
      if (next.lastOp !== previous.lastOp) {
        const result = event.type === "op_done" ? event.output.status : event.failure.code;
        entries.push({ time, kind: "operation_end", detail: `${event.op}: ${result}`, model });
        if (event.type === "op_failed") problem = event.failure;
      }
      break;
    case "shared":
    case "share_failed":
      if (previous.phase.name === "share") {
        const result = event.type === "shared" ? next.phase.name === "share" && next.phase.state : event.failure.code;
        entries.push({ time, kind: "share", detail: `${previous.phase.what}: ${result}`, model });
        if (event.type === "share_failed") problem = event.failure;
      }
      break;
    case "link_failed":
      problem = event.failure;
      break;
    case "action":
      if (next.phase.name === "working" && previous.phase.name !== "working") {
        entries.push({ time, kind: "operation_start", detail: next.phase.op, model });
      }
      if (previous.phase.name === "check" && next.phase.name === "outcome") {
        entries.push({ time, kind: "answer", detail: `result: ${answersFor(previous).result}`, model });
      }
      break;
  }
  if (!entries.length && !problem) return next;
  return {
    ...next,
    events: [...next.events, ...entries].slice(-MAX_EVENTS),
    lastProblem: problem ? { time, code: problem.code, holder: problem.busy?.holder ?? null } : next.lastProblem,
  };
}

function transition(previous: FlowState, event: FlowEvent): FlowState {
  // Each transition's effects are its own; the runner has run the previous ones.
  const fresh: FlowState = previous.effects.length ? { ...previous, effects: [] } : previous;
  // A read that finds no watch, or the watch over MTP, ends its appearance in Garmin mode: the
  // next one may be switched again (§4.2).
  const gone =
    (event.type === "inspected" || (event.type === "inspect_failed" && event.failure.code === "no_device")) &&
    event.token === fresh.token;
  const state: FlowState = gone && fresh.switchTried ? { ...fresh, switchTried: false } : fresh;
  const phase = state.phase;
  switch (event.type) {
    case "start": {
      const next = { ...state, helpToken: state.helpToken + 1 };
      const started = withEffects(next, { kind: "help_timer", token: next.helpToken, delay: HELP_MS });
      // The update check runs once per run, behind the first screen (§12).
      const update = state.updateAsked ? started : withEffects({ ...started, updateAsked: true }, { kind: "check_update" });
      return pollNow(update);
    }
    case "help_due":
      if (event.token !== state.helpToken || phase.name !== "connect" || phase.mode !== "searching") return state;
      return { ...state, phase: { ...phase, showHelp: true } };
    case "inspect_slow":
      // The watch answered and is being read: "Checking the watch…" (§4.1).
      if (event.token !== state.token || phase.name !== "restart" || phase.phase !== "restart") return state;
      return { ...state, phase: { ...phase, phase: "verifying" } };
    case "started_up":
      if (event.token !== state.startingToken || phase.name !== "restart") return state;
      return { ...state, phase: { ...phase, starting: false } };
    case "inspected":
      if (event.token !== state.token) return state;
      switch (phase.name) {
        case "connect":
        case "drive":
        case "error":
        case "watch":
          return pollNext(showWatch(state, event.output));
        case "restart":
          return restartRead(state, phase, event.output);
        case "result":
          // After a restore (the only result that polls): the same watch stays on "Garmin's font
          // is back"; another one gets its own screen.
          return state.inspect?.device.part_number === event.output.device.part_number
            ? pollNext({ ...state, inspect: event.output })
            : pollNext(showWatch(state, event.output));
        default:
          return state;
      }
    case "inspect_failed":
      if (event.token !== state.token) return state;
      switch (phase.name) {
        case "connect":
        case "watch":
        case "result":
          return readFailed(state, event.failure);
        case "error":
          // Disconnected during the write: the error (and its rollback) stays until the watch is
          // back; back in Garmin mode, it is switched as on connect (§4.2).
          return event.failure.code === "garmin_mode" ? readFailed(state, event.failure) : pollNext(state);
        case "drive":
          // Still a drive? inspect_drive says; anything else is read as usual.
          return event.failure.code === "no_device"
            ? pollNext(withEffects(state, { kind: "inspect_drive", token: state.token }))
            : readFailed(state, event.failure);
        case "restart":
          return restartFailed(state, phase, event.failure);
        default:
          return state;
      }
    case "drive_inspected":
      if (event.token !== state.token) return state;
      if (phase.name === "drive" || (phase.name === "connect" && phase.mode === "searching")) {
        return {
          ...state,
          phase: { name: "drive", drive: event.output },
          effects: [...state.effects, ...askSharedBefore(state, event.output)],
        };
      }
      if (phase.name === "restart" && phase.phase !== "unplug") {
        return { ...state, phase: { ...phase, interruption: { kind: "drive", drive: event.output } } };
      }
      return state;
    case "drive_failed":
      if (event.token !== state.token) return state;
      if (phase.name === "restart" && phase.interruption?.kind === "drive") {
        return { ...state, phase: { ...phase, interruption: null } };
      }
      return phase.name === "drive" ? searching(state) : state;
    case "progress":
      return phase.name === "working" ? { ...state, phase: { ...phase, progress: event.progress } } : state;
    case "op_done":
      if (phase.name !== "working" || phase.op !== event.op) return state;
      return startRestart(
        { ...state, lastOp: { op: event.op, output: event.output } },
        event.op,
        1,
        verifiable(event.output.packs),
      );
    case "op_failed": {
      if (phase.name !== "working" || phase.op !== event.op) return state;
      const next = { ...state, lastOp: { op: event.op, failure: event.failure } };
      if (event.failure.code === "garmin_mode") {
        // Replugged in Garmin mode before the write: nothing was written; switch it as on connect.
        return readFailed(next, event.failure);
      }
      if (event.failure.code === "device_busy") {
        return go(next, { name: "connect", mode: "busy", failure: event.failure });
      }
      return go(next, { name: "error", op: event.op, failure: event.failure });
    }
    case "phone_loaded":
      return { ...state, phone: { lang: event.lang, page: event.page } };
    case "marks_loaded":
      return { ...state, marks: event.marks };
    case "link_ready":
      return phase.name === "share" ? { ...state, phase: { ...phase, link: event.link } } : state;
    case "link_failed":
      return phase.name === "share" ? { ...state, phase: { ...phase, state: "failed" } } : state;
    case "shared": {
      if (phase.name !== "share") return state;
      const source = shownDrive(phase.back) ?? state.inspect;
      const result = phase.what === "result" ? answersFor(state).result : undefined;
      const key = source && phase.what !== "problem" ? keyFor(source, result ?? "font_list") : null;
      const next: FlowState = {
        ...state,
        phase: { ...phase, state: event.shared.complete ? "opened" : "opened_paste" },
        shared: key ? { ...state.shared, [shareKeyId(key)]: true } : state.shared,
      };
      return key ? withEffects(next, { kind: "mark_shared", key }) : next;
    }
    case "share_failed":
      return phase.name === "share" ? { ...state, phase: { ...phase, state: "failed" } } : state;
    case "shared_before":
      return { ...state, shared: { ...state.shared, [shareKeyId(event.key)]: event.value } };
    case "language": {
      const next = withEffects({ ...state, lang: event.lang }, { kind: "save_language", lang: event.lang });
      return phase.name === "check" ? withEffects(next, { kind: "phone_page", lang: event.lang }) : next;
    }
    case "diagnosed":
      return phase.name === "diagnostics" ? { ...state, phase: { ...phase, state: "ready", output: event.output } } : state;
    case "diagnostics_saved":
      return phase.name === "diagnostics" ? { ...state, phase: { ...phase, state: "saved", savedPath: event.path } } : state;
    case "diagnostics_copied":
      return phase.name === "diagnostics" ? { ...state, phase: { ...phase, state: "copied" } } : state;
    case "diagnostics_failed":
      return phase.name === "diagnostics" ? { ...state, phase: { ...phase, state: "failed" } } : state;
    case "switched":
    case "switch_failed":
      return switchEnded(state, afterSwitch(event));
    case "update_checked":
      return { ...state, update: event.output.newer ? event.output.latest : null };
    case "action":
      return act(state, event.id, event.payload);
  }
}
