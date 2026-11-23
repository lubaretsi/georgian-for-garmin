import type { Platform } from "../core/backend";
import { type Copy, fmt } from "../copy";
import { LINUX_UDEV_COMMANDS } from "../copy/shared";
import type { BusyView, ScreenModel } from "../model/screens";
import { Waiting } from "../ui/bits";
import { CopyBlock } from "../ui/CopyBlock";
import { Disclosure } from "../ui/Disclosure";
import { Icon } from "../ui/icons";
import { Notice } from "../ui/Notice";
import { STEPS } from "../ui/StepIndicator";
import { copyLabels, emitting, reportProblem } from "./common";
import type { ScreenContext, ScreenView } from "./types";

type Searching = Extract<ScreenModel, { screen: "connect"; mode: "searching" }>;
type Busy = Extract<ScreenModel, { screen: "connect"; mode: "busy" }>;
type Problem = Extract<ScreenModel, { screen: "connect"; mode: "problem" }>;

const STEP_ICONS = { connect: "cable", install: "watch", restart: "restart", check: "check" } as const;

/** connect / searching: what the app does, the four steps, and help after a while. */
export function ConnectSearching(model: Searching, ctx: ScreenContext): ScreenView {
  const c = ctx.copy.connect.searching;
  const help = ctx.copy.connect.help;
  return {
    step: "connect",
    title: c.title,
    illustration: { icon: "cable" },
    status: c.looking,
    content: (
      <>
        <p className="lead">{c.lead}</p>
        <Waiting>{c.looking}</Waiting>
        {model.showHelp ? (
          <Notice tone="info" title={help.title}>
            <ul className="tight-list">
              <li>{help.express[ctx.platform]}</li>
              <li>{help.replug}</li>
              <li>{help.cable}</li>
            </ul>
          </Notice>
        ) : null}
        <h2 className="section-title">{c.previewTitle}</h2>
        <ol className="preview">
          {STEPS.map((step) => (
            <li key={step} className="preview-step">
              <span className="preview-icon">
                <Icon name={STEP_ICONS[step]} />
              </span>
              <span>
                <strong>{ctx.copy.steps[step]}</strong>
                <span className="preview-text">{c.preview[step]}</span>
              </span>
            </li>
          ))}
        </ol>
      </>
    ),
    secondary: emitting(ctx, ctx.copy.header.more, "show_more"),
  };
}

/** macOS gives process names cut to 16 bytes (core `busy.rs`). */
const MACOS_NAME_BYTES = 16;

/** The program holding the watch, named once (never "Garmin Express (Garmin Express)"). */
export function programName(copy: Copy, platform: Platform, busy: BusyView): { program: string; cut: boolean } {
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
  }
}

/** connect / busy: who holds the watch, and this platform's steps to free it. */
export function ConnectBusy(model: Busy, ctx: ScreenContext): ScreenView {
  const b = ctx.copy.connect.busy;
  const { program, cut } = programName(ctx.copy, ctx.platform, model.busy);
  const steps = b.steps[model.busy.kind][ctx.platform];
  return {
    step: "connect",
    title: fmt(model.busy.confirmed ? b.title.confirmed : b.title.unconfirmed, { program }),
    illustration: { icon: "watch", tone: "muted" },
    content: (
      <>
        {cut ? <p className="muted">{b.nameCut}</p> : null}
        <p>{b.lead}</p>
        <ol className="numbered-list">
          {steps.map((step) => (
            <li key={step}>{step}</li>
          ))}
        </ol>
        <Waiting>{b.retrying}</Waiting>
      </>
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
  const report = reportProblem(ctx);
  const details =
    model.detail.trim() !== "" ? (
      <Disclosure summary={ctx.copy.actions.details}>
        <pre className="detail-text">{model.detail}</pre>
      </Disclosure>
    ) : null;
  const base = {
    step: "connect" as const,
    title: text.title,
    illustration: { icon: "cable" as const, tone: "muted" as const },
  };
  switch (model.problem) {
    case "permission_denied":
      return {
        ...base,
        content: (
          <>
            <p>{text.body}</p>
            <CopyBlock
              text={LINUX_UDEV_COMMANDS}
              label={ctx.copy.actions.copyCommands}
              labels={copyLabels(ctx.copy)}
              lang="en"
            />
            {details}
          </>
        ),
        primary: { label: ctx.copy.actions.copyCommands, copyText: LINUX_UDEV_COMMANDS },
        secondary: retry,
      };
    case "multiple_devices":
    case "device_error":
      return { ...base, content: <><p>{text.body}</p>{details}</>, primary: retry };
    case "garmin_folder_not_found":
    case "extdata_not_found":
    case "internal":
      return { ...base, content: <><p>{text.body}</p>{details}</>, primary: report, secondary: retry };
  }
}
