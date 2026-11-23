import { Check } from "lucide-react";
import { motion } from "motion/react";

import type { Copy } from "../../../copy";
import { cn } from "../lib/cn";

export const STEPS = ["connect", "install", "restart", "check"] as const;
export type StepId = (typeof STEPS)[number];

/**
 * The journey as a compact segmented indicator in the toolbar: the current step is the
 * raised segment with its name; the others are a check (done) or their number (next). Every
 * name is read by screen readers. It is a list, not a control: nothing here is clickable.
 */
export function StepIndicator({ current, labels }: { current: StepId; labels: Copy["steps"] }) {
  const index = STEPS.indexOf(current);
  return (
    <nav aria-label={labels.label} className="nb:min-w-0">
      <ol className="nb:flex nb:h-7 nb:items-center nb:gap-0.5 nb:rounded-[9px] nb:bg-fill nb:p-0.5">
        {STEPS.map((step, i) => {
          const state = i < index ? "done" : i === index ? "current" : "next";
          return (
            <li
              key={step}
              aria-current={state === "current" ? "step" : undefined}
              className={cn(
                "nb:relative nb:flex nb:h-full nb:items-center nb:justify-center nb:gap-1.5 nb:rounded-[7px]",
                state === "current" ? "nb:px-2.5" : "nb:w-7",
              )}
            >
              {state === "current" ? (
                <motion.span
                  layoutId="b-step-thumb"
                  transition={{ type: "spring", stiffness: 520, damping: 40 }}
                  className="nb:absolute nb:inset-0 nb:-z-0 nb:rounded-[7px] nb:bg-control-bg nb:shadow-thumb"
                  aria-hidden="true"
                />
              ) : null}
              <span
                aria-hidden="true"
                className={cn(
                  "nb:relative nb:flex nb:size-[18px] nb:items-center nb:justify-center nb:rounded-full nb:text-label nb:font-semibold nb:tabular-nums",
                  state === "current" && "nb:bg-accent nb:text-on-accent",
                  state === "done" && "nb:text-fg",
                  state === "next" && "nb:text-fg-2",
                )}
              >
                {state === "done" ? <Check size={14} strokeWidth={2.6} /> : i + 1}
              </span>
              <span
                className={cn(
                  "nb:relative nb:text-label nb:font-semibold nb:whitespace-nowrap nb:text-fg",
                  state !== "current" && "nb:sr-only",
                )}
              >
                {labels[step]}
                {state === "done" ? <span className="nb:sr-only">, {labels.done}</span> : null}
              </span>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
