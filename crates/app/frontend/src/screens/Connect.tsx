import { BellRing, Cable, LifeBuoy, MonitorX, RotateCw, Unplug, Watch } from "lucide-react";

import type { Platform } from "../core/backend";
import { type Copy, fmt } from "../copy";
import { LINUX_UDEV_COMMANDS } from "../copy/shared";
import type { BusyView, ScreenModel } from "../model/screens";
import { ConnectScene, WatchScene } from "../ui/art/Scenes";
import { CopyBlock } from "../ui/CopyBlock";
import { Disclosure } from "../ui/Disclosure";
import { Group, GroupFootnote, Row, StepNumber, Tile, TILE_INSET } from "../ui/Group";
import { STEPS, type StepId } from "../ui/StepIndicator";
import { copyLabels, emitting, reportProblem } from "./common";
import type { ScreenContext, ScreenView } from "./types";

type Searching = Extract<ScreenModel, { screen: "connect"; mode: "searching" }>;
type Switching = Extract<ScreenModel, { screen: "connect"; mode: "switching" }>;
type Busy = Extract<ScreenModel, { screen: "connect"; mode: "busy" }>;
type Problem = Extract<ScreenModel, { screen: "connect"; mode: "problem" }>;

const STEP_ICONS: Record<StepId, typeof Cable> = { connect: Cable, install: Watch, restart: RotateCw, check: BellRing };

/**
 * connect / searching: what the app does, help after a while, and the four steps (collapsed
 * once the help shows). No footer action: More is in the toolbar (§10).
 */
export function ConnectSearching(model: Searching, ctx: ScreenContext): ScreenView {
  const c = ctx.copy.connect.searching;
  const help = ctx.copy.connect.help;
  const tips = [
    { icon: MonitorX, ...help.express[ctx.platform] },
    { icon: Unplug, ...help.replug },
    { icon: Cable, ...help.cable },
  ];
  return {
    step: "connect",
    hero: <ConnectScene />,
    title: c.title,
    lead: c.lead,
    status: { text: c.looking, busy: true },
    content: (
      <>
        {model.showHelp ? (
          <section aria-labelledby="help-title">
            <Group className={TILE_INSET}>
              <div className="flex items-center gap-3 px-3.5 py-3">
                <Tile tone="accent">
                  <LifeBuoy size={16} />
                </Tile>
                <h2 id="help-title" className="text-row font-semibold text-fg">
                  {help.title}
                </h2>
              </div>
              {tips.map(({ icon: Icon, title, body }) => (
                <Row key={title} lead={<Tile><Icon size={15} /></Tile>} title={title} detail={body} strong />
              ))}
            </Group>
          </section>
        ) : null}
        <Disclosure summary={c.previewTitle} heading defaultOpen={!model.showHelp}>
          <Group as="ol" className={TILE_INSET}>
            {STEPS.map((step, i) => {
              const Icon = STEP_ICONS[step];
              return (
                <li key={step}>
                  <Row
                    lead={<Tile tone={i === 0 ? "accent" : "graphite"}><Icon size={15} /></Tile>}
                    title={ctx.copy.steps[step]}
                    detail={c.preview[step]}
                    strong
                  />
                </li>
              );
            })}
          </Group>
        </Disclosure>
      </>
    ),
  };
}

/** connect / switching: the watch is in Garmin mode and the app switches it to MTP (§4.2). */
export function ConnectSwitching(_model: Switching, ctx: ScreenContext): ScreenView {
  const s = ctx.copy.connect.switching;
  return {
    step: "connect",
    hero: <WatchScene face="verifying" cable="plugged" />,
    title: s.title,
    lead: s.lead,
    status: { text: s.title, busy: true },
  };
}

/** macOS gives process names cut to 16 bytes (core `busy.rs`). */
const MACOS_NAME_BYTES = 16;

/**
 * The program holding the watch, named once (never "Garmin Express (Garmin Express)").
 * `platform` null where the screen's platform isn't known (the diagnostics summary): a named
 * holder is shown as the system gives it.
 */
