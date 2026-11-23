import { ChevronRight, CircleHelp, Info, ListChecks, RotateCcw, ScanSearch } from "lucide-react";
import { Collapsible, Dialog } from "radix-ui";
import type { ReactNode, RefObject } from "react";

import type { Lang, Platform } from "../../../core/backend";
import { type Copy, fmt } from "../../../copy";
import { LANGUAGE_NAMES } from "../../../copy/shared";
import type { ActionId, MoreModel } from "../../../model/screens";
import { Button } from "../ui/button";
import { Segmented } from "../ui/controls";
import { Group, GroupHeader, Tile, ValueRow } from "../ui/group";

/**
 * More, as a macOS-style sheet that drops from the toolbar over a dimmed window: language,
 * Restore original font when the watch allows it, what the app sees, help, which watches
 * work, and about.
 */
export function MoreSheet(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  container: RefObject<HTMLElement | null>;
  more: MoreModel;
  copy: Copy;
  lang: Lang;
  platform: Platform;
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
        <Dialog.Overlay className="nb:absolute nb:inset-0 nb:z-40 nb:bg-[rgb(0_0_0/0.22)] nb:data-[state=open]:animate-[b-fade_160ms_ease-out]" />
        <Dialog.Content
          className="nb:absolute nb:inset-x-3 nb:top-2 nb:bottom-3 nb:z-50 nb:mx-auto nb:flex nb:max-w-[460px] nb:flex-col nb:overflow-hidden nb:rounded-group nb:bg-canvas nb:shadow-window nb:data-[state=open]:animate-[b-sheet_220ms_cubic-bezier(0.25,0.1,0.25,1)]"
          aria-describedby={undefined}
        >
          <Dialog.Title className="nb:px-5 nb:pt-4 nb:pb-3 nb:text-[17px] nb:leading-6 nb:font-semibold nb:text-fg">{m.title}</Dialog.Title>
          <div className="nb:flex nb:flex-1 nb:flex-col nb:gap-5 nb:overflow-y-auto nb:px-4 nb:pb-4">
            <section>
              <GroupHeader id="b-more-lang">{m.language}</GroupHeader>
              <Segmented
                labelledBy="b-more-lang"
                value={props.lang}
                onChange={props.onLanguage}
                choices={(["ka", "en"] as const).map((l) => ({ value: l, label: LANGUAGE_NAMES[l] }))}
              />
            </section>

            <Group>
              {more.restorable ? (
                <SheetRow icon={<RotateCcw size={15} />} title={copy.actions.restore} detail={m.restore.body} onPress={() => run("restore")} />
              ) : null}
              <Disclosure icon={<ScanSearch size={15} />} title={m.seen.title}>
                {more.seen ? (
                  <dl className="nb:-mx-3.5">
                    {more.seen.map((s) => (
                      <ValueRow key={s.label} label={s.label} value={<span className="nb:break-all">{s.value}</span>} />
                    ))}
                  </dl>
                ) : (
                  <p className="nb:text-row nb:text-fg-2">{m.seen.noWatch}</p>
                )}
              </Disclosure>
              <Disclosure icon={<CircleHelp size={15} />} title={m.help.title}>
                <ul className="nb:flex nb:list-disc nb:flex-col nb:gap-2 nb:pl-4 nb:text-row nb:text-fg-2">
                  <li>{m.help.connecting}</li>
                  <li>{m.help.mtp}</li>
                  <li>{m.help.express[props.platform]}</li>
                </ul>
              </Disclosure>
              <SheetRow icon={<ListChecks size={15} />} title={copy.actions.whichWatches} onPress={() => run("which_watches")} />
            </Group>

            <Group>
              <div className="nb:flex nb:items-start nb:gap-3 nb:px-3.5 nb:py-3">
                <Tile>
                  <Info size={15} />
                </Tile>
                <div className="nb:text-row nb:text-fg-2">
                  <p className="nb:text-fg">{fmt(m.about.version, { version: more.version })}</p>
                  <p>{m.about.unofficial}</p>
                </div>
              </div>
            </Group>
          </div>
          <div className="nb:flex nb:justify-end nb:border-t nb:border-sep nb:px-4 nb:py-3">
            <Dialog.Close asChild>
              <Button variant="primary">{copy.actions.done}</Button>
            </Dialog.Close>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function SheetRow(props: { icon: ReactNode; title: string; detail?: string; onPress: () => void }) {
  return (
    <button
      type="button"
      data-focus="inset"
      onClick={props.onPress}
      className="nb:flex nb:min-h-11 nb:w-full nb:items-center nb:gap-3 nb:px-3.5 nb:py-2.5 nb:text-left nb:hover:bg-group-hover nb:active:bg-group-active"
    >
      <Tile>{props.icon}</Tile>
      <span className="nb:flex-1">
        <span className="nb:block nb:text-row nb:text-fg">{props.title}</span>
        {props.detail ? <span className="nb:block nb:text-row nb:text-fg-2">{props.detail}</span> : null}
      </span>
      <ChevronRight size={16} className="nb:text-fg-2" aria-hidden="true" />
    </button>
  );
}

function Disclosure(props: { icon: ReactNode; title: string; children: ReactNode }) {
  return (
    <Collapsible.Root className="nb:group/d">
      <Collapsible.Trigger
        data-focus="inset"
        className="nb:flex nb:min-h-11 nb:w-full nb:items-center nb:gap-3 nb:px-3.5 nb:py-2.5 nb:text-left nb:hover:bg-group-hover"
      >
        <Tile>{props.icon}</Tile>
        <span className="nb:flex-1 nb:text-row nb:text-fg">{props.title}</span>
        <ChevronRight
          size={16}
          aria-hidden="true"
          className="nb:text-fg-2 nb:transition-transform nb:duration-200 nb:group-data-[state=open]/d:rotate-90"
        />
      </Collapsible.Trigger>
      <Collapsible.Content className="nb:px-3.5 nb:pb-3 nb:pl-[52px]">{props.children}</Collapsible.Content>
    </Collapsible.Root>
  );
}
