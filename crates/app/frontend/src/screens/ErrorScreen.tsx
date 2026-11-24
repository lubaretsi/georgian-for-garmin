import { CircleCheck } from "lucide-react";

import { type Copy, fmt } from "../copy";
import type { ErrorCode } from "../core/types.gen";
import type { ScreenModel } from "../model/screens";
import { WatchScene } from "../ui/art/Scenes";
import { Alert } from "../ui/Alert";
import { CopyBlock } from "../ui/CopyBlock";
import { Disclosure } from "../ui/Disclosure";
import { GroupFootnote } from "../ui/Group";
import { emitting, reportProblem, shareFontList, WatchFacts } from "./common";
import type { FooterAction, ScreenContext, ScreenView } from "./types";

type ErrorModel = Extract<ScreenModel, { screen: "error" }>;

/** The codes that have their own text; every other code reads as the app's own failure. */
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
      primary = emitting(ctx, ctx.copy.actions.tryAgain, "try_again");
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
  const hasDetail = model.detail.trim() !== "";

  return {
    step: "install",
    hero: <WatchScene face="alert" cable={group === "no_device" ? "pulled" : "plugged"} />,
    title: text.title,
    content: (
      <>
        {incomplete ? (
          // The undo left files in an unknown state: say so instead of the code's body (which
          // may say "try again"), and point at the Restore main action. File names: Details.
          <Alert tone="danger" live>
            <p>{e.rollback.incomplete}</p>
            <p>{e.rollback.restoreHint}</p>
          </Alert>
        ) : (
          <Alert tone="danger" live>
            <p>{text.body}</p>
          </Alert>
        )}
        <div>
          <WatchFacts copy={ctx.copy} watch={model.watch} />
          {rollback?.status === "complete" ? (
            <GroupFootnote icon={<CircleCheck size={14} />}>{e.rollback.complete}</GroupFootnote>
          ) : null}
        </div>
        {incomplete || hasDetail ? (
          <Disclosure summary={ctx.copy.actions.details}>
            <div className="flex flex-col gap-2">
              {incomplete ? <p className="px-3.5 text-row text-fg-2">{fmt(e.rollback.files, { files: rollback.files.join(", ") })}</p> : null}
              {hasDetail ? <CopyBlock text={model.detail} label={ctx.copy.actions.details} variant="report" lang="en" /> : null}
            </div>
          </Disclosure>
        ) : null}
      </>
    ),
    primary,
    secondary,
  };
}
