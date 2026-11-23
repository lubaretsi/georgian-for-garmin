import { Info, LifeBuoy, ScanEye } from "lucide-react";

import type { Lang, Platform } from "@/core/backend";
import { type Copy, fmt } from "@/copy";
import { LANGUAGE_NAMES, REPO_URL } from "@/copy/shared";
import type { ActionId, MoreModel } from "@/model/screens";

import { Button } from "../ui/button";
import { Segmented } from "../ui/choice";
import { CollapsibleCard } from "../ui/disclosure";
import { Card } from "../ui/parts";
import { Sheet } from "../ui/sheet";

/** More (§5.2): language, restore, what the app sees, help, about. */
export function MoreSheet(props: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  container: HTMLElement | null;
  more: MoreModel;
  copy: Copy;
  lang: Lang;
  platform: Platform;
  onLanguage: (lang: Lang) => void;
  onAction: (id: ActionId) => void;
}) {
  const { copy, more } = props;
  const m = copy.more;
  const act = (id: ActionId) => {
    props.onOpenChange(false);
    props.onAction(id);
  };
  return (
    <Sheet open={props.open} onOpenChange={props.onOpenChange} title={m.title} closeLabel={copy.actions.close} container={props.container}>
      <Segmented
        legend={m.language}
        options={(["ka", "en"] as const).map((value) => ({ value, label: LANGUAGE_NAMES[value] }))}
        value={props.lang}
        onChange={props.onLanguage}
      />
      {more.restorable ? (
        <Card className="wa:grid wa:gap-3">
          <div>
            <p className="wa:font-semibold">{m.restore.title}</p>
            <p className="wa:text-ink-2">{m.restore.body}</p>
          </div>
          <Button variant="secondary" size="md" onClick={() => act("restore")}>
            {copy.actions.restore}
          </Button>
        </Card>
      ) : null}
      <CollapsibleCard title={m.seen.title} icon={<ScanEye />}>
        {more.seen ? (
          <dl className="wa:grid wa:gap-2">
            {more.seen.map((row) => (
              <div key={row.label} className="wa:grid wa:gap-0.5">
                <dt className="wa:text-[13px] wa:text-ink-2">{row.label}</dt>
                <dd className="wa:font-medium wa:break-words wa:select-text">{row.value}</dd>
              </div>
            ))}
          </dl>
        ) : (
          <p className="wa:text-ink-2">{m.seen.noWatch}</p>
        )}
      </CollapsibleCard>
      <CollapsibleCard title={m.help.title} icon={<LifeBuoy />}>
        <ul className="wa:grid wa:gap-2 wa:text-[15px]">
          <li>{m.help.connecting}</li>
          <li>{m.help.mtp}</li>
          <li>{m.help.express[props.platform]}</li>
        </ul>
      </CollapsibleCard>
      <Button variant="secondary" size="md" onClick={() => act("which_watches")}>
        {copy.actions.whichWatches}
      </Button>
      <div className="wa:flex wa:items-start wa:gap-3 wa:px-1 wa:text-[14px] wa:text-ink-2">
        <Info aria-hidden className="wa:mt-0.5 wa:size-4 wa:shrink-0" />
        <p>
          {fmt(m.about.version, { version: more.version })} · {m.about.unofficial} {m.about.source}{" "}
          <span lang="en" className="wa:break-all">
            {REPO_URL}
          </span>
        </p>
      </div>
    </Sheet>
  );
}
