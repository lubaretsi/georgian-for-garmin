import { fmt } from "../copy";
import type { ScreenModel } from "../model/screens";
import type { FooterAction } from "../ui/ActionBar";
import { Waiting } from "../ui/bits";
import { Notice } from "../ui/Notice";
import { checkWhy, confidenceLine, emitting, reportProblem, shareFontList } from "./common";
import type { ScreenContext, ScreenView } from "./types";

type Drive = Extract<ScreenModel, { screen: "drive" }>;
type Watch = Extract<ScreenModel, { screen: "watch" }>;

/** The font-list offer: the button when wanted, a thank-you when already shared. */
function fontListOffer(
  ctx: ScreenContext,
  model: { reportWanted: boolean; sharedBefore: boolean },
): { line: string | null; action: FooterAction | undefined } {
  if (!model.reportWanted) return { line: null, action: undefined };
  if (model.sharedBefore) return { line: ctx.copy.watch.sharedBefore, action: undefined };
  return { line: ctx.copy.watch.shareWhy, action: shareFontList(ctx) };
}

/** drive: the watch is a USB drive. Either it can switch to MTP, or it can't be changed yet. */
export function DriveScreen(model: Drive, ctx: ScreenContext): ScreenView {
  const d = ctx.copy.drive;
  const offer = fontListOffer(ctx, model);
  const follow = model.driveOnly && model.issue !== null;
  return {
    step: "connect",
    title: fmt(d.title, { model: model.watch.model }),
    illustration: { icon: "cable", tone: model.driveOnly ? "muted" : "accent" },
    status: model.driveOnly ? undefined : d.mtp.waiting,
    content: model.driveOnly ? (
      <>
        <p className="lead">{d.driveOnly.body}</p>
        {follow ? <p>{d.driveOnly.follow}</p> : null}
        {offer.line ? <p className="muted">{offer.line}</p> : null}
      </>
    ) : (
      <>
        <p className="lead">{d.mtp.body}</p>
        <Notice tone="info">
          <p>{d.mtp.how}</p>
        </Notice>
        <Waiting>{d.mtp.waiting}</Waiting>
        {offer.line ? <p className="muted">{offer.line}</p> : null}
      </>
    ),
    primary: follow ? emitting(ctx, ctx.copy.actions.followUpdates, "open_issue") : undefined,
    secondary: offer.action,
  };
}

/** watch: the connected watch and what the app can do for it, one verdict at a time. */
export function WatchScreen(model: Watch, ctx: ScreenContext): ScreenView {
  const w = ctx.copy.watch;
  const { watch, verdict } = model;
  const base = { step: "install" as const, watch };
  switch (verdict.kind) {
    case "install": {
      const confirmed = verdict.confidence === "confirmed";
      return {
        ...base,
        title: confirmed ? w.install.titleConfirmed : w.install.title,
        illustration: { icon: "watch", tone: confirmed ? "success" : "accent" },
        content: (
          <>
            <p className="lead">{confidenceLine(ctx.copy, ctx.lang, watch, verdict.confidence)}</p>
            {verdict.confidence === "unconfirmed" ? <p>{w.install.unconfirmedFallback}</p> : null}
            {verdict.note ? (
              <Notice tone="info">
                <p>{w.install.note[verdict.note]}</p>
              </Notice>
            ) : null}
          </>
        ),
        primary: emitting(ctx, ctx.copy.actions.install, "install"),
      };
    }
    case "installed": {
      const why = checkWhy(ctx.copy, watch, verdict.confidence);
      return {
        ...base,
        step: "check",
        title: w.installed.title,
        illustration: { icon: "check", tone: "success" },
        content: (
          <>
            <p className="lead">{confidenceLine(ctx.copy, ctx.lang, watch, verdict.confidence)}</p>
            <p>{why ?? w.installed.unplug}</p>
          </>
        ),
        primary: why ? emitting(ctx, ctx.copy.actions.check, "check") : undefined,
        secondary: why ? undefined : emitting(ctx, ctx.copy.actions.didntWorkCheck, "check"),
      };
    }
    case "restart_pending":
      return {
        ...base,
        step: "restart",
        title: w.restartPending.title,
        illustration: { icon: "restart" },
        content: <p className="lead">{w.restartPending[verdict.variant]}</p>,
      };
    case "blocked": {
      const text = w.blocked[verdict.reason];
      const offer =
        verdict.reason === "unknown_font"
          ? fontListOffer(ctx, { reportWanted: true, sharedBefore: model.sharedBefore })
          : { line: null, action: undefined };
      return {
        ...base,
        title: text.title,
        illustration: { icon: verdict.reason === "waiting_other_font" ? "restart" : "watch", tone: "muted" },
        content: (
          <>
            <p className="lead">{text.body}</p>
            {offer.line && model.sharedBefore ? <p className="muted">{offer.line}</p> : null}
          </>
        ),
        primary: offer.action,
      };
    }
    case "unsupported": {
      const offer = fontListOffer(ctx, model);
      const follow = !verdict.restorable && model.issue !== null;
      return {
        ...base,
        title: fmt(w.unsupported.title, { model: watch.model }),
        illustration: { icon: "watch", tone: "muted" },
        content: (
          <>
            {verdict.reason ? <p className="lead">{verdict.reason[ctx.lang]}</p> : null}
            {verdict.restorable ? <p>{w.unsupported.restorable}</p> : null}
            {follow ? <p>{w.unsupported.follow}</p> : null}
            {offer.line ? <p className="muted">{offer.line}</p> : null}
          </>
        ),
        primary: verdict.restorable
          ? emitting(ctx, ctx.copy.actions.restore, "restore")
          : follow
            ? emitting(ctx, ctx.copy.actions.followUpdates, "open_issue")
            : undefined,
        secondary: offer.action,
      };
    }
    case "no_font":
      return {
        ...base,
        title: fmt(w.noFont.title, { model: watch.model }),
        illustration: { icon: "watch", tone: "muted" },
        content: (
          <>
            <p className="lead">{w.noFont.body}</p>
            {model.sharedBefore ? <p className="muted">{w.sharedBefore}</p> : null}
          </>
        ),
        primary: model.sharedBefore ? undefined : shareFontList(ctx),
        secondary: emitting(ctx, ctx.copy.actions.whichWatches, "which_watches"),
      };
    case "unreadable":
      return {
        ...base,
        title: w.unreadable.title,
        illustration: { icon: "watch", tone: "muted" },
        content: <p className="lead">{w.unreadable.body}</p>,
        primary: emitting(ctx, ctx.copy.actions.tryAgain, "retry_now"),
        secondary: reportProblem(ctx),
      };
  }
}
