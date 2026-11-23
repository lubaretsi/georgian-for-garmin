import { fmt } from "../copy";
import type { ScreenModel } from "../model/screens";
import type { FooterAction } from "../ui/ActionBar";
import { Facts } from "../ui/bits";
import { CopyBlock } from "../ui/CopyBlock";
import { Disclosure } from "../ui/Disclosure";
import { Notice } from "../ui/Notice";
import { copyLabels, emitting } from "./common";
import type { ScreenContext, ScreenView } from "./types";

type Outcome = Extract<ScreenModel, { screen: "outcome" }>;
type Share = Extract<ScreenModel, { screen: "share" }>;

/** Restarts after which the outcome stops suggesting another one (§4.1). */
const RESTARTS_BEFORE_SHARE = 2;

/** outcome: what the answer means, and the one next step. */
export function OutcomeScreen(model: Outcome, ctx: ScreenContext): ScreenView {
  const o = ctx.copy.outcome;
  const values = { model: model.watch.model };
  const shareResult: FooterAction = {
    label: ctx.copy.actions.shareResult,
    onPress: () => ctx.emit("share", { action: "share", what: "result" }),
  };
  const done = emitting(ctx, ctx.copy.actions.done, "done");
  const thanks = model.sharedBefore ? <p className="muted">{o.sharedBefore}</p> : null;
  switch (model.kind) {
    case "works_confirmed":
      return {
        step: "check",
        title: o.works_confirmed.title,
        illustration: { icon: "check", tone: "success" },
        content: <p className="lead">{o.works_confirmed.body}</p>,
        primary: done,
      };
    case "works_first":
      return {
        step: "check",
        title: fmt(o.works_first.title, values),
        illustration: { icon: "check", tone: "success" },
        content: model.sharedBefore ? thanks : <p className="lead">{fmt(o.works_first.body, values)}</p>,
        primary: model.sharedBefore ? done : shareResult,
        secondary: model.sharedBefore ? undefined : emitting(ctx, ctx.copy.actions.notNow, "done"),
      };
    case "didnt_show": {
      const restart = emitting(ctx, ctx.copy.actions.restartAgain, "restart_again");
      const restartFirst = model.restarts < RESTARTS_BEFORE_SHARE;
      // Already shared: restarting is the only thing left to offer besides More › Restore.
      const share = model.sharedBefore ? undefined : shareResult;
      return {
        step: "check",
        title: o.didnt_show.title,
        illustration: { icon: "restart", tone: "muted" },
        content: (
          <>
            <p className="lead">{restartFirst ? o.didnt_show.tryRestart : o.didnt_show.share}</p>
            {thanks}
            <p className="muted">{o.didnt_show.restoreHint}</p>
          </>
        ),
        primary: restartFirst || !share ? restart : share,
        secondary: restartFirst || !share ? share : restart,
      };
    }
    case "no_notification":
      return {
        step: "check",
        title: o.no_notification.title,
        illustration: { icon: "phone", tone: "muted" },
        content: (
          <>
            <p className="lead">{o.no_notification.body}</p>
            <ul className="tight-list">
              <li>{ctx.copy.phone.android}</li>
              <li>{ctx.copy.phone.iphone}</li>
            </ul>
            <p>{ctx.copy.phone.either}</p>
          </>
        ),
        primary: emitting(ctx, ctx.copy.actions.tryAgain, "check"),
      };
  }
}

/** share: what goes to GitHub, then what to do there. Nothing is sent without the click. */
export function ShareScreen(model: Share, ctx: ScreenContext): ScreenView {
  const s = ctx.copy.share;
  // No watch: shared from connect (busy, problem). Only the texts that name no model are used
  // then: the problem's why and the "we'll look" thanks; the summary has no watch rows.
  const watch = model.watch;
  const why = watch ? fmt(s.why[model.what], { model: watch.model }) : s.why.problem;
  const thanksText =
    model.thanks === "tested_next" && watch
      ? fmt(s.thanks.tested_next, { model: watch.model })
      : model.thanks
        ? s.thanks.we_look
        : null;
  const step = watch ? "check" : "connect";
  const open: FooterAction = {
    label: ctx.copy.actions.openGithub,
    onPress: () => ctx.emit("share", { action: "share", what: model.what }),
  };
  const exactText = (
    <Disclosure summary={s.exactText}>
      <CopyBlock text={model.reportText} label={s.exactText} labels={copyLabels(ctx.copy)} variant="report" lang="en" />
    </Disclosure>
  );
  const thanks = thanksText ? <p className="lead">{thanksText}</p> : null;
  switch (model.state) {
    case "ready":
      return {
        step,
        title: s.title[model.what],
        illustration: { icon: "share" },
        watch: watch ?? undefined,
        content: (
          <>
            <p className="lead">{why}</p>
            <Notice tone="info">
              <p>{s.account}</p>
              <p>{s.nothingSent}</p>
            </Notice>
            <h2 className="section-title">{s.whatTitle}</h2>
            <Facts items={model.summary} />
            <p className="muted">{s.private}</p>
            {exactText}
          </>
        ),
        primary: open,
      };
    case "opened":
    case "opened_paste":
      return {
        step,
        title: s.opened.title,
        illustration: { icon: "check", tone: "success" },
        status: s.opened.title,
        content: (
          <>
            {model.state === "opened_paste" ? (
              <Notice tone="warning">
                <p>
                  <strong>{s.opened.pasted}</strong>
                </p>
              </Notice>
            ) : null}
            <p className="lead">{s.opened.body}</p>
            {thanks}
            {exactText}
          </>
        ),
        primary: emitting(ctx, ctx.copy.actions.done, "done"),
      };
    case "failed":
      return {
        step,
        title: s.failed.title,
        illustration: { icon: "share", tone: "muted" },
        content: (
          <>
            <Notice tone="error" live>
              <p>{s.failed.body}</p>
            </Notice>
            {exactText}
          </>
        ),
        primary: { ...open, label: ctx.copy.actions.tryAgain },
        secondary: emitting(ctx, ctx.copy.actions.back, "back"),
      };
  }
}
