import { Info, RotateCcw, Smartphone } from "lucide-react";

import { fmt } from "../copy";
import type { ScreenModel } from "../model/screens";
import { InstallScene, WatchScene } from "../ui/art/Scenes";
import { Group, GroupFootnote, Row, Tile, TILE_INSET } from "../ui/Group";
import { checkWhy, confidenceLine, confidenceRow, emitting, reportProblem, shareFontList, WatchFacts } from "./common";
import type { FooterAction, ScreenContext, ScreenView } from "./types";

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
  const base = {
    // Back as a drive while the app follows a restart: the journey stays on Restart.
    step: model.duringRestart ? ("restart" as const) : ("connect" as const),
    hero: <WatchScene face="drive" cable="plugged" />,
    title: fmt(d.title, { model: model.watch.model }),
  };
  if (model.driveOnly) {
    return {
      ...base,
      lead: d.driveOnly.body,
      content: (
        <div>
          <WatchFacts copy={ctx.copy} watch={model.watch} />
          {follow ? <GroupFootnote>{d.driveOnly.follow}</GroupFootnote> : null}
          {offer.line ? <GroupFootnote>{offer.line}</GroupFootnote> : null}
        </div>
      ),
      primary: follow ? emitting(ctx, ctx.copy.actions.followUpdates, "open_issue") : undefined,
      secondary: offer.action,
    };
  }
  return {
    ...base,
    lead: d.mtp.body,
    status: { text: d.mtp.waiting, busy: true },
    content: (
      <div>
        <Group className={TILE_INSET}>
          <Row lead={<Tile><Smartphone size={15} /></Tile>} title={d.mtp.how} />
        </Group>
        {model.duringRestart ? <GroupFootnote>{d.duringRestart}</GroupFootnote> : null}
        {offer.line ? <GroupFootnote>{offer.line}</GroupFootnote> : null}
      </div>
    ),
    secondary: offer.action,
  };
}

/** watch: the connected watch and what the app can do for it, one verdict at a time. */
export function WatchScreen(model: Watch, ctx: ScreenContext): ScreenView {
  const w = ctx.copy.watch;
  const { watch, verdict } = model;
  const facts = <WatchFacts copy={ctx.copy} watch={watch} />;
  switch (verdict.kind) {
    case "install": {
      const confirmed = verdict.confidence === "confirmed";
      return {
        step: "install",
        hero: <InstallScene />,
        title: confirmed ? w.install.titleConfirmed : w.install.title,
        lead: confidenceLine(ctx.copy, ctx.lang, watch, verdict.confidence),
        content: (
          <div>
            <WatchFacts copy={ctx.copy} watch={watch} extra={confidenceRow(ctx.copy, verdict.confidence)} />
            {verdict.note ? <GroupFootnote icon={<Info size={14} />}>{w.install.note[verdict.note]}</GroupFootnote> : null}
            {confirmed ? null : <GroupFootnote icon={<RotateCcw size={14} />}>{w.install.unconfirmedFallback}</GroupFootnote>}
          </div>
        ),
        primary: emitting(ctx, ctx.copy.actions.install, "install"),
      };
    }
    case "installed": {
      const why = checkWhy(ctx.copy, watch, verdict.confidence);
      return {
        step: "check",
        hero: <WatchScene face="georgian" cable="plugged" />,
        title: w.installed.title,
        lead: why ?? w.installed.unplug,
        content: <WatchFacts copy={ctx.copy} watch={watch} extra={confidenceRow(ctx.copy, verdict.confidence)} />,
        primary: why ? emitting(ctx, ctx.copy.actions.check, "check") : undefined,
        secondary: why ? undefined : emitting(ctx, ctx.copy.actions.didntWorkCheck, "check"),
      };
    }
    case "restart_pending":
      return {
        step: "restart",
        hero: <WatchScene face="restart" power />,
        title: w.restartPending.title,
        lead: w.restartPending[verdict.variant],
        content: facts,
      };
    case "blocked": {
      const text = w.blocked[verdict.reason];
      const offer =
        verdict.reason === "unknown_font"
          ? fontListOffer(ctx, { reportWanted: true, sharedBefore: model.sharedBefore })
          : { line: null, action: undefined };
      return {
        step: "install",
        hero: verdict.reason === "waiting_other_font" ? <WatchScene face="restart" power /> : <WatchScene face="alert" cable="plugged" />,
        title: text.title,
        lead: text.body,
        content: (
          <div>
            {facts}
            {model.sharedBefore && offer.line ? <GroupFootnote>{offer.line}</GroupFootnote> : null}
          </div>
        ),
        primary: offer.action,
      };
    }
    case "unsupported": {
      const offer = fontListOffer(ctx, model);
      const follow = !verdict.restorable && model.issue !== null;
      return {
        step: "install",
        hero: <WatchScene face="boxes" cable="plugged" />,
        title: fmt(w.unsupported.title, { model: watch.model }),
        lead: verdict.reason?.[ctx.lang],
        content: (
          <div>
            {facts}
            {verdict.restorable ? <GroupFootnote>{w.unsupported.restorable}</GroupFootnote> : null}
            {follow ? <GroupFootnote>{w.unsupported.follow}</GroupFootnote> : null}
            {offer.line ? <GroupFootnote>{offer.line}</GroupFootnote> : null}
          </div>
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
        step: "install",
        hero: <WatchScene face="boxes" cable="plugged" />,
        title: fmt(w.noFont.title, { model: watch.model }),
        lead: w.noFont.body,
        content: (
          <div>
            {facts}
            {model.sharedBefore ? <GroupFootnote>{w.sharedBefore}</GroupFootnote> : null}
          </div>
        ),
        primary: model.sharedBefore ? undefined : shareFontList(ctx),
        secondary: emitting(ctx, ctx.copy.actions.whichWatches, "which_watches"),
      };
    case "unreadable":
      return {
        step: "install",
        hero: <WatchScene face="alert" cable="plugged" />,
        title: w.unreadable.title,
        lead: w.unreadable.body,
        content: facts,
        primary: emitting(ctx, ctx.copy.actions.tryAgain, "retry_now"),
        secondary: reportProblem(ctx),
      };
  }
}
