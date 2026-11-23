import { Check } from "lucide-react";

import { cn } from "./cn";

export const JOURNEY = ["connect", "install", "restart", "check"] as const;
export type StepId = (typeof JOURNEY)[number];

/**
 * The journey as a pill track: done stops are ticked discs, the current stop opens into a
 * pill with its number and name, the stops ahead wait as numbered discs, joined by equal
 * rails. It fits any window width and label length. `complete` ticks every stop (the end).
 * The track is drawn for the eye; an ordered list with aria-current carries it for readers.
 */
export function Journey(props: {
  current: StepId;
  complete?: boolean;
  labels: Record<StepId, string> & { label: string; current: string; done: string };
}) {
  const at = props.complete ? JOURNEY.length : JOURNEY.indexOf(props.current);
  const stateOf = (i: number) => (i < at ? "done" : i === at ? "current" : "next");
  return (
    <>
      <ol aria-label={props.labels.label} className="wa:sr-only">
        {JOURNEY.map((step, i) => (
          <li key={step} aria-current={stateOf(i) === "current" ? "step" : undefined}>
            {props.labels[step]}
            {stateOf(i) === "done" ? `, ${props.labels.done}` : stateOf(i) === "current" ? `, ${props.labels.current}` : ""}
          </li>
        ))}
      </ol>
      <div aria-hidden className="wa:flex wa:items-center">
        {JOURNEY.map((step, i) => {
          const state = stateOf(i);
          return [
            i > 0 ? (
              <span key={`${step}-rail`} className={cn("wa:mx-1.5 wa:h-0.5 wa:min-w-3 wa:flex-1 wa:rounded-full", i <= at ? "wa:bg-accent" : "wa:bg-line")} />
            ) : null,
            state === "current" ? (
              <span
                key={step}
                className="wa:inline-flex wa:h-8 wa:shrink-0 wa:items-center wa:gap-2 wa:rounded-full wa:bg-accent-soft wa:py-1 wa:pr-3.5 wa:pl-1 wa:ring-[1.5px] wa:ring-accent/70"
              >
                <span className="wa:grid wa:size-6 wa:place-items-center wa:rounded-full wa:bg-accent wa:text-[13px] wa:font-bold wa:text-on-accent">{i + 1}</span>
                <span className="wa:text-[14px] wa:font-semibold wa:whitespace-nowrap wa:text-accent-soft-ink">{props.labels[step]}</span>
              </span>
            ) : (
              <span
                key={step}
                className={cn(
                  "wa:grid wa:size-7 wa:shrink-0 wa:place-items-center wa:rounded-full wa:text-[13px] wa:font-semibold",
                  state === "done" ? "wa:bg-accent wa:text-on-accent" : "wa:bg-surface wa:text-ink-2 wa:ring-[1.5px] wa:ring-control wa:ring-inset",
                )}
              >
                {state === "done" ? <Check strokeWidth={3} className="wa:size-3.5" /> : i + 1}
              </span>
            ),
          ];
        })}
      </div>
    </>
  );
}
