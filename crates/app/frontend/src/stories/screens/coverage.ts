// The §8 coverage list for Screens/… stories, and which items a story's model covers.
// coverage.test.ts fails when an item has no story.

import type { Platform } from "../../core/backend";
import type { HolderKind } from "../../core/types.gen";
import type { ConnectProblem, ScreenModel } from "../../model/screens";
import { en } from "../../copy/en";
import { errorGroup } from "../../screens/ErrorScreen";

const PLATFORMS: Platform[] = ["macos", "windows", "linux"];
const HOLDERS: HolderKind[] = ["garmin_express", "macos_camera_service", "desktop_mtp", "this_app", "other", "unknown"];
const PROBLEMS: ConnectProblem[] = [
  "permission_denied",
  "multiple_devices",
  "garmin_folder_not_found",
  "extdata_not_found",
  "device_error",
  "internal",
];
const CONFIDENCES = ["confirmed", "confirmed_other_software", "confirmed_some_fonts", "unconfirmed"] as const;

/** Every item of §8's Screens list. */
export const REQUIRED: string[] = [
  // connect: searching (with and without help), busy (each holder kind × platform), each problem
  "connect/searching",
  "connect/searching/help",
  ...HOLDERS.flatMap((kind) => PLATFORMS.map((p) => `connect/busy/${kind}/${p}`)),
  "connect/busy/during_restart",
  ...PROBLEMS.map((p) => `connect/problem/${p}`),
  // drive: switch to MTP, drive-only
  "drive/mtp",
  "drive/drive_only",
  "drive/during_restart",
  // watch: every verdict, every confidence, every note
  "watch/install",
  "watch/installed",
  "watch/restart_pending/georgian",
  "watch/restart_pending/stock",
  "watch/blocked/waiting_other_font",
  "watch/blocked/unknown_font",
  "watch/unsupported/restorable",
  "watch/unsupported/follow",
  "watch/no_font",
  "watch/unreadable",
  ...CONFIDENCES.map((c) => `watch/install/${c}`),
  ...CONFIDENCES.map((c) => `watch/installed/${c}`),
  "watch/install/note/partly",
  "watch/install/note/beta",
  "watch/install/note/not_visible",
  // confirm_restore, working (install and restore, with and without progress)
  "confirm_restore",
  "working/install/progress",
  "working/install/no_progress",
  "working/restore/progress",
  "working/restore/no_progress",
  // restart: each phase, attempt 2, not verifiable
  ...["unplug", "restart", "verifying", "again", "other_watch"].map((p) => `restart/${p}`),
  "restart/attempt_2",
  "restart/not_verifiable",
  // result: install (confirmed and unconfirmed) and restore
  "result/install/confirmed",
  "result/install/unconfirmed",
  "result/restore",
  // check: send, answer (empty, answered, beta marker)
  "check/send",
  "check/answer/empty",
  "check/answer/answered",
  "check/answer/marker",
  // outcome: each kind, restarts 0 and 2, shared before
  ...["works_confirmed", "works_first", "didnt_show", "no_notification"].map((k) => `outcome/${k}`),
  "outcome/restarts_0",
  "outcome/restarts_2",
  "outcome/shared_before",
  // share: each what × state
  ...["result", "font_list", "problem"].flatMap((w) =>
    ["ready", "opened", "opened_paste", "failed"].map((s) => `share/${w}/${s}`),
  ),
  // error: each code group, rollback complete and incomplete
  ...["conflict", "restore_first", "restart_first", "verification_failed", "device_error", "no_device", "unsupported_device", "internal"].map(
    (g) => `error/${g}`,
  ),
  "error/rollback/complete",
  "error/rollback/incomplete",
  // diagnostics: collecting, ready (no watch, watch found, busy), saved, copied, failed
  "diagnostics/collecting",
  "diagnostics/ready/no_watch",
  "diagnostics/ready/watch",
  "diagnostics/ready/busy",
  "diagnostics/saved",
  "diagnostics/copied",
  "diagnostics/failed",
];

/** The coverage items one story's model (on its platform) stands for. */
export function covers(model: ScreenModel, platform: Platform): string[] {
  switch (model.screen) {
    case "connect":
      switch (model.mode) {
        case "searching":
          return [model.showHelp ? "connect/searching/help" : "connect/searching"];
        case "busy":
          return [
            `connect/busy/${model.busy.kind}/${platform}`,
            ...(model.duringRestart ? ["connect/busy/during_restart"] : []),
          ];
        case "problem":
          return [`connect/problem/${model.problem}`];
      }
      return [];
    case "drive":
      return [model.driveOnly ? "drive/drive_only" : "drive/mtp", ...(model.duringRestart ? ["drive/during_restart"] : [])];
    case "watch": {
      const v = model.verdict;
      switch (v.kind) {
        case "install":
          return ["watch/install", `watch/install/${v.confidence}`, ...(v.note ? [`watch/install/note/${v.note}`] : [])];
        case "installed":
          return ["watch/installed", `watch/installed/${v.confidence}`];
        case "restart_pending":
          return [`watch/restart_pending/${v.variant}`];
        case "blocked":
          return [`watch/blocked/${v.reason}`];
        case "unsupported":
          return [v.restorable ? "watch/unsupported/restorable" : "watch/unsupported/follow"];
        case "no_font":
        case "unreadable":
          return [`watch/${v.kind}`];
      }
      return [];
    }
    case "confirm_restore":
      return ["confirm_restore"];
    case "working":
      return [`working/${model.op}/${model.progress ? "progress" : "no_progress"}`];
    case "restart":
      return [
        `restart/${model.phase}`,
        ...(model.attempt >= 2 ? ["restart/attempt_2"] : []),
        ...(model.verifiable ? [] : ["restart/not_verifiable"]),
      ];
    case "result":
      return [model.op === "restore" ? "result/restore" : `result/install/${model.confidence === "confirmed" ? "confirmed" : "unconfirmed"}`];
    case "check":
      if (model.step === "send") return ["check/send"];
      if (model.markPictures.length > 0) return ["check/answer/marker"];
      return [model.answers.result ? "check/answer/answered" : "check/answer/empty"];
    case "outcome":
      return [
        `outcome/${model.kind}`,
        ...(model.restarts === 0 ? ["outcome/restarts_0"] : model.restarts >= 2 ? ["outcome/restarts_2"] : []),
        ...(model.sharedBefore ? ["outcome/shared_before"] : []),
      ];
    case "share":
      return [`share/${model.what}/${model.state}`];
    case "error":
      return [
        `error/${errorGroup(model.code)}`,
        ...(model.rollback ? [`error/rollback/${model.rollback.status}`] : []),
      ];
    case "diagnostics": {
      if (model.state !== "ready") return [`diagnostics/${model.state}`];
      // Read from the en summary the flow builds: the watch row, and the connection's plain words
      // (busy reads as the busy screen's title).
      const s = en.diagnostics.summary;
      const value = (label: string) => model.summary.find((row) => row.label === label)?.value;
      const connection = value(s.connection) ?? "";
      const busyTitles = Object.values(en.connect.busy.title).map((t) => new RegExp(`^${t.replace("{program}", ".+")}$`));
      if (busyTitles.some((title) => title.test(connection))) return ["diagnostics/ready/busy"];
      return [value(s.watch) === s.noWatch ? "diagnostics/ready/no_watch" : "diagnostics/ready/watch"];
    }
  }
}
