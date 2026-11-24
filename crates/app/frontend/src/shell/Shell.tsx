import { ChevronLeft, Ellipsis, Globe } from "lucide-react";
import { MotionConfig, motion } from "motion/react";
import { type ReactNode, useEffect, useRef, useState } from "react";

import type { Lang, Platform } from "../core/backend";
import { copyFor } from "../copy";
import { LANGUAGE_NAMES } from "../copy/shared";
import type { ActionId, ActionPayload, MoreModel, ScreenModel } from "../model/screens";
import { type Emit, type FooterAction, renderScreen, screenKey, TITLE_ID } from "../screens";
import { copyLabels } from "../screens/common";
import { Button } from "../ui/Button";
import { useCopy } from "../ui/clipboard";
import { cn } from "../ui/cn";
import { Badge, Spinner } from "../ui/Controls";
import type { CopyLabels } from "../ui/CopyBlock";
import { StepIndicator } from "../ui/StepIndicator";
import { MAC_TOOLBAR_INSET } from "../ui/WindowFrame";
import { MoreSheet } from "./MoreSheet";

export type ShellProps = {
  model: ScreenModel;
  more: MoreModel;
  lang: Lang;
  platform: Platform;
  onAction: (id: ActionId, payload?: ActionPayload) => void;
  onLanguage: (lang: Lang) => void;
};

/** Focus on one of these means Enter belongs to that control, not to the main action. */
const CONTROLS =
  'button, a[href], input, select, textarea, summary, [role="button"], [role="radio"], [role="checkbox"], [contenteditable="true"], pre[tabindex]';

/**
 * The app shell (§5, §10): a toolbar (the journey, language, More) over the scrolling screen,
 * and an opaque footer with the status on the left and the actions on the right in the
 * platform's order. The heading gets focus on every screen change, status changes go to a
 * polite live region, and Enter runs the main action when no control has focus.
 */
