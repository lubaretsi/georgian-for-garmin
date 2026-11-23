import {
  BellRing,
  Cable,
  ChevronRight,
  LifeBuoy,
  MonitorX,
  RotateCcw,
  RotateCw,
  ShieldCheck,
  Unplug,
  Watch,
} from "lucide-react";
import { Collapsible } from "radix-ui";
import type { ReactNode } from "react";

import type { Lang, Platform } from "../../../core/backend";
import { type Copy, fmt } from "../../../copy";
import type { ActionId, ActionPayload, Answers, Result, ScreenModel, View } from "../../../model/screens";
import { checkWhy, confidenceLine } from "../../../screens/common";
import type { CopyB } from "../copy";
import { ConnectScene, InstallScene, RestartScene, SuccessScene } from "../illustrations/scenes";
import { MiniFace } from "../illustrations/watch";
import { Badge, CheckboxRow, RadioRows, Segmented } from "../ui/controls";
import { Group, GroupFootnote, GroupHeader, Row, Tile, ValueRow } from "../ui/group";
import { STEPS, type StepId } from "../ui/steps";

export type Emit = (id: ActionId, payload?: ActionPayload) => void;
export type ScreenContext = { copy: Copy; b: CopyB; lang: Lang; platform: Platform; emit: Emit };
export type ViewAction = { label: string; onPress: () => void; disabled?: boolean };

/** What a screen draws; the shell places it. The heading's id is TITLE_ID. */
export type ViewB = {
  step: StepId | null;
  title: string;
  hero?: ReactNode;
  badge?: ReactNode;
  lead?: ReactNode;
  /** Shown in the footer's status area and announced politely. */
  status?: { text: string; busy: boolean };
  content?: ReactNode;
  primary?: ViewAction;
  secondary?: ViewAction;
};

export const TITLE_ID = "b-screen-title";

/** Screens change on these; the heading takes focus on every change. */
export function screenKey(model: ScreenModel): string {
  switch (model.screen) {
    case "connect":
      return `connect:${model.mode}`;
    case "restart":
      return `restart:${model.phase}:${model.attempt}`;
    case "check":
      return `check:${model.step}`;
    case "watch":
      return `watch:${model.verdict.kind}`;
    default:
      return model.screen;
  }
}

const act = (ctx: ScreenContext, label: string, id: ActionId): ViewAction => ({ label, onPress: () => ctx.emit(id) });

export function renderScreen(model: ScreenModel, ctx: ScreenContext): ViewB {
  switch (model.screen) {
    case "connect":
      if (model.mode === "searching") return connectSearching(model, ctx);
      break;
    case "watch":
      return watchScreen(model, ctx);
    case "restart":
      return restartScreen(model, ctx);
    case "check":
      if (model.step === "answer") return checkAnswer(model, ctx);
      break;
    case "outcome":
      return outcomeScreen(model, ctx);
  }
  // The other screens are outside this direction's checkpoint set.
  return { step: null, title: model.screen };
}

/* ---- connect / searching ---- */

