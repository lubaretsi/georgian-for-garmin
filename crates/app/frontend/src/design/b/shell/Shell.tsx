import { Ellipsis, Globe } from "lucide-react";
import { MotionConfig, motion } from "motion/react";
import { type ReactNode, useEffect, useRef, useState } from "react";

import type { Lang, Platform } from "../../../core/backend";
import { copyFor } from "../../../copy";
import { LANGUAGE_NAMES } from "../../../copy/shared";
import type { ActionId, ActionPayload, MoreModel, ScreenModel } from "../../../model/screens";
import { copyB } from "../copy";
import { cn } from "../lib/cn";
import { type Emit, renderScreen, screenKey, TITLE_ID, type ViewAction } from "../screens";
import { Button } from "../ui/button";
import { Spinner } from "../ui/controls";
import { StepIndicator } from "../ui/steps";
import { MoreSheet } from "./MoreSheet";
import { MAC_INSET } from "./WindowFrame";

export type ShellProps = {
  model: ScreenModel;
  more: MoreModel;
  lang: Lang;
  platform: Platform;
  onAction: (id: ActionId, payload?: ActionPayload) => void;
  onLanguage: (lang: Lang) => void;
};

/** Focus on one of these means Enter belongs to that control, not to the main action. */
const CONTROLS = 'button, a[href], input, select, textarea, summary, [role="button"], [role="radio"], [role="checkbox"], [contenteditable="true"]';

/**
 * Direction B's shell: a translucent toolbar (the journey, language, More), the screen
 * scrolling beneath it, and a toolbar-style footer with a status on the left and the
 * actions on the right in the platform's order.
 */