export function programName(copy: Copy, platform: Platform | null, busy: BusyView): { program: string; cut: boolean } {
  const p = copy.connect.busy.program;
  const named = (): { program: string; cut: boolean } => {
    if (!busy.name) return { program: p.unknown, cut: false };
    const cut = platform === "macos" && new TextEncoder().encode(busy.name).length >= MACOS_NAME_BYTES;
    return { program: fmt(p.named, { program: cut ? `${busy.name}…` : busy.name }), cut };
  };
  switch (busy.kind) {
    case "garmin_express":
      return { program: p.garminExpress, cut: false };
    case "macos_camera_service":
      return { program: p.cameraService, cut: false };
    case "desktop_mtp":
      return platform === "linux" ? { program: p.fileManager, cut: false } : named();
    case "other":
      return named();
    case "unknown":
      return { program: p.unknown, cut: false };
    case "this_app":
      return { program: p.thisApp, cut: false };
  }
}

/** connect / busy: who holds the watch, and this platform's steps to free it. */
export function ConnectBusy(model: Busy, ctx: ScreenContext): ScreenView {
  const b = ctx.copy.connect.busy;
  const { program, cut } = programName(ctx.copy, ctx.platform, model.busy);
  const steps = b.steps[model.busy.kind][ctx.platform];
  // This very app still on an earlier step: its own words, not "free it from another program".
  const thisApp = model.busy.kind === "this_app";
  return {
    // Busy while the app follows a restart: the journey stays on Restart.
    step: model.duringRestart ? "restart" : "connect",
    hero: <WatchScene face="busy" cable="plugged" />,
    title: thisApp
      ? b.thisApp.title
      : fmt(model.busy.confirmed ? b.title.confirmed : b.title.unconfirmed, { program }),
    lead: thisApp ? b.thisApp.lead : b.lead,
    status: { text: b.retrying, busy: true },
    content: (
      <div>
        <Group as="ol" className={steps.length > 1 ? TILE_INSET : undefined}>
          {steps.map((step, i) => (
            <li key={step}>
              <Row lead={steps.length > 1 ? <StepNumber n={i + 1} /> : undefined} title={step} />
            </li>
          ))}
        </Group>
        {cut ? <GroupFootnote>{b.nameCut}</GroupFootnote> : null}
        {model.duringRestart ? <GroupFootnote>{b.duringRestart}</GroupFootnote> : null}
      </div>
    ),
    primary: emitting(ctx, ctx.copy.actions.tryAgainNow, "retry_now"),
    secondary: {
      label: ctx.copy.actions.stillStuck,
      onPress: () => ctx.emit("share", { action: "share", what: "problem" }),
    },
  };
}

/**
 * connect / problem: a per-problem title, body and action (§6). A problem the user can fix
 * shows the fix and no Report; one without a user fix offers Report this problem.
 */
export function ConnectProblem(model: Problem, ctx: ScreenContext): ScreenView {
  const text = ctx.copy.connect.problem[model.problem];
  const retry = emitting(ctx, ctx.copy.actions.tryAgainNow, "retry_now");
  const details =
    model.detail.trim() !== "" ? (
      <Disclosure summary={ctx.copy.actions.details}>
        <CopyBlock text={model.detail} label={ctx.copy.actions.details} variant="report" lang="en" />
      </Disclosure>
    ) : null;
  const base = {
    step: "connect" as const,
    hero: <WatchScene face="alert" cable="plugged" />,
    title: text.title,
    lead: text.body,
  };
  switch (model.problem) {
    case "permission_denied":
      return {
        ...base,
        content: (
          <>
            <CopyBlock text={LINUX_UDEV_COMMANDS} label={ctx.copy.actions.copyCommands} labels={copyLabels(ctx.copy)} lang="en" />
            {details}
          </>
        ),
        primary: { label: ctx.copy.actions.copyCommands, copyText: LINUX_UDEV_COMMANDS },
        secondary: retry,
      };
    case "multiple_devices":
    case "device_error":
      return { ...base, content: details, primary: retry };
    case "garmin_folder_not_found":
    case "extdata_not_found":
    case "internal":
      return { ...base, content: details, primary: reportProblem(ctx), secondary: retry };
    // In Garmin mode (§4.2): what to do on the watch; the app keeps reading and goes on by itself.
    case "garmin_mode":
    case "device_locked":
      return {
        ...base,
        hero: <WatchScene face={model.problem === "device_locked" ? "busy" : "alert"} cable="plugged" />,
        status: { text: ctx.copy.restart.waiting, busy: true },
        content: details,
      };
  }
}