function connectSearching(model: Extract<ScreenModel, { screen: "connect"; mode: "searching" }>, ctx: ScreenContext): ViewB {
  const { copy, b, platform } = ctx;
  const help = b.connect.help;
  const icons: Record<StepId, ReactNode> = {
    connect: <Cable size={15} />,
    install: <Watch size={15} />,
    restart: <RotateCw size={15} />,
    check: <BellRing size={15} />,
  };
  return {
    step: "connect",
    hero: <ConnectScene />,
    title: copy.connect.searching.title,
    lead: b.connect.lead,
    status: { text: copy.connect.searching.looking, busy: true },
    content: (
      <>
        {model.showHelp ? (
          <section aria-labelledby="b-help-title">
            <Group className="nb:[--b-inset:55px]">
              <div className="nb:flex nb:items-center nb:gap-3 nb:px-3.5 nb:py-3">
                <Tile tone="accent">
                  <LifeBuoy size={16} />
                </Tile>
                <h2 id="b-help-title" className="nb:text-row nb:font-semibold nb:text-fg">
                  {copy.connect.help.title}
                </h2>
              </div>
              {[
                { icon: <MonitorX size={15} />, ...help.express[platform] },
                { icon: <Unplug size={15} />, ...help.replug },
                { icon: <Cable size={15} />, ...help.cable },
              ].map((item) => (
                <Row
                  key={item.title}
                  lead={<Tile>{item.icon}</Tile>}
                  title={item.title}
                  titleClassName="nb:font-medium"
                  detail={item.body}
                />
              ))}
            </Group>
          </section>
        ) : null}
        <Collapsible.Root defaultOpen={!model.showHelp} className="nb:group/how">
          <h2>
            <Collapsible.Trigger className="nb:mb-1.5 nb:flex nb:items-center nb:gap-1 nb:rounded-[6px] nb:px-3.5 nb:text-label nb:font-semibold nb:text-fg-2 nb:hover:text-fg">
              {copy.connect.searching.previewTitle}
              <ChevronRight
                size={14}
                strokeWidth={2.4}
                aria-hidden="true"
                className="nb:transition-transform nb:duration-200 nb:group-data-[state=open]/how:rotate-90"
              />
            </Collapsible.Trigger>
          </h2>
          <Collapsible.Content>
            <Group as="ol" className="nb:[--b-inset:55px]">
              {STEPS.map((step, i) => (
                <li key={step}>
                  <Row
                    lead={<Tile tone={i === 0 ? "accent" : "graphite"}>{icons[step]}</Tile>}
                    title={copy.steps[step]}
                    titleClassName="nb:font-medium"
                    detail={b.connect.preview[step]}
                  />
                </li>
              ))}
            </Group>
          </Collapsible.Content>
        </Collapsible.Root>
      </>
    ),
  };
}

/* ---- watch ---- */

function watchScreen(model: Extract<ScreenModel, { screen: "watch" }>, ctx: ScreenContext): ViewB {
  const { copy, b, lang } = ctx;
  const { watch, verdict } = model;
  if (verdict.kind !== "install" && verdict.kind !== "installed") return { step: "install", title: model.screen };
  const confirmed = verdict.confidence === "confirmed";
  const status = (
    <Badge tone={confirmed ? "success" : "neutral"}>{b.watch.status[verdict.confidence]}</Badge>
  );
  const facts = (
    <Group as="dl">
      <ValueRow label={b.watch.rows.watch} value={<span className="nb:font-medium nb:text-fg">{watch.model}</span>} />
      <ValueRow label={b.watch.rows.software} value={watch.software} />
      <ValueRow label={b.watch.rows.georgian} value={status} />
    </Group>
  );
  if (verdict.kind === "installed") {
    const why = checkWhy(copy, watch, verdict.confidence);
    return {
      step: "install",
      hero: <InstallScene />,
      title: copy.watch.installed.title,
      lead: why ?? copy.watch.installed.unplug,
      content: facts,
      primary: confirmed ? undefined : act(ctx, copy.actions.check, "check"),
      secondary: confirmed ? act(ctx, copy.actions.didntWorkCheck, "check") : undefined,
    };
  }
  return {
    step: "install",
    hero: <InstallScene />,
    title: confirmed ? copy.watch.install.titleConfirmed : b.watch.title,
    lead: confidenceLine(copy, lang, watch, verdict.confidence),
    content: (
      <div>
        {facts}
        {verdict.note ? <GroupFootnote>{copy.watch.install.note[verdict.note]}</GroupFootnote> : null}
        {confirmed ? null : <GroupFootnote icon={<RotateCcw size={14} />}>{b.watch.safe}</GroupFootnote>}
      </div>
    ),
    primary: act(ctx, copy.actions.install, "install"),
  };
}

