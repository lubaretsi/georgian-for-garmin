import { Lock, ShieldCheck, Smartphone } from "lucide-react";
import type { ReactNode } from "react";

import { fmt } from "../copy";
import type { Result, ScreenModel } from "../model/screens";
import { CelebrateScene, MissedScene, ShareScene, WatchScene } from "../ui/art/Scenes";
import { MiniFace } from "../ui/art/Watch";
import { Alert } from "../ui/Alert";
import { Badge } from "../ui/Controls";
import { CopyBlock } from "../ui/CopyBlock";
import { Disclosure } from "../ui/Disclosure";
import { Facts, Group, GroupFootnote, GroupHeader, Row, Tile, TILE_INSET } from "../ui/Group";
import { copyLabels, emitting, WatchFacts } from "./common";
import type { FooterAction, ScreenContext, ScreenView } from "./types";

type Outcome = Extract<ScreenModel, { screen: "outcome" }>;
type Share = Extract<ScreenModel, { screen: "share" }>;

/** Restarts after which the outcome stops suggesting another one (§4.1). */
const RESTARTS_BEFORE_SHARE = 2;

/** "The watch showed: [face] Boxes" for the result list. */
function shownRow(ctx: ScreenContext, result: Result | null): { label: string; value: ReactNode } | undefined {
  if (!result || result === "not_installed") return undefined;
  const label = ctx.copy.check.answer.results[result];
  const sample = result === "no_notification" ? "none" : result;
  return {
    label: ctx.copy.outcome.result.showed,
    value: (
      <span className="inline-flex items-center gap-2">
        <Badge tone={result === "georgian" ? "success" : result === "no_notification" ? "neutral" : "caution"}>{label}</Badge>
        <MiniFace sample={sample} size={28} />
      </span>
    ),
  };
}

/** outcome: what the answer means, and the one next step. */
export function OutcomeScreen(model: Outcome, ctx: ScreenContext): ScreenView {
  const o = ctx.copy.outcome;
  const values = { model: model.watch.model };
  const shareResult: FooterAction = {
    label: ctx.copy.actions.shareResult,
    onPress: () => ctx.emit("share", { action: "share", what: "result" }),
  };
  const done = emitting(ctx, ctx.copy.actions.done, "done");
  const resultList = (footnotes: ReactNode) => (
    <section aria-labelledby="result-title">
      <GroupHeader id="result-title">{o.result.title}</GroupHeader>
      <WatchFacts copy={ctx.copy} watch={model.watch} extra={shownRow(ctx, model.result)} />
      {footnotes}
    </section>
  );
  const thanks = model.sharedBefore ? <GroupFootnote>{o.sharedBefore}</GroupFootnote> : null;
  switch (model.kind) {
    case "works_confirmed":
      return {
        step: "check",
        allDone: true,
        hero: <CelebrateScene />,
        title: o.works_confirmed.title,
        lead: o.works_confirmed.body,
        content: resultList(null),
        primary: done,
      };
    case "works_first":
      return {
        step: "check",
        allDone: true,
        hero: <CelebrateScene />,
        title: o.works_first.title,
        lead: fmt(o.works_first.body, values),
        content: resultList(
          model.sharedBefore ? thanks : <GroupFootnote icon={<ShieldCheck size={14} />}>{o.works_first.account}</GroupFootnote>,
        ),
        primary: model.sharedBefore ? done : shareResult,
        secondary: model.sharedBefore ? undefined : emitting(ctx, ctx.copy.actions.notNow, "done"),
      };
    case "didnt_show": {
      const restart = emitting(ctx, ctx.copy.actions.restartAgain, "restart_again");
      const restartFirst = model.restarts < RESTARTS_BEFORE_SHARE;
      // Already shared: restarting is the only thing left to offer besides More › Restore.
      const share = model.sharedBefore ? undefined : shareResult;
      const face = model.result === "question_marks" || model.result === "some_letters" ? model.result : "boxes";
      return {
        step: "check",
        hero: <WatchScene face={face} />,
        title: o.didnt_show.title,
        lead: restartFirst ? o.didnt_show.tryRestart : o.didnt_show.share,
        content: resultList(
          <>
            {thanks}
            <GroupFootnote>{o.didnt_show.restoreHint}</GroupFootnote>
          </>,
        ),
        primary: restartFirst || !share ? restart : share,
        secondary: restartFirst || !share ? share : restart,
      };
    }
    case "no_notification":
      return {
        step: "check",
        hero: <MissedScene />,
        title: o.no_notification.title,
        lead: o.no_notification.body,
        content: (
          <div>
            <Group className={TILE_INSET}>
              <Row lead={<Tile><Smartphone size={15} /></Tile>} title={ctx.copy.phone.android} />
              <Row lead={<Tile><Smartphone size={15} /></Tile>} title={ctx.copy.phone.iphone} />
            </Group>
            <GroupFootnote>{ctx.copy.phone.either}</GroupFootnote>
          </div>
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
  switch (model.state) {
    case "ready":
      return {
        step,
        hero: <ShareScene state="ready" />,
        title: s.title[model.what],
        lead: why,
        content: (
          <>
            <section aria-labelledby="shared-title">
              <GroupHeader id="shared-title">{s.whatTitle}</GroupHeader>
              <Facts items={model.summary} />
              <GroupFootnote icon={<Lock size={14} />}>{s.private}</GroupFootnote>
            </section>
            <Alert tone="info">
              <p>{s.account}</p>
              <p className="text-fg-2">{s.nothingSent}</p>
            </Alert>
            {exactText}
          </>
        ),
        primary: model.reading ? { ...open, disabled: true } : open,
        ...(model.reading && { status: { text: s.reading, busy: true } }),
      };
    case "opened":
    case "opened_paste":
      return {
        step,
        hero: <ShareScene state="opened" />,
        title: s.opened.title,
        lead: s.opened.body,
        content: (
          <>
            {model.state === "opened_paste" ? (
              <Alert tone="caution">
                <p className="font-medium">{s.opened.pasted}</p>
              </Alert>
            ) : null}
            {thanksText ? <Alert tone="success">{thanksText}</Alert> : null}
            {exactText}
          </>
        ),
        primary: emitting(ctx, ctx.copy.actions.done, "done"),
      };
    case "failed":
      return {
        step,
        hero: <ShareScene state="failed" />,
        title: s.failed.title,
        content: (
          <>
            <Alert tone="danger" live>
              <p>{s.failed.body}</p>
            </Alert>
            {exactText}
          </>
        ),
        primary: { ...open, label: ctx.copy.actions.tryAgain },
        secondary: emitting(ctx, ctx.copy.actions.back, "back"),
      };
  }
}