export function Shell(props: ShellProps) {
  const { model, lang, platform } = props;
  const copy = copyFor(lang);
  const [moreOpen, setMoreOpen] = useState(false);
  const [scrolled, setScrolled] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const mainRef = useRef<HTMLElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const moreButtonRef = useRef<HTMLButtonElement>(null);

  const emit: Emit = (id, payload) => {
    if (id === "show_more" || id === "help") setMoreOpen(true);
    props.onAction(id, payload);
  };
  const view = renderScreen(model, { copy, lang, platform, emit });
  const key = screenKey(model);
  const watchModel = "watch" in model && model.watch ? model.watch.model : null;

  // A new screen: start at its top, focus its heading.
  useEffect(() => {
    mainRef.current?.scrollTo?.({ top: 0 });
    headingRef.current?.focus({ preventScroll: true });
  }, [key]);

  // Status changes inside a screen ("Looking for your watch…", progress) are announced politely.
  const [announcement, setAnnouncement] = useState("");
  useEffect(() => setAnnouncement(view.status?.text ?? ""), [view.status?.text]);

  // Enter runs the main action when no other control has focus.
  const primary = view.primary;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Enter" || e.defaultPrevented || e.isComposing || moreOpen) return;
      if (!primary || primary.disabled) return;
      const target = e.target instanceof Element ? e.target : null;
      const root = rootRef.current;
      if (!root) return;
      const inShell = target === null || target === document.body || root.contains(target);
      if (!inShell || target?.closest(CONTROLS)) return;
      e.preventDefault();
      root.querySelector<HTMLButtonElement>("[data-main-action]")?.click();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [primary, moreOpen]);

  const other: Lang = lang === "ka" ? "en" : "ka";
  const labels = copyLabels(copy);
  // Windows puts the main button first; macOS and GNOME put it last, on the right.
  const actions = [
    view.secondary ? <FooterButton key="secondary" action={view.secondary} main={false} labels={labels} /> : null,
    view.primary ? <FooterButton key="primary" action={view.primary} main labels={labels} /> : null,
  ];
  if (platform === "windows") actions.reverse();

  return (
    <MotionConfig reducedMotion="user">
      <div
        ref={rootRef}
        lang={lang}
        data-platform={platform}
        className="app absolute inset-0 flex flex-col overflow-hidden bg-canvas text-fg"
      >
        <header
          inert={moreOpen}
          data-tauri-drag-region
          className={cn(
            "absolute inset-x-0 top-0 z-20 flex h-[var(--toolbar-h)] items-center gap-2 border-b pr-3",
            "transition-[background-color,border-color] duration-200",
            scrolled ? "border-sep bg-toolbar backdrop-blur-xl backdrop-saturate-150" : "border-transparent bg-canvas",
          )}
          style={{ paddingLeft: platform === "macos" ? MAC_TOOLBAR_INSET : 12 }}
        >
          {view.back ? (
            <Button variant="toolbar" size="sm" onClick={view.back.onPress} className="-ml-1 pr-3 pl-1">
              <ChevronLeft size={18} strokeWidth={2.2} aria-hidden="true" />
              {view.back.label}
            </Button>
          ) : view.step ? (
            <StepIndicator current={view.step} allDone={view.allDone} labels={copy.steps} />
          ) : null}
          <div className="ml-auto flex items-center gap-0.5">
            <Button variant="toolbar" size="sm" lang={other} onClick={() => props.onLanguage(other)}>
              <Globe size={15} strokeWidth={1.8} aria-hidden="true" />
              {LANGUAGE_NAMES[other]}
            </Button>
            <Button
              variant="toolbar"
              size="icon"
              ref={moreButtonRef}
              aria-label={copy.header.more}
              title={copy.header.more}
              aria-haspopup="dialog"
              aria-expanded={moreOpen}
              onClick={() => emit("show_more")}
            >
              <span className="flex size-[19px] items-center justify-center rounded-full border-[1.6px] border-current">
                <Ellipsis size={13} strokeWidth={2.4} aria-hidden="true" />
              </span>
            </Button>
          </div>
        </header>

        <main
          ref={mainRef}
          inert={moreOpen}
          onScroll={(e) => setScrolled(e.currentTarget.scrollTop > 2)}
          className="min-h-0 flex-1 overflow-y-auto overscroll-contain"
          style={{ scrollPaddingTop: "var(--toolbar-h)" }}
        >
          <div className="flex min-h-full flex-col pt-[calc(var(--toolbar-h)+4px)] pb-6">
            {/* Short screens sit a little above the middle; long ones start at the top and scroll. */}
            <div className="grow" />
            <motion.div
              key={key}
              // A short settle, no fade: text is at full contrast from the first frame.
              initial={{ y: 6 }}
              animate={{ y: 0 }}
              transition={{ duration: 0.2, ease: [0.25, 0.1, 0.25, 1] }}
              className="mx-auto flex w-full max-w-[520px] flex-col gap-6 px-5"
            >
              <div className="flex flex-col items-center text-center">
                {view.hero ? <div className="mb-4">{view.hero}</div> : null}
                {view.badge ? (
                  <div className="mb-2">
                    <Badge>{view.badge}</Badge>
                  </div>
                ) : null}
                <h1 ref={headingRef} id={TITLE_ID} tabIndex={-1} className="max-w-[26ch] text-title font-[650] text-balance text-fg">
                  {keepTogether(view.title, watchModel)}
                </h1>
                {view.lead ? (
                  <p className="mt-2 max-w-[42ch] text-lead text-pretty text-fg-2">{keepTogether(view.lead, watchModel)}</p>
                ) : null}
              </div>
              {view.content}
            </motion.div>
            <div className="grow-[1.6]" />
          </div>
        </main>

        {view.status || view.primary || view.secondary ? (
          <footer
            inert={moreOpen}
            className="relative z-20 flex min-h-[var(--footer-h)] shrink-0 flex-wrap items-center justify-end gap-x-3 gap-y-2 border-t border-sep bg-canvas px-4 py-3"
          >
            {view.status ? (
              <p className="flex min-w-[12rem] flex-1 items-center gap-2 text-label text-fg-2">
                {view.status.busy ? <Spinner /> : null}
                <span className="min-w-0">{view.status.text}</span>
              </p>
            ) : null}
            <div className="ml-auto flex flex-wrap items-center justify-end gap-2">{actions}</div>
          </footer>
        ) : null}

        <div className="sr-only" role="status" aria-live="polite">
          {announcement}
        </div>

        <MoreSheet
          open={moreOpen}
          onOpenChange={setMoreOpen}
          container={rootRef}
          opener={moreButtonRef}
          more={props.more}
          copy={copy}
          lang={lang}
          platform={platform}
          onLanguage={props.onLanguage}
          onAction={(id) => props.onAction(id)}
        />
      </div>
    </MotionConfig>
  );
}

/** A footer button; with `copyText` it copies and says so on the button for a moment. */
function FooterButton(props: { action: FooterAction; main: boolean; labels: CopyLabels }) {
  const [state, copy] = useCopy();
  const { action } = props;
  const label = state === "copied" ? props.labels.copied : state === "failed" ? props.labels.copyFailed : action.label;
  return (
    <Button
      variant={props.main ? "primary" : "secondary"}
      disabled={action.disabled}
      data-main-action={props.main ? "" : undefined}
      onClick={() => {
        if (action.copyText !== undefined) void copy(action.copyText);
        action.onPress?.();
      }}
    >
      <span aria-live={action.copyText !== undefined ? "polite" : undefined}>{label}</span>
    </Button>
  );
}

/**
 * Keeps a model name and the suffix attached to it ("Lily 2 Active-ზე") on one line, so
 * Georgian never breaks at the hyphen or inside the name (§10).
 */
function keepTogether(text: string, model: string | null): ReactNode {
  if (!model || !text.includes(model)) return text;
  const escaped = model.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return text.split(new RegExp(`(${escaped}[^\\s,.!?]*)`)).map((part, i) =>
    i % 2 === 1 ? (
      <span key={i} className="whitespace-nowrap">
        {part}
      </span>
    ) : (
      part
    ),
  );
}