/* ---- restart ---- */

function Numbered({ n }: { n: number }) {
  return (
    <span
      aria-hidden="true"
      className="nb:flex nb:size-7 nb:shrink-0 nb:items-center nb:justify-center nb:rounded-full nb:bg-fill nb:text-row nb:font-semibold nb:text-fg nb:tabular-nums"
    >
      {n}
    </span>
  );
}

function restartScreen(model: Extract<ScreenModel, { screen: "restart" }>, ctx: ScreenContext): ViewB {
  const { copy, b } = ctx;
  const r = copy.restart;
  const base = {
    step: "restart" as const,
    badge: model.attempt >= 2 ? <Badge>{fmt(r.attempt, { attempt: model.attempt })}</Badge> : undefined,
  };
  const waiting = { text: b.restart.waiting, busy: true };
  switch (model.phase) {
    case "unplug":
      return { ...base, hero: <InstallScene />, title: r.unplug.title, lead: r.unplug[model.op], status: { text: r.unplug.waiting, busy: true } };
    case "restart":
      return {
        ...base,
        hero: <RestartScene />,
        title: r.restart.title,
        lead: model.op === "install" ? b.restart.lead : r.unplug.restore,
        status: model.verifiable ? waiting : undefined,
        content: (
          <Group as="ol" className="nb:[--b-inset:55px]">
            <li>
              <Row lead={<Numbered n={1} />} title={b.restart.hold.title} titleClassName="nb:font-medium" detail={b.restart.hold.body} />
            </li>
            <li>
              {model.verifiable ? (
                <Row lead={<Numbered n={2} />} title={b.restart.reconnect.title} titleClassName="nb:font-medium" detail={b.restart.reconnect.body} />
              ) : (
                <Row
                  lead={<Numbered n={2} />}
                  title={model.op === "install" ? r.restart.notVerifiable : r.restart.notVerifiableRestore}
                />
              )}
            </li>
          </Group>
        ),
        primary: model.verifiable
          ? undefined
          : act(ctx, model.op === "install" ? copy.actions.check : copy.actions.continue, "skip_verify"),
        secondary: model.verifiable
          ? act(ctx, model.op === "install" ? b.restart.skip : copy.actions.skip, "skip_verify")
          : undefined,
      };
    case "verifying":
      return { ...base, hero: <InstallScene />, title: r.verifying.title, lead: r.verifying.body, status: { text: r.verifying.title, busy: true } };
    case "again":
      return { ...base, hero: <RestartScene />, title: r.again.title, lead: `${r.again.body} ${r.again.how}`, status: waiting };
    case "other_watch":
      return { ...base, hero: <InstallScene />, title: r.otherWatch.title, lead: fmt(r.otherWatch.body, { model: model.watch.model }), status: waiting };
  }
}

/* ---- check / answer ---- */

const OFFERED: Exclude<Result, "not_installed">[] = ["georgian", "boxes", "question_marks", "some_letters", "no_notification"];
const VIEWS: View[] = ["popup", "list", "opened"];

