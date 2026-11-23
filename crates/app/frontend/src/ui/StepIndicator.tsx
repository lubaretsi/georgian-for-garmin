import { Icon } from "./icons";

export const STEPS = ["connect", "install", "restart", "check"] as const;
export type StepId = (typeof STEPS)[number];

/** Connect · Install · Restart · Check, with the current step marked. */
export function StepIndicator(props: {
  current: StepId;
  labels: Record<StepId, string> & { label: string; current: string; done: string };
}) {
  const index = STEPS.indexOf(props.current);
  return (
    <nav className="steps" aria-label={props.labels.label}>
      <ol>
        {STEPS.map((step, i) => {
          const state = i < index ? "done" : i === index ? "current" : "next";
          return (
            <li key={step} className={`step step-${state}`} aria-current={state === "current" ? "step" : undefined}>
              <span className="step-mark" aria-hidden="true">
                {state === "done" ? <Icon name="check" size={14} /> : i + 1}
              </span>
              <span className="step-label">{props.labels[step]}</span>
              {state === "done" ? <span className="sr-only">, {props.labels.done}</span> : null}
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
