import { type ReactNode, useId } from "react";

export type Choice<T extends string> = {
  value: T;
  label: string;
  /** Shown before the label: a rendered sample or a picture. Hidden from screen readers. */
  sample?: ReactNode;
  /** Language of the sample, so it renders in the right font. */
  sampleLang?: string | undefined;
  /** Language of the label when it differs from the page's (the language names). */
  labelLang?: string;
};

/** A real radio group: arrow keys move between options, the legend names the group. */
export function RadioChoices<T extends string>(props: {
  legend: string;
  hint?: string;
  choices: Choice<T>[];
  value: T | undefined;
  onChange: (value: T) => void;
  layout?: "list" | "inline" | "grid";
  /** The legend is still read out; hide it when the screen's heading already says it. */
  hideLegend?: boolean;
}) {
  const name = useId();
  const hintId = `${name}-hint`;
  return (
    <fieldset className={`choices choices-${props.layout ?? "list"}`} aria-describedby={props.hint ? hintId : undefined}>
      <legend className={props.hideLegend ? "sr-only" : "choices-legend"}>{props.legend}</legend>
      {props.hint ? (
        <p className="choices-hint" id={hintId}>
          {props.hint}
        </p>
      ) : null}
      <div className="choices-options">
        {props.choices.map((choice) => (
          <label key={choice.value} className="choice">
            <input
              type="radio"
              name={name}
              value={choice.value}
              checked={props.value === choice.value}
              onChange={() => props.onChange(choice.value)}
            />
            {choice.sample !== undefined ? (
              <span className="choice-sample" aria-hidden="true" lang={choice.sampleLang}>
                {choice.sample}
              </span>
            ) : null}
            <span className="choice-label" lang={choice.labelLang}>
              {choice.label}
            </span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}

/** A group of independent checkboxes. */
export function CheckboxChoices<T extends string>(props: {
  legend: string;
  choices: Choice<T>[];
  value: readonly T[];
  onChange: (value: T[]) => void;
}) {
  const toggle = (value: T, on: boolean) => {
    const rest = props.value.filter((v) => v !== value);
    // Keep the options' order, whatever order they were ticked in.
    const next = on ? [...rest, value] : rest;
    props.onChange(props.choices.map((c) => c.value).filter((v) => next.includes(v)));
  };
  return (
    <fieldset className="choices choices-list">
      <legend className="choices-legend">{props.legend}</legend>
      <div className="choices-options">
        {props.choices.map((choice) => (
          <label key={choice.value} className="choice">
            <input
              type="checkbox"
              value={choice.value}
              checked={props.value.includes(choice.value)}
              onChange={(e) => toggle(choice.value, e.currentTarget.checked)}
            />
            <span className="choice-label">{choice.label}</span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}
