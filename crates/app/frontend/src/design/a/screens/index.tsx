import {
  ArrowLeft,
  BadgeCheck,
  BellOff,
  Cable,
  Footprints,
  LifeBuoy,
  RotateCw,
  Share,
  ShieldCheck,
  Smartphone,
  Undo2,
  UserRound,
  UsersRound,
  Watch,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

import type { Lang, Platform } from "@/core/backend";
import { type Copy, fmt } from "@/copy";
import { RESULT_SAMPLES } from "@/copy/shared";
import type { ActionId, ActionPayload, Answers, Result, ScreenModel, View as WatchView } from "@/model/screens";
import { confidenceLine, softwareLine } from "@/screens/common";

import { CheckScene, ConnectScene, InstallScene, RestartScene, SuccessScene } from "../art/scenes";
import type { DesignCopy } from "../copy";
import { Alert } from "../ui/alert";
import { Badge } from "../ui/badge";
import { CheckboxList, RadioGroup, RadioTile, Segmented } from "../ui/choice";
import { CollapsibleCard } from "../ui/disclosure";
import { JOURNEY, type StepId } from "../ui/journey";
import { Card, Fact, Steps, WatchCard } from "../ui/parts";

export type Emit = (id: ActionId, payload?: ActionPayload) => void;
export type Ctx = { copy: Copy; extra: DesignCopy; lang: Lang; platform: Platform; emit: Emit };
export type Action = { label: string; onPress: () => void; disabled?: boolean; icon?: LucideIcon };

/** What a screen draws; the shell places it (journey, hero, heading, content, footer). */
export type View = {
  step: StepId | null;
  hero?: { scene: ReactNode; size?: "full" | "compact"; tone?: "warm" | "success" };
  /** Every stop of the journey is done (the end of the check). */
  journeyComplete?: boolean;
  eyebrow?: ReactNode;
  title: string;
  lead?: ReactNode;
  content?: ReactNode;
  primary?: Action;
  secondary?: Action;
  /** The secondary sits beside the main action (Back), not under it. */
  secondaryInline?: boolean;
  /** Shown in the main action's place while the app waits on the user or the watch. */
  waiting?: string;
  /** Announced in the polite live region. */
  status?: string;
};

const act = (ctx: Ctx, label: string, id: ActionId, icon?: LucideIcon): Action => ({ label, onPress: () => ctx.emit(id), icon });

/**
 * Direction A draws the five checkpoint screens (and their close siblings). Any other screen
 * is the existing UI's until the chosen direction is rolled out.
 */
export function renderScreen(model: ScreenModel, ctx: Ctx): View {
  switch (model.screen) {
    case "connect":
      if (model.mode === "searching") return connectSearching(model, ctx);
      break;
    case "watch":
      if (model.verdict.kind === "install") return watchInstall(model, model.verdict, ctx);
      break;
    case "restart":
      return restart(model, ctx);
    case "check":
      if (model.step === "answer") return checkAnswer(model, ctx);
      break;
    case "outcome":
      return outcome(model, ctx);
  }
  throw new Error(`Direction A doesn't draw ${model.screen} yet`);
}

/* ---- connect / searching ---------------------------------------------------------- */

const STEP_ICONS: Record<StepId, LucideIcon> = { connect: Cable, install: Watch, restart: RotateCw, check: Smartphone };

function connectSearching(model: Extract<ScreenModel, { screen: "connect"; mode: "searching" }>, ctx: Ctx): View {
  const c = ctx.copy.connect.searching;
  const help = ctx.copy.connect.help;
  return {
    step: "connect",
    hero: { scene: <ConnectScene /> },
    title: c.title,
    lead: ctx.extra.connect.lead,
    waiting: c.looking,
    status: c.looking,
    content: (
      <>
        {model.showHelp ? (
          <Alert tone="sand" icon={<LifeBuoy />} title={help.title}>
            <ul className="wa:grid wa:gap-2 wa:pt-1">
              {[help.express[ctx.platform], help.replug, help.cable].map((line) => (
                <li key={line} className="wa:relative wa:pl-4 wa:before:absolute wa:before:top-[0.62em] wa:before:left-0 wa:before:size-1.5 wa:before:rounded-full wa:before:bg-sand-ink/60">
                  {line}
                </li>
              ))}
            </ul>
          </Alert>
        ) : null}
        <CollapsibleCard title={ctx.extra.connect.previewTitle} icon={<Footprints />} defaultOpen={!model.showHelp}>
          <ol className="wa:grid wa:gap-3 wa:pt-1">
            {JOURNEY.map((step, i) => {
              const Icon = STEP_ICONS[step];
              return (
                <li key={step} className="wa:flex wa:items-start wa:gap-3">
                  <span aria-hidden className="wa:grid wa:size-8 wa:shrink-0 wa:place-items-center wa:rounded-[10px] wa:bg-sunken wa:text-ink-2 wa:[&_svg]:size-4">
                    <Icon />
                  </span>
                  <span className="wa:min-w-0 wa:text-[15px] wa:leading-normal">
                    <span className="wa:block wa:font-semibold">
                      {i + 1}. {ctx.copy.steps[step]}
                    </span>
                    <span className="wa:block wa:text-ink-2">{c.preview[step]}</span>
                  </span>
                </li>
              );
            })}
          </ol>
        </CollapsibleCard>
      </>
    ),
  };
}

/* ---- watch / install -------------------------------------------------------------- */

function watchInstall(
  model: Extract<ScreenModel, { screen: "watch" }>,
  verdict: Extract<Extract<ScreenModel, { screen: "watch" }>["verdict"], { kind: "install" }>,
  ctx: Ctx,
): View {
  const w = ctx.copy.watch;
  const confirmed = verdict.confidence === "confirmed";
  return {
    step: "install",
    hero: { scene: <InstallScene /> },
    title: confirmed ? w.install.titleConfirmed : ctx.extra.watch.installTitle,
    content: (
      <>
        <WatchCard model={model.watch.model} software={softwareLine(ctx.copy, model.watch)}>
          <Fact icon={confirmed ? <BadgeCheck /> : <UsersRound />} tone={confirmed ? "success" : "accent"}>
            {confidenceLine(ctx.copy, ctx.lang, model.watch, verdict.confidence)}
          </Fact>
          {verdict.confidence === "unconfirmed" ? (
            <Fact icon={<Undo2 />} tone="neutral">
              {w.install.unconfirmedFallback}
            </Fact>
          ) : null}
        </WatchCard>
        {verdict.note ? (
          <Alert tone="sand" icon={<LifeBuoy />}>
            {w.install.note[verdict.note]}
          </Alert>
        ) : null}
      </>
    ),
    primary: act(ctx, ctx.copy.actions.install, "install"),
  };
}

/* ---- restart ---------------------------------------------------------------------- */

function restart(model: Extract<ScreenModel, { screen: "restart" }>, ctx: Ctx): View {
  const r = ctx.copy.restart;
  const steps = ctx.extra.restart.steps;
  const base = {
    step: "restart" as const,
    hero: { scene: <RestartScene /> },
    eyebrow:
      model.attempt >= 2 ? (
        <Badge tone="accent" size="lg">
          <RotateCw aria-hidden />
          {fmt(r.attempt, { attempt: model.attempt })}
        </Badge>
      ) : undefined,
  };
  switch (model.phase) {
    case "restart": {
      const skip = model.op === "install" ? ctx.copy.actions.skipVerify : ctx.copy.actions.skip;
      return {
        ...base,
        title: r.restart.title,
        lead: r.unplug[model.op],
        content: (
          <Card>
            <Steps items={model.verifiable ? steps : steps.slice(0, 2)} />
            {model.verifiable ? null : (
              <p className="wa:mt-4 wa:border-t wa:border-line wa:pt-3 wa:text-ink-2">
                {model.op === "install" ? r.restart.notVerifiable : r.restart.notVerifiableRestore}
              </p>
            )}
          </Card>
        ),
        waiting: model.verifiable ? ctx.extra.restart.waiting : undefined,
        status: model.verifiable ? ctx.extra.restart.waiting : undefined,
        primary: model.verifiable
          ? undefined
          : act(ctx, model.op === "install" ? ctx.copy.actions.check : ctx.copy.actions.continue, "skip_verify"),
        secondary: model.verifiable ? act(ctx, skip, "skip_verify") : undefined,
      };
    }
    case "unplug":
      return { ...base, title: r.unplug.title, lead: r.unplug[model.op], waiting: r.unplug.waiting, status: r.unplug.waiting };
    case "verifying":
      return { ...base, title: r.verifying.title, lead: r.verifying.body, waiting: r.verifying.title, status: r.verifying.title };
    case "again":
      return {
        ...base,
        title: r.again.title,
        lead: r.again.body,
        content: (
          <Card>
            <Steps items={steps} />
          </Card>
        ),
        waiting: ctx.extra.restart.waiting,
        status: ctx.extra.restart.waiting,
      };
    case "other_watch":
      return { ...base, title: r.otherWatch.title, lead: fmt(r.otherWatch.body, { model: model.watch.model }) };
  }
}

/* ---- check / answer --------------------------------------------------------------- */

const OFFERED: Exclude<Result, "not_installed">[] = ["georgian", "boxes", "question_marks", "some_letters", "no_notification"];
const VIEWS: WatchView[] = ["popup", "list", "opened"];

/**
 * The answer's sample on a tiny watch. Letters are real text in the app's font; each "□" is
 * drawn as the tall empty box a watch shows for a letter its font lacks.
 */
function MiniWatch(props: { result: Exclude<Result, "not_installed"> }) {
  const r = props.result;
  return (
    <span aria-hidden className="a-mini-watch">
      <span className="a-mini-face">
        {r === "no_notification" ? (
          <BellOff className="wa:size-[18px] wa:opacity-70" />
        ) : (
          <span lang="ka" className="wa:flex wa:items-center wa:text-[14px] wa:leading-none wa:font-semibold">
            {[...RESULT_SAMPLES[r]].map((ch, i) =>
              ch === "□" ? (
                <span key={i} className="wa:mx-px wa:inline-block wa:h-[11px] wa:w-[7px] wa:rounded-[1.5px] wa:border-[1.5px] wa:border-current wa:opacity-85" />
              ) : (
                <span key={i}>{ch}</span>
              ),
            )}
          </span>
        )}
      </span>
    </span>
  );
}

function checkAnswer(model: Extract<ScreenModel, { screen: "check" }>, ctx: Ctx): View {
  const a = ctx.copy.check.answer;
  const x = ctx.extra.check;
  const answers = model.answers;
  const change = (next: Answers) => ctx.emit("answer", { action: "answer", answers: next });
  const letters = answers.result === "georgian" || answers.result === "some_letters";
  return {
    step: "check",
    title: a.title,
    content: (
      <>
        <RadioGroup
          aria-label={a.title}
          value={answers.result === "not_installed" ? "" : (answers.result ?? "")}
          onValueChange={(v) => change({ ...answers, result: v as Result })}
          className="wa:gap-2"
        >
          {OFFERED.map((value) => (
            <RadioTile
              key={value}
              value={value}
              layout="row"
              media={<MiniWatch result={value} />}
              label={a.results[value]}
            />
          ))}
        </RadioGroup>
        {letters ? (
          <CheckboxList
            legend={x.viewsLegend}
            optional={x.optional}
            options={VIEWS.map((value) => ({ value, label: a.views[value] }))}
            value={answers.views}
            onChange={(views) => change({ ...answers, views })}
          />
        ) : null}
        <Segmented
          legend={x.phoneLegend}
          optional={x.optional}
          options={[
            { value: "android", label: a.phone.android },
            { value: "iphone", label: a.phone.iphone },
          ]}
          value={answers.phone}
          onChange={(phone) => change({ ...answers, phone })}
        />
        {answers.result === undefined ? <p className="wa:text-ink-2">{a.required}</p> : null}
      </>
    ),
    primary: { label: ctx.copy.actions.continue, onPress: () => ctx.emit("done"), disabled: answers.result === undefined },
    secondary: act(ctx, ctx.copy.actions.back, "back", ArrowLeft),
    secondaryInline: true,
  };
}

/* ---- outcome ---------------------------------------------------------------------- */

function outcome(model: Extract<ScreenModel, { screen: "outcome" }>, ctx: Ctx): View {
  const o = ctx.copy.outcome;
  const values = { model: model.watch.model };
  const share: Action = {
    label: ctx.copy.actions.shareResult,
    onPress: () => ctx.emit("share", { action: "share", what: "result" }),
    icon: Share,
  };
  const done = act(ctx, ctx.copy.actions.done, "done");
  const modelBadge = (
    <Badge tone="success" size="lg">
      <BadgeCheck aria-hidden />
      {model.watch.model}
    </Badge>
  );
  switch (model.kind) {
    case "works_first":
      return {
        step: "check",
        hero: { scene: <SuccessScene />, tone: "success" },
        journeyComplete: true,
        eyebrow: modelBadge,
        title: fmt(o.works_first.title, values),
        lead: model.sharedBefore ? o.sharedBefore : fmt(ctx.extra.outcome.worksFirstBody, values),
        content: model.sharedBefore ? undefined : (
          <ul className="wa:grid wa:gap-3">
            <Fact icon={<UserRound />} tone="neutral">
              {ctx.copy.share.account}
            </Fact>
            <Fact icon={<ShieldCheck />} tone="neutral">
              {ctx.copy.share.nothingSent}
            </Fact>
          </ul>
        ),
        primary: model.sharedBefore ? done : share,
        secondary: model.sharedBefore ? undefined : act(ctx, ctx.copy.actions.notNow, "done"),
      };
    case "works_confirmed":
      return {
        step: "check",
        hero: { scene: <SuccessScene />, tone: "success" },
        journeyComplete: true,
        eyebrow: modelBadge,
        title: o.works_confirmed.title,
        lead: o.works_confirmed.body,
        primary: done,
      };
    case "didnt_show": {
      const restartFirst = model.restarts < 2;
      const again = act(ctx, ctx.copy.actions.restartAgain, "restart_again", RotateCw);
      const shareOrNone = model.sharedBefore ? undefined : share;
      return {
        step: "check",
        hero: { scene: <RestartScene /> },
        title: o.didnt_show.title,
        lead: restartFirst ? o.didnt_show.tryRestart : o.didnt_show.share,
        content: <p className="wa:text-ink-2">{o.didnt_show.restoreHint}</p>,
        primary: restartFirst || !shareOrNone ? again : shareOrNone,
        secondary: restartFirst || !shareOrNone ? shareOrNone : again,
      };
    }
    case "no_notification":
      return {
        step: "check",
        hero: { scene: <CheckScene /> },
        title: o.no_notification.title,
        lead: o.no_notification.body,
        content: (
          <Card>
            <ul className="wa:grid wa:gap-3">
              <Fact icon={<Smartphone />}>{ctx.copy.phone.android}</Fact>
              <Fact icon={<Smartphone />}>{ctx.copy.phone.iphone}</Fact>
            </ul>
          </Card>
        ),
        primary: act(ctx, ctx.copy.actions.tryAgain, "check"),
      };
  }
}
