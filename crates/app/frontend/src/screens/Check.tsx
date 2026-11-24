import { fmt } from "../copy";
import { TEST_TEXT } from "../copy/shared";
import { type Answers, type Result, type ScreenModel, sawLetters, VIEWS } from "../model/screens";
import { MiniFace } from "../ui/art/Watch";
import { CheckboxRow, RadioRows, Segmented } from "../ui/Controls";
import { CopyBlock } from "../ui/CopyBlock";
import { Group, GroupFootnote, GroupHeader } from "../ui/Group";
import { imageSource, Qr } from "../ui/Qr";
import { copyLabels, emitting } from "./common";
import { TITLE_ID, type ScreenContext, type ScreenView } from "./types";

type Check = Extract<ScreenModel, { screen: "check" }>;

/** The answers a user picks, in the issue form's order; `not_installed` is the flow's own. */
export const OFFERED: Exclude<Result, "not_installed">[] = ["georgian", "boxes", "question_marks", "some_letters", "no_notification"];
const NO_MARKER = "none";

/** check: send yourself the test notification, then say what the watch showed. */
export function CheckScreen(model: Check, ctx: ScreenContext): ScreenView {
  return model.step === "send" ? sendStep(model, ctx) : answerStep(model, ctx);
}

function sendStep(model: Check, ctx: ScreenContext): ScreenView {
  const s = ctx.copy.check.send;
  return {
    step: "check",
    title: s.title,
    content: (
      <>
        {model.phone ? (
          <Group>
            <div className="flex items-center gap-4 p-3.5">
              <Qr image={model.phone.qr} label={s.qrLabel} />
              <p className="text-row text-fg">{s.scan}</p>
            </div>
          </Group>
        ) : null}
        <div>
          <p className="mb-2 px-3.5 text-row text-fg-2">{model.phone ? s.orText : s.textOnly}</p>
          <CopyBlock
            text={TEST_TEXT}
            label={s.title}
            labels={copyLabels(ctx.copy)}
            variant="sample"
            lang="ka"
            onCopied={() => ctx.emit("copy_test_text")}
          />
          <GroupFootnote>{s.expect}</GroupFootnote>
        </div>
      </>
    ),
    primary: emitting(ctx, ctx.copy.actions.sentIt, "send_test_done"),
  };
}

function answerStep(model: Check, ctx: ScreenContext): ScreenView {
  const a = ctx.copy.check.answer;
  const answers = model.answers;
  const change = (next: Answers) => ctx.emit("answer", { action: "answer", answers: next });
  const letters = sawLetters(answers.result);
  return {
    step: "check",
    title: a.title,
    content: (
      <>
        <div>
          <RadioRows
            labelledBy={TITLE_ID}
            describedBy={answers.result === undefined ? "answer-required" : undefined}
            value={answers.result === "not_installed" ? undefined : answers.result}
            onChange={(result) => change({ ...answers, result })}
            choices={OFFERED.map((value) => ({
              value,
              label: a.results[value],
              trailing: <MiniFace sample={value === "no_notification" ? "none" : value} />,
            }))}
          />
          {answers.result === undefined ? (
            <GroupFootnote>
              <span id="answer-required">{a.required}</span>
            </GroupFootnote>
          ) : null}
        </div>
        {letters ? (
          <section aria-labelledby="views-legend">
            <GroupHeader id="views-legend" aside={a.optional}>
              {a.views.legend}
            </GroupHeader>
            <Group role="group" aria-labelledby="views-legend" className="[--inset:44px]">
              {VIEWS.map((view) => (
                <CheckboxRow
                  key={view}
                  label={a.views[view]}
                  checked={answers.views.includes(view)}
                  onChange={(on) =>
                    change({ ...answers, views: on ? [...answers.views, view] : answers.views.filter((v) => v !== view) })
                  }
                />
              ))}
            </Group>
          </section>
        ) : null}
        {letters && model.markPictures.length > 0 ? (
          <section aria-labelledby="marker-legend">
            <GroupHeader id="marker-legend">{a.marker.legend}</GroupHeader>
            <RadioRows
              labelledBy="marker-legend"
              describedBy="marker-hint"
              value={answers.marker}
              onChange={(marker) => change({ ...answers, marker })}
              choices={[
                ...model.markPictures.map((m) => ({
                  value: m.variant,
                  label: fmt(a.marker.picture, { variant: m.variant }),
                  trailing: <img src={imageSource(m.picture)} alt="" className="h-8 w-40 rounded-[6px] object-contain" />,
                })),
                { value: NO_MARKER, label: a.marker.none },
              ]}
            />
            <GroupFootnote>
              <span id="marker-hint">{a.marker.hint}</span>
            </GroupFootnote>
          </section>
        ) : null}
        <section>
          <GroupHeader id="phone-legend" aside={a.optional}>
            {a.phone.legend}
          </GroupHeader>
          <Segmented
            labelledBy="phone-legend"
            value={answers.phone}
            onChange={(phone) => change({ ...answers, phone })}
            choices={[
              { value: "android", label: a.phone.android },
              { value: "iphone", label: a.phone.iphone },
            ]}
          />
        </section>
      </>
    ),
    primary: { label: ctx.copy.actions.continue, onPress: () => ctx.emit("done"), disabled: answers.result === undefined },
    secondary: emitting(ctx, ctx.copy.actions.back, "back"),
  };
}
