import type { ScreenModel } from "../model/screens";
import { OutcomeScreen, ShareScreen } from "./After";
import { CheckScreen } from "./Check";
import { ConnectBusy, ConnectProblem, ConnectSearching } from "./Connect";
import { DiagnosticsScreen } from "./Diagnostics";
import { ErrorScreen } from "./ErrorScreen";
import { ConfirmRestoreScreen, RestartScreen, ResultScreen, WorkingScreen } from "./Operation";
import type { ScreenContext, ScreenView } from "./types";
import { DriveScreen, WatchScreen } from "./Watch";

export type { Emit, FooterAction, ScreenContext, ScreenView } from "./types";
export { TITLE_ID } from "./types";

/** The view for a screen model: one screen function per screen and mode. */
export function renderScreen(model: ScreenModel, ctx: ScreenContext): ScreenView {
  switch (model.screen) {
    case "connect":
      switch (model.mode) {
        case "searching":
          return ConnectSearching(model, ctx);
        case "busy":
          return ConnectBusy(model, ctx);
        case "problem":
          return ConnectProblem(model, ctx);
      }
      break;
    case "drive":
      return DriveScreen(model, ctx);
    case "watch":
      return WatchScreen(model, ctx);
    case "confirm_restore":
      return ConfirmRestoreScreen(model, ctx);
    case "working":
      return WorkingScreen(model, ctx);
    case "restart":
      return RestartScreen(model, ctx);
    case "result":
      return ResultScreen(model, ctx);
    case "check":
      return CheckScreen(model, ctx);
    case "outcome":
      return OutcomeScreen(model, ctx);
    case "share":
      return ShareScreen(model, ctx);
    case "error":
      return ErrorScreen(model, ctx);
    case "diagnostics":
      return DiagnosticsScreen(model, ctx);
  }
}

/**
 * What makes two models "the same screen" for focus: the screen and its mode, phase, step,
 * verdict or state. A change here moves focus to the new heading; progress or answer
 * changes within a screen don't.
 */
export function screenKey(model: ScreenModel): string {
  switch (model.screen) {
    case "connect":
      return model.mode === "problem" ? `connect/problem/${model.problem}` : `connect/${model.mode}`;
    case "drive":
      return `drive/${model.driveOnly}`;
    case "watch":
      return `watch/${model.verdict.kind}`;
    case "working":
      return `working/${model.op}`;
    case "restart":
      return `restart/${model.phase}/${model.attempt}`;
    case "result":
      return `result/${model.op}`;
    case "check":
      return `check/${model.step}`;
    case "outcome":
      return `outcome/${model.kind}`;
    case "share":
      return `share/${model.what}/${model.state === "opened_paste" ? "opened" : model.state}`;
    case "error":
      return `error/${model.code}`;
    case "confirm_restore":
      return "confirm_restore";
    // Saving or copying keeps the user where they are; only arriving moves focus.
    case "diagnostics":
      return "diagnostics";
  }
}
