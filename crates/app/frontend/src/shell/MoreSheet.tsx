import type { Lang, Platform } from "../core/backend";
import { type Copy, fmt } from "../copy";
import { LANGUAGE_NAMES, REPO_URL } from "../copy/shared";
import type { ActionId, MoreModel } from "../model/screens";
import { copyLabels } from "../screens/common";
import { Facts } from "../ui/bits";
import { Button } from "../ui/Button";
import { RadioChoices } from "../ui/Choices";
import { CopyBlock } from "../ui/CopyBlock";
import { Disclosure } from "../ui/Disclosure";
import { Sheet } from "../ui/Sheet";

/**
 * The More sheet (§5.2): language, Restore original font, what the app sees, help, which
 * watches work, about. The only place the app's internal words may appear (under "What the
 * app sees", from the flow's data).
 */
export function MoreSheet(props: {
  more: MoreModel;
  copy: Copy;
  lang: Lang;
  platform: Platform;
  onLanguage: (lang: Lang) => void;
  onAction: (id: ActionId) => void;
  onClose: () => void;
}) {
  const { copy, more } = props;
  const m = copy.more;
  const act = (id: ActionId) => {
    props.onClose();
    props.onAction(id);
  };
  return (
    <Sheet title={m.title} closeLabel={copy.actions.close} onClose={props.onClose}>
      <section className="more-section">
        <RadioChoices
          legend={m.language}
          choices={(["ka", "en"] as const).map((value) => ({
            value,
            label: LANGUAGE_NAMES[value],
            labelLang: value,
          }))}
          value={props.lang}
          onChange={props.onLanguage}
          layout="inline"
        />
      </section>

      {more.restorable ? (
        <section className="more-section">
          <h3>{m.restore.title}</h3>
          <p>{m.restore.body}</p>
          <Button onClick={() => act("restore")}>{copy.actions.restore}</Button>
        </section>
      ) : null}

      <section className="more-section">
        <h3>{m.seen.title}</h3>
        {more.seen ? <Facts items={more.seen} /> : <p className="muted">{m.seen.noWatch}</p>}
        {more.reportText ? (
          <Disclosure summary={m.seen.reportTitle}>
            <CopyBlock
              text={more.reportText}
              label={m.seen.reportTitle}
              labels={copyLabels(copy)}
              variant="report"
              lang="en"
            />
          </Disclosure>
        ) : null}
      </section>

      <section className="more-section">
        <h3>{m.help.title}</h3>
        <ul className="tight-list">
          <li>{m.help.connecting}</li>
          <li>{m.help.mtp}</li>
          <li>{m.help.express[props.platform]}</li>
        </ul>
        <h4>{m.help.phoneTitle}</h4>
        <ul className="tight-list">
          <li>{copy.phone.android}</li>
          <li>{copy.phone.iphone}</li>
          <li>{copy.phone.either}</li>
        </ul>
        <Button kind="quiet" onClick={() => act("which_watches")}>
          {copy.actions.whichWatches}
        </Button>
      </section>

      <section className="more-section">
        <h3>{m.about.title}</h3>
        <p>{fmt(m.about.version, { version: more.version })}</p>
        <p>{m.about.unofficial}</p>
        <p>
          {m.about.source} <span className="url" lang="en">{REPO_URL}</span>
        </p>
      </section>
    </Sheet>
  );
}
