import { RotateCw } from "lucide-react";

import { fmt } from "../copy";
import type { ScreenModel } from "../model/screens";
import { WatchScene } from "../ui/art/Scenes";
import { Alert } from "../ui/Alert";
import { Group, GroupFootnote, Row, StepNumber, Tile, TILE_INSET } from "../ui/Group";
import { Progress } from "../ui/Progress";
import { checkWhy, confidenceLine, confidenceRow, emitting, WatchFacts } from "./common";
import type { ScreenContext, ScreenView } from "./types";

type ConfirmRestore = Extract<ScreenModel, { screen: "confirm_restore" }>;
type Working = Extract<ScreenModel, { screen: "working" }>;
type Restart = Extract<ScreenModel, { screen: "restart" }>;
type Result = Extract<ScreenModel, { screen: "result" }>;

/** confirm_restore: one question before putting Garmin's font back. */
export function ConfirmRestoreScreen(model: ConfirmRestore, ctx: ScreenContext): ScreenView {
  const c = ctx.copy.confirmRestore;
  return {
    step: "install",
    hero: <WatchScene face="boxes" cable="plugged" />,
    title: c.title,
    lead: c.body,
    content: <WatchFacts copy={ctx.copy} watch={model.watch} />,
    primary: emitting(ctx, ctx.copy.actions.restore, "confirm_restore"),
    secondary: emitting(ctx, ctx.copy.actions.cancel, "cancel"),
  };
}

/** working: writing to the watch. No actions: it must not be interrupted. */
export function WorkingScreen(model: Working, ctx: ScreenContext): ScreenView {
  const w = ctx.copy.working;
  const { progress } = model;
  const text = progress ? fmt(w.progress, { done: progress.done, total: progress.total }) : undefined;
  return {
    step: "install",
    hero: <WatchScene face={{ progress: progress && progress.total > 0 ? progress.done / progress.total : null }} cable="plugged" />,
    title: w[model.op],
    status: { text: text ?? w[model.op], busy: true },
    content: (
      <>
        <Progress label={w[model.op]} value={progress} text={text} />
        <Alert tone="caution">
          <p className="font-medium">{w.keepConnected}</p>
        </Alert>
        <WatchFacts copy={ctx.copy} watch={model.watch} />
      </>
    ),
  };
}

/** restart: the phases the app follows by watching the USB connection (§4.1). */
export function RestartScreen(model: Restart, ctx: ScreenContext): ScreenView {
  const r = ctx.copy.restart;
  const base = {
    step: "restart" as const,
    badge: model.attempt >= 2 ? fmt(r.attempt, { attempt: model.attempt }) : undefined,
  };
  const waiting = { text: r.waiting, busy: true };
  switch (model.phase) {
    case "unplug":
      return {
        ...base,
        hero: <WatchScene face={model.op === "install" ? "georgian" : "boxes"} cable="pulled" />,
        title: r.unplug.title,
        lead: r.unplug[model.op],
        status: { text: r.unplug.waiting, busy: true },
      };
    case "restart": {
      const last = model.verifiable
        ? { title: r.restart.reconnect.title, detail: r.restart.reconnect.body }
        : { title: model.op === "install" ? r.restart.notVerifiable : r.restart.notVerifiableRestore, detail: undefined };
      return {
        ...base,
        hero: <WatchScene face="restart" power />,
        title: r.restart.title,
        lead: r.unplug[model.op],
        status: model.verifiable ? waiting : undefined,
        content: (
          <Group as="ol" className={TILE_INSET}>
            <li>
              <Row lead={<StepNumber n={1} />} title={r.restart.hold.title} detail={r.restart.hold.body} strong />
            </li>
            <li>
              <Row lead={<StepNumber n={2} />} title={last.title} detail={last.detail} strong={last.detail !== undefined} />
            </li>
          </Group>
        ),
        // Skipping (or a watch the app can't verify) goes to the check after an install and
        // back to the watch after a restore (§4.1); the flow decides by `op`.
        primary: model.verifiable
          ? undefined
          : emitting(ctx, model.op === "install" ? ctx.copy.actions.check : ctx.copy.actions.continue, "skip_verify"),
        secondary: model.verifiable
          ? emitting(ctx, model.op === "install" ? ctx.copy.actions.skipVerify : ctx.copy.actions.skip, "skip_verify")
          : undefined,
      };
    }
    case "verifying":
      return {
        ...base,
        hero: <WatchScene face="verifying" cable="plugged" />,
        title: r.verifying.title,
        lead: r.verifying.body,
        status: { text: r.verifying.title, busy: true },
      };
    case "again":
      return {
        ...base,
        hero: <WatchScene face="restart" power />,
        title: r.again.title,
        lead: r.again.body,
        status: waiting,
        content: (
          <Group className={TILE_INSET}>
            <Row lead={<Tile><RotateCw size={15} /></Tile>} title={r.again.how} />
          </Group>
        ),
      };
    case "other_watch":
      return {
        ...base,
        hero: <WatchScene face="alert" cable="plugged" />,
        title: r.otherWatch.title,
        lead: fmt(r.otherWatch.body, { model: model.watch.model }),
        status: waiting,
      };
  }
}

/** result: the app saw the change on the watch after the restart. */
export function ResultScreen(model: Result, ctx: ScreenContext): ScreenView {
  const r = ctx.copy.result;
  if (model.op === "restore") {
    return {
      step: "restart",
      hero: <WatchScene face="boxes" cable="plugged" />,
      title: r.restore.title,
      lead: r.restore.body,
      content: <WatchFacts copy={ctx.copy} watch={model.watch} />,
    };
  }
  const why = checkWhy(ctx.copy, model.watch, model.confidence);
  return {
    step: "restart",
    hero: <WatchScene face="georgian" cable="plugged" done />,
    title: r.install.title,
    lead: confidenceLine(ctx.copy, ctx.lang, model.watch, model.confidence),
    content: (
      <div>
        <WatchFacts copy={ctx.copy} watch={model.watch} extra={confidenceRow(ctx.copy, model.confidence)} />
        <GroupFootnote>{why ?? r.install.confirmed}</GroupFootnote>
      </div>
    ),
    primary: why ? emitting(ctx, ctx.copy.actions.check, "check") : emitting(ctx, ctx.copy.actions.sendTest, "check"),
  };
}
