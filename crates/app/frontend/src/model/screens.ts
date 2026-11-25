import type { ErrorCode, HolderKind as BusyHolderKind } from "../core/types.gen";

/** `confirmedSoftware`: InspectOutput.tested.verified_software, the versions Georgian is confirmed on for this model ([] if none). */
export type WatchInfo = { model: string; modelPart: string; software: string; confirmedSoftware: string[] };
export type Confidence = "confirmed" | "confirmed_other_software" | "confirmed_some_fonts" | "unconfirmed";
export type Result = "georgian" | "boxes" | "question_marks" | "some_letters" | "no_notification" | "not_installed";
export type View = "popup" | "list" | "opened";
export type Answers = { result?: Result; views: View[]; phone?: "android" | "iphone"; marker?: string };

/** Where letters can show, in the issue form's order. */
export const VIEWS: readonly View[] = ["popup", "list", "opened"];

/** Whether `result` means letters showed, the only case where they have views and a test mark. */
export function sawLetters(result: Result | undefined): boolean {
  return result === "georgian" || result === "some_letters";
}

export type Op = "install" | "restore";
export type Localized = { ka: string; en: string };

export type Verdict =
  | { kind: "install"; confidence: Confidence; note?: "partly" | "beta" | "not_visible" }
  | { kind: "installed"; confidence: Confidence }
  | { kind: "restart_pending"; variant: "georgian" | "stock" }
  | { kind: "blocked"; reason: "waiting_other_font" | "unknown_font" }
  | { kind: "unsupported"; reason: Localized | null; restorable: boolean }
  | { kind: "no_font" }
  | { kind: "unreadable" };

export type BusyView = { kind: BusyHolderKind; name: string | null; confirmed: boolean };
export type ConnectProblem =
  | "permission_denied" | "multiple_devices" | "garmin_folder_not_found"
  | "extdata_not_found" | "device_error" | "garmin_mode" | "device_locked" | "internal";
/** The watch in Garmin mode during the restart (§4.2): the switch running, or its guidance. */
export type GarminModeView = "switching" | "guidance" | "locked";
export type RollbackView = { status: "complete" | "incomplete"; files: string[] };

export type ScreenModel =
  | { screen: "connect"; mode: "searching"; showHelp: boolean }
  | { screen: "connect"; mode: "switching" } // the watch in Garmin mode, being switched (§4.2)
  | { screen: "connect"; mode: "busy"; busy: BusyView; duringRestart: boolean } // duringRestart: the step indicator stays on Restart
  | { screen: "connect"; mode: "problem"; problem: ConnectProblem; detail: string }
  | { screen: "drive"; watch: WatchInfo; driveOnly: boolean; issue: number | null; reportWanted: boolean; sharedBefore: boolean; duringRestart: boolean }
  | { screen: "watch"; watch: WatchInfo; verdict: Verdict; issue: number | null; reportWanted: boolean; sharedBefore: boolean }
  | { screen: "confirm_restore"; watch: WatchInfo }
  | { screen: "working"; op: Op; watch: WatchInfo; progress: { done: number; total: number } | null }
  | { screen: "restart"; op: Op; watch: WatchInfo; phase: "unplug" | "restart" | "verifying" | "again" | "other_watch"; attempt: number; verifiable: boolean; garminMode: GarminModeView | null }
  | { screen: "result"; op: Op; watch: WatchInfo; confidence: Confidence }
  | { screen: "check"; watch: WatchInfo; step: "send" | "answer"; answers: Answers; phone: { url: string; qr: string } | null; markPictures: { variant: string; picture: string }[] } // qr, picture: the core's data URLs
  | { screen: "outcome"; watch: WatchInfo; kind: "works_confirmed" | "works_first" | "didnt_show" | "no_notification"; restarts: number; sharedBefore: boolean; result: Result | null } // result: the answer given
  | { screen: "share"; watch: WatchInfo | null; what: "result" | "font_list" | "problem"; summary: { label: string; value: string }[]; reportText: string; state: "ready" | "opened" | "opened_paste" | "failed"; thanks: "tested_next" | "we_look" | null } // watch null: shared from connect (busy, problem)
  | { screen: "error"; op: Op; watch: WatchInfo; code: ErrorCode; rollback: RollbackView | null; detail: string }
  | { screen: "diagnostics"; summary: { label: string; value: string }[]; text: string; state: "collecting" | "ready" | "saved" | "copied" | "failed"; savedPath: string | null }; // from More; Back returns to the screen before

/** What the screen's buttons do. Screens emit these; only the flow acts on them. */
export type ActionId =
  | "install" | "restore" | "confirm_restore" | "cancel" | "check" | "skip_verify"
  | "send_test_done" | "answer" | "share" | "open_issue"
  | "restart_again" | "try_again" | "back" | "retry_now" | "done"
  | "copy_test_text" | "show_more" | "which_watches" | "help"
  | "diagnostics" | "save_diagnostics" | "copy_diagnostics" | "show_file";

/** Extra data some actions carry; every other action carries none. */
export type ActionPayload =
  | { action: "answer"; answers: Answers }   // the whole answer set after a change
  | { action: "share"; what: "result" | "font_list" | "problem" }; // the only way to share:
  // "Share the font list" and "Report this problem" are `share` with that `what`.

/** The More sheet: the flow supplies the data, the shell draws it. */
export type MoreModel = {
  restorable: boolean;                               // shows "Restore original font"
  seen: { label: string; value: string }[] | null;   // "What the app sees"; null with no watch
  reportText: string | null;                         // the exact report, copyable
  version: string;                                   // the app's version
};

/**
 * The app shell: header (language, More), the screen, the fixed footer with its actions.
 *   <Shell model={ScreenModel} more={MoreModel} lang={Lang} platform={Platform}
 *          onAction={(id: ActionId, payload?: ActionPayload) => void}
 *          onLanguage={(lang: Lang) => void} />
 * Restoring from More emits `restore`; the flow answers with `confirm_restore`.
 */