function checkAnswer(model: Extract<ScreenModel, { screen: "check" }>, ctx: ScreenContext): ViewB {
  const { copy, b } = ctx;
  const a = copy.check.answer;
  const answers = model.answers;
  const change = (next: Answers) => ctx.emit("answer", { action: "answer", answers: next });
  const letters = answers.result === "georgian" || answers.result === "some_letters";
  return {
    step: "check",
    title: a.title,
    content: (
      <>
        <RadioRows
          labelledBy={TITLE_ID}
          value={answers.result === "not_installed" ? undefined : answers.result}
          onChange={(result) => change({ ...answers, result })}
          choices={OFFERED.map((value) => ({
            value,
            label: a.results[value],
            trailing: <MiniFace sample={value === "no_notification" ? "none" : value} />,
          }))}
        />
        {letters ? (
          <section aria-labelledby="b-views">
            <GroupHeader id="b-views" aside={b.check.optional}>
              {b.check.viewsLegend}
            </GroupHeader>
            <Group className="nb:[--b-inset:44px]" role="group" aria-labelledby="b-views">
              {VIEWS.map((view) => (
                <CheckboxRow
                  key={view}
                  label={b.check.views[view]}
                  checked={answers.views.includes(view)}
                  onChange={(on) =>
                    change({ ...answers, views: on ? [...answers.views, view] : answers.views.filter((v) => v !== view) })
                  }
                />
              ))}
            </Group>
          </section>
        ) : null}
        <section>
          <GroupHeader id="b-phone" aside={b.check.optional}>
            {b.check.phoneLegend}
          </GroupHeader>
          <Segmented
            labelledBy="b-phone"
            value={answers.phone}
            onChange={(phone) => change({ ...answers, phone })}
            choices={[
              { value: "android", label: a.phone.android },
              { value: "iphone", label: a.phone.iphone },
            ]}
          />
        </section>
        {answers.result === undefined ? <p className="nb:text-center nb:text-row nb:text-fg-2">{a.required}</p> : null}
      </>
    ),
    primary: { label: copy.actions.continue, onPress: () => ctx.emit("done"), disabled: answers.result === undefined },
    secondary: act(ctx, copy.actions.back, "back"),
  };
}

/* ---- outcome ---- */

function outcomeScreen(model: Extract<ScreenModel, { screen: "outcome" }>, ctx: ScreenContext): ViewB {
  const { copy, b } = ctx;
  const o = copy.outcome;
  const values = { model: model.watch.model };
  const share: ViewAction = {
    label: copy.actions.shareResult,
    onPress: () => ctx.emit("share", { action: "share", what: "result" }),
  };
  const done = act(ctx, copy.actions.done, "done");
  const result = (
    <section aria-labelledby="b-result">
      <GroupHeader id="b-result">{b.outcome.resultTitle}</GroupHeader>
      <Group as="dl">
        <ValueRow label={b.watch.rows.watch} value={<span className="nb:font-medium nb:text-fg">{model.watch.model}</span>} />
        <ValueRow label={b.watch.rows.software} value={model.watch.software} />
        <ValueRow label={b.outcome.showed} value={<Badge tone="success">{copy.check.answer.results.georgian}</Badge>} />
      </Group>
      {model.kind === "works_first" && !model.sharedBefore ? (
        <GroupFootnote icon={<ShieldCheck size={14} />}>
          {b.outcome.account}
        </GroupFootnote>
      ) : null}
      {model.sharedBefore ? <GroupFootnote>{o.sharedBefore}</GroupFootnote> : null}
    </section>
  );
  switch (model.kind) {
    case "works_first":
      return {
        step: "check",
        hero: <SuccessScene />,
        title: b.outcome.title,
        lead: fmt(b.outcome.lead, values),
        content: result,
        primary: model.sharedBefore ? done : share,
        secondary: model.sharedBefore ? undefined : act(ctx, copy.actions.notNow, "done"),
      };
    case "works_confirmed":
      return { step: "check", hero: <SuccessScene />, title: o.works_confirmed.title, lead: o.works_confirmed.body, content: result, primary: done };
    case "didnt_show":
      return {
        step: "check",
        hero: <RestartScene />,
        title: o.didnt_show.title,
        lead: model.restarts < 2 ? o.didnt_show.tryRestart : o.didnt_show.share,
        primary: model.restarts < 2 || model.sharedBefore ? act(ctx, copy.actions.restartAgain, "restart_again") : share,
      };
    case "no_notification":
      return {
        step: "check",
        title: o.no_notification.title,
        lead: o.no_notification.body,
        content: (
          <Group>
            <Row title={copy.phone.android} />
            <Row title={copy.phone.iphone} />
          </Group>
        ),
        primary: act(ctx, copy.actions.tryAgain, "check"),
      };
  }
}