export function Shell(props: ShellProps) {
  const { model, lang, platform } = props;
  const copy = copyFor(lang);
  const b = copyB[lang];
  const [moreOpen, setMoreOpen] = useState(false);
  const [scrolled, setScrolled] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const mainRef = useRef<HTMLElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);

  const emit: Emit = (id, payload) => {
    if (id === "show_more" || id === "help") setMoreOpen(true);
    props.onAction(id, payload);
  };
  const view = renderScreen(model, { copy, b, lang, platform, emit });
  const key = screenKey(model);
  const watchModel = "watch" in model && model.watch ? model.watch.model : null;

  useEffect(() => {
    mainRef.current?.scrollTo?.({ top: 0 });
    headingRef.current?.focus({ preventScroll: true });
  }, [key]);

  const [announcement, setAnnouncement] = useState("");
  useEffect(() => setAnnouncement(view.status?.text ?? ""), [view.status?.text]);

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
  // Windows puts the primary button first; macOS and GNOME put it last (rightmost).
  const actions = [view.secondary && actionButton(view.secondary, false), view.primary && actionButton(view.primary, true)];
  if (platform === "windows") actions.reverse();

  return (
    <MotionConfig reducedMotion="user">
      <div ref={rootRef} lang={lang} className="b-shell nb:absolute nb:inset-0 nb:overflow-hidden nb:bg-canvas">
        <header
          inert={moreOpen}
          data-tauri-drag-region
          className={cn(
            "nb:absolute nb:inset-x-0 nb:top-0 nb:z-20 nb:flex nb:h-[var(--b-toolbar-h)] nb:items-center nb:gap-2 nb:pr-3",
            "nb:border-b nb:transition-[background-color,border-color] nb:duration-200",
            scrolled
              ? "nb:border-sep nb:bg-toolbar nb:backdrop-blur-xl nb:backdrop-saturate-150"
              : "nb:border-transparent nb:bg-canvas",
          )}
          style={{ paddingLeft: platform === "macos" ? MAC_INSET : 12 }}
        >
          {view.step ? <StepIndicator current={view.step} labels={copy.steps} /> : null}
          <div className="nb:ml-auto nb:flex nb:items-center nb:gap-0.5">
            <Button variant="toolbar" size="sm" lang={other} onClick={() => props.onLanguage(other)}>
              <Globe size={15} strokeWidth={1.8} aria-hidden="true" />
              {LANGUAGE_NAMES[other]}
            </Button>
            <Button
              variant="toolbar"
              size="icon"
              aria-label={copy.header.more}
              title={copy.header.more}
              aria-haspopup="dialog"
              aria-expanded={moreOpen}
              onClick={() => emit("show_more")}
            >
              <span className="nb:flex nb:size-[19px] nb:items-center nb:justify-center nb:rounded-full nb:border-[1.6px] nb:border-current">
                <Ellipsis size={13} strokeWidth={2.4} aria-hidden="true" />
              </span>
            </Button>
          </div>
        </header>

        <main
          ref={mainRef}
          inert={moreOpen}
          onScroll={(e) => setScrolled(e.currentTarget.scrollTop > 2)}
          className="nb:absolute nb:inset-0 nb:overflow-y-auto nb:overscroll-contain"
          style={{ scrollPaddingTop: "var(--b-toolbar-h)" }}
        >
          <div className="nb:flex nb:min-h-full nb:flex-col nb:pt-[calc(var(--b-toolbar-h)+4px)] nb:pb-[calc(var(--b-footer-h)+24px)]">
            {/* Short screens sit a little above the middle; long ones start at the top and scroll. */}
            <div className="nb:grow" />
            <motion.div
              key={key}
              // A short settle, no fade: text is at full contrast from the first frame.
              initial={{ y: 6 }}
              animate={{ y: 0 }}
              transition={{ duration: 0.2, ease: [0.25, 0.1, 0.25, 1] }}
              className="nb:mx-auto nb:flex nb:w-full nb:max-w-[520px] nb:flex-col nb:gap-6 nb:px-5"
            >
              <div className="nb:flex nb:flex-col nb:items-center nb:text-center">
                {view.hero ? <div className="nb:mb-4">{view.hero}</div> : null}
                {view.badge ? <div className="nb:mb-2">{view.badge}</div> : null}
                <h1
                  ref={headingRef}
                  id={TITLE_ID}
                  tabIndex={-1}
                  className="nb:max-w-[26ch] nb:text-title nb:font-[650] nb:text-balance nb:text-fg"
                >
                  {keepTogether(view.title, watchModel)}
                </h1>
                {view.lead ? (
                  <p className="nb:mt-2 nb:max-w-[42ch] nb:text-lead nb:text-pretty nb:text-fg-2">
                    {typeof view.lead === "string" ? keepTogether(view.lead, watchModel) : view.lead}
                  </p>
                ) : null}
              </div>
              {view.content}
            </motion.div>
            <div className="nb:grow-[1.6]" />
          </div>
        </main>

        <footer
          inert={moreOpen}
          className={cn(
            "nb:absolute nb:inset-x-0 nb:bottom-0 nb:z-20 nb:flex nb:h-[var(--b-footer-h)] nb:items-center nb:gap-2 nb:px-4",
            "nb:border-t nb:border-sep nb:bg-toolbar nb:backdrop-blur-xl nb:backdrop-saturate-150",
          )}
        >
          {view.status ? (
            <p className="nb:flex nb:min-w-0 nb:items-center nb:gap-2 nb:text-label nb:text-fg-2">
              {view.status.busy ? <Spinner /> : null}
              <span className="nb:truncate">{view.status.text}</span>
            </p>
          ) : null}
          <div className="nb:ml-auto nb:flex nb:shrink-0 nb:items-center nb:gap-2">{actions}</div>
        </footer>

        <div className="nb:sr-only" role="status" aria-live="polite">
          {announcement}
        </div>

        <MoreSheet
          open={moreOpen}
          onOpenChange={setMoreOpen}
          container={rootRef}
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

/**
 * Keeps a model name and the suffix attached to it ("Lily 2 Active-ზე") on one line, so
 * Georgian never breaks at the hyphen or inside the name.
 */
function keepTogether(text: string, model: string | null): ReactNode {
  if (!model || !text.includes(model)) return text;
  const escaped = model.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return text.split(new RegExp(`(${escaped}[^\\s,.!?]*)`)).map((part, i) =>
    i % 2 === 1 ? (
      <span key={i} className="nb:whitespace-nowrap">
        {part}
      </span>
    ) : (
      part
    ),
  );
}

function actionButton(action: ViewAction, main: boolean): ReactNode {
  return (
    <Button
      key={main ? "primary" : "secondary"}
      variant={main ? "primary" : "secondary"}
      disabled={action.disabled}
      data-main-action={main ? "" : undefined}
      onClick={action.onPress}
    >
      {action.label}
    </Button>
  );
}
