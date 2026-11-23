import { fmt } from "../copy";
import type { ScreenModel } from "../model/screens";
import { ProgressBar, Waiting } from "../ui/bits";
import { Notice } from "../ui/Notice";
import { checkWhy, confidenceLine, emitting } from "./common";
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
    title: c.title,
    watch: model.watch,
    content: <p className="lead">{c.body}</p>,
    primary: emitting(ctx, ctx.copy.actions.restore, "confirm_restore"),
    secondary: emitting(ctx, ctx.copy.actions.cancel, "cancel"),
  };
}

/** working: writing to the watch. No actions: it must not be interrupted. */
export function WorkingScreen(model: Working, ctx: ScreenContext): ScreenView {
  const w = ctx.copy.working;
  const progress = model.progress
    ? fmt(w.progress, { done: model.progress.done, total: model.progress.total })
    : undefined;
  return {
    step: "install",
    title: w[model.op],
    watch: model.watch,
    status: progress,
    content: (
      <>
        <ProgressBar label={w[model.op]} value={model.progress} text={progress} />
        <Notice tone="warning">
          <p>
            <strong>{w.keepConnected}</strong>
          </p>
        </Notice>
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
  switch (model.phase) {
    case "unplug":
      return {
        ...base,
        title: r.unplug.title,
        illustration: { icon: "cable" },
        status: r.unplug.waiting,
        content: (
          <>
            <p className="lead">{r.unplug[model.op]}</p>
            <Waiting>{r.unplug.waiting}</Waiting>
          </>
        ),
      };
    case "restart":
      return {
        ...base,
        title: r.restart.title,
        illustration: { icon: "restart" },
        content: (
          <>
            <p className="lead">{r.restart.how}</p>
            <p>
              {model.verifiable
                ? r.restart.reconnect
                : model.op === "install"
                  ? r.restart.notVerifiable
                  : r.restart.notVerifiableRestore}
            </p>
          </>
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
    case "verifying":
      return {
        ...base,
        title: r.verifying.title,
        illustration: { icon: "watch" },
        status: r.verifying.title,
        content: <Waiting>{r.verifying.body}</Waiting>,
      };
    case "again":
      return {
        ...base,
        title: r.again.title,
        illustration: { icon: "restart" },
        content: (
          <>
            <p className="lead">{r.again.body}</p>
            <p>{r.again.how}</p>
          </>
        ),
      };
    case "other_watch":
      return {
        ...base,
        title: r.otherWatch.title,
        illustration: { icon: "watch", tone: "muted" },
        content: <p className="lead">{fmt(r.otherWatch.body, { model: model.watch.model })}</p>,
      };
  }
}

/** result: the app saw the change on the watch after the restart. */
export function ResultScreen(model: Result, ctx: ScreenContext): ScreenView {
  const r = ctx.copy.result;
  if (model.op === "restore") {
    return {
      step: "restart",
      title: r.restore.title,
      illustration: { icon: "check", tone: "success" },
      watch: model.watch,
      content: <p className="lead">{r.restore.body}</p>,
    };
  }
  const why = checkWhy(ctx.copy, model.watch, model.confidence);
  return {
    step: "restart",
    title: r.install.title,
    illustration: { icon: "check", tone: "success" },
    watch: model.watch,
    content: (
      <>
        <p className="lead">{confidenceLine(ctx.copy, ctx.lang, model.watch, model.confidence)}</p>
        <p>{why ?? r.install.confirmed}</p>
      </>
    ),
    primary: why
      ? emitting(ctx, ctx.copy.actions.check, "check")
      : emitting(ctx, ctx.copy.actions.sendTest, "check"),
  };
}
