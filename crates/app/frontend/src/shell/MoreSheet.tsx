import { ChevronRight, CircleHelp, FileText, Info, ListChecks, RotateCcw, ScanSearch, Smartphone } from "lucide-react";
import { Collapsible, Dialog } from "radix-ui";
import { type ReactNode, type RefObject, useId } from "react";

import type { Lang, Platform } from "../core/backend";
import { type Copy, fmt } from "../copy";
import { LANGUAGE_NAMES, REPO_URL } from "../copy/shared";
import type { ActionId, MoreModel } from "../model/screens";
import { copyLabels } from "../screens/common";
import { Button } from "../ui/Button";
import { cn } from "../ui/cn";
import { Segmented } from "../ui/Controls";
import { CopyBlock } from "../ui/CopyBlock";
import { Disclosure } from "../ui/Disclosure";
import { Group, GroupHeader, Hairline, Tile, TILE_INSET, ValueRow } from "../ui/Group";

/**
 * More (§5.2, §11), a sheet that drops from the toolbar over the dimmed window: language,
 * Restore original font when the watch allows it, Diagnostic info, what the app sees, help,
 * which watches work, and about. It traps focus, closes on Esc and returns focus to the
 * button that opened it. The only place the app's internal words may appear (under "What the
 * app sees", from the flow's data).
 */
