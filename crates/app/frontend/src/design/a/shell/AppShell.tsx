import { Ellipsis, Globe } from "lucide-react";
import { motion, MotionConfig } from "motion/react";
import { useEffect, useRef, useState } from "react";

import type { Lang, Platform } from "@/core/backend";
import { copyFor } from "@/copy";
import { LANGUAGE_NAMES } from "@/copy/shared";
import type { ActionId, ActionPayload, MoreModel, ScreenModel } from "@/model/screens";
import { screenKey } from "@/screens";

import { designCopy } from "../copy";
import { renderScreen } from "../screens";
import { Button } from "../ui/button";
import { cn } from "../ui/cn";
import { Spinner } from "../ui/disclosure";
import { Journey } from "../ui/journey";
import { AppIcon } from "../art/app-icon";
import { MoreSheet } from "./MoreSheet";

/** Focus on one of these means Enter belongs to that control, not to the main action. */
const CONTROLS = 'button, a[href], input, select, textarea, summary, [role="button"], [role="radio"], [role="checkbox"]';

/**
 * The app: header (name, language, More), the journey, the screen (hero, heading, content;
 * it scrolls), and the fixed footer with one full-width main action and a quiet secondary.
 */
export function AppShell(props: {
  model: ScreenModel;
  more: MoreModel;
  lang: Lang;
  platform: Platform;
  onAction: (id: ActionId, payload?: ActionPayload) => void;
  onLanguage: (lang: Lang) => void;
}) {
  const { model, lang, platform } = props;
  const copy = copyFor(lang);
  const extra = designCopy[lang];
  const [moreOpen, setMoreOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const mainRef = useRef<HTMLElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);

  const emit = (id: ActionId, payload?: ActionPayload) => {
    if (id === "show_more" || id === "help") setMoreOpen(true);
    props.onAction(id, payload);
  };
  const view = renderScreen(model, { copy, extra, lang, platform, emit });
  const key = screenKey(model);

  useEffect(() => {
    mainRef.current?.scrollTo?.({ top: 0 });
    headingRef.current?.focus({ preventScroll: true });
  }, [key]);

  const [announcement, setAnnouncement] = useState("");
  useEffect(() => setAnnouncement(view.status ?? ""), [view.status]);

  const primary = view.primary;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Enter" || e.defaultPrevented || e.isComposing || moreOpen) return;
      if (!primary || primary.disabled) return;
      const root = rootRef.current;
      const target = e.target instanceof Element ? e.target : null;
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
  const SecondaryIcon = view.secondary?.icon;
  const PrimaryIcon = primary?.icon;

  return (
    <MotionConfig reducedMotion="user">
      <div ref={rootRef} lang={lang} className="a-app wa:relative wa:flex wa:h-full wa:flex-col wa:overflow-hidden wa:bg-paper">
        <header className="wa:flex wa:items-center wa:gap-2 wa:px-4 wa:pt-3 wa:pb-2" inert={moreOpen}>
          <AppIcon size={28} className="wa:drop-shadow-[0_1px_1.5px_rgb(120_20_35/0.3)]" />
          <span className="wa:ml-1 wa:min-w-0 wa:truncate wa:text-[14px] wa:font-semibold wa:text-ink wa:@max-[460px]:sr-only">
            {copy.app.name}
          </span>
          <span className="wa:ml-auto wa:flex wa:shrink-0 wa:gap-2">
            <Button variant="pill" size="sm" lang={other} onClick={() => props.onLanguage(other)}>
              <Globe aria-hidden className="wa:text-ink-2" />
              {LANGUAGE_NAMES[other]}
            </Button>
            <Button variant="pill" size="sm" aria-haspopup="dialog" aria-expanded={moreOpen} onClick={() => emit("show_more")}>
              <Ellipsis aria-hidden className="wa:text-ink-2" />
              {copy.header.more}
            </Button>
          </span>
        </header>

        {view.step ? (
          <nav aria-label={copy.steps.label} className="wa:px-5 wa:pt-2 wa:pb-4" inert={moreOpen}>
            <Journey current={view.step} complete={view.journeyComplete} labels={copy.steps} />
          </nav>
        ) : null}

        <main
          ref={mainRef}
          className="wa:min-h-0 wa:flex-1 wa:overflow-y-auto wa:px-5 wa:pt-1 wa:pb-6 wa:[mask-image:linear-gradient(to_bottom,transparent,black_12px)]"
          inert={moreOpen}
        >
          {/* A new screen settles in: the text slides (it stays fully opaque, so contrast holds
              at every frame), the scene fades and grows a touch. */}
          <motion.div key={key} initial={{ y: 8 }} animate={{ y: 0 }} transition={{ duration: 0.32, ease: [0.22, 1, 0.36, 1] }}>
            {view.hero ? (
              <motion.div
                aria-hidden
                initial={{ opacity: 0, scale: 0.97 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ duration: 0.4, ease: [0.22, 1, 0.36, 1] }}
                data-size={view.hero.size ?? "full"}
                data-tone={view.hero.tone ?? "warm"}
                className="a-hero a-stage wa:mt-1 wa:flex wa:items-center wa:justify-center wa:overflow-hidden wa:rounded-[24px] wa:px-4 wa:py-3"
              >
                {view.hero.scene}
              </motion.div>
            ) : null}
            {view.eyebrow ? <div className="wa:mt-5 wa:-mb-2">{view.eyebrow}</div> : null}
            <h1 ref={headingRef} tabIndex={-1} className={cn("a-title wa:text-ink", view.hero ? "wa:mt-5" : "wa:mt-2")}>
              {view.title}
            </h1>
            {view.lead ? <p className="a-lead wa:mt-2">{view.lead}</p> : null}
            {view.content ? <div className="wa:mt-5 wa:grid wa:gap-4">{view.content}</div> : null}
          </motion.div>
        </main>

        <footer className="wa:relative wa:z-10 wa:grid wa:gap-1 wa:bg-paper wa:px-5 wa:pt-3 wa:pb-4 wa:shadow-footer" inert={moreOpen}>
          <div className={cn("wa:grid wa:gap-2", view.secondaryInline && view.secondary && "wa:grid-cols-[auto_1fr]")}>
            {view.secondaryInline && view.secondary ? (
              <Button variant="secondary" size="lg" onClick={view.secondary.onPress}>
                {SecondaryIcon ? <SecondaryIcon aria-hidden /> : null}
                {view.secondary.label}
              </Button>
            ) : null}
            {primary ? (
              <Button data-main-action block size="lg" disabled={primary.disabled} onClick={primary.onPress}>
                {primary.label}
                {PrimaryIcon ? <PrimaryIcon aria-hidden /> : null}
              </Button>
            ) : view.waiting ? (
              <div className="wa:flex wa:min-h-12 wa:items-center wa:justify-center wa:gap-2.5 wa:rounded-[14px] wa:bg-sunken wa:px-4 wa:py-2 wa:text-center wa:text-[15px] wa:font-semibold wa:text-ink-2">
                <Spinner className="wa:text-accent-ink" />
                {view.waiting}
              </div>
            ) : null}
          </div>
          {!view.secondaryInline && view.secondary ? (
            <Button variant="quiet" size="md" block onClick={view.secondary.onPress}>
              {SecondaryIcon ? <SecondaryIcon aria-hidden /> : null}
              {view.secondary.label}
            </Button>
          ) : null}
        </footer>

        <div className="wa:sr-only" role="status" aria-live="polite">
          {announcement}
        </div>

        <MoreSheet
          open={moreOpen}
          onOpenChange={setMoreOpen}
          container={rootRef.current}
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
