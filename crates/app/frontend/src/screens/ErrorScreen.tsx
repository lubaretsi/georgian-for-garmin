import type { Copy } from "../copy";
import { fmt } from "../copy";
import type { ErrorCode } from "../core/types.gen";
import type { ScreenModel } from "../model/screens";
import type { FooterAction } from "../ui/ActionBar";
import { Disclosure } from "../ui/Disclosure";
import { Notice } from "../ui/Notice";
import { emitting, reportProblem, shareFontList } from "./common";
import type { ScreenContext, ScreenView } from "./types";

type ErrorModel = Extract<ScreenModel, { screen: "error" }>;

/** The §6 groups: codes that share a title, body and action. */
export type ErrorGroup = keyof Copy["error"] & ErrorCode;

export function errorGroup(code: ErrorCode): ErrorGroup {
  switch (code) {
    case "conflict":
    case "restore_first":
    case "verification_failed":
    case "device_error":
    case "no_device":
    case "unsupported_device":
      return code;
    // device_busy, permission_denied and multiple_devices go to connect (§6); should one
    // reach this screen, it reads as the app's own failure and offers a report.
    default:
      return "internal";
  }
}

/** Codes whose failure may have written something, so the rollback is shown. */
const SHOWS_ROLLBACK: Record<ErrorGroup, boolean> = {
  conflict: false,
  restore_first: false,
  verification_failed: true,
  device_error: true,
  no_device: true,
  unsupported_device: false,
  internal: true,
};

/** error: what failed, what state the watch is in, and the one way forward. */
export function ErrorScreen(model: ErrorModel, ctx: ScreenContext): ScreenView {
  const e = ctx.copy.error;
  const group = errorGroup(model.code);
  const text = e[group];
  const rollback = SHOWS_ROLLBACK[group] ? model.rollback : null;
  const incomplete = rollback?.status === "incomplete";
  const back = emitting(ctx, ctx.copy.actions.back, "back");
  const restore = emitting(ctx, ctx.copy.actions.restore, "restore");
  const tryAgain = emitting(ctx, ctx.copy.actions.tryAgain, "try_again");

  let primary: FooterAction | undefined;
  switch (group) {
    case "conflict":
      primary = shareFontList(ctx);
      break;
    case "restore_first":
      primary = restore;
      break;
    case "verification_failed":
    case "device_error":
      primary = tryAgain;
      break;
    case "no_device":
      primary = undefined;
      break;
    case "unsupported_device":
      primary = back;
      break;
    case "internal":
      primary = reportProblem(ctx);
      break;
  }
  // Files left in an unknown state: putting Garmin's font back comes first (§6).
  if (incomplete) primary = restore;
  // Back is the secondary unless it is already the main action.
  const secondary = primary === back ? undefined : back;

  return {
    step: "install",
    title: text.title,
    watch: model.watch,
    content: (
      <>
        <Notice tone="error" live>
          <p>{text.body}</p>
        </Notice>
        {rollback ? (
          rollback.status === "incomplete" ? (
            <Notice tone="warning">
              <p>{fmt(e.rollback.incomplete, { files: rollback.files.join(", ") })}</p>
              <p>{e.rollback.restoreHint}</p>
            </Notice>
          ) : (
            <p className="muted">{e.rollback.complete}</p>
          )
        ) : null}
        {model.detail.trim() !== "" ? (
          <Disclosure summary={ctx.copy.actions.details}>
            <pre className="detail-text">{model.detail}</pre>
          </Disclosure>
        ) : null}
      </>
    ),
    primary,
    secondary,
  };
}