export function MoreSheet(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  container: RefObject<HTMLElement | null>;
  /** The button that opened the sheet: focus goes back to it on close. */
  opener: RefObject<HTMLElement | null>;
  more: MoreModel;
  copy: Copy;
  lang: Lang;
  platform: Platform;
  /** macOS's window buttons over the toolbar: the sheet opens below the toolbar, clear of them. */
  overlayTitleBar: boolean;
  /** The demo build: About says so, since its watch, sharing and saving are simulated. */
  demo: boolean;
  onLanguage: (lang: Lang) => void;
  onAction: (id: ActionId) => void;
}) {
  const { copy, more } = props;
  const m = copy.more;
  const run = (id: ActionId) => {
    props.onOpenChange(false);
    props.onAction(id);
  };
  return (
    <Dialog.Root open={props.open} onOpenChange={props.onOpenChange}>
      <Dialog.Portal container={props.container.current}>
        <Dialog.Overlay className="anim-scrim absolute inset-0 z-40 bg-[var(--scrim)]" />
        <Dialog.Content
          aria-modal="true"
          onCloseAutoFocus={(e) => {
            e.preventDefault();
            props.opener.current?.focus();
          }}
          aria-describedby={undefined}
          className={cn(
            "anim-sheet absolute inset-x-3 bottom-3 z-50 mx-auto flex max-w-[460px] flex-col overflow-hidden rounded-group bg-canvas shadow-window",
            props.overlayTitleBar ? "top-[var(--toolbar-h)]" : "top-2",
          )}
        >
          <Dialog.Title className="px-5 pt-4 pb-3 text-[17px] leading-6 font-semibold text-fg">{m.title}</Dialog.Title>
          {/* The sections keep their height and the list scrolls: a group clips (rounded corners),
              so as a shrinking flex item it would cut off its own rows in a short window. */}
          <div className="flex flex-1 flex-col gap-5 overflow-y-auto px-4 pb-4 *:shrink-0">
            <section>
              <GroupHeader id="more-language">{m.language}</GroupHeader>
              <Segmented
                labelledBy="more-language"
                value={props.lang}
                onChange={props.onLanguage}
                choices={(["ka", "en"] as const).map((l) => ({ value: l, label: LANGUAGE_NAMES[l], lang: l }))}
              />
            </section>

            <Group className={TILE_INSET}>
              {more.restorable ? (
                <SheetRow icon={<RotateCcw size={15} />} title={copy.actions.restore} detail={m.restore.body} onPress={() => run("restore")} />
              ) : null}
              <SheetRow icon={<FileText size={15} />} title={m.diagnostics.title} detail={m.diagnostics.body} onPress={() => run("diagnostics")} />
              <SheetRow icon={<ListChecks size={15} />} title={copy.actions.whichWatches} onPress={() => run("which_watches")} />
            </Group>

            <Group className={TILE_INSET}>
              <Section icon={<ScanSearch size={15} />} title={m.seen.title}>
                {more.seen ? (
                  <dl className="-mx-3.5">
                    {more.seen.map((s) => (
                      <ValueRow key={s.label} label={s.label} value={<span className="wrap-anywhere">{s.value}</span>} />
                    ))}
                  </dl>
                ) : (
                  <p className="text-row text-fg-2">{m.seen.noWatch}</p>
                )}
                {more.reportText ? (
                  <div className="mt-2 -ml-3.5">
                    <Disclosure summary={m.seen.reportTitle}>
                      <CopyBlock text={more.reportText} label={m.seen.reportTitle} labels={copyLabels(copy)} variant="report" lang="en" />
                    </Disclosure>
                  </div>
                ) : null}
              </Section>
              <Section icon={<CircleHelp size={15} />} title={m.help.title}>
                <ul className="flex list-disc flex-col gap-2 pl-4 text-row text-fg-2">
                  <li>{m.help.connecting}</li>
                  <li>{m.help.mtp}</li>
                  <li>{m.help.express[props.platform]}</li>
                </ul>
              </Section>
              <Section icon={<Smartphone size={15} />} title={m.help.phoneTitle}>
                <ul className="flex list-disc flex-col gap-2 pl-4 text-row text-fg-2">
                  <li>{copy.phone.android}</li>
                  <li>{copy.phone.iphone}</li>
                  <li>{copy.phone.either}</li>
                </ul>
              </Section>
            </Group>

            <Group>
              <div className="flex items-start gap-3 px-3.5 py-3">
                <Tile>
                  <Info size={15} />
                </Tile>
                <div className="min-w-0 text-row text-fg-2">
                  <p className="text-fg">{fmt(m.about.version, { version: more.version })}</p>
                  {props.demo ? <p className="text-fg">{copy.app.demo}</p> : null}
                  <p>{m.about.unofficial}</p>
                  <p>
                    {m.about.source}{" "}
                    <span lang="en" className="wrap-anywhere select-text">
                      {REPO_URL}
                    </span>
                  </p>
                </div>
              </div>
            </Group>
          </div>
          <div className="flex justify-end border-t border-sep px-4 py-3">
            <Dialog.Close asChild>
              <Button variant="primary">{copy.actions.close}</Button>
            </Dialog.Close>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

/**
 * A command row. The button holds only the title, so its name is exactly what it shows; the
 * detail line describes it, and the button's hit area stretches over the whole row.
 */
function SheetRow(props: { icon: ReactNode; title: string; detail?: string; onPress: () => void }) {
  const id = useId();
  return (
    <div className="relative flex min-h-11 w-full items-center gap-3 px-3.5 py-2.5 hover:bg-group-hover active:bg-group-active">
      <Hairline />
      <Tile>{props.icon}</Tile>
      <div className="min-w-0 flex-1">
        <button
          type="button"
          aria-describedby={props.detail ? `${id}-detail` : undefined}
          onClick={props.onPress}
          className="block w-full rounded-[4px] text-left text-row text-fg after:absolute after:inset-0"
        >
          {props.title}
        </button>
        {props.detail ? (
          <p id={`${id}-detail`} className="text-row text-fg-2">
            {props.detail}
          </p>
        ) : null}
      </div>
      <ChevronRight size={16} className="text-fg-2" aria-hidden="true" />
    </div>
  );
}

/** A row that opens in place, like a disclosure row in System Settings. */
function Section(props: { icon: ReactNode; title: string; children: ReactNode }) {
  return (
    <Collapsible.Root className="group/section relative">
      <Hairline />
      <h3>
        <Collapsible.Trigger
          data-focus="inset"
          className="flex min-h-11 w-full items-center gap-3 px-3.5 py-2.5 text-left hover:bg-group-hover"
        >
          <Tile>{props.icon}</Tile>
          <span className="flex-1 text-row text-fg">{props.title}</span>
          <ChevronRight
            size={16}
            aria-hidden="true"
            className="text-fg-2 transition-transform duration-200 group-data-[state=open]/section:rotate-90"
          />
        </Collapsible.Trigger>
      </h3>
      <Collapsible.Content className="px-3.5 pb-3 pl-[55px]">{props.children}</Collapsible.Content>
    </Collapsible.Root>
  );
}
