import { useEffect, useRef, useState } from "react";

import type { Lang, Platform } from "../core/backend";
import { copyFor } from "../copy";
import { LANGUAGE_NAMES } from "../copy/shared";
import type { ActionId, ActionPayload, MoreModel, ScreenModel } from "../model/screens";
import { copyLabels } from "../screens/common";
import { type Emit, renderScreen, screenKey } from "../screens";
import { ActionBar } from "../ui/ActionBar";
import { Badge, Illustration, WatchCard } from "../ui/bits";
import { softwareLine } from "../screens/common";
import { Icon } from "../ui/icons";
import { StepIndicator } from "../ui/StepIndicator";
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
const CONTROLS = 'button, a[href], input, select, textarea, summary, [role="button"], [contenteditable="true"], pre[tabindex]';

/**
 * The app shell (§5): header with the language switch and More, the screen (its heading
 * gets focus on every screen change, its content scrolls), and the fixed footer with the
 * screen's actions. Status changes go to a polite live region.
 */
export function Shell(props: ShellProps) {
  const { model, lang, platform } = props;
  const copy = copyFor(lang);
  const [moreOpen, setMoreOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const mainRef = useRef<HTMLElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);

  const emit: Emit = (id, payload) => {
    if (id === "show_more" || id === "help") setMoreOpen(true);
    props.onAction(id, payload);
  };
  const view = renderScreen(model, { copy, lang, platform, emit });
  const key = screenKey(model);

  // A new screen: start at its top, focus its heading.
  useEffect(() => {
    mainRef.current?.scrollTo?.({ top: 0 });
    headingRef.current?.focus({ preventScroll: true });
  }, [key]);

  // Status changes inside a screen ("Looking for your watch…", progress) are announced politely.
  const [announcement, setAnnouncement] = useState("");
  useEffect(() => setAnnouncement(view.status ?? ""), [view.status]);

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

  return (
    <div className="shell" ref={rootRef} lang={lang}>
      <header className="shell-header" inert={moreOpen}>
        <span className="shell-app-name">{copy.app.name}</span>
        <button
          type="button"
          className="header-button"
          lang={other}
          onClick={() => props.onLanguage(other)}
        >
          {LANGUAGE_NAMES[other]}
        </button>
        <button
          type="button"
          className="header-button"
          aria-haspopup="dialog"
          aria-expanded={moreOpen}
          onClick={() => emit("show_more")}
        >
          <Icon name="more" />
          <span>{copy.header.more}</span>
        </button>
      </header>

      <main className="shell-main" ref={mainRef} inert={moreOpen}>
        <div className="screen">
          {view.step ? <StepIndicator current={view.step} labels={copy.steps} /> : null}
          {view.illustration ? <Illustration icon={view.illustration.icon} tone={view.illustration.tone} /> : null}
          {view.badge ? <Badge>{view.badge}</Badge> : null}
          <h1 className="screen-title" ref={headingRef} tabIndex={-1}>
            {view.title}
          </h1>
          {view.watch ? <WatchCard model={view.watch.model} software={softwareLine(copy, view.watch)} /> : null}
          <div className="screen-content">{view.content}</div>
        </div>
      </main>

      <footer className="shell-footer" inert={moreOpen}>
        <ActionBar primary={view.primary} secondary={view.secondary} labels={copyLabels(copy)} />
      </footer>

      <div className="sr-only" role="status" aria-live="polite">
        {announcement}
      </div>

      {moreOpen ? (
        <MoreSheet
          more={props.more}
          copy={copy}
          lang={lang}
          platform={platform}
          onLanguage={props.onLanguage}
          onAction={(id) => props.onAction(id)}
          onClose={() => setMoreOpen(false)}
        />
      ) : null}
    </div>
  );
}
