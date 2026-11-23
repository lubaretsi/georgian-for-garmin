import { fmt } from "../copy";
import { RESULT_SAMPLES, TEST_TEXT } from "../copy/shared";
import type { Answers, Result, ScreenModel, View } from "../model/screens";
import { Qr } from "../ui/bits";
import { type Choice, CheckboxChoices, RadioChoices } from "../ui/Choices";
import { CopyBlock } from "../ui/CopyBlock";
import { copyLabels, emitting } from "./common";
import type { ScreenContext, ScreenView } from "./types";

type Check = Extract<ScreenModel, { screen: "check" }>;

/** The answers a user picks; `not_installed` is the flow's own, never offered. */
const OFFERED: Exclude<Result, "not_installed">[] = ["georgian", "boxes", "question_marks", "some_letters", "no_notification"];
const VIEWS: View[] = ["popup", "list", "opened"];
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
    illustration: { icon: "phone" },
    content: (
      <>
        {model.phone ? (
          <div className="qr-row">
            <Qr svg={model.phone.qrSvg} label={s.qrLabel} />
            <p>{s.scan}</p>
          </div>
        ) : null}
        <p>{model.phone ? s.orText : s.textOnly}</p>
        <CopyBlock
          text={TEST_TEXT}
          label={s.title}
          labels={copyLabels(ctx.copy)}
          variant="sample"
          lang="ka"
          onCopied={() => ctx.emit("copy_test_text")}
        />
        <p className="muted">{s.expect}</p>
      </>
    ),
    primary: emitting(ctx, ctx.copy.actions.sentIt, "send_test_done"),
  };
}

function answerStep(model: Check, ctx: ScreenContext): ScreenView {
  const a = ctx.copy.check.answer;
  const answers = model.answers;
  const change = (next: Answers) => ctx.emit("answer", { action: "answer", answers: next });
  const letters = answers.result === "georgian" || answers.result === "some_letters";
  const results: Choice<Exclude<Result, "not_installed">>[] = OFFERED.map((value) => ({
    value,
    label: a.results[value],
    sample: value === "no_notification" ? "—" : RESULT_SAMPLES[value],
    sampleLang: value === "georgian" || value === "some_letters" ? "ka" : undefined,
  }));
  const markers: Choice<string>[] = [
    ...model.markPictures.map((m) => ({
      value: m.variant,
      label: fmt(a.marker.picture, { variant: m.variant }),
      sample: <span className="marker-picture" dangerouslySetInnerHTML={{ __html: m.svg }} />,
    })),
    { value: NO_MARKER, label: a.marker.none },
  ];
  return {
    step: "check",
    title: a.title,
    content: (
      <div className="answer-form">
        <RadioChoices
          legend={a.title}
          hideLegend
          choices={results}
          value={answers.result === "not_installed" ? undefined : answers.result}
          onChange={(result) => change({ ...answers, result })}
        />
        {letters ? (
          <CheckboxChoices
            legend={a.views.legend}
            choices={VIEWS.map((value) => ({ value, label: a.views[value] }))}
            value={answers.views}
            onChange={(views) => change({ ...answers, views })}
          />
        ) : null}
        {letters && model.markPictures.length > 0 ? (
          <RadioChoices
            legend={a.marker.legend}
            hint={a.marker.hint}
            choices={markers}
            value={answers.marker}
            onChange={(marker) => change({ ...answers, marker })}
            layout="grid"
          />
        ) : null}
        <RadioChoices
          legend={a.phone.legend}
          choices={[
            { value: "android", label: a.phone.android },
            { value: "iphone", label: a.phone.iphone },
          ]}
          value={answers.phone}
          onChange={(phone) => change({ ...answers, phone })}
          layout="inline"
        />
        {answers.result === undefined ? <p className="muted">{a.required}</p> : null}
      </div>
    ),
    primary: { label: ctx.copy.actions.continue, onPress: () => ctx.emit("done"), disabled: answers.result === undefined },
    secondary: emitting(ctx, ctx.copy.actions.back, "back"),
  };
}
